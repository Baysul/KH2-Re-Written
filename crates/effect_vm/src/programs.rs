//! Program functions from the table at 0x140779a00 (0x50-byte records:
//! update +8, draw +0x18, create +0x38, destroy +0x48), translated from
//! their decompilations. `V[o]` is work offset 0xa0 + o (`Part::vars[o]`);
//! `u[k]` is the program's k-th work-variable offset.
use crate::math::{self, Mat, Vec4};
use crate::models;
use crate::{Ctx, Part, Quad, key_i32};
use package_reader::effect::{self, Program, f32_at, i16_at, u32_at};

pub(crate) struct Birth {
    pub entry: usize,
    pub position_var: usize,
    pub position: Vec4,
    /// Floats written at `position_var` (3 = xyz only, 4 = with w).
    pub position_len: usize,
    pub velocity_var: Option<usize>,
    pub velocity: Vec4,
}

const ACCELE: u32 = 28;
const ANG_ACCELE: u32 = 29;
const SCL_ACCELE: u32 = 30;
const COL_ACCELE: u32 = 31;
const MOVE: u32 = 36;
const ANG_MOVE: u32 = 37;
const SCL_MOVE: u32 = 38;
const COL_MOVE: u32 = 39;
const POINT: u32 = 40;
const ANGLE: u32 = 41;
const SCALE: u32 = 42;
const COLOR: u32 = 43;
const RAND_FV: u32 = 63;
const RAND_IV: u32 = 66;
const MATRIX_XYZ: u32 = 87;
const MATRIX_SCL: u32 = 94;
const DRAW_MATRIX_FRONT: u32 = 113;
const KE_Z_CRCT: u32 = 124;
const KE_Z_CRCT_SHP: u32 = 125;
const DRAW_SHAPE_X: u32 = 142;
const KE_BORN_RND5: u32 = 183;
pub(crate) const MEGA_BIRTH: u32 = 201;
pub(crate) const MNG_FLAG: u32 = 202;
pub(crate) const MEGA_BIRTH_MODEL: u32 = 208;

pub(crate) fn has_update(kind: u32) -> bool {
    kind != models::DRAW_MDL3
}

pub(crate) fn has_draw(kind: u32) -> bool {
    matches!(
        kind,
        DRAW_SHAPE_X
            | MEGA_BIRTH
            | MEGA_BIRTH_MODEL
            | models::VT_MIME
            | models::DRAW_MDL3
            | models::DRAW_MDL_TS3
    )
}

pub(crate) fn use_val(ctx: &Ctx, p: &Program, k: usize) -> usize {
    key_i32(ctx.dpd, p.use_val + 4 * k) as usize
}

pub(crate) fn word(ctx: &Ctx, at: usize) -> u32 {
    u32_at(&ctx.dpd.bytes, at).unwrap_or(0)
}

pub(crate) fn float(ctx: &Ctx, at: usize) -> f32 {
    f32_at(&ctx.dpd.bytes, at).unwrap_or(0.)
}

pub(crate) fn byte(ctx: &Ctx, at: usize) -> u8 {
    ctx.dpd.bytes.get(at).copied().unwrap_or(0)
}

/// Updates run only on even part frames outside pause (every program
/// checks `(frame & 1) == 0`).
pub(crate) fn active(part: &Part) -> bool {
    part.frame & 1 == 0
}

pub(crate) fn key(ctx: &Ctx, part: &Part, cursor: usize) -> bool {
    word(ctx, cursor) == part.frame
}

pub(crate) fn create(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        // Integrators clear their rate variable u1.
        ACCELE | ANG_ACCELE | SCL_ACCELE | MOVE | ANG_MOVE | SCL_MOVE => {
            let o = use_val(ctx, p, 1);
            part.zero(o, 12);
        }
        COL_ACCELE | COL_MOVE => {
            let o = use_val(ctx, p, 1);
            part.zero(o, 8);
        }
        POINT | ANGLE | SCALE => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 12);
        }
        COLOR => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 8);
        }
        DRAW_SHAPE_X => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 6);
        }
        KE_BORN_RND5 => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 3);
        }
        MEGA_BIRTH => crate::mega_birth::create(p, part, ctx),
        MEGA_BIRTH_MODEL => crate::mega_model::create(p, part, ctx),
        models::VT_MIME | models::DRAW_MDL_TS3 | models::VERTEX_AP => models::create(p, part, ctx),
        _ => {}
    }
}

