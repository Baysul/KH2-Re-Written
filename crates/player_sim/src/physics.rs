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
}
