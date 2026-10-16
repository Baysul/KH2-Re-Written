//! What the host game supplies to the enemy engine: motion clip data, the
//! collision world and the other actors. All of it is read from the installed
//! game by the host; this crate holds no game data.

use crate::sysobj::Vec3;

/// Motion data of one model, in ticks. The host selects an explicit
/// evaluation profile; absence of a range or flag is `None`, never a guess.
pub trait Clips {
    /// Clip length in ticks (0 = the motion slot does not exist).
    fn length(&self, slot: usize) -> u32;
    fn loop_start(&self, slot: usize) -> u32;
    /// Root displacement over the step after `tick`, in model space.
    fn root_delta(&self, slot: usize, tick: u32) -> Vec3;
    /// Motion clock producer: the native 0 -> 2 first step, then +1.
    fn advance(&self, _slot: usize, tick: u32) -> u32 {
        tick.saturating_add(1)
    }
    /// The clip's trigger tables (`None` = the slot has none).
    fn events(&self, slot: usize) -> Option<&crate::motion::MotionEvents>;
    /// The clip finished on the step that produced `tick` (the end pulse).
    fn ended(&self, slot: usize, tick: u32) -> bool {
        tick >= self.length(slot)
    }
    fn in_loop(&self, slot: usize, tick: u32) -> bool {
        tick >= self.loop_start(slot)
    }
}

/// The world around the actor.
pub trait Env {
    /// Highest walkable floor at (x, z) below `top`.
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32>;
    /// The final position after a requested displacement (walls and floor).
    fn movement(&self, from: Vec3, wished: Vec3) -> Vec3 {
        let mut to = [0, 1, 2].map(|k| from[k] + wished[k]);
        if let Some(y) = self.floor(to[0], to[2], from[1].max(to[1]) + 200.) {
            to[1] = to[1].max(y);
        }
        to
    }
    /// The engine's ground probe (0 level, 1 higher, 2 lower, 3 no ground).
    fn check_step(
        &self,
        _position: Vec3,
        _direction: Vec3,
        _probe: f32,
        _up: f32,
        _down: f32,
    ) -> Option<u8> {
        None
    }
    /// The motion clips of an object's model.
    fn clips(&self, model: u32) -> &dyn Clips;
    /// Sora.
    fn player_position(&self) -> Vec3;
    /// Whether the map blocks the line between two points (`bghit_check_line`).
    fn line_blocked(&self, _from: Vec3, _to: Vec3) -> bool {
        false
    }
    /// Sora's heading about Y (the direction scripts read from the player object).
    fn player_yaw(&self) -> f32 {
        0.
    }
    /// Position of a target actor (centre of its TARGET shape).
    fn target_position(&self, id: u32) -> Option<Vec3>;
    /// The battle target table's choice for an actor (`None`: no entry).
    fn pick_target(&self, _kind: i32, _from: Vec3) -> Option<u32> {
        None
    }
    fn is_culled(&self, _position: Vec3) -> bool {
        false
    }
    /// `DAT_142a11400` bit 2: enemies may not leave.
    fn no_leave(&self) -> bool {
        false
    }
    /// `DAT_142a11400` bit 7: every enemy leaves.
    fn force_leave(&self) -> bool {
        false
    }
    fn event_running(&self) -> bool {
        false
    }
    fn tutorial_open(&self) -> bool {
        false
    }
    fn area_world(&self) -> i32 {
        0
    }
    /// A game progress flag (FUN_14039a980).
    fn progress_flag(&self, _flag: u32) -> bool {
        false
    }
    /// An effect handle `trap_obj_effect_start_bind` returned is still alive.
    fn effect_active(&self, _handle: u32) -> bool {
        false
    }
    fn effect_position(&self, _handle: u32) -> Option<Vec3> {
        None
    }
}

/// A host with nothing in it: a flat floor at zero, no motion data, Sora at
/// the origin. For tools and tests.
pub struct NullEnv;

struct NoClips;
impl Clips for NoClips {
    fn length(&self, _: usize) -> u32 {
        0
    }
    fn loop_start(&self, _: usize) -> u32 {
        0
    }
    fn root_delta(&self, _: usize, _: u32) -> Vec3 {
        [0.; 3]
    }
    fn events(&self, _: usize) -> Option<&crate::motion::MotionEvents> {
        None
    }
}

impl Env for NullEnv {
    fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
        Some(0.)
    }
    fn clips(&self, _: u32) -> &dyn Clips {
        &NoClips
    }
    fn player_position(&self) -> Vec3 {
        [0.; 3]
    }
    fn target_position(&self, _: u32) -> Option<Vec3> {
        None
    }
}
