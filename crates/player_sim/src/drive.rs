//! Sora's Drive gauge in normal mode, translated from the PC executable:
//! the integer gauge add FUN_1403c0ad0, the payment FUN_1403c2780, the gain
//! from hits FUN_1403a8800 and the central gain FUN_1403a9cf0. One bar is
//! 100 points.

/// SSTM Drive fields (`DAT_142ae5760`): DriveEnemy +0x60 (hits on ZAKO
/// ×0.75) and the MP-charge multiplier +0x78 (2.0).
pub const DRIVE_ENEMY: f32 = 0.75;
pub const CHARGE_MULTIPLIER: f32 = 2.0;

/// stat+0x1b0 / +0x1b1 / +0x1b2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gauge {
    /// Partial bar, 0..=100 (100 only when full at the maximum).
    pub partial: u8,
    /// Full bars.
    pub level: u8,
    /// Maximum bars.
    pub max: u8,
}

impl Gauge {
    pub fn new(max: u8) -> Self {
        Gauge {
            partial: 0,
            level: 0,
            max,
        }
    }

    /// FUN_1403c0ad0: a negative add borrows at most one bar; a positive
    /// one carries into bars up to the maximum.
    pub fn add(&mut self, delta: i32) {
        let mut v = self.partial as i32 + delta;
        if delta < 0 {
            if v < 0 {
                if self.level != 0 {
                    self.level -= 1;
                    self.partial = (v + 100) as u8;
                } else {
                    self.partial = 0;
                }
                return;
            }
        } else if v > 99 {
            let mut lv = self.level;
            loop {
                if self.max <= lv {
                    self.level = lv;
                    self.partial = 100;
                    return;
                }
                lv += 1;
                self.level = lv;
                if self.max <= lv {
                    self.partial = 100;
                    return;
                }
                v -= 100;
                if v <= 99 {
                    break;
                }
            }
        }
        self.partial = v as u8;
    }

    /// FUN_1403c2780: pay a form's cost in bars; a full gauge also loses
    /// its partial bar, and paying more than held empties it.
    pub fn pay(&mut self, cost: u8) {
        if self.level == self.max || self.level < cost {
            self.partial = 0;
        }
        self.level = self.level.saturating_sub(cost);
    }

    /// FUN_1403a9cf0 in mode 1: while MP charges the gain is ×2 × Drive
    /// Boost; the result truncates to whole points (`area` is the event
    /// script's rate, 1.0 by default).
    pub fn gain(&mut self, v: f32, mp_charging: bool, drive_boost: f32, area: f32) {
        let v = if mp_charging {
            v * CHARGE_MULTIPLIER * drive_boost
        } else {
            v
        };
        self.add((v * area) as i32);
    }

    /// FUN_1403a8800: a hit's DriveDrain on an enemy (×0.75 on ZAKO).
    pub fn hit(&mut self, drain: u8, zako: bool, mp_charging: bool, drive_boost: f32) {
        if drain == 0 {
            return;
        }
        let mut v = drain as f32;
        if zako {
            v *= DRIVE_ENEMY;
        }
        self.gain(v, mp_charging, drive_boost, 1.);
    }

    /// FUN_1403a85f0: damage taken gives `damage / MaxHP × Damage Drive`.
    pub fn damaged(&mut self, damage: i32, max_hp: i32, damage_drive: f32) {
        if damage > 0 && max_hp > 0 && damage_drive > 0. {
            self.gain(damage as f32 / max_hp as f32 * damage_drive, false, 1., 1.);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_carry_into_bars_and_stop_at_the_maximum() {
        let mut g = Gauge::new(3);
        g.add(60);
        g.add(60);
        assert_eq!((g.level, g.partial), (1, 20));
        g.add(250);
        assert_eq!((g.level, g.partial), (3, 100));
        g.add(10);
        assert_eq!((g.level, g.partial), (3, 100));
    }

    #[test]
    fn a_negative_add_borrows_one_bar() {
        let mut g = Gauge {
            partial: 10,
            level: 2,
            max: 3,
        };
        g.add(-30);
        assert_eq!((g.level, g.partial), (1, 80));
        let mut z = Gauge::new(3);
        z.add(-5);
        assert_eq!((z.level, z.partial), (0, 0));
    }

    #[test]
    fn paying_empties_the_partial_bar_of_a_full_gauge() {
        let mut g = Gauge {
            partial: 100,
            level: 3,
            max: 3,
        };
        g.pay(3);
        assert_eq!((g.level, g.partial), (0, 0));
        let mut h = Gauge {
            partial: 40,
            level: 5,
            max: 9,
        };
        h.pay(3);
        assert_eq!((h.level, h.partial), (2, 40));
        let mut short = Gauge {
            partial: 40,
            level: 2,
            max: 9,
        };
        short.pay(3);
        assert_eq!((short.level, short.partial), (0, 0));
    }

    #[test]
    fn hits_gain_by_drain_with_zako_and_charge_factors() {
        let mut g = Gauge::new(3);
        g.hit(8, true, false, 1.);
        assert_eq!(g.partial, 6);
        g.hit(8, false, true, 1.5);
        assert_eq!(g.partial, 6 + 24);
        g.hit(0, true, false, 1.);
        assert_eq!(g.partial, 30);
        g.damaged(10, 20, 20.);
        assert_eq!(g.partial, 40);
    }
}
