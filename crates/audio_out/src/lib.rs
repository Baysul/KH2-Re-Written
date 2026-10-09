//! Sound output for the test room: a software mixer (safe) feeding the
//! Windows waveOut device (the only FFI, in `winmm`). Clips arrive already
//! decoded; voices are resampled linearly to the device rate.
use std::sync::{Arc, Mutex};

#[cfg(windows)]
mod winmm;

/// Device format: 16-bit stereo.
pub const RATE: u32 = 48_000;
const MAX_VOICES: usize = 32;

/// A decoded sound: interleaved 16-bit samples with an optional loop range
/// in frames (end inclusive).
#[derive(Debug, Clone)]
pub struct Clip {
    pub channels: u16,
    pub rate: u32,
    pub samples: Arc<[i16]>,
    pub loop_frames: Option<(u32, u32)>,
}

impl Clip {
    fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    fn frame(&self, index: usize) -> [f32; 2] {
        let c = self.channels as usize;
        let at = index * c;
        let left = self.samples[at] as f32;
        let right = if c > 1 {
            self.samples[at + 1] as f32
        } else {
            left
        };
        [left, right]
    }
}

struct Voice {
    clip: Clip,
    key: u32,
    position: f64,
    step: f64,
    gain: f32,
}

#[derive(Default)]
pub struct Mixer {
    voices: Vec<Voice>,
}

impl Mixer {
    /// Starts `clip`; `key` identifies the request for `stop`. The oldest
    /// voice is dropped past the voice limit.
    pub fn play(&mut self, clip: Clip, key: u32, gain: f32, pitch: f32) {
        if clip.frames() == 0 {
            return;
        }
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        let step = clip.rate as f64 / RATE as f64 * pitch as f64;
        self.voices.push(Voice {
            clip,
            key,
            position: 0.,
            step,
            gain,
        });
    }

    pub fn stop(&mut self, key: u32) {
        self.voices.retain(|v| v.key != key);
    }

    pub fn playing(&self, key: u32) -> bool {
        self.voices.iter().any(|v| v.key == key)
    }

    pub fn voices(&self) -> usize {
        self.voices.len()
    }

    /// Mixes into interleaved stereo `out` at `RATE`.
    pub fn mix(&mut self, out: &mut [i16]) {
        let mut sum = vec![0f32; out.len()];
        self.voices.retain_mut(|v| {
            let frames = v.clip.frames();
            for pair in sum.as_chunks_mut::<2>().0 {
                let mut index = v.position as usize;
                if let Some((start, end)) = v.clip.loop_frames {
                    let end = (end as usize).min(frames - 1);
                    if index > end && (start as usize) < end {
                        v.position -= (end + 1 - start as usize) as f64;
                        index = v.position as usize;
                    }
                }
                if index >= frames {
                    return false;
                }
                let t = (v.position - index as f64) as f32;
                let a = v.clip.frame(index);
                let b = if index + 1 < frames {
                    v.clip.frame(index + 1)
                } else {
                    a
                };
                pair[0] += (a[0] + (b[0] - a[0]) * t) * v.gain;
                pair[1] += (a[1] + (b[1] - a[1]) * t) * v.gain;
                v.position += v.step;
            }
            true
        });
        for (o, s) in out.iter_mut().zip(sum) {
            *o = s.clamp(-32768., 32767.) as i16;
        }
    }
}

/// The open device and its shared mixer. Dropping it stops playback.
pub struct Output {
    pub mixer: Arc<Mutex<Mixer>>,
    #[cfg(windows)]
    _device: winmm::Device,
}

impl Output {
    pub fn open() -> std::io::Result<Self> {
        let mixer = Arc::new(Mutex::new(Mixer::default()));
        #[cfg(windows)]
        {
            let feed = mixer.clone();
            let device = winmm::Device::open(RATE, move |buffer| {
                if let Ok(mut m) = feed.lock() {
                    m.mix(buffer);
                } else {
                    buffer.fill(0);
                }
            })?;
            Ok(Self {
                mixer,
                _device: device,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = mixer;
            Err(std::io::Error::other("no audio output on this platform"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(samples: &[i16], channels: u16, rate: u32) -> Clip {
        Clip {
            channels,
            rate,
            samples: samples.into(),
            loop_frames: None,
        }
    }

    #[test]
    fn mixes_mono_to_both_channels_and_ends() {
        let mut m = Mixer::default();
        m.play(clip(&[100, 200], 1, RATE), 7, 1., 1.);
        let mut out = [0i16; 8];
        m.mix(&mut out);
        assert_eq!(out, [100, 100, 200, 200, 0, 0, 0, 0]);
        assert_eq!(m.voices(), 0);
    }

    #[test]
    fn sums_clamps_resamples_and_stops() {
        let mut m = Mixer::default();
        m.play(clip(&[30000, 30000, 0, 0], 2, RATE), 1, 1., 1.);
        m.play(clip(&[30000, -100], 2, RATE), 2, 1., 1.);
        let mut out = [0i16; 2];
        m.mix(&mut out);
        assert_eq!(out, [32767, 29900]);
        // Half the device rate: each source frame spans two output frames.
        let mut m = Mixer::default();
        m.play(clip(&[0, 100], 1, RATE / 2), 3, 0.5, 1.);
        let mut out = [0i16; 6];
        m.mix(&mut out);
        assert_eq!(out, [0, 0, 25, 25, 50, 50]);
        assert!(m.playing(3));
        m.stop(3);
        assert!(!m.playing(3));
    }

    #[test]
    fn loops_between_loop_frames() {
        let mut m = Mixer::default();
        let mut c = clip(&[1, 2, 3], 1, RATE);
        c.loop_frames = Some((1, 2));
        m.play(c, 9, 1., 1.);
        let mut out = [0i16; 12];
        m.mix(&mut out);
        let left: Vec<i16> = out.iter().step_by(2).copied().collect();
        assert_eq!(left, [1, 2, 3, 2, 3, 2]);
    }
}
