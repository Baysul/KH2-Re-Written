//! Host for the native AEMS runtime (`crates/skate-audio`): retail's own patch programs and voice
//! graph. An install without the data (AEMS banks, MixMap, grain recordings, Splice trees) logs an
//! error naming what is missing and those sounds stay silent: there is no fallback.
//!
//! - At startup every Csis project is installed and `emitter_utility.abk` is loaded and posted
//!   (retail posts `c_emitter_utility` once at boot; it feeds the `*_snd` / `random_*_gbl` globals).
//! - The player's banks are loaded at start, with their decoded WAVs as PCM.
//! - One pulled stream ([`StreamPull`]): the host's mixer pulls 48 kHz stereo; each 256-frame block
//!   renders the voices and ticks the evaluator, so programs run on the audio clock exactly like
//!   retail. Game code posts, redelivers and releases between blocks under the runtime's lock.
//! - The MixMap mixer (`MixMapSK8.mxb`, `skate_audio::mixmap`) runs on the game thread, clocked by
//!   the physics steps ([`HostClock`]: one host tick per step since the last pass): [`mixmap_frame`]
//!   writes the inputs we can supply (category gains, the local player's physics and 3-D position),
//!   ticks, and the systems read its outputs (the rolling bed's levels, pitch, filters and pan).
//!
//! skate-sim: skate-game's `game_audio/native.rs` reduced to the skater's sounds. Left out: the
//! world emitters and their emitter states, world / NPC owners, map bank loading and the bank
//! prefetch worker, the front-end sounds, the audio content overlays (mod banks and projects, hot
//! swap, restart carry), mod MixMap input writes (`mixmap_inputs`, none without mods), the timing
//! readout, the settings menu (master / category volumes: the host's mixer owns the volume; the
//! runtime's own gains stay at retail level 1.0) and the Bevy audio source. The reverb preset is
//! the default one (no map regions, no reverb-zone emitters).
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::log::{error, info, warn};
use bevy::prelude::Transform;
use skate_audio::formats::Project;
use skate_audio::mixmap::{MixMap, keys};
use skate_audio::runtime::Runtime;

use super::library::Library;
use super::skate_events::Cues;

/// The host's step: one 60 Hz physics step. The host ticks once per physics step ([`HostClock`]);
/// the MixMap evaluates on every second one (the console's 30 Hz, `skate_audio::mixmap::cadence`).
pub(crate) const MIX_STEP: f32 = 1.0 / 60.0;
/// The most host ticks one frame takes (the steps beyond are dropped; their pulses stay latched).
/// Retail's audio manager runs once per rendered frame and never catches up: a long frame is one
/// call with its long dt (both halves once). Our host counts physics steps to stay on the console
/// grid at any frame rate, so it bounds the catch-up instead: 4 steps = 2 console evaluations,
/// enough for frame-rate independence down to 15 fps (the old Real-time accumulator's cap, 4 ×
/// [`MIX_STEP`]), while a hitch (Bevy runs up to 15 physics steps after a 250 ms frame) never
/// releases a burst of evaluations, Jitter steps or poster calls.
pub(crate) const MAX_STEPS_PER_FRAME: u32 = 4;

/// What a frame's host pass runs ([`HostClock::pass`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pass {
    /// Host ticks: the physics steps taken (capped at [`MAX_STEPS_PER_FRAME`]).
    pub ticks: usize,
    /// Console evaluations they complete (the console cadence).
    pub calls: usize,
    /// `sub_82491180` (the eEQChain clear) runs in this pass.
    pub clear_eq: bool,
}

/// The audio host's clock: the physics steps `skate_events::observe` published since the last
/// frame (2026-10-03, PR #32 review: before, a `Time<Real>` accumulator of its own drifted from
/// the physics grid after hitches, re-processed a stale sample on frames without a step, dropped
/// the first of two steps' pulses and kept ticking while the menu paused the game).
#[derive(Default)]
pub(crate) struct HostClock {
    /// The console's 30 Hz evaluation grid over the steps.
    cadence: skate_audio::mixmap::cadence::Cadence,
}

