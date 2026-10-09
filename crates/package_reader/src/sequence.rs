//! SEQD 2D sequences (HUD and menu sprites), following the public OpenKH
//! `Sequence.cs` layout and `SequenceRenderer.cs` playback at
//! 7a3b945c538d32c6a285128c98aefba093f52ceb. Evaluation yields textured
//! quads in sequence space; the caller places and draws them.
use std::io;

pub const DISABLE_CURVE: i32 = 0x1;
pub const IS_ACTIVE: i32 = 0x2;
pub const BOUNCE_DISABLE: i32 = 0x10;
pub const ROTATION_DISABLE: i32 = 0x20;
pub const SCALING_DISABLE: i32 = 0x40;
pub const COLOR_INTERPOLATION: i32 = 0x80;
pub const COLOR_MASK: i32 = 0x400;
pub const TRANSLATION_INTERPOLATION: i32 = 0x1000;
pub const PIVOT_DISABLE: i32 = 0x4000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    /// Texture rectangle in original (pre-HD) texel units.
    pub uv: [i32; 4],
    pub scroll: [f32; 2],
    /// Corner colors (left/top/right/bottom), RGBA bytes where 0x80 is 1.0.
    pub colors: [u32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpritePart {
    pub rect: [i32; 4],
    pub sprite: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animation {
    pub flags: i32,
    pub sprite_group: usize,
    pub frames: [i32; 2],
    pub translate: [i32; 4],
    pub pivot: [i32; 4],
    pub rotation: [f32; 6],
    pub scale: [f32; 2],
    pub scale_x: [f32; 2],
    pub scale_y: [f32; 2],
    pub bounce: [f32; 4],
    pub bounce_count: [i16; 2],
    pub blend: i32,
    pub color: [u32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimationGroup {
    pub animations: std::ops::Range<usize>,
    pub do_not_loop: bool,
    pub loop_frames: [i32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sequence {
    pub sprites: Vec<Sprite>,
    pub parts: Vec<SpritePart>,
    pub sprite_groups: Vec<std::ops::Range<usize>>,
    pub animations: Vec<Animation>,
    pub groups: Vec<AnimationGroup>,
}

/// One textured quad: destination corners in sequence space (top-left,
/// top-right, bottom-left, bottom-right), texel rectangle and RGBA
/// multipliers per corner (1.0 = unchanged).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub corners: [[f32; 2]; 4],
    pub uv: [i32; 4],
    pub colors: [[f32; 4]; 4],
    pub blend: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Evaluated {
    pub quads: Vec<Quad>,
    /// Position published by the last active animation (IsActive flag), for
    /// attaching child elements.
    pub child_position: Option<[f32; 2]>,
}

fn i32_at(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn i16_at(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}

pub fn read(bytes: &[u8]) -> io::Result<Sequence> {
    let invalid = |m| io::Error::new(io::ErrorKind::InvalidData, m);
    if bytes.len() < 48 || !bytes.starts_with(b"SEQD") || bytes.len() > 16 * 1024 * 1024 {
        return Err(invalid("SEQD signature/bounds"));
    }
    let table = |at: usize, stride: usize| -> io::Result<(usize, usize)> {
        let (count, offset) = (i32_at(bytes, at), i32_at(bytes, at + 4));
        let (count, offset) = (
            usize::try_from(count).map_err(|_| invalid("negative SEQD count"))?,
            usize::try_from(offset).map_err(|_| invalid("negative SEQD offset"))?,
        );
        if offset + count * stride > bytes.len() {
            return Err(invalid("SEQD table exceeds file"));
        }
        Ok((count, offset))
    };
    let (sc, so) = table(8, 44)?;
    let (pc, po) = table(16, 20)?;
    let (gc, go) = table(24, 4)?;
    let (ac, ao) = table(32, 0x90)?;
    let (agc, ago) = table(40, 0x24)?;
    let sprites = (0..sc)
        .map(|i| {
            let a = so + i * 44;
            Sprite {
                uv: std::array::from_fn(|k| i32_at(bytes, a + 4 + k * 4)),
                scroll: [f32_at(bytes, a + 20), f32_at(bytes, a + 24)],
                colors: std::array::from_fn(|k| i32_at(bytes, a + 28 + k * 4) as u32),
            }
        })
        .collect::<Vec<_>>();
    let parts = (0..pc)
        .map(|i| {
            let a = po + i * 20;
            let sprite = i32_at(bytes, a + 16);
            usize::try_from(sprite)
                .ok()
                .filter(|s| *s < sc)
                .map(|sprite| SpritePart {
                    rect: std::array::from_fn(|k| i32_at(bytes, a + k * 4)),
                    sprite,
                })
                .ok_or_else(|| invalid("SEQD part sprite outside table"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let range = |start: i64, count: i64, len: usize| -> io::Result<std::ops::Range<usize>> {
        if start < 0 || count < 0 || (start + count) as usize > len {
            return Err(invalid("SEQD range outside table"));
        }
        Ok(start as usize..(start + count) as usize)
    };
    let sprite_groups = (0..gc)
        .map(|i| {
            let a = go + i * 4;
            range(i16_at(bytes, a) as i64, i16_at(bytes, a + 2) as i64, pc)
        })
        .collect::<io::Result<Vec<_>>>()?;
    let animations = (0..ac)
        .map(|i| {
            let a = ao + i * 0x90;
            let int = |k: usize| i32_at(bytes, a + k * 4);
            let float = |k: usize| f32_at(bytes, a + 48 + k * 4);
            let sprite_group = usize::try_from(int(1))
                .ok()
                .filter(|g| *g < gc)
                .ok_or_else(|| invalid("SEQD animation sprite group outside table"))?;
            Ok(Animation {
                flags: int(0),
                sprite_group,
                frames: [int(2), int(3)],
                translate: [int(4), int(5), int(6), int(7)],
                pivot: [int(8), int(9), int(10), int(11)],
                rotation: std::array::from_fn(float),
                scale: [float(6), float(7)],
                scale_x: [float(8), float(9)],
                scale_y: [float(10), float(11)],
                bounce: [float(16), float(17), float(18), float(19)],
                bounce_count: [i16_at(bytes, a + 128), i16_at(bytes, a + 130)],
                blend: i32_at(bytes, a + 132),
                color: [i32_at(bytes, a + 136) as u32, i32_at(bytes, a + 140) as u32],
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let groups = (0..agc)
        .map(|i| {
            let a = ago + i * 0x24;
            Ok(AnimationGroup {
                animations: range(i16_at(bytes, a) as i64, i16_at(bytes, a + 2) as i64, ac)?,
                do_not_loop: i16_at(bytes, a + 4) != 0,
                loop_frames: [i32_at(bytes, a + 8), i32_at(bytes, a + 12)],
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok(Sequence {
        sprites,
        parts,
        sprite_groups,
        animations,
        groups,
    })
}

fn color(c: u32) -> [f32; 4] {
    std::array::from_fn(|k| ((c >> (8 * k)) & 0xff) as f32 / 128.)
}

fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a * (1. - t) + b * t
}

impl Sequence {
    /// The frame actually shown for `frame` frames since the group started.
    pub fn group_frame(&self, group: usize, frame: i32) -> i32 {
        let g = &self.groups[group];
        if g.do_not_loop {
            return frame;
        }
        let mut end = g.loop_frames[1];
        if end == 0 {
            end = self.animations[g.animations.clone()]
                .iter()
                .map(|a| a.frames[1])
                .max()
                .unwrap_or(0);
        }
        let start = g.loop_frames[0];
        if frame < end {
            frame
        } else if end <= start {
            start
        } else {
            start + (frame - start).rem_euclid(end - start)
        }
    }

    /// Evaluates an animation group `frame` frames after it started, placed
    /// at `origin` in sequence space.
    pub fn evaluate(&self, group: usize, frame: i32, origin: [f32; 2]) -> Evaluated {
        let shown = self.group_frame(group, frame);
        let mut out = Evaluated::default();
        for animation in &self.animations[self.groups[group].animations.clone()] {
            let [start, end] = animation.frames;
            if shown < start || shown > end {
                continue;
            }
            let delta = if end == start {
                0.
            } else {
                (shown - start) as f32 / (end - start) as f32
            };
            let flags = animation.flags;
            let t = if flags & DISABLE_CURVE != 0 {
                delta
            } else {
                ((delta * std::f32::consts::PI - std::f32::consts::FRAC_PI_2).sin() + 1.) / 2.
            };
            let tr = animation.translate.map(|v| v as f32);
            let (tx, ty) = (lerp(t, tr[0], tr[1]), lerp(t, tr[2], tr[3]));
            let mut position = if flags & TRANSLATION_INTERPOLATION == 0 {
                [origin[0] + tx, origin[1] + ty]
            } else {
                [origin[0] + tr[0], origin[1] + tr[2]]
            };
            let (sx, sy) = if flags & SCALING_DISABLE == 0 {
                let s = lerp(t, animation.scale[0], animation.scale[1]);
                (
                    s * lerp(t, animation.scale_x[0], animation.scale_x[1]),
                    s * lerp(t, animation.scale_y[0], animation.scale_y[1]),
                )
            } else {
                (1., 1.)
            };
            let tint = if flags & COLOR_MASK != 0 {
                [1.; 4]
            } else if flags & COLOR_INTERPOLATION == 0 {
                let (a, b) = (color(animation.color[0]), color(animation.color[1]));
                std::array::from_fn(|k| lerp(t, a[k], b[k]))
            } else {
                color(animation.color[0])
            };
            let rotation_z = if flags & ROTATION_DISABLE == 0 {
                lerp(t, animation.rotation[4], animation.rotation[5])
            } else {
                0.
            };
            let pivot = if flags & PIVOT_DISABLE == 0 {
                let p = animation.pivot.map(|v| v as f32);
                [lerp(t, p[0], p[1]), lerp(t, p[2], p[3])]
            } else {
                [0., 0.]
            };
            if flags & BOUNCE_DISABLE == 0 {
                let b = animation.bounce;
                let bx = (delta * animation.bounce_count[0] as f32 * std::f32::consts::PI).sin();
                let by = (delta * animation.bounce_count[1] as f32 * std::f32::consts::PI).sin();
                position[0] += bx * lerp(t, b[0], b[1]);
                position[1] += by * lerp(t, b[2], b[3]);
            }
            if flags & IS_ACTIVE != 0 {
                let mut child = [position[0] + pivot[0], position[1] + pivot[1]];
                if flags & TRANSLATION_INTERPOLATION != 0 {
                    child[0] += tx - tr[0];
                    child[1] += ty - tr[2];
                }
                out.child_position = Some(child);
            }
            let (s, c) = (-rotation_z).sin_cos();
            for part in &self.parts[self.sprite_groups[animation.sprite_group].clone()] {
                let sprite = &self.sprites[part.sprite];
                let [l, top, r, b] = part.rect.map(|v| v as f32);
                let corners = [[l, top], [r, top], [l, b], [r, b]].map(|[x, y]| {
                    let (x, y) = ((x + pivot[0]) * sx, (y + pivot[1]) * sy);
                    [x * c - y * s + position[0], x * s + y * c + position[1]]
                });
                let corner = sprite.colors.map(color);
                out.quads.push(Quad {
                    corners,
                    uv: sprite.uv,
                    colors: corner.map(|cc| std::array::from_fn(|k| cc[k] * tint[k])),
                    blend: animation.blend,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> Vec<u8> {
        let mut b = vec![0u8; 48];
        b[..4].copy_from_slice(b"SEQD");
        let put =
            |b: &mut Vec<u8>, at: usize, v: i32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        // One sprite, one part, one sprite group, one animation, one group.
        let so = b.len();
        b.extend(std::iter::repeat_n(0, 44));
        for (k, v) in [0, 10, 20, 30, 40].iter().enumerate() {
            put(&mut b, so + k * 4, *v);
        }
        for k in 0..4 {
            put(&mut b, so + 28 + k * 4, 0x8080_8080u32 as i32);
        }
        let po = b.len();
        b.extend(std::iter::repeat_n(0, 20));
        for (k, v) in [0, 0, 8, 4, 0].iter().enumerate() {
            put(&mut b, po + k * 4, *v);
        }
        let go = b.len();
        b.extend([0, 0, 1, 0]);
        let ao = b.len();
        b.extend(std::iter::repeat_n(0, 0x90));
        put(
            &mut b,
            ao,
            DISABLE_CURVE | IS_ACTIVE | BOUNCE_DISABLE | ROTATION_DISABLE,
        );
        put(&mut b, ao + 12, 10);
        put(&mut b, ao + 20, 100);
        for k in 6..12 {
            b[ao + 48 + k * 4..ao + 52 + k * 4].copy_from_slice(&1f32.to_le_bytes());
        }
        put(&mut b, ao + 136, 0x8080_8080u32 as i32);
        put(&mut b, ao + 140, 0x0080_8080);
        let ago = b.len();
        b.extend(std::iter::repeat_n(0, 0x24));
        b[ago + 2] = 1;
        for (at, (count, offset)) in [(1, so), (1, po), (1, go), (1, ao), (1, ago)]
            .into_iter()
            .enumerate()
        {
            put(&mut b, 8 + at * 8, count);
            put(&mut b, 12 + at * 8, offset as i32);
        }
        b
    }

    #[test]
    fn evaluates_linear_translation_color_and_child_position() {
        let seq = read(&synthetic()).unwrap();
        assert_eq!(seq.groups.len(), 1);
        let e = seq.evaluate(0, 5, [1., 2.]);
        assert_eq!(e.quads.len(), 1);
        let q = e.quads[0];
        assert_eq!(q.uv, [10, 20, 30, 40]);
        // Halfway: translate x 0 -> 100 gives 50.
        assert_eq!(q.corners[0], [51., 2.]);
        assert_eq!(q.corners[3], [59., 6.]);
        assert!((q.colors[0][3] - 0.5).abs() < 1e-6);
        assert_eq!(e.child_position, Some([51., 2.]));
        // Looping group wraps to the start after its end.
        assert_eq!(seq.group_frame(0, 25), 5);
    }

    #[test]
    fn rejects_bad_signature_and_tables() {
        assert!(read(&[0u8; 64]).is_err());
        let mut b = synthetic();
        b[8..12].copy_from_slice(&1000i32.to_le_bytes());
        assert!(read(&b).is_err());
    }
}
