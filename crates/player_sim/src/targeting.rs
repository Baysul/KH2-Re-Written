//! The player's automatic target, translated from the PC executable: the
//! chooser FUN_1403be4b0 (mode 6 of FUN_1403be2b0) and the target holder's
//! automatic states in FUN_1403dcd70 / FUN_1403dd650 / FUN_1403dccd0.
//!
//! The parameters come from the player's 03system `plyr` record (obj+0x150):
//! +0x5c range, +0x60 / +0x64 Y-down height band, +0x68 hold ticks, +0x6c
//! height weight, +0x70 angle weight. Heights here are Y-up.

use crate::Vec3;
use crate::ptya::Target;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub range: f32,
    pub dy_min: f32,
    pub dy_max: f32,
    pub hold: f32,
    pub height_weight: f32,
    pub angle_weight: f32,
}

impl Params {
    /// Reads the fields from a `plyr` record (None when short).
    pub fn read(record: &[u8]) -> Option<Self> {
        let f = |at: usize| {
            record
                .get(at..at + 4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        };
        Some(Params {
            range: f(0x5c)?,
            dy_min: f(0x60)?,
            dy_max: f(0x64)?,
            hold: f(0x68)?,
            height_weight: f(0x6c)?,
            angle_weight: f(0x70)?,
        })
    }
}

/// `DAT_142a1141c` (FUN_1403aba10), 1.0 after FUN_1403abab0.
pub const SCALE: f32 = 1.;
/// `DAT_140623a4c`.
const HALF: f32 = 0.5;

/// FUN_1403be4b0: the lowest-scoring candidate shape, by index. `range` 0
/// or less skips the band and range tests.
pub fn choose(
    p: &Params,
    origin: Vec3,
    facing: [f32; 2],
    candidates: &[Target],
    range: f32,
) -> Option<usize> {
    let (mut dy_min, mut dy_max) = (p.dy_min, p.dy_max);
    if 1. < SCALE {
        dy_min *= SCALE;
        dy_max *= SCALE;
    }
    let mut best: Option<(usize, f32)> = None;
    for (i, t) in candidates.iter().enumerate() {
        let v = [t.centre[0] - origin[0], t.centre[2] - origin[2]];
        let dy = origin[1] - t.centre[1];
        let d = v[0].hypot(v[1]);
        let dir = if d != 0. { [v[0] / d, v[1] / d] } else { v };
        if range > 0.
            && !(dy_min - t.height <= dy && dy <= t.height + dy_max && d - t.radius <= range)
        {
            continue;
        }
        let cos = facing[0] * dir[0] + facing[1] * dir[1];
        let score = (d + dy.abs() * p.height_weight)
            * ((1. - cos) * HALF * (1. - p.angle_weight) + p.angle_weight);
        if best.is_none_or(|(_, s)| score < s) {
            best = Some((i, score));
        }
    }
    best.map(|(i, _)| i)
}

/// The holder's automatic states: 0 none, 1 automatic target.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Holder {
    /// The held candidate (an index the caller keeps stable).
    pub target: Option<usize>,
    /// +0x20: ticks the target is held without re-choosing.
    pub timer: f32,
}

impl Holder {
    /// FUN_1403dcd70 states 0 and 1, once per frame. `valid` says whether
    /// a candidate index can still be targeted (FUN_1403bde30).
    pub fn update(
        &mut self,
        p: &Params,
        origin: Vec3,
        facing: [f32; 2],
        candidates: &[Target],
        valid: impl Fn(usize) -> bool,
        dt: f32,
    ) {
        if let Some(t) = self.target
            && 0. < self.timer
            && valid(t)
        {
            self.timer -= dt;
            return;
        }
        let found = choose(p, origin, facing, candidates, SCALE * p.range).filter(|&i| valid(i));
        match (self.target, found) {
            // State 0 → 1 (FUN_1403dd650 with timer 0) or a switch.
            (_, Some(i)) => {
                if self.target != Some(i) {
                    self.timer = 0.;
                }
                self.target = Some(i);
            }
            // Nothing in range: released (FUN_1403dd510).
            (_, None) => *self = Holder::default(),
        }
    }

    /// FUN_1403dccd0: an attack turning to the target holds it for the
    /// `plyr` hold time.
    pub fn hold(&mut self, p: &Params) {
        if self.target.is_some() {
            self.timer = p.hold;
        }
    }

    /// FUN_1403aa560: when an attack starts without a target, take the best
    /// candidate at any range and hold it.
    pub fn acquire(
        &mut self,
        p: &Params,
        origin: Vec3,
        facing: [f32; 2],
        candidates: &[Target],
        valid: impl Fn(usize) -> bool,
    ) {
        if self.target.is_some_and(&valid) {
            return;
        }
        if let Some(i) = choose(p, origin, facing, candidates, 0.).filter(|&i| valid(i)) {
            self.target = Some(i);
            self.timer = p.hold;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params {
            range: 300.,
            dy_min: -435.,
            dy_max: 50.,
            hold: 60.,
            height_weight: 0.4,
            angle_weight: 0.25,
        }
    }

    fn at(x: f32, z: f32) -> Target {
        Target {
            centre: [x, 50., z],
            radius: 45.,
            height: 45.,
        }
    }

    #[test]
    fn ahead_beats_behind_and_range_limits() {
        let p = params();
        let c = [at(0., -150.), at(0., 200.)];
        assert_eq!(choose(&p, [0.; 3], [0., 1.], &c, 300.), Some(1));
        let far = [at(0., 400.)];
        assert_eq!(choose(&p, [0.; 3], [0., 1.], &far, 300.), None);
        assert_eq!(choose(&p, [0.; 3], [0., 1.], &far, 0.), Some(0));
    }

    #[test]
    fn holding_keeps_a_target_out_of_range_until_the_timer_runs_out() {
        let p = params();
        let mut h = Holder::default();
        let near = [at(0., 100.)];
        h.update(&p, [0.; 3], [0., 1.], &near, |_| true, 1.);
        assert_eq!(h.target, Some(0));
        h.hold(&p);
        let gone = [at(0., 900.)];
        for _ in 0..60 {
            h.update(&p, [0.; 3], [0., 1.], &gone, |_| true, 1.);
            assert_eq!(h.target, Some(0));
        }
        h.update(&p, [0.; 3], [0., 1.], &gone, |_| true, 1.);
        assert_eq!(h.target, None);
    }
}
