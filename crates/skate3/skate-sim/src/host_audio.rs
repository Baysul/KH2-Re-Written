//! skate-sim: the local skater's sounds, skate-game's native audio runtime (`game_audio`: the
//! AEMS patch programs, the MixMap, the player components and the granular rolling bed) driven by
//! [`Sim`] instead of skate-game's Bevy systems.
//!
//! Clocking. skate-game runs `skate_events::observe` after every physics step (FixedUpdate) and
//! one audio pass per rendered frame (Update: `native::mixmap_frame` → `native::mixmap_tick` →
//! `grain_bed::update` → `native::reverb_frame`) over the steps that frame took (at most 4). Here
//! [`SkaterAudio::tick`] is one physics step followed by one such pass, i.e. skate-game rendering
//! one frame per physics step (60 fps at the 60 Hz physics clock). The pass's own clocks are
//! unchanged: one host tick per physics step, the MixMap and the Jitter / posters on every second
//! one (the console's 30 Hz cadence), the voices and programs on the pulled stream's 256-frame
//! blocks. Per-frame inputs take their one-step values: the frame's game time = the step, the
//! rendered board and camera = the latest sample (`overstep` 1: the host draws the current state,
//! not an interpolation), and the listener is the camera after the previous step (skate-game copies
//! the camera onto its listener in PostUpdate, so a pass hears the last frame's camera).
//!
//! The host's mixer owns the volume and pausing: skate-game's master volume (75 % by default),
//! category volumes and menu / replay silence are not applied here; the runtime plays at retail
//! level with the default option volumes.
use crate::audio::{RetailVolumes, grain_bed, library::Library, native, skate_events};
use crate::host::Sim;
use bevy::prelude::Transform;
use std::path::Path;

/// The stream's rate: interleaved stereo frames per second ([`SkaterStream::fill`]).
pub const SAMPLE_RATE: u32 = 48000;
const _: () = assert!(SAMPLE_RATE == skate_audio::MIX_RATE);

/// skate-game's `Presentation::cuts` (its snapshot capture after each physics step): snaps for a
/// teleport, a camera cut or a clock period change, which the audio host sees as a cut.
#[derive(Default)]
struct Cuts {
    generation: u64,
    period: f32,
    cuts: u64,
}

impl Cuts {
    fn capture(&mut self, sim: &Sim) {
        let skater = &sim.skater;
        if skater.pose_generation == self.generation && !sim.camera.manual_cam.manual_cam_active() {
            return;
        }
        let Some(frame) = sim.camera.presentation_frame() else {
            return;
        };
        let reset = frame.discontinuity
            || skater.player_input.processed.flags_2472 & (1 << 10) != 0
            || self.period != sim.audio_step;
        self.cuts += u64::from(reset);
        self.generation = skater.pose_generation;
        self.period = sim.audio_step;
    }
}

/// The skater's sounds for one [`Sim`].
pub struct SkaterAudio {
    library: Library,
    native: native::Native,
    cues: skate_events::Cues,
    seen: skate_events::Seen,
    cuts: Cuts,
    /// skate-game's `GameAudioListener` (spawned at the origin, then the camera of the last frame).
    listener: Transform,
}

/// A handle on the runtime's output for the host mixer's thread.
pub struct SkaterStream {
    pull: native::StreamPull,
}

impl SkaterStream {
    /// Fill `out` with interleaved 48 kHz stereo (left, right, ...), rendering the runtime's
    /// 256-frame blocks as they are needed (skate-game's `NativeDecoder`, `Runtime::fill_stereo`).
    pub fn fill(&mut self, out: &mut [f32]) {
        for v in out {
            *v = self.pull.next_sample();
        }
    }
}

impl SkaterAudio {
    /// Starts the native runtime from `<asset_root>/private/audio` (the prepared
    /// `audio_manifest.json` layout skate-game reads): every Csis project, the MixMap, the
    /// player's AEMS and Splice banks, the wheel spins and the grain recordings, decoded now.
    /// Missing optional parts are logged and stay silent, as in skate-game; a missing manifest,
    /// project or MixMap file is an error.
    pub fn load(asset_root: &Path, _sim: &Sim) -> Result<SkaterAudio, String> {
        static DSP: std::sync::Once = std::sync::Once::new();
        let library = Library::load(asset_root)?;
        // Which compiled copy of the DSP loops runs (hardware FMA or plain; same output bits): chosen
        // once, before the first render (doc 11 "Hardware FMA dispatch").
        DSP.call_once(|| bevy::log::info!("AUDIO_DSP {}", skate_audio::dsp::init_fma()));
        let native = native::Native::start(&library)?;
        Ok(SkaterAudio {
            library,
            native,
            cues: skate_events::Cues::default(),
            seen: skate_events::Seen::default(),
            cuts: Cuts::default(),
            listener: Transform::default(),
        })
    }

    /// The audio work of one physics step: call once after each successful [`Sim::tick`].
    pub fn tick(&mut self, sim: &Sim) {
        let dt = sim.audio_step;
        skate_events::observe(
            &sim.physics,
            &sim.skater,
            sim.audio_events,
            dt,
            &mut self.cues,
            &mut self.seen,
        );
        self.cuts.capture(sim);
        let inputs = native::FrameInputs {
            listener: self.listener,
            frame_dt: dt,
            overstep: 1.0,
            silenced: false,
            cuts: (self.cuts.cuts, 0),
            teleport: None,
            master_inputs: RetailVolumes::default().master_inputs(),
        };
        native::mixmap_frame(&mut self.native, &mut self.cues, &inputs);
        native::mixmap_tick(&mut self.native);
        grain_bed::update(&mut self.native, &self.library, &self.cues);
        native::reverb_frame(&self.native, &self.listener, dt);
        // PostUpdate: the listener follows the gameplay camera for the next pass.
        if let Some((camera, _)) = sim.camera() {
            self.listener = camera;
        }
    }

    /// A pull handle for the host's audio thread; every handle reads the same runtime (one
    /// consumer is expected).
    pub fn stream(&self) -> SkaterStream {
        SkaterStream {
            pull: native::StreamPull::new(self.native.shared.clone()),
        }
    }
}
