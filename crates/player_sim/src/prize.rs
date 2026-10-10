//! Enemy prizes and EXP, translated from the PC executable: the drop
//! routine FUN_1403dc170 (przt counts scaled by Jackpot, munny turned into
//! drive by Drive Converter), the pickup rule FUN_1403dc500, the effect
//! table at 0x1405c7420 and the kill EXP of FUN_140410af0.

use crate::Vec3;
use crate::physics;

/// Prize kinds, in przt count order.
pub mod kind {
    pub const SMALL_HP: u8 = 0;
    pub const BIG_HP: u8 = 1;
    pub const BIG_MUNNY: u8 = 2;
    pub const MEDIUM_MUNNY: u8 = 3;
    pub const SMALL_MUNNY: u8 = 4;
    pub const SMALL_MP: u8 = 5;
    pub const BIG_MP: u8 = 6;
    pub const SMALL_DRIVE: u8 = 7;
    pub const BIG_DRIVE: u8 = 8;
}

/// What a prize does when collected (table 0x1405c7420 bytes +4..+7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effect {
    pub hp: i32,
    pub mp: i32,
    pub drive: i32,
    pub munny: i32,
}

pub fn effect(kind: u8) -> Effect {
    let e = |hp, mp, drive, munny| Effect {
        hp,
        mp,
        drive,
        munny,
    };
    match kind {
        kind::SMALL_HP => e(1, 0, 0, 0),
        kind::BIG_HP => e(5, 0, 0, 0),
        kind::BIG_MUNNY => e(0, 0, 0, 10),
        kind::MEDIUM_MUNNY => e(0, 0, 0, 5),
        kind::SMALL_MUNNY => e(0, 0, 0, 1),
        kind::SMALL_MP => e(0, 1, 0, 0),
        kind::BIG_MP => e(0, 3, 0, 0),
        kind::SMALL_DRIVE => e(0, 0, 1, 0),
        kind::BIG_DRIVE => e(0, 0, 3, 0),
        _ => Effect::default(),
    }
}

/// The pickup centre's height above the prize's origin (descriptor +4).
pub fn centre_height(kind: u8) -> f32 {
    const H: [f32; 9] = [12.6, 27., 38.4, 25.6, 12.8, 16., 28.8, 14.4, 27.2];
    H.get(kind as usize).copied().unwrap_or(0.)
}

/// Munny turns into drive with Drive Converter: big and medium into big
/// drive, small into small drive (table 0x1405c7978).
fn converted(kind: u8) -> u8 {
    match kind {
        kind::BIG_MUNNY | kind::MEDIUM_MUNNY => kind::BIG_DRIVE,
        _ => kind::SMALL_DRIVE,
    }
}

/// FUN_1403ecef0: ceil(count x Jackpot rate).
fn scaled(count: u8, jackpot: f32) -> i32 {
    (count as f32 * jackpot).ceil() as i32
}

/// The prizes one przt record drops: (kind, count) pairs. `jackpot` is
/// 1.0 + the party's Jackpot terms; `converter` the product of the Drive
/// Converter factors (1.0 without it).
pub fn drops(counts: &[u8; 9], jackpot: f32, converter: f32) -> Vec<(u8, i32)> {
    let mut out = Vec::new();
    for (k, &c) in counts.iter().enumerate() {
        let k = k as u8;
        let n = scaled(c, jackpot);
        if (kind::BIG_MUNNY..=kind::SMALL_MUNNY).contains(&k) {
            let keep = (n as f32 * converter) as i32;
            if n - keep > 0 {
                out.push((converted(k), n - keep));
            }
            if keep > 0 {
                out.push((k, keep));
            }
        } else if n > 0 {
            out.push((k, n));
        }
    }
    out
}

/// [I] The drawn velocities (+-20 / 20..40) are scaled by 0.1 per tick,
/// like the knock-slide's strength; the prize physics FUN_1404132d0 was not
/// traced.
const VELOCITY_SCALE: f32 = 0.1;

/// A prize in the world: it falls and bounces, then waits to be collected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orb {
    pub kind: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub landed: bool,
}

impl Orb {
    /// FUN_1403dc3d0: the initial velocity from three draws in [0, 1):
    /// x = r3 x 40 - 20, up 20..40 (the exe's Y-down -40..-20), z = r1 x 40 - 20.
    pub fn spawn(kind: u8, at: Vec3, r: [f32; 3]) -> Orb {
        Orb {
            kind,
            position: at,
            velocity: [
                (r[2] * 40. - 20.) * VELOCITY_SCALE,
                (40. - r[1] * 20.) * VELOCITY_SCALE,
                (r[0] * 40. - 20.) * VELOCITY_SCALE,
            ],
            landed: false,
        }
    }

