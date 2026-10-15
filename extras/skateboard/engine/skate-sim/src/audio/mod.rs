//! skate-game's `game_audio`, the parts that make the local skater's sounds (rolling, grain bed,
//! wheels, contacts, tricks, grinds, landings, bails, footsteps, foley, pushes) and the MixMap,
//! without Bevy's audio and ECS. GPL-3.0-only like the rest of skate-sim. The public entry point is
//! [`crate::host_audio::SkaterAudio`].
pub(crate) mod grain_bed;
pub(crate) mod library;
pub(crate) mod native;
pub(crate) mod player_audio;
pub(crate) mod skate_events;

pub(crate) use library::Library;

/// Retail's option volumes, 0..1 (skate-game `game_audio::RetailVolumes`, the Settings menu rows
/// SFX / Dialog / Music, default 1.0 each). Retail's routing (TU3 `sub_824D5160`, the MixMap Master
/// controller's step, every pass): input = trunc(volume × 32767.0), clamped to 0..32767, written to
/// Master.in1 (Music), Master.in2 (SFX) and Master.in3 (Dialog).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RetailVolumes {
    pub(crate) sfx: f32,
    pub(crate) dialog: f32,
    pub(crate) music: f32,
}
impl Default for RetailVolumes {
    fn default() -> Self {
        Self {
            sfx: 1.0,
            dialog: 1.0,
            music: 1.0,
        }
    }
}
impl RetailVolumes {
    /// The Master controller input of one option volume (`sub_824D5160`).
    pub(crate) fn master_input(volume: f32) -> i32 {
        // `as` truncates toward zero like `fctiwz` and maps NaN to 0.
        ((volume * 32767.0) as i32).clamp(0, 32767)
    }
    /// (Master input id, value) in retail's write order: in1 Music, in2 SFX, in3 Dialog.
    pub(crate) fn master_inputs(&self) -> [(usize, i32); 3] {
        [
            (1, Self::master_input(self.music)),
            (2, Self::master_input(self.sfx)),
            (3, Self::master_input(self.dialog)),
        ]
    }
}
