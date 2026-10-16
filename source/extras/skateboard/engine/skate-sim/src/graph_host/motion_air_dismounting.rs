//! TU3 AirDismounting Begin82BB93E0, Update82BB94C0, empty End82B61BB8.
use skate_core::animation::{output::attributes::AnimationAttribute, skeleton_input::name::encode};

#[derive(Default)]
pub(super) struct State {
    frames: i32,
}
impl State {
    pub(super) fn begin(&mut self, length: f32, time: f32) -> Result<(), String> {
        let frames = (length - time) * 59.999996_f32;
        // Keep malformed animation timing diagnosable instead of silently
        // converting NaN/infinity to an integer. This does not repair physics.
        if !frames.is_finite() || frames >= 2_147_483_648.0 {
            return Err(format!(
                "AirDismounting invalid animation timing: length={length} time={time}"
            ));
        }
        // Native fsel clamps to one, then fctiwz truncates. It captures once
        // on Begin; this is neither a countdown nor based on the render dt.
        self.frames = frames.max(1.0) as i32;
        Ok(())
    }
    pub(super) fn request(&self, attributes: &[AnimationAttribute]) -> Option<i32> {
        // Animator v96 is the collected-tree membership query. Payload/status
        // do not gate this event, and an absent marker leaves the count alone.
        attributes
            .iter()
            .any(|a| a.name == encode(b"airdismountrevert"))
            .then_some(self.frames)
    }
}