pub(crate) fn destroy(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        MEGA_BIRTH => crate::mega_birth::destroy(p, part, ctx),
        MEGA_BIRTH_MODEL => crate::mega_model::destroy(p, part, ctx),
        _ => {}
    }
}

pub(crate) fn update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        ACCELE | SCL_ACCELE | MOVE | SCL_MOVE => integrate_f32(p, cursor, part, ctx),
        ANG_ACCELE | ANG_MOVE => integrate_i32(p, cursor, part, ctx),
        COL_ACCELE | COL_MOVE => integrate_i16(p, cursor, part, ctx),
        POINT | SCALE => {
            if active(part) && key(ctx, part, cursor) {
                let o = use_val(ctx, p, 0);
                for i in 0..3 {
                    let v = part.f32(o + 4 * i) + float(ctx, cursor + 4 + 4 * i);
                    part.set_f32(o + 4 * i, v);
                }
            }
        }
        ANGLE => {
            if active(part) && key(ctx, part, cursor) {
                let o = use_val(ctx, p, 0);
                for i in 0..3 {
                    let v = part
                        .i32(o + 4 * i)
                        .wrapping_add(word(ctx, cursor + 4 + 4 * i) as i32);
                    part.set_i32(o + 4 * i, v);
                }
            }
        }
        COLOR => color(p, cursor, part, ctx),
        RAND_FV => rand_fv(p, cursor, part, ctx),
        RAND_IV => rand_iv(p, cursor, part, ctx),
        MATRIX_XYZ => {
            let (u0, u1, u2) = (use_val(ctx, p, 0), use_val(ctx, p, 1), use_val(ctx, p, 2));
            let angles = [part.i32(u1), part.i32(u1 + 4), part.i32(u1 + 8)];
            let mut m = math::rot_xyz(angles);
            for (c, col) in m.iter_mut().take(3).enumerate() {
                *col = math::scale(*col, part.f32(u2 + 4 * c));
            }
            m[3] = [part.f32(u0), part.f32(u0 + 4), part.f32(u0 + 8), m[3][3]];
            part.cur = m;
        }
        MATRIX_SCL => {
            let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
            let mut m = math::IDENTITY;
            m[0][0] = part.f32(u1);
            m[1][1] = part.f32(u1 + 4);
            m[2][2] = part.f32(u1 + 8);
            m[3] = [part.f32(u0), part.f32(u0 + 4), part.f32(u0 + 8), 1.];
            part.cur = m;
        }
        DRAW_MATRIX_FRONT => {
            // FUN_1404c6490: axes × world scale, translation through
            // view·world, then the draw-space matrix.
            let mut d = part.cur;
            for (col, s) in d.iter_mut().zip(ctx.scales) {
                *col = math::scale(*col, s);
            }
            d[3] = math::apply(&ctx.ed60, part.cur[3]);
            part.draw = math::mul(&ctx.ede0, &d);
        }
        KE_Z_CRCT => ke_z_crct(p, cursor, part, ctx),
        KE_Z_CRCT_SHP => ke_z_crct_shp(p, cursor, part, ctx),
        DRAW_SHAPE_X => shape_advance(p, cursor, part, ctx),
        KE_BORN_RND5 => born_rnd5(p, cursor, part, ctx),
        MEGA_BIRTH => crate::mega_birth::update(p, cursor, part, ctx),
        MEGA_BIRTH_MODEL => crate::mega_model::update(p, cursor, part, ctx),
        MNG_FLAG => crate::mega_model::mng_flag(cursor, part, ctx),
        models::VT_MIME
        | models::MATRIX_ZXY
        | models::DRAW_MATRIX
        | models::DRAW_MDL_TS3
        | models::VERTEX_AP => models::update(p, cursor, part, ctx),
        _ => {
            if !ctx.unsupported.contains(&p.kind) {
                ctx.unsupported.push(p.kind);
            }
        }
    }
}

pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        DRAW_SHAPE_X => draw_shape(p, cursor, part, ctx),
        MEGA_BIRTH => crate::mega_birth::draw(p, cursor, part, ctx),
        MEGA_BIRTH_MODEL => crate::mega_model::draw(p, cursor, part, ctx),
        models::VT_MIME | models::DRAW_MDL3 | models::DRAW_MDL_TS3 => {
            models::draw(p, cursor, part, ctx)
        }
        _ => {
            if !ctx.unsupported.contains(&p.kind) {
                ctx.unsupported.push(p.kind);
            }
        }
    }
}

