//! Linux playback through CPAL's ALSA host (on desktops usually PipeWire's
//! ALSA bridge). A thread owns the stream, so `Device` is `Send + Sync` like
//! the waveOut device, and dropping it stops playback. The mixer's stereo is
//! played at its own rate: a device without a matching stereo 16-bit or
//! float configuration is an error, never a silent resample (T179).
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    io,
    sync::mpsc,
    thread::{self, JoinHandle},
};

pub struct Device {
    /// Dropping the sender wakes the thread, which then drops the stream.
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Device {
    pub fn open(rate: u32, fill: impl FnMut(&mut [i16]) + Send + 'static) -> io::Result<Self> {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let thread =
            thread::Builder::new()
                .name("audio".into())
                .spawn(move || match start(rate, fill) {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        let _ = stop_rx.recv();
                        drop(stream);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                })?;
        let started = ready_rx
            .recv()
            .unwrap_or_else(|_| Err(io::Error::other("the audio thread ended while opening")));
        match started {
            Ok(()) => Ok(Self {
                stop: Some(stop_tx),
                thread: Some(thread),
            }),
            Err(e) => {
                let _ = thread.join();
                Err(e)
            }
        }
    }
}

fn start(rate: u32, mut fill: impl FnMut(&mut [i16]) + Send + 'static) -> io::Result<cpal::Stream> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| io::Error::other("no default audio output device"))?;
    let chosen = stereo_config(&device, rate)?;
    let config = chosen.config();
    let on_error = |e: cpal::StreamError| eprintln!("audio output error: {e}");
    let stream = match chosen.sample_format() {
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config,
            move |out: &mut [i16], _: &cpal::OutputCallbackInfo| fill(out),
            on_error,
            None,
        ),
        // The mixer's 16-bit samples, scaled to -1..1; the scratch buffer
        // only grows, so steady playback does not allocate here.
        cpal::SampleFormat::F32 => {
            let mut mixed = Vec::new();
            device.build_output_stream(
                &config,
                move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    mixed.resize(out.len(), 0);
                    fill(&mut mixed);
                    for (o, &s) in out.iter_mut().zip(&mixed) {
                        *o = f32::from(s) / 32768.;
                    }
                },
                on_error,
                None,
            )
        }
        other => {
            return Err(io::Error::other(format!(
                "unexpected output sample format {other:?}"
            )));
        }
    }
    .map_err(io::Error::other)?;
    stream.play().map_err(io::Error::other)?;
    eprintln!(
        "audio output: {}, {:?}, {} Hz, {} channels",
        device.name().unwrap_or_else(|_| "unnamed device".into()),
        chosen.sample_format(),
        rate,
        config.channels,
    );
    Ok(stream)
}

/// A stereo configuration at exactly `rate`, 16-bit preferred over float.
fn stereo_config(device: &cpal::Device, rate: u32) -> io::Result<cpal::SupportedStreamConfig> {
    let ranges: Vec<_> = device
        .supported_output_configs()
        .map_err(io::Error::other)?
        .collect();
    [cpal::SampleFormat::I16, cpal::SampleFormat::F32]
        .into_iter()
        .find_map(|format| {
            ranges.iter().find(|r| {
                r.channels() == 2
                    && r.sample_format() == format
                    && (r.min_sample_rate().0..=r.max_sample_rate().0).contains(&rate)
            })
        })
        .map(|r| r.with_sample_rate(cpal::SampleRate(rate)))
        .ok_or_else(|| {
            io::Error::other(format!(
                "the default audio device offers no stereo 16-bit or float output at {rate} Hz"
            ))
        })
}

impl Drop for Device {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
