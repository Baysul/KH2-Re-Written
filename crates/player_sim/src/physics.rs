//! Shared actor physics translated from the KH2FM executable (Steam build
//! 15194255): the per-object movement parameters, the closed-form jump and
//! fall curve, air steering, the knock-slide and the blow flight.
//!
//! The executable works in Y-down space (a negative Y velocity rises). These
//! routines keep its arithmetic and take or return Y-up values at the edges,
//! so callers stay in the simulation's Y-up space. One tick is one 60 Hz
//! frame; `DT` is the frame time `DAT_140717480` (1.0 at normal speed).

/// Frame time per tick (`DAT_140717480`, statically 1.0).
pub const DT: f32 = 1.0;
/// Gravity, units per tick squared (`DAT_1405ce184` = 0x3ed0fac7).
pub const G: f32 = 0.408_163_28;
/// Air steering: the share of last tick's velocity kept while the stick is
/// held (`DAT_1405aaa40`, passed by FUN_1403cb360).
pub const AIR_INERTIA: f32 = 0.9;
/// Air drag with the stick released (`DAT_1405b1c74`, FUN_1403cb360).
pub const AIR_DRAG: f32 = 0.95;
/// Motion blend for locomotion and the fall motion (`DAT_140623bac`,
/// FUN_140422520 / FUN_1403cb610), in motion-clock ticks: FUN_1403c6dc0
/// adds it to the motion time, which advances by DT per tick.
pub const MOTION_BLEND: f32 = 8.0;
/// The fall motion starts this many ticks before the apex
/// (`DAT_140623b98`, FUN_1403cb680).
pub const FALL_MOTION_LEAD: f32 = 5.0;

/// Default stick magnitude from which the run speed is used
/// (`DAT_140585c7c`, FUN_1403a8fa0).
pub const RUN_MAGNITUDE: f32 = 0.8;

/// The movement block at obj+0x128 (FUN_14040a440 defaults, overwritten by
/// FUN_14040a480 from the object's PRTY record).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjParams {
    /// +0x128, units per tick.
    pub walk: f32,
    /// +0x12c, units per tick.
    pub run: f32,
    /// +0x130, jump apex height.
    pub jump: f32,
    /// +0x134, radians per tick.
    pub turn: f32,
    /// +0x138, maximum downward speed per tick (SSTM FallMax).
    pub fall_max: f32,
    /// +0x13c (PRTY word 8).
    pub word8: f32,
    /// +0x140 (PRTY word 10).
    pub word10: f32,
}

impl ObjParams {
    /// FUN_14040a440: walk 2, run 8, jump 160, turn π/16, fall max from
    /// SSTM +0x14, +0x140 = 0.95.
    pub fn defaults(sstm_fall_max: f32) -> Self {
        ObjParams {
            walk: f32::from_bits(0x4000_0000),
            run: f32::from_bits(0x4100_0000),
            jump: f32::from_bits(0x4320_0000),
            turn: f32::from_bits(0x3e49_0fdb),
            fall_max: sstm_fall_max,
            word8: 0.,
            word10: f32::from_bits(0x3f73_3333),
        }
    }

    /// FUN_14040a480: PRTY words 0–3 to +0x128..+0x134, word 8 to +0x13c,
    /// word 10 to +0x140. +0x138 (fall max) is left alone.
    pub fn apply_prty(&mut self, words: &[u32; 13]) {
        self.walk = f32::from_bits(words[0]);
        self.run = f32::from_bits(words[1]);
        self.jump = f32::from_bits(words[2]);
        self.turn = f32::from_bits(words[3]);
        self.word8 = f32::from_bits(words[8]);
        self.word10 = f32::from_bits(words[10]);
    }
}

/// FUN_1403e9590: the record offset for a PRTY index. `offsets` is the raw
/// table after the count word; an index past the table or naming a zero
/// offset falls back to index 1. Returns None when that is zero too.
pub fn prty_offset(offsets: &[u32], index: i32) -> Option<u32> {
    let pick = |i: i32| -> u32 {
        usize::try_from(i)
            .ok()
            .and_then(|i| offsets.get(i).copied())
            .unwrap_or(0)
    };
    let i = if (offsets.len() as i64) <= index as i64 || pick(index) == 0 {
        1
    } else {
        index
    };
    Some(pick(i)).filter(|&o| o != 0)
}