/// FUN_1404bfcf0 family (Accele/Move/SclAccele/SclMove): at a key the rate
/// u1 gains the keyframe vector; every tick the value u0 gains the rate.
fn integrate_f32(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let hit = key(ctx, part, cursor);
    for i in 0..3 {
        if hit {
            let r = part.f32(u1 + 4 * i) + float(ctx, cursor + 4 + 4 * i);
            part.set_f32(u1 + 4 * i, r);
        }
        let v = part.f32(u0 + 4 * i) + part.f32(u1 + 4 * i);
        part.set_f32(u0 + 4 * i, v);
    }
}

fn integrate_i32(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let hit = key(ctx, part, cursor);
    for i in 0..3 {
        if hit {
            let r = part
                .i32(u1 + 4 * i)
                .wrapping_add(word(ctx, cursor + 4 + 4 * i) as i32);
            part.set_i32(u1 + 4 * i, r);
        }
        let v = part.i32(u0 + 4 * i).wrapping_add(part.i32(u1 + 4 * i));
        part.set_i32(u0 + 4 * i, v);
    }
}

fn integrate_i16(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let hit = key(ctx, part, cursor);
    for i in 0..4 {
        if hit {
            let k = i16_at(&ctx.dpd.bytes, cursor + 4 + 2 * i).unwrap_or(0);
            let r = part.i16(u1 + 2 * i).wrapping_add(k);
            part.set_i16(u1 + 2 * i, r);
        }
        let v = part.i16(u0 + 2 * i).wrapping_add(part.i16(u1 + 2 * i));
        part.set_i16(u0 + 2 * i, v);
    }
}

/// FUN_1404c0f60: keyed RGBA16 adds, then every call the 8-bit colour
/// bytes at u0+8: (u16 value × instance colour), clamped to 32767, /128.
fn color(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let u0 = use_val(ctx, p, 0);
    if active(part) && key(ctx, part, cursor) {
        for i in 0..4 {
            let k = i16_at(&ctx.dpd.bytes, cursor + 4 + 2 * i).unwrap_or(0);
            let v = part.i16(u0 + 2 * i).wrapping_add(k);
            part.set_i16(u0 + 2 * i, v);
        }
    }
    for i in 0..4 {
        let c = (part.i16(u0 + 2 * i) as u16 as f32 * ctx.colour[i]).min(32767.);
        part.set_u8(u0 + 8 + i, (c * (1. / 128.)) as i32 as u8);
    }
}

/// FUN_1404c2610: at frame 0 draws r (rnd·2, or rnd+rnd when byte +0x14 is
/// set) into u0; at frame 0 or a key, target V[p1] (−1: scratch) gains
/// p·r − p per axis.
fn rand_fv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    if part.frame == 0 {
        let r = if byte(ctx, cursor + 0x14) == 0 {
            ctx.rng.unit() * 2.
        } else {
            ctx.rng.unit() + ctx.rng.unit()
        };
        part.set_f32(u0, r);
    } else if !key(ctx, part, cursor) {
        return;
    }
    let r = part.f32(u0);
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let t = target as usize;
    for i in 0..3 {
        let k = float(ctx, cursor + 8 + 4 * i);
        let v = (k * r - k) + part.f32(t + 4 * i);
        part.set_f32(t + 4 * i, v);
    }
}

/// FUN_1404c2950: the integer variant, only at keys.
fn rand_iv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) || !key(ctx, part, cursor) {
        return;
    }
    let r = if byte(ctx, cursor + 0x14) == 0 {
        ctx.rng.unit() * 2.
    } else {
        ctx.rng.unit() + ctx.rng.unit()
    };
    let u0 = use_val(ctx, p, 0);
    part.set_f32(u0, r);
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let t = target as usize;
    for i in 0..3 {
        let k = word(ctx, cursor + 8 + 4 * i) as i32 as f32;
        let v = part.i32(t + 4 * i).wrapping_add((k * r - k) as i32);
        part.set_i32(t + 4 * i, v);
    }
}

fn params_vec3(ctx: &Ctx, at: usize) -> [f32; 3] {
    [float(ctx, at), float(ctx, at + 4), float(ctx, at + 8)]
}