impl HostClock {
    /// Take the frame's steps. `None` (no pass: no inputs, process, tick or update) when nothing
    /// stepped, or while the game is silenced (menu, replay): those steps and their pulses are
    /// dropped, as the stream is paused.
    pub(crate) fn pass(&mut self, cues: &mut Cues, silenced: bool) -> Option<Pass> {
        let steps = cues.take_steps();
        if steps == 0 || silenced {
            return None;
        }
        let ticks = steps.min(MAX_STEPS_PER_FRAME) as usize;
        // The console cadence (`skate_audio::mixmap::cadence`): retail's audio manager evaluates the
        // MixMap, steps the Jitter and clears the eEQChain buses once per 1/30 s console frame with
        // dt 1/30; here every second step, the flag inputs held in between.
        let calls = self.cadence.advance(ticks);
        // sub_82491180 in half 1 of the audio manager: every console frame (both halves run when the
        // frame is longer than 20 ms).
        let clear_eq = calls > 0;
        Some(Pass {
            ticks,
            calls,
            clear_eq,
        })
    }
}

/// The runtime, shared with the audio thread.
pub(crate) type Shared = Arc<Mutex<Runtime>>;

/// skate-game's `NativeDecoder` without rodio: an endless 48 kHz stereo stream pulled from the
/// runtime, one block of stereo per lock.
pub(crate) struct StreamPull {
    shared: Shared,
    buffer: Vec<f32>,
    at: usize,
}

impl StreamPull {
    pub(crate) fn new(shared: Shared) -> Self {
        // One block of stereo per lock.
        Self {
            shared,
            buffer: vec![0.0; 2 * skate_audio::BLOCK],
            at: usize::MAX,
        }
    }

    /// The next sample (`NativeDecoder::next`).
    pub(crate) fn next_sample(&mut self) -> f32 {
        if self.at >= self.buffer.len() {
            match self.shared.lock() {
                // The buffer is one block of stereo: one render per fill.
                Ok(mut runtime) => runtime.fill_stereo(&mut self.buffer),
                Err(_) => self.buffer.fill(0.0),
            }
            self.at = 0;
        }
        let v = self.buffer[self.at];
        self.at += 1;
        v
    }
}

/// The running native runtime and what the game has loaded into it.
pub(crate) struct Native {
    pub(crate) shared: Shared,
    /// Bank stem → runtime bank id.
    banks: HashMap<String, usize>,
    /// The MixMap, when the install has `MixMapSK8.mxb`.
    pub(crate) mixmap: Option<MixMap>,
    /// The host's clock (the physics steps).
    clock: HostClock,
    /// This frame's host ticks (0: no pass), for the systems after [`mixmap_frame`] (the bed).
    pub(crate) frame_ticks: usize,
    /// Camera cuts seen (teleport, map change; [`mixmap_frame`]).
    pub(crate) cuts: u64,
    /// The cut signal last seen (`Presentation::cuts`, the map generation) and the wheel sample
    /// of the step at the cut (no seam interpolation across it).
    cut_seen: Option<(u64, u64)>,
    cut_wheels: Option<[[f32; 3]; 4]>,
    /// The flag inputs are held between evaluations (set once, at the first pass).
    holds: bool,
    /// The local player's MixMap inputs and components (None without a MixMap).
    pub(crate) player: Option<super::player_audio::PlayerAudio>,
    /// The granular rolling bed's game-side state (None: no rolling sound; an error is logged).
    pub(crate) bed: Option<super::grain_bed::Bed>,
    /// This frame's pass between [`mixmap_frame`] (inputs, the local player's process) and
    /// [`mixmap_tick`] (the ticks, the update).
    pub(crate) pending: Option<PendingPass>,
}

/// A pass [`mixmap_frame`] began and [`mixmap_tick`] finishes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PendingPass {
    /// Console evaluations (MixMap ticks) in this pass.
    pub(crate) calls: usize,
    s: skate_audio::player::AudioState,
    speed_scale: Option<f32>,
    loose: u32,
    reverb: bool,
}

