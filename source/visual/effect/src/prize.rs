//! The prizes' sprites: program 203 of `obj/PRIZE.mdlx` (program table
//! record 0x14077d970, draw FUN_1404d7ed0). Its parameter block is a
//! four-byte header and ten twelve-byte groups: the shape id (0xffff =
//! none), the colour bytes (0x80 = 1.0) and a float scale. A prize of kind
//! `k` draws group `k`; each tick it steps one flip-book frame, and the
//! shape's last sequence entry says whether the flip-book loops (starting
//! at a random frame) or holds its last frame.
use crate::programs::sprite_quads;
use crate::{Quad, math::Mat};
use data::object_effect::{self, Dpd, f32_at, u32_at};

/// The effect program number of the prize draw.
pub const PROGRAM: u32 = 203;
const GROUPS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Group {
    anim: usize,
    colour: [u8; 3],
    scale: f32,
    count: usize,
    looping: bool,
}

/// The ten prize groups of a PRIZE effect.
#[derive(Debug, Default)]
pub struct Prize {
    groups: [Option<Group>; GROUPS],
}

/// The flip-book frame a prize shows `age` ticks after it appeared:
/// looping shapes start at `seed` (0..1) of their length, others at frame 0
/// and hold the last frame.
pub fn frame_at(count: usize, looping: bool, age: u32, seed: f32) -> usize {
    if count == 0 {
        return 0;
    }
    if looping {
        let start = ((count - 1) as f32 * seed.clamp(0., 1.)) as usize;
        (start + age as usize) % count
    } else {
        (age as usize).min(count - 1)
    }
}

impl Prize {
    /// Reads the groups of the DPD's prize draw program; `None` when the
    /// DPD has no such program.
    pub fn new(dpd: &Dpd) -> Option<Prize> {
        let program = dpd
            .pdata
            .iter()
            .flat_map(|p| p.entries.iter())
            .flat_map(|e| e.programs.iter())
            .find(|p| p.kind == PROGRAM)?;
        let b = &dpd.bytes;
        let mut prize = Prize::default();
        for (g, slot) in prize.groups.iter_mut().enumerate() {
            let at = program.params + 4 + g * 12;
            let id = u32_at(b, at).ok()? as i32;
            let Some(shape) = i16::try_from(id).ok().and_then(|i| dpd.shapes.get(&i)) else {
                continue;
            };
            let anim = shape.anim;
            let count = object_effect::shape_sequence_count(b, anim).ok()?.max(0) as usize;
            let looping = count > 0
                && object_effect::shape_sequence(b, anim, count - 1)
                    .map(|(_, flag)| flag & 0x80 != 0)
                    .unwrap_or(false);
            *slot = Some(Group {
                anim,
                colour: [*b.get(at + 4)?, *b.get(at + 5)?, *b.get(at + 6)?],
                scale: f32_at(b, at + 8).ok()?,
                count,
                looping,
            });
        }
        Some(prize)
    }

    /// The frame of a prize of `kind` at `age`, or `None` for a kind
    /// without a shape.
    pub fn frame(&self, kind: u8, age: u32, seed: f32) -> Option<usize> {
        let g = self.groups.get(kind as usize)?.as_ref()?;
        Some(frame_at(g.count, g.looping, age, seed))
    }

    /// The sprites of a prize of `kind` at flip-book `frame`, centred at
    /// `centre` in the effect view space (x right, y down, z forward).
    pub fn quads(&self, dpd: &Dpd, kind: u8, frame: usize, centre: [f32; 3]) -> Vec<Quad> {
        let Some(Some(g)) = self.groups.get(kind as usize) else {
            return Vec::new();
        };
        let s = g.scale;
        let m: Mat = [
            [s, 0., 0., 0.],
            [0., s, 0., 0.],
            [0., 0., s, 0.],
            [centre[0], centre[1], centre[2], 1.],
        ];
        let mut quads = sprite_quads(
            &dpd.bytes,
            g.anim,
            frame,
            &m,
            [g.colour[0], g.colour[1], g.colour[2], 0x80],
        );
        // The prize sprites' S and T are 12-bit fractions of the whole
        // texture (4096 = 1.0) [I: read off the installed atlases, where
        // every sprite then lands on its own image]; `sprite_quads` scales
        // them as the other shapes' (S / 16, T / 8) texels.
        for q in &mut quads {
            let (w, h) = (1 << ((q.tex0 >> 26) & 0xf), 1 << ((q.tex0 >> 30) & 0xf));
            for uv in &mut q.uv {
                uv[0] *= w as f32 / 256.;
                uv[1] *= h as f32 / 512.;
            }
        }
        quads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looping_shapes_wrap_and_others_hold() {
        assert_eq!(frame_at(24, true, 30, 0.), 6);
        assert_eq!(frame_at(24, true, 0, 1.), 23);
        assert_eq!(frame_at(24, false, 30, 0.5), 23);
        assert_eq!(frame_at(24, false, 5, 0.5), 5);
        assert_eq!(frame_at(0, true, 5, 0.), 0);
    }
}