/// FUN_1404c7200: a model-view matrix into u0. Modes: 0 offset before
/// view·world, 1 offset after it, 2 world then view with scaled offset.
fn ke_z_crct(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let offset = params_vec3(ctx, cursor + 4);
    let s = params_vec3(ctx, cursor + 0x10);
    let mut m = part.cur;
    for (col, s) in m.iter_mut().zip(s) {
        *col = math::scale(*col, s);
    }
    let m = match byte(ctx, cursor + 0x1c) {
        0 => {
            for i in 0..3 {
                m[3][i] += offset[i];
            }
            math::mul(&ctx.ed60, &m)
        }
        2 => {
            let mut w = math::mul(&ctx.world, &m);
            for i in 0..3 {
                w[3][i] += ctx.scales[i] * offset[i];
            }
            math::mul(&ctx.view, &w)
        }
        mode => {
            if mode != 1 && !ctx.unsupported.contains(&(0x2000_0000 | mode as u32)) {
                ctx.unsupported.push(0x2000_0000 | mode as u32);
            }
            let mut w = math::mul(&ctx.ed60, &m);
            for i in 0..3 {
                w[3][i] += offset[i];
            }
            w
        }
    };
    write_matrix(part, use_val(ctx, p, 0), &math::mul(&ctx.ede0, &m));
}

/// FUN_1404c73d0: shape variant: axes × world scale stay in draw space,
/// only the translation goes through the parent.
fn ke_z_crct_shp(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let offset = params_vec3(ctx, cursor + 4);
    let s = params_vec3(ctx, cursor + 0x10);
    let mut m = part.cur;
    for (col, s) in m.iter_mut().zip(ctx.scales) {
        *col = math::scale(*col, s);
    }
    let mut t = part.cur[3];
    for i in 0..3 {
        t[i] *= s[i];
    }
    m[3] = match byte(ctx, cursor + 0x1c) {
        0 => {
            for i in 0..3 {
                t[i] += offset[i];
            }
            math::apply(&ctx.ed60, t)
        }
        1 => {
            let mut w = math::apply(&ctx.ed60, t);
            for i in 0..3 {
                w[i] += offset[i];
            }
            w
        }
        2 => {
            let mut w = math::apply(&ctx.world, t);
            for i in 0..3 {
                w[i] += ctx.scales[i] * offset[i];
            }
            math::apply(&ctx.view, w)
        }
        _ => [0., 0., 0., 1.],
    };
    write_matrix(part, use_val(ctx, p, 0), &math::mul(&ctx.ede0, &m));
}

pub(crate) fn write_matrix(part: &mut Part, o: usize, m: &Mat) {
    for (c, col) in m.iter().enumerate() {
        part.set_vec4(o + 16 * c, *col);
    }
}

pub(crate) fn read_matrix(part: &Part, o: usize) -> Mat {
    std::array::from_fn(|c| part.vec4(o + 16 * c))
}

fn shape_anim(ctx: &Ctx, cursor: usize) -> Option<usize> {
    let shape = word(ctx, cursor + 4);
    if shape == 0xffff {
        return None;
    }
    ctx.dpd.shapes.get(&(shape as i16)).map(|s| s.anim)
}

/// FUN_1404c9b30: flip-book timer u0: [t, index, previous index]. The wait
/// read is the previous index's, and the index drawn is the previous one.
fn shape_advance(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let Some(anim) = shape_anim(ctx, cursor) else {
        return;
    };
    let b = &ctx.dpd.bytes;
    let o = use_val(ctx, p, 0);
    let index = part.i16(o + 2) as u16;
    let previous = part.i16(o + 4) as u16;
    let (wait, flag) = effect::shape_sequence(b, anim, previous as usize).unwrap_or((1, 0));
    part.set_i16(o + 4, index as i16);
    let speed = i16_at(b, cursor + 8).unwrap_or(0);
    let t = (part.i16(o) as u16).wrapping_add(speed as u16);
    part.set_i16(o, t as i16);
    if wait as i32 <= t as i32 {
        part.set_i16(o, t.wrapping_sub(wait as u16) as i16);
        part.set_i16(o + 2, index.wrapping_add(1) as i16);
        let count = effect::shape_sequence_count(b, anim).unwrap_or(0);
        if count as i32 <= index.wrapping_add(1) as i32 {
            part.set_i16(o, 0);
            if flag & 0x80 != 0 {
                part.set_i16(o + 2, 0);
                return;
            }
            part.set_i16(o + 2, index as i16);
        }
    }
}

/// FUN_1404c99c0 → FUN_1404ae220 → FUN_1404ada10/FUN_1404ad7b0: the
/// previous flip-book frame's sprites through the draw matrix (or the u3
/// matrix when byte +0xe is set), coloured by the part colour bytes.
fn draw_shape(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let Some(anim) = shape_anim(ctx, cursor) else {
        return;
    };
    let o = use_val(ctx, p, 0);
    let index = part.i16(o + 4) as u16 as usize;
    let colour_var = use_val(ctx, p, 1);
    let m = if byte(ctx, cursor + 0xe) != 0 {
        read_matrix(part, use_val(ctx, p, 3))
    } else {
        part.draw
    };
    let packet = std::array::from_fn(|i| part.u8(colour_var + 8 + i));
    emit_sprites(ctx, anim, index, &m, packet);
}