/// The MixMap's instances per slot: retail's free-skate layout (`WorldInstances::RETAIL`: 4
/// traffic, 15 pedestrian, 2 Player = the local player + 1 NPC skater). Only instance 0 of the
/// player slots is driven here.
fn retail_instances() -> [usize; 14] {
    let mut n = skate_audio::mixmap::RETAIL_INSTANCES;
    n[skate_audio::world::keys::TRAFFIC as usize] = 4;
    n[skate_audio::world::keys::PEDESTRIAN as usize] = 15;
    n[1] = 1 + 1;
    n
}

impl Native {
    pub(crate) fn start(library: &Library) -> Result<Self, String> {
        let files = library.aems();
        if files.projects.is_empty() {
            return Err("this install has no AEMS banks (run setup to refresh the audio)".into());
        }
        let mut runtime = Runtime::new();
        runtime.eval.continue_nodes(1);
        for file in &files.projects {
            let bytes = library.read(file).map_err(|e| format!("{file}: {e}"))?;
            let project = Project::parse(file, &bytes).map_err(|e| e.to_string())?;
            runtime.install_project(&project);
        }
        let mixmap = match &files.mixmap {
            Some(file) => {
                let bytes = library.read(file).map_err(|e| format!("{file}: {e}"))?;
                let file =
                    skate_audio::mixmap::MixMapFile::parse(&bytes).map_err(|e| e.to_string())?;
                let m = MixMap::new(&file, &retail_instances());
                info!(
                    "Game audio: MixMap {} controllers ({} output blocks)",
                    m.controller_count(),
                    m.output_blocks()
                );
                Some(m)
            }
            None => {
                error!(
                    "Game audio: this install has no MixMapSK8.mxb: the skater's sounds and rolling are silent (run setup to refresh the audio)"
                );
                None
            }
        };
        let bed = if mixmap.is_some() {
            super::grain_bed::Bed::new(library)
        } else {
            None
        };
        let player = mixmap
            .as_ref()
            .map(|_| super::player_audio::PlayerAudio::new(library.player_tuning(), true));
        if bed.is_none() {
            error!(
                "Game audio: no MixMap, whole grain recordings or grain tuning in this install: rolling is silent (run setup to refresh the audio)"
            );
        }
        let mut native = Self {
            shared: Arc::new(Mutex::new(runtime)),
            banks: HashMap::new(),
            mixmap,
            clock: HostClock::default(),
            frame_ticks: 0,
            cuts: 0,
            cut_seen: None,
            cut_wheels: None,
            holds: false,
            player,
            bed,
            pending: None,
        };
        // The environment (reverb) network and the eEQChain buses (optional install data).
        let (presets, eq) = library.bus_tuning();
        if !presets.is_empty() || !eq.is_empty() {
            let mut runtime = native.shared.lock().map_err(|_| "audio lock poisoned")?;
            info!(
                "Game audio: native buses on ({} reverb presets, {} eEQChain buses)",
                presets.len(),
                eq.len()
            );
            runtime.mixer.buses.env.presets = presets;
            runtime.mixer.buses.eq.set_records(&eq);
        }
        // The FlangeSub effect returns (GRINDS and the skid send into return A).
        if let Some([a, b]) = library.flange_presets() {
            let mut runtime = native.shared.lock().map_err(|_| "audio lock poisoned")?;
            info!("Game audio: native FlangeSub returns on");
            runtime.mixer.buses.flange.set_presets(a, b);
        }
        // The FootStep SubMix graphs (`sub_82494188`: each foot sound through its EQ record, env
        // send and panner).
        if let Ok(mut runtime) = native.shared.lock() {
            runtime.mixer.buses.submix.enabled = true;
        }
        native.ensure_bank(library, "emitter_utility")?;
        native.load_player_banks(library);
        let mut runtime = native.shared.lock().map_err(|_| "audio lock poisoned")?;
        let utility = runtime
            .eval
            .class_id("c_emitter_utility")
            .ok_or("no c_emitter_utility class")?;
        runtime.post(utility, &[]);
        drop(runtime);
        // Retail's second boot utility (Start_up_Play_ctl, Common.abk): the Seams program's sample
        // shuffles. Older installs without Common.abk keep the fixed seam samples.
        match native.ensure_bank(library, skate_audio::player::seams::UTILITY_BANK) {
            Ok(_) => {
                let mut runtime = native.shared.lock().map_err(|_| "audio lock poisoned")?;
                if let Some(class) = runtime.eval.class_id(skate_audio::player::seams::UTILITY) {
                    runtime.post(class, &[]);
                }
            }
            Err(e) => warn!("Game audio: {e}; seam hits play fixed samples (rerun setup)"),
        }
        // Then the foley utility (the cloth_trick programs call it): retail's boot order
        // c_emitter_utility → Start_up_Play_ctl → c_foley_utility, which matters because every
        // program shares one random generator.
        if native
            .player
            .as_ref()
            .is_some_and(|p| p.components && p.tricks_on)
        {
            let mut runtime = native.shared.lock().map_err(|_| "audio lock poisoned")?;
            if let Some(id) = runtime
                .eval
                .class_id(skate_audio::player::tricks::FOLEY_UTILITY)
            {
                runtime.post(id, &[]);
            }
        }
        Ok(native)
    }