/// The SSTM fields the movement code reads (`DAT_142ae5760`, set by
/// FUN_1403e9360 from 03system pref/sstm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sstm {
    /// +0x14 FallMax.
    pub fall_max: f32,
    /// +0x18 BlowBrakeXZ.
    pub blow_brake_xz: f32,
    /// +0x1c BlowMinXZ.
    pub blow_min_xz: f32,
    /// +0x20 BlowBrakeUp.
    pub blow_brake_up: f32,
    /// +0x24 BlowUp, +0x28 BlowSpeed (defaults when an ATKP's KB1 and KB2
    /// are both 0).
    pub blow_up: f32,
    pub blow_speed: f32,
    /// +0x30 HitBack, +0x34 HitBackSmall (KB1 0x7fff).
    pub hit_back: f32,
    pub hit_back_small: f32,
}

impl Sstm {
    /// Reads the fields from the SSTM record (the bytes at the record
    /// offset). None when the record is too short.
    pub fn read(record: &[u8]) -> Option<Self> {
        let f = |at: usize| {
            record
                .get(at..at + 4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        };
        Some(Sstm {
            fall_max: f(0x14)?,
            blow_brake_xz: f(0x18)?,
            blow_min_xz: f(0x1c)?,
            blow_brake_up: f(0x20)?,
            blow_up: f(0x24)?,
            blow_speed: f(0x28)?,
            hit_back: f(0x30)?,
            hit_back_small: f(0x34)?,
        })
    }

    /// FUN_1403ceab0: knock-slide strength for an ATKP KB1. 0x7fff selects
    /// HitBackSmall for reaction class 3 and HitBack otherwise; reactions
    /// 12 and 13 halve it.
    pub fn knock_strength(&self, kb1: i16, class: u8, reaction: u8) -> f32 {
        let base = if kb1 == 0x7fff {
            if class == 3 {
                self.hit_back_small
            } else {
                self.hit_back
            }
        } else {
            kb1 as f32
        };
        if matches!(reaction, 12 | 13) {
            base * 0.5
        } else {
            base
        }
    }
}

/// The jump/fall state at obj+0xf0..+0xfc, kept in the executable's
/// Y-down terms.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Arc {
    /// +0xf0: ticks since the arc began.
    pub t: f32,
    /// +0xf4: rise duration √(2h/g); 0 for a plain fall.
    pub rise: f32,
    /// +0xf8: Y-down base height.
    pub base: f32,
    /// +0xfc: Y-down rise span (−h).
    pub span: f32,
}

impl Arc {
    /// FUN_1403cbb00 / FUN_1403cbc30 with the base offset FUN_1403cbd30
    /// adds on entering the jump mode. `height` is the Y-down request: a
    /// value ≥ 0 rises by that much; a negative value starts a fall already
    /// moving down as if dropped from that far above. `y` is Y-up.
    pub fn launch(y: f32, height: f32) -> Self {
        let mut arc = if height < 0. {
            // fVar1 < 0: t = √|y·(−2.0) / g| (DAT_140623c60 = −2.0), base = y.
            Arc {
                t: ((height * -2.0) / G).abs().sqrt(),
                rise: 0.,
                base: height,
                span: 0.,
            }
        } else {
            Arc {
                t: 0.,
                rise: rise_time(height),
                base: 0.,
                span: -height,
            }
        };
        arc.base += -y;
        arc
    }

    /// FUN_1403b8150 / FUN_1403b4780: leaving the ground with a Y-up
    /// vertical speed. The parabola continues from that speed
    /// (t = v_down / g when moving down, else 0, as in FUN_1403b4780).
    pub fn fall(y: f32, vertical_up: f32) -> Self {
        let v_down = -vertical_up;
        let t = if v_down <= 0. { 0. } else { v_down / G };
        Arc {
            t,
            rise: 0.,
            base: -y - t * G * t * 0.5,
            span: 0.,
        }
    }

    /// FUN_1403cb500's target height (Y-down) at the current `t`.
    fn target(&self) -> f32 {
        if self.rise <= self.t {
            let k = self.t - self.rise;
            self.span + self.base + G * k * k * 0.5
        } else {
            let u = if self.rise != 0. {
                self.t / self.rise
            } else {
                self.t
            };
            let pi = std::f32::consts::PI;
            let s = (u * pi * 0.5).sin();
            let s = (s * pi * 0.5).sin();
            s * self.span + self.base
        }
    }

    /// FUN_1403cb360 / FUN_1403cb500, one tick: advance `t`, then return
    /// the Y-up vertical displacement toward the curve, with downward
    /// motion capped at `fall_max` per tick. `frozen` is the no-gravity
    /// state (obj+0x18c & 0x1000020 or bit 6), which holds the height while
    /// `t` keeps running.
    pub fn step(&mut self, y: f32, fall_max: f32, frozen: bool) -> f32 {
        self.t += DT;
        if frozen {
            return 0.;
        }
        let dy = (self.target() - -y).min(fall_max * DT);
        -dy
    }

    /// FUN_1403cb680: true once `rise ≤ t + 5`, when the fall motion plays.
    pub fn falling_motion(&self) -> bool {
        self.rise <= self.t + FALL_MOTION_LEAD
    }
}

