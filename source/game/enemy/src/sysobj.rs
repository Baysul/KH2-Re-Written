//! The engine object ("sysobj") an enemy script drives. Offsets in comments
//! are the executable's. Subsystems own their state in their own modules so
//! that each can be translated and extended independently.

pub type Vec3 = [f32; 3];

#[derive(Clone, Debug, Default)]
pub struct Sysobj {
    /// +0x670 (+0x70 when attached): world position.
    pub position: Vec3,
    /// +0x7a8-adjacent: heading about Y.
    pub yaw: f32,
    /// +0x30: scale; +0x34 is the Y component scripts animate.
    pub scale: Vec3,
    /// +0x120: script-settable flag word (`obj_set_flag` / `obj_reset_flag`).
    pub flags: u32,
    /// +0x18c: motion state bits (bit 1 motion ended, bit 9 no control, bit 18 act manager paused).
    pub motion_state: u32,
    /// +0x6c8: status bits (bit 1/3 hit-stun motion classes, bit 7 attacker is the player).
    pub status: u32,
}