/// FUN_1404ae220 with FUN_1404ada10/FUN_1404ad7b0: sprites of flip-book
/// frame `index` of the shape at `anim`, through matrix `m`, modulated by
/// the packet colour bytes (0x80 = 1.0).
pub(crate) fn emit_sprites(ctx: &mut Ctx, anim: usize, index: usize, m: &Mat, packet: [u8; 4]) {
    let b = &ctx.dpd.bytes;
    let packet: [f32; 4] = packet.map(|c| c as f32 / 128.);
    let Ok(sprites) = effect::shape_frame(b, anim, index) else {
        return;
    };
    let address = i16_at(b, anim + 0x10 + index * 8).unwrap_or(0) as isize;
    let frame = (anim as isize + address) as usize;
    let tex0 = u32_at(b, frame + 0x10).unwrap_or(0) as u64
        | (u32_at(b, frame + 0x14).unwrap_or(0) as u64) << 32;
    let tbp = (tex0 & 0x3fff) as u16;
    for s in sprites {
        let corners = s.corners.map(|[x, y]| {
            let v = math::apply(m, [x as f32 / 16., y as f32 / 16., 0., 1.]);
            [v[0], v[1], v[2]]
        });
        let [su, tv, w, h] = s.uv.map(|x| x as f32);
        let uv = [
            [su / 16., tv / 8.],
            [(su + w) / 16., tv / 8.],
            [(su + w) / 16., (tv + h) / 8.],
            [su / 16., (tv + h) / 8.],
        ];
        let colour = std::array::from_fn(|i| s.colour[i] as f32 / 128. * packet[i]);
        ctx.quads.push(Quad {
            corners,
            uv,
            colour,
            blend: s.colour[4],
            tbp,
            tex0,
        });
    }
}

/// FUN_1404d4ea0 → FUN_1404e1780 → FUN_1404d4f80: every `interval` ticks
/// `count` children of entry +0x10 are born at VSF vertices (sequential or
/// random), placed through the emitter's matrix, moving away from its
/// centre at `speed`.
fn born_rnd5(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let interval = byte(ctx, cursor + 0xd);
    if !active(part) || interval == 0 {
        return;
    }
    let o = use_val(ctx, p, 0);
    let mut countdown = part.u8(o + 2);
    if countdown == 0 {
        part.set_u8(o + 2, interval);
        let child = word(ctx, cursor + 0x10) as i32;
        let velocity_var = word(ctx, cursor + 0x1c) as i32;
        if child == -1 || word(ctx, cursor + 0x18) as i32 == -1 {
            return;
        }
        let vsf_index = i16_at(&ctx.dpd.bytes, cursor + 4).unwrap_or(0) as i32;
        let Some(vsf) = ctx.dpd.vsf.get(&vsf_index) else {
            return;
        };
        let n = vsf.indices.len();
        let random = byte(ctx, cursor + 0xe) != 0;
        let mut k = if random {
            (ctx.rng.rand() as usize) % n.max(1)
        } else {
            part.i16(o) as u16 as usize
        };
        let speed = float(ctx, cursor + 8);
        let position_var = word(ctx, cursor + 0x14) as usize;
        for _ in 0..byte(ctx, cursor + 0xc) {
            let vertex = vsf
                .indices
                .get(k)
                .and_then(|&i| ctx.dpd.vertex(vsf.model, i))
                .unwrap_or([0; 3]);
            let at = math::apply(
                &part.cur,
                [vertex[0] as f32, vertex[1] as f32, vertex[2] as f32, 1.],
            );
            let t = part.cur[3];
            let away = math::normalize([at[0] - t[0], at[1] - t[1], at[2] - t[2], at[3] - t[3]]);
            ctx.births.push(Birth {
                entry: child as usize,
                position_var,
                position: at,
                position_len: 4,
                velocity_var: Some(velocity_var as usize),
                velocity: math::scale(away, speed),
            });
            k = if random {
                (ctx.rng.rand() as usize) % n.max(1)
            } else if k + 1 >= n {
                0
            } else {
                k + 1
            };
        }
        part.set_i16(o, k as i16);
        countdown = interval;
    }
    part.set_u8(o + 2, countdown.wrapping_sub(1));
}