    /// Load a bank (and its samples) unless it is loaded already.
    pub(crate) fn ensure_bank(&mut self, library: &Library, stem: &str) -> Result<usize, String> {
        if let Some(&id) = self.banks.get(stem) {
            return Ok(id);
        }
        let (bank, pcm) = library.bank_source(stem)?.load()?;
        let mut runtime = self.shared.lock().map_err(|_| "audio lock poisoned")?;
        let id = runtime.load_bank(bank, pcm);
        drop(runtime);
        self.banks.insert(stem.to_owned(), id);
        Ok(id)
    }

    /// The player components' banks, in the player volume group; without them the components
    /// stay off and the skater's sounds are silent.
    fn load_player_banks(&mut self, library: &Library) {
        if !self.player.as_ref().is_some_and(|p| p.components) {
            return;
        }
        for stem in super::player_audio::BANKS {
            match self.ensure_bank(library, stem) {
                Ok(id) => {
                    if let Ok(mut runtime) = self.shared.lock() {
                        runtime
                            .mixer
                            .set_bank_group(id, skate_audio::mixer::GROUP_PLAYER);
                    }
                }
                Err(e) => {
                    error!(
                        "Game audio: the skater's sounds are silent ({e}; run setup to refresh the audio)"
                    );
                    if let Some(p) = &mut self.player {
                        p.components = false;
                    }
                    return;
                }
            }
        }
        info!(
            "Game audio: native player components on (Class_grind, SenseOfSpeed, Class_foot_drag)"
        );
        self.load_optional_player_banks(library);
        // The Splice banks (pops, landings, touchdowns), decoded up front so a sample's first
        // trigger sounds like every later one.
        let mut first = false;
        for (i, stem) in super::player_audio::SPLICE_BANKS.iter().enumerate() {
            match library.splice_bank(stem) {
                Some((bank, pcm)) => {
                    if let Ok(mut runtime) = self.shared.lock() {
                        let rt = &mut *runtime;
                        rt.splice.load_bank(stem, bank, pcm, &mut rt.mixer);
                        first |= i == 0;
                    }
                }
                None => error!(
                    "Game audio: {stem} has no patch tree in this install: its sounds are silent (run setup to refresh the audio)"
                ),
            }
        }
        // SFXObj_Wheels' spin-down recordings, decoded now (first trigger = later triggers).
        let streams: Vec<_> = super::player_audio::WHEEL_STREAMS
            .iter()
            .map(|n| library.wheels_pcm(n))
            .collect();
        let wheels = streams.iter().all(Option::is_some);
        if let Ok(mut runtime) = self.shared.lock() {
            runtime.load_streams(streams);
        }
        if let Some(p) = &mut self.player {
            p.contacts_on = first;
            p.set_footstep_materials(library.footstep_materials());
            p.footsteps_on = first;
            p.contact_tuning = library.contacts_tuning();
            p.wheels_on = wheels;
            if p.contacts_on {
                info!(
                    "Game audio: native board contacts on (Splice: pops, landings, touchdowns, foot taps, scuffs; collision pairs: {} materials)",
                    p.tuning.collision.materials.len()
                );
            }
            if wheels {
                info!("Game audio: native wheel spin on (SFXObj_Wheels)");
            }
        }
    }

