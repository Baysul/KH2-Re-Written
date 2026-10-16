//! Attack objects (`YS::ATTACK`): the damage volumes scripts create. A script
//! creates one, then keeps moving and shaping it each frame; the host tests it
//! against what it can hit and records the result in its flags.
//!
//! The state and the flag words follow the executable's object (offsets are
//! its own: flags +0x8, position +0xe4, previous position +0xf4, radii
//! +0x104/+0x108, ...; Steam build 15194255). [U] The per-frame hit test and
//! the damage it applies are not part of the engine yet: `hit` flags stay clear
//! until a host sets them with [`Attack::mark_hit`].

use crate::world::ObjId;

/// Flag bits of +0x8.
pub mod flag {
    /// The position has been set at least once.
    pub const HAS_POSITION: u32 = 0x1;
    /// The attack struck something.
    pub const STRUCK: u32 = 0x2;
    /// The position moved farther than a step since the last frame.
    pub const MOVED_FAR: u32 = 0x4;
    /// Hit the map: wall (bit 3) or floor (bit 4).
    pub const HIT_WALL: u32 = 0x8;
    pub const HIT_FLOOR: u32 = 0x10;
    /// The volume is a line from the position to the previous position.
    pub const LINE: u32 = 0x20;
    /// Reflected by a guard (bit 9).
    pub const REFLECTED: u32 = 0x200;
    /// Freed by its script; the host removes it.
    pub const FREED: u32 = 0x800;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attack {
    /// The engine object that owns it (`None` when the script passed none).
    pub owner: Option<ObjId>,
    /// +0x18 and +0x1c: the attack parameter record's key.
    pub param_id: i32,
    pub param_extra: i32,
    /// +0x24: the combo group the hits count toward.
    pub group: i32,
    /// +0x8.
    pub flags: u32,
    /// +0xe4 and +0xf4: the position (a line's start) and the previous
    /// position (a line's end), four lanes each.
    pub position: [f32; 4],
    pub previous: [f32; 4],
    /// +0x104 / +0x108.
    pub radius: (f32, f32),
    /// +0xcc..+0xd8: a timed shape change (elapsed, duration, radii).
    pub time: [f32; 4],
    /// +0x38 / +0x39: the team and the mask of teams it can hit.
    pub team: u8,
    pub team_mask: u8,
    /// +0x98..+0xa0: the script entry called when it hits, and the AI object
    /// it runs on.
    pub hit_callback: Option<(i32, u32)>,
    /// +0xc8: the effect handle attached to the volume.
    pub effect: u32,
    /// +0x28: the engine object whose PAX container supplies its effects.
    pub pax_object: Option<ObjId>,
    /// +0x114..+0x124: the receiver and its parameters (`attack_set_rc`).
    pub receiver: u32,
    pub rc: [i32; 3],
    /// +0xb4: where it hit the map.
    pub hit_position: [f32; 4],
}

impl Attack {
    /// FUN_1404371b0 / FUN_1403cf830: a new attack. `team` is the owner's team
    /// (3 counts as 4).
    pub fn new(
        owner: Option<ObjId>,
        param_id: i32,
        group: i32,
        param_extra: i32,
        team: i32,
    ) -> Self {
        let mut a = Self {
            owner,
            param_id,
            param_extra,
            group,
            flags: 0,
            position: [0.; 4],
            previous: [0.; 4],
            radius: (1., 1.),
            time: [0.; 4],
            team: 0,
            team_mask: 0,
            hit_callback: None,
            effect: 0,
            pax_object: None,
            receiver: 0,
            rc: [0; 3],
            hit_position: [0.; 4],
        };
        a.set_team(team);
        a
    }

    /// FUN_1403d2390.
    pub fn set_team(&mut self, team: i32) {
        let t = if team == 3 { 4 } else { team as u8 };
        self.team = t;
        self.team_mask = !((1u32.checked_shl(u32::from(t) & 0x1f).unwrap_or(0) | 1) as u8);
    }

    /// FUN_140437c00: the radii; a non-positive second radius repeats the first.
    pub fn set_radius(&mut self, r0: f32, r1: f32) {
        self.radius = (r0, if r1 > 0. { r1 } else { r0 });
    }

    /// FUN_140437a80: move the volume. `step` is twice the frame time.
    pub fn set_position(&mut self, v: [f32; 4], frame_time: f32) {
        if self.flags & flag::HAS_POSITION == 0 {
            self.previous = v;
        } else {
            self.previous = self.position;
            let d = [
                v[0] - self.position[0],
                v[1] - self.position[1],
                v[2] - self.position[2],
            ];
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if len <= frame_time + frame_time {
                self.flags &= !flag::MOVED_FAR;
            } else {
                self.flags |= flag::MOVED_FAR;
            }
        }
        self.position = v;
        self.flags = self.flags & !flag::LINE | flag::HAS_POSITION;
    }

    /// FUN_140437a40: a line from `start` to `end`.
    pub fn set_line(&mut self, start: [f32; 4], end: [f32; 4]) {
        self.position = start;
        self.previous = end;
        self.flags |= flag::LINE;
    }

    /// FUN_140437c30: a timed shape change. A non-positive radius keeps the
    /// attack's own.
    pub fn set_time(&mut self, duration: f32, r0: f32, r1: f32) {
        self.time = [
            0.,
            duration,
            if r0 > 0. { r0 } else { self.radius.0 },
            if r1 > 0. { r1 } else { self.radius.1 },
        ];
    }

    /// FUN_1403d1f20: struck anything (a target, the map or a guard).
    pub fn is_hit(&self) -> bool {
        self.flags & (flag::STRUCK | flag::HIT_WALL | flag::HIT_FLOOR | flag::REFLECTED) != 0
    }

    /// For the host: record that the attack struck a target, the map or a guard.
    pub fn mark_hit(&mut self, bits: u32) {
        self.flags |= bits;
    }
}

#[cfg(test)]
mod tests;