    /// One tick of the fall and bounce [I: the exe's bounce constants were
    /// not traced; gravity is the actors' 0.408 and a bounce keeps half the
    /// speed]. `floor` is the ground height below the orb.
    pub fn tick(&mut self, floor: f32) {
        if self.landed {
            return;
        }
        self.velocity[1] -= physics::G;
        for i in 0..3 {
            self.position[i] += self.velocity[i];
        }
        if self.position[1] <= floor {
            self.position[1] = floor;
            if self.velocity[1].abs() < 2. {
                self.velocity = [0.; 3];
                self.landed = true;
            } else {
                self.velocity[1] = -self.velocity[1] * 0.5;
                self.velocity[0] *= 0.5;
                self.velocity[2] *= 0.5;
            }
        }
    }

    /// FUN_1403dc500: within 120 (+ Draw) horizontally and 30 vertically
    /// of the prize's centre.
    pub fn collected_by(&self, body: Vec3, draw: f32) -> bool {
        if !self.landed {
            return false;
        }
        let centre = [
            self.position[0],
            self.position[1] + centre_height(self.kind),
            self.position[2],
        ];
        let d = [
            centre[0] - body[0],
            centre[1] - body[1],
            centre[2] - body[2],
        ];
        d[1].abs() <= 30. && d[0].hypot(d[2]) <= 120. + draw
    }
}

/// FUN_140410af0: EXP of a kill: (enemy EXP x the level row's EXP + 99) /
/// 100, three quarters on Critical (at least 1), times Experience Boost
/// copies at or below half HP.
pub fn kill_exp(enemy_exp: u16, level_exp: u16, critical: bool, boost: Option<u32>) -> u32 {
    let mut exp = (enemy_exp as u32 * level_exp as u32).div_ceil(100);
    if exp > 0 {
        if critical {
            exp = (exp * 75 / 100).max(1);
        }
        if let Some(b) = boost {
            exp *= b;
        }
    }
    exp
}

/// FUN_1403ecf30's loop for one character: the level reached by `exp`
/// total, given the EXP needed to leave each level (None past the table).
pub fn level_for(exp: u32, mut level: u8, needed: impl Fn(u8) -> Option<u32>) -> u8 {
    while level < 99 {
        match needed(level) {
            Some(n) if exp >= n => level += 1,
            _ => break,
        }
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shadow_drops_one_hp_orb_and_one_munny() {
        let d = drops(&[1, 0, 0, 0, 1, 0, 0, 0, 0], 1., 1.);
        assert_eq!(d, vec![(kind::SMALL_HP, 1), (kind::SMALL_MUNNY, 1)]);
        // Jackpot rounds counts up; Drive Converter moves munny to drive.
        let j = drops(&[1, 0, 0, 0, 1, 0, 0, 0, 0], 1.5, 0.5);
        assert_eq!(
            j,
            vec![
                (kind::SMALL_HP, 2),
                (kind::SMALL_DRIVE, 1),
                (kind::SMALL_MUNNY, 1)
            ]
        );
    }

    #[test]
    fn kill_exp_follows_the_row_difficulty_and_boost() {
        // The Shadow: 15 x 4 -> 1; 15 x 16 -> 3.
        assert_eq!(kill_exp(15, 4, false, None), 1);
        assert_eq!(kill_exp(15, 16, false, None), 3);
        assert_eq!(kill_exp(15, 16, true, None), 2);
        assert_eq!(kill_exp(15, 4, true, None), 1);
        assert_eq!(kill_exp(15, 16, false, Some(2)), 6);
        assert_eq!(kill_exp(0, 16, false, Some(2)), 0);
    }

    #[test]
    fn levels_rise_while_exp_passes_the_thresholds() {
        let need = |l: u8| [40, 100, 184].get(l as usize - 1).copied();
        assert_eq!(level_for(39, 1, need), 1);
        assert_eq!(level_for(40, 1, need), 2);
        assert_eq!(level_for(190, 1, need), 4);
    }

    #[test]
    fn an_orb_falls_bounces_rests_and_is_collected_in_range() {
        let mut o = Orb::spawn(kind::SMALL_HP, [0., 10., 0.], [0.5, 0.5, 0.5]);
        for _ in 0..400 {
            o.tick(0.);
        }
        assert!(o.landed);
        assert_eq!(o.position[1], 0.);
        assert!(o.collected_by([o.position[0] + 100., 0., o.position[2]], 0.));
        assert!(!o.collected_by([o.position[0] + 130., 0., o.position[2]], 0.));
        assert!(o.collected_by([o.position[0] + 200., 0., o.position[2]], 125.));
    }
}