    /// The optional components' banks (rolling layers, rattle, board slide, tricks, treatment):
    /// each component runs when its first bank is in the install; missing later banks only drop
    /// their layers (a Class_rolling post reaches every bank bound to the class).
    fn load_optional_player_banks(&mut self, library: &Library) {
        use super::player_audio::{
            RATTLE_BANKS, ROLLING_BANKS, SLIDE_BANKS, TREATMENT_BANKS, TRICKS_BANKS,
        };
        let load = |me: &mut Self, banks: &[&str]| -> bool {
            let mut first = false;
            for (i, stem) in banks.iter().enumerate() {
                match me.ensure_bank(library, stem) {
                    Ok(id) => {
                        if let Ok(mut runtime) = me.shared.lock() {
                            runtime
                                .mixer
                                .set_bank_group(id, skate_audio::mixer::GROUP_PLAYER);
                        }
                        first |= i == 0;
                    }
                    Err(e) => warn!(
                        "Game audio: {e}; its native layers stay off (run setup to refresh the audio)"
                    ),
                }
            }
            first
        };
        let rolling = load(self, ROLLING_BANKS);
        let rattle = load(self, RATTLE_BANKS);
        let slide = load(self, SLIDE_BANKS);
        let tricks = load(self, TRICKS_BANKS);
        let treatment = load(self, TREATMENT_BANKS);
        // With the tricks the foley utility is posted at boot, after Start_up_Play_ctl (`start`).
        if let Some(p) = &mut self.player {
            (
                p.rolling_on,
                p.rattle_on,
                p.slide_on,
                p.tricks_on,
                p.treatment_on,
            ) = (rolling, rattle, slide, tricks, treatment);
            info!(
                "Game audio: native rolling layers {rolling}, rattle {rattle}, board slide {slide}, tricks {tricks}, treatment {treatment}"
            );
        }
    }
}

/// The one-frame flag inputs the console's writers set for a whole evaluation, held between our 60
/// Hz writes and the 30 Hz evaluations ([`MixMap::hold_input`]): Contacts 1 / 6 (the landing
/// swell and the landing-material flag), Rail 1 (the grind ended), SkateBoard 0 / 4 (the surface
/// change, the push plant), Cracks 0 (a seam hit). Everything else the host writes is a level that
/// holds between writes.
pub(crate) fn hold_flag_inputs(m: &mut MixMap) {
    for (key, id) in [
        (keys::contacts(0), 1),
        (keys::contacts(0), 6),
        (keys::rail(0), 1),
        (keys::skateboard(0), 0),
        (keys::skateboard(0), 4),
        (keys::cracks(0), 0),
    ] {
        m.hold_input(key, id);
    }
}

/// What the Bevy systems' resources gave [`mixmap_frame`] in skate-game, supplied by the host.
pub(crate) struct FrameInputs {
    /// The `GameAudioListener` transform (it follows the gameplay camera, copied in PostUpdate:
    /// the camera as presented on the previous frame).
    pub listener: Transform,
    /// `Time<Virtual>`'s delta: the rendered frame's game time.
    pub frame_dt: f32,
    /// `Time<Fixed>::overstep_fraction`: where the rendered board lies between the last two
    /// physics samples.
    pub overstep: f32,
    /// The menu or a replay silences the game.
    pub silenced: bool,
    /// (`Presentation::cuts`, the map generation).
    pub cuts: (u64, u64),
    /// `ui_audio::TeleportEffect::amount` (the session marker's Go To Marker hold).
    pub teleport: Option<f32>,
    /// Master.in1..in3 (`AudioSettings::master_inputs`, the retail option volumes).
    pub master_inputs: [(usize, i32); 3],
}