/// FUN_1403b73f0: √(2h/g).
pub fn rise_time(height: f32) -> f32 {
    ((height + height) / G).abs().sqrt()
}

/// FUN_1403b7ad0 (horizontal part): with a stick velocity (`speed` > 0)
/// the velocity moves toward `direction × speed` keeping `inertia` of
/// itself; otherwise it is scaled by `drag`. Returns the new velocity; the
/// displacement is velocity × DT.
pub fn steer(
    velocity: [f32; 2],
    direction: [f32; 2],
    speed: f32,
    inertia: f32,
    drag: f32,
) -> [f32; 2] {
    if speed <= 0. {
        velocity.map(|v| v * drag)
    } else {
        let target = [direction[0] * speed, direction[1] * speed];
        // FUN_140141ae0(out, a, b, t) = b·(1 − t) + a·t.
        [
            target[0] * (1. - inertia) + velocity[0] * inertia,
            target[1] * (1. - inertia) + velocity[1] * inertia,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_prty_follow_the_init_routines() {
        let mut p = ObjParams::defaults(16.);
        assert_eq!((p.walk, p.run, p.jump), (2., 8., 160.));
        assert!((p.turn - std::f32::consts::PI / 16.).abs() < 1e-6);
        let mut words = [0u32; 13];
        for (w, v) in words.iter_mut().zip([2.5f32, 9., 185., 0.25]) {
            *w = v.to_bits();
        }
        words[8] = 3f32.to_bits();
        p.apply_prty(&words);
        assert_eq!(
            (p.walk, p.run, p.jump, p.turn, p.word8),
            (2.5, 9., 185., 0.25, 3.)
        );
        assert_eq!(p.fall_max, 16.);
    }

    #[test]
    fn prty_index_falls_back_to_record_one() {
        let offsets = [0, 40, 92, 0];
        assert_eq!(prty_offset(&offsets, 2), Some(92));
        assert_eq!(prty_offset(&offsets, 3), Some(40));
        assert_eq!(prty_offset(&offsets, 9), Some(40));
        assert_eq!(prty_offset(&[0, 0], 5), None);
    }

    #[test]
    fn jump_rises_on_the_double_sine_and_falls_on_the_parabola() {
        let mut arc = Arc::launch(0., 185.);
        assert!((arc.rise - 30.108_14).abs() < 1e-3);
        let mut y = 0.;
        let mut heights = Vec::new();
        let mut landed = None;
        for tick in 1..200 {
            y += arc.step(y, 16., false);
            heights.push(y);
            if y <= 0. && tick > 1 {
                landed = Some(tick);
                break;
            }
        }
        // h·sin(π/2·sin(π/2·t/T)) at t = 5, 10, 15, 20.
        for (t, want) in [(5, 72.9), (10, 130.5), (15, 165.5), (20, 180.8)] {
            assert!(
                (heights[t - 1] - want).abs() < 0.1,
                "t {t}: {}",
                heights[t - 1]
            );
        }
        let apex = heights.iter().copied().fold(0., f32::max);
        assert!(apex > 184.9 && apex <= 185.0 + 1e-3, "{apex}");
        // Falls back over ≈ √(2h/g) ticks: lands at tick 61 (t ≈ 60.2).
        assert_eq!(landed, Some(61));
    }

    #[test]
    fn long_falls_cap_at_fall_max() {
        let mut arc = Arc::fall(1000., 0.);
        let mut y = 1000.;
        let mut last = 0.;
        for _ in 0..80 {
            last = arc.step(y, 16., false);
            y += last;
        }
        assert_eq!(last, -16.);
    }

    #[test]
    fn frozen_ticks_hold_height_but_advance_time() {
        let mut arc = Arc::launch(0., 185.);
        assert_eq!(arc.step(0., 16., true), 0.);
        assert_eq!(arc.t, 1.);
    }

    #[test]
    fn negative_launch_starts_already_falling() {
        let mut arc = Arc::launch(50., -100.);
        assert_eq!(arc.rise, 0.);
        let dy = arc.step(50., 16., false);
        // Starts at speed g·t0 = √(2·100·g) ≈ 9.0 downward.
        assert!(dy < -9. && dy > -9.5, "{dy}");
    }

    #[test]
    fn air_steering_lerps_and_drags() {
        let v = steer([0., 0.], [1., 0.], 8., AIR_INERTIA, AIR_DRAG);
        assert!((v[0] - 0.8).abs() < 1e-6);
        let v = steer([8., 0.], [0., 0.], 0., AIR_INERTIA, AIR_DRAG);
        assert!((v[0] - 7.6).abs() < 1e-6);
    }
}