/// The host pass of a frame (spec §1: inputs → components' process → tick → components' update),
/// on frames that took physics steps ([`HostClock::pass`]: as many host ticks as steps, at most
/// [`MAX_STEPS_PER_FRAME`]; none on a frame without a step, none while silenced). Written here:
/// Master.in1–4 (in1..3 = the option volumes), Music.in1/2/5, Reverb.in0..6 (free-skate values,
/// §7.4); through `player_audio`: PlayerPhysics 0–14, the two 3DObjPos blocks (skater COM and
/// board, with relative speeds and the sign-flip bits), Jitter, Contacts 1/2/6, Rail 0/1, OffBoard
/// 0; the board owner inputs (`grain_bed.rs`). Left at their free-skate 0: VU, Menu / NIS / HOM /
/// Challenge / Speech flags, HandGrabs, Music.in3/6, and Pause.in0. Class_Seams' console cadence
/// runs on every rendered frame that is not silenced
/// ([`PlayerAudio::seam_frame`](super::player_audio::PlayerAudio::seam_frame), game time).
pub(crate) fn mixmap_frame(native: &mut Native, cues: &mut Cues, f: &FrameInputs) {
    let silenced = f.silenced;
    // For the systems after this one (the bed): no pass until the clock says so.
    native.frame_ticks = 0;
    native.pending = None;
    if let Some(bed) = &mut native.bed {
        bed.slew_calls = Some(0);
    }
    if native.mixmap.is_none() {
        cues.take_steps();
        return;
    }
    // A teleport, camera cut or map change: no camera velocity across it (Doppler), and no seam
    // interpolation from the old place (the step at the cut is both ends of the pair).
    let signal = f.cuts;
    let now = cues.riding.audio.wheel_position;
    if native.cut_seen.is_some_and(|seen| seen != signal) {
        native.cuts += 1;
        native.cut_wheels = Some(now);
        if let Some(player) = &mut native.player {
            player.reset_listener();
        }
    }
    native.cut_seen = Some(signal);
    if native.cut_wheels.is_some_and(|w| w != now) {
        native.cut_wheels = None;
    }
    let before = if native.cut_wheels.is_some() {
        now
    } else {
        cues.riding.wheels_before
    };
    // Class_Seams on the console's 30 fps process cadence at the rendered board's wheels, on every
    // rendered frame (Listening test 9), in game time (`Time<Virtual>`: still while paused).
    let Some(m) = &mut native.mixmap else { return };
    if let Some(player) = &mut native.player {
        player.seam_alpha = Some(f.overstep);
        player.step_wheels(before, now);
        if !silenced {
            if let Ok(mut runtime) = native.shared.lock() {
                player.seam_frame(m, &cues.riding.audio, f.frame_dt, &mut runtime);
            }
        }
    }
    let Some(pass) = native.clock.pass(cues, silenced) else {
        return;
    };
    native.frame_ticks = pass.ticks;
    // Master.in1..in3 = the option volumes (Settings menu Music / SFX / Dialog) as retail's Master
    // step `sub_824D5160` writes them every pass; in4 is not an option volume (32767 in free skate).
    for (id, value) in f.master_inputs {
        m.set_input(keys::MASTER, id, value);
    }
    m.set_input(keys::MASTER, 4, 32767);
    for id in [1, 2, 5] {
        m.set_input(keys::MUSIC, id, 32767);
    }
    // SFXObj_Reverb's first step (`sub_824DF468`): Reverb.in0..6 from the number of the preset
    // being faded to (in5 = reverb11/12/15/16/22, which ducks the global env scale out4 by 4 dB).
    // (Without the runtime lock: in5 = 32767 and no scale this pass.)
    let reverb = native
        .shared
        .lock()
        .ok()
        .map(|r| r.mixer.buses.env.reverb_inputs());
    match reverb {
        Some(v) => {
            for (id, x) in v.into_iter().enumerate() {
                m.set_input(keys::REVERB, id, x);
            }
        }
        None => m.set_input(keys::REVERB, 5, 32767),
    }
    m.set_input(keys::PAUSE, 0, 0);
    let (ticks, calls) = (pass.ticks, pass.calls);
    // The bed's turn / brake slews step once per console evaluation (`grain_bed::Bed::slew_calls`).
    if let Some(bed) = &mut native.bed {
        bed.slew_calls = Some(calls);
    }
    if !native.holds {
        hold_flag_inputs(m);
        native.holds = true;
    }
    // sub_82491180 (half 1): the jittered eEQChain buses take the walk's values and every bus may
    // re-roll again.
    if pass.clear_eq {
        let jitter = native.player.as_ref().and_then(|p| p.eq_jitter());
        if let Ok(mut runtime) = native.shared.lock() {
            runtime.mixer.buses.eq.clear(jitter);
        }
    }
    let s = cues.riding.audio;
    if let Some(player) = &mut native.player {
        let dt = ticks as f32 * MIX_STEP;
        let t = &f.listener;
        let l = player.listener(
            t.translation.to_array(),
            t.forward().as_vec3().to_array(),
            dt,
            &s,
        );
        // SFXObj_Jitter's walk steps once per console evaluation (half 1's process).
        player.jitter_steps = Some(calls);
        // The presentation block's teleport field as of this pass (Class_Treatment's update reads it).
        player.teleport_effect = f.teleport;
        player.write_inputs(m, &s, Some(&l));
    }
    // With the native rolling layers the owner's surface routing (player::rolling) writes
    // SkateBoard inputs 0 / 6 and drives the bed's binds; otherwise the bed routes itself.
    let routing = native
        .player
        .as_ref()
        .is_some_and(|p| p.components && p.rolling_on);
    if let Some(bed) = &mut native.bed {
        bed.write_inputs(m, &s, !routing);
    }
    let speed_scale = native.bed.as_ref().and_then(|b| b.push_scale());
    let loose = super::player_audio::PlayerAudio::loose_board(&s, &cues.riding);
    if let (Some(player), Ok(mut runtime)) = (&mut native.player, native.shared.lock()) {
        player.process(m, &s, &mut runtime, speed_scale, loose);
    }
    // (The world / NPC owners' process ran next in skate-game; then [`mixmap_tick`].)
    native.pending = Some(PendingPass {
        calls,
        s,
        speed_scale,
        loose,
        reverb: reverb.is_some(),
    });
}

/// The second half of [`mixmap_frame`]'s pass: the console evaluations (MixMap ticks), the local
/// player's update and SFXObj_Reverb's update.
pub(crate) fn mixmap_tick(native: &mut Native) {
    let Some(PendingPass {
        calls,
        s,
        speed_scale,
        loose,
        reverb,
    }) = native.pending
    else {
        return;
    };
    let Some(m) = &mut native.mixmap else { return };
    for _ in 0..calls {
        m.tick(skate_audio::mixmap::cadence::CONSOLE_DT);
    }
    if let (Some(player), Ok(mut runtime)) = (&mut native.player, native.shared.lock()) {
        player.update(m, &s, &mut runtime, speed_scale, loose);
    }
    // SFXObj_Reverb's update (`sub_824DF220`): the FlangeSub returns' levels and the global env
    // scale (manager +104 = out4) from the Reverb owner.
    if let Ok(mut runtime) = native.shared.lock() {
        runtime
            .mixer
            .buses
            .flange
            .frame(std::array::from_fn(|i| m.level(keys::REVERB, i)));
        if reverb {
            runtime
                .mixer
                .buses
                .env
                .scale_frame(m.level(keys::REVERB, 4));
        }
    }
}

/// The reverb preset (`SFXObj_Reverb`, `sub_824DE548`): the district's `audio_reverb` region at the
/// skater's x, z (reverb01 when none: always here, there are no map regions), crossfaded over 1 s
/// of game time; per frame. `dt`: `Time<Real>`'s delta.
pub(crate) fn reverb_frame(native: &Native, listener: &Transform, dt: f32) {
    let key = skate_audio::bus::env::DEFAULT_PRESET;
    let camera = Some(skate_audio::bus::zones::Camera {
        position: listener.translation.to_array(),
        forward: listener.forward().as_vec3().to_array(),
    });
    if let Ok(mut runtime) = native.shared.lock() {
        let env = &mut runtime.mixer.buses.env;
        if env.enabled() {
            // SFXObj_Reverb's update (`sub_824DE548`) in retail's order (no reverb-zone emitters).
            env.update(dt.clamp(0.0, 0.25), key, &[], camera.as_ref());
        }
    }
}
