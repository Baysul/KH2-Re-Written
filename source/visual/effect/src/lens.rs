//! The lens-flare programs (table 0x140779a00): pppKeLnsLpSft (188),
//! pppKeLnsLp (189) and pppKeLnsClm (191), translated from their
//! decompilations. pppKeLnsLp keeps a lens block at its u0 (the light's
//! view-space frame, the flare's axis and its strength), each pppKeLnsClm
//! draws a column of flip-book sprites along that axis, and pppKeLnsLpSft
//! moves the block's angle limit and fade length over time.
//!
//! The lens block L (work offsets from pppKeLnsLp's u0): +0x00 the light's
//! frame (identity, translation its view position); +0x40 the flare frame
//! (+0x40 the axis on screen, +0x50 its normal, +0x60 z, +0x70 / +0x74 the
//! step along the axis, +0x78 the light's depth / 512, +0x7c the strength);
//! +0x80 the light's position in the instance; +0x8e the fade start; +0x8f
//! set for a light at infinity; +0x90 the instance's forward point; +0x9c
//! the angle limit; +0xa4 the light's distance on screen; +0xa8 the fade
//! length; +0xac 1.
//!
//! A column block C (pppKeLnsClm's u0): +0 the lens block (offset + 1; the
//! exe keeps a handle), +4 the shape (index + 1), +8 the sprite count, +9
//! set once the sprites' places are drawn, +0xa the spacing, +0xc the
//! start, +0xe the fade reversed; from +0x10 each sprite's 8 bytes: the
//! flip-book timer (i16), the frame (u8 at +2) and its place (f32 at +4).
use crate::math::{self, Mat, Vec4};
use crate::programs::{
    active, byte, emit_sprites, float, key, read_matrix, use_val, word, write_matrix,
};
use crate::{Ctx, Part};
use data::object_effect::{Program, i16_at};

pub(crate) const LNS_LP_SFT: u32 = 188;
pub(crate) const LNS_LP: u32 = 189;
pub(crate) const LNS_CLM: u32 = 191;
/// FUN_1404d60b0: the sprites of a column.
const SPRITES: u8 = 4;
/// `DAT_140623c24`: the place draw's divisor.
const PLACE_SCALE: f32 = 65535.;
/// `DAT_1405b1ca8`, `DAT_1405b1390`: screen units per depth unit.
const SCREEN: f32 = 512.;

pub(crate) fn handles(kind: u32) -> bool {
    matches!(kind, LNS_LP_SFT | LNS_LP | LNS_CLM)
}

pub(crate) fn draws(kind: u32) -> bool {
    matches!(kind, LNS_LP | LNS_CLM)
}

/// FUN_1404d5ce0 (pppKeLnsLpSft: the shift cleared), FUN_1404d6cb0
/// (pppKeLnsLp: FUN_1404e2690 puts the instance on draw layer 2, the flare
/// frame to identity and +0xac to 1, then the restart's clear) and
/// FUN_1404d60b0 (pppKeLnsClm: FUN_1404e1f10's empty block, four sprites).
pub(crate) fn create(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    let u0 = use_val(ctx, p, 0);
    match p.kind {
        LNS_LP_SFT => part.zero(u0, 8),
        LNS_LP => {
            ctx.flags[0] = 2;
            write_matrix(part, u0 + 0x40, &math::IDENTITY);
            part.set_f32(u0 + 0xac, 1.);
            restart(p, part, ctx);
        }
        LNS_CLM => {
            part.set_u8(u0 + 9, 0);
            part.set_i32(u0, 0);
            part.set_i32(u0 + 4, 0);
            part.set_u8(u0 + 8, SPRITES);
        }
        _ => {}
    }
}

/// The restart hooks: FUN_1404d5ce0 again, and FUN_1404d6cf0 (pppKeLnsLp's
/// angle limit and fade length cleared). pppKeLnsClm has none.
pub(crate) fn restart(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    let u0 = use_val(ctx, p, 0);
    match p.kind {
        LNS_LP_SFT => part.zero(u0, 8),
        LNS_LP => {
            part.set_f32(u0 + 0x9c, 0.);
            part.set_f32(u0 + 0xa8, 0.);
        }
        _ => {}
    }
}

pub(crate) fn update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    match p.kind {
        // FUN_1404d5c20: a key adds its two floats to the shift (u0); every
        // update then adds the shift to the lens block's angle limit and
        // fade length, except at the first frame of a part never restarted.
        LNS_LP_SFT => {
            let l = use_val(ctx, p, 1);
            if key(ctx, part, cursor) {
                part.set_f32(u0, part.f32(u0) + float(ctx, cursor + 4));
                part.set_f32(u0 + 4, part.f32(u0 + 4) + float(ctx, cursor + 8));
                if part.frame == 0 && !part.restarted {
                    return;
                }
            }
            part.set_f32(l + 0x9c, part.f32(l + 0x9c) + part.f32(u0));
            part.set_f32(l + 0xa8, part.f32(l + 0xa8) + part.f32(u0 + 4));
        }
        // FUN_1404d6c30: a key adds to the fade length and the angle limit
        // and sets the fade start and the infinity flag.
        LNS_LP if key(ctx, part, cursor) => {
            part.set_f32(u0 + 0xa8, part.f32(u0 + 0xa8) + float(ctx, cursor + 8));
            part.set_u8(u0 + 0x8e, byte(ctx, cursor + 0xc));
            part.set_f32(u0 + 0x9c, part.f32(u0 + 0x9c) + float(ctx, cursor + 4));
            part.set_u8(u0 + 0x8f, byte(ctx, cursor + 0xd));
        }
        // FUN_1404d63a0: a key binds the lens block (u1) and the shape (a
        // new one resets the sprites, FUN_1404e1f40) and sets the spacing,
        // the start and the reversed fade.
        LNS_CLM if key(ctx, part, cursor) => {
            part.set_i32(u0, use_val(ctx, p, 1) as i32 + 1);
            let shape = word(ctx, cursor + 4);
            if shape == 0xffff {
                part.set_i32(u0 + 4, 0);
            } else if part.i32(u0 + 4) != shape as i32 + 1 {
                set_shape(part, u0, shape, u16::from(byte(ctx, cursor + 0x16)), ctx);
            }
            let b = &ctx.dpd.bytes;
            part.set_i16(u0 + 0xa, i16_at(b, cursor + 0x12).unwrap_or(0));
            part.set_i16(u0 + 0xc, word(ctx, cursor + 0x10) as i16);
            part.set_u8(u0 + 0xe, byte(ctx, cursor + 0x17));
        }
        _ => {}
    }
}

fn shape_anim(ctx: &Ctx, shape: u32) -> Option<usize> {
    ctx.dpd.shapes.get(&(shape as i16)).map(|s| s.anim)
}

/// FUN_1404e1f40: each sprite's timer takes the first frame's wait and its
/// frame 0; the first time, the places are drawn from the seed
/// (s = s · 0x80d + 7, place = s / 65535).
fn set_shape(part: &mut Part, c: usize, shape: u32, seed: u16, ctx: &Ctx) {
    let Some(anim) = shape_anim(ctx, shape) else {
        part.set_i32(c + 4, 0);
        return;
    };
    part.set_i32(c + 4, shape as i32 + 1);
    let wait = i16_at(&ctx.dpd.bytes, anim + 0x12).unwrap_or(0);
    let n = usize::from(part.u8(c + 8));
    for i in 0..n {
        part.set_i16(c + 0x10 + 8 * i, wait);
        part.set_u8(c + 0x12 + 8 * i, 0);
    }
    if part.u8(c + 9) & 1 == 0 {
        let mut s = seed;
        for i in 0..n {
            s = s.wrapping_mul(0x80d).wrapping_add(7);
            part.set_f32(c + 0x14 + 8 * i, f32::from(s) / PLACE_SCALE);
        }
        part.set_u8(c + 9, part.u8(c + 9) | 1);
    }
}

pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        LNS_LP => draw_lens(p, part, ctx),
        LNS_CLM => draw_column(p, cursor, part, ctx),
        _ => {}
    }
}

fn sub(a: Vec4, b: Vec4) -> Vec4 {
    std::array::from_fn(|i| a[i] - b[i])
}

fn vec3(part: &Part, o: usize) -> Vec4 {
    [part.f32(o), part.f32(o + 4), part.f32(o + 8), 0.]
}

/// The screen point of the instance-space point at translation `t` (ede0 ·
/// [I | t] · origin, divided by w).
fn screen(ctx: &Ctx, t: Vec4) -> Vec4 {
    let mut m = math::IDENTITY;
    m[3] = t;
    let v = math::apply(&math::mul(&ctx.ede0, &m), [0., 0., 0., 1.]);
    math::scale(v, 1. / v[3])
}

/// FUN_1404d6d10 (pppKeLnsLp's draw): for a light at infinity the
/// instance's forward point (cur · (0, 0, 1, 1)) into +0x90; the light's
/// instance position (cur's translation) into +0x80; then FUN_1404e26c0.
fn draw_lens(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    let l = use_val(ctx, p, 0);
    if part.u8(l + 0x8f) != 0 {
        let v = math::apply(&part.cur, [0., 0., 1., 1.]);
        for (i, x) in v.iter().take(3).enumerate() {
            part.set_f32(l + 0x90 + 4 * i, *x);
        }
    }
    let t = part.cur[3];
    for (i, x) in t.iter().take(3).enumerate() {
        part.set_f32(l + 0x80 + 4 * i, *x);
    }
    lens_frame(part, l, t, ctx);
}

/// FUN_1404e26c0: the flare's frame and strength. `d` runs from the light
/// toward the screen centre: for a point light between the screen points
/// of the instance origin and of the light (z 512), for a light at
/// infinity between the view origin and the light's view position.
fn lens_frame(part: &mut Part, l: usize, t: Vec4, ctx: &Ctx) {
    let ed60 = ctx.ed60;
    let infinite = part.u8(l + 0x8f) != 0;
    let at = vec3(part, l + 0x80);
    let (mut from, light, mut d) = if infinite {
        let mut r = ed60;
        r[3] = [0., 0., 0., r[3][3]];
        let light = math::apply(&r, t);
        let from = math::scale(ed60[3], -1.);
        (from, light, sub(from, light))
    } else {
        let light = screen(
            ctx,
            [
                ed60[3][0] + at[0],
                ed60[3][1] + at[1],
                ed60[3][2] + at[2],
                1.,
            ],
        );
        let from = screen(ctx, ed60[3]);
        let mut d = sub(from, light);
        d[2] = SCREEN;
        (from, light, d)
    };
    let mut frame = math::IDENTITY;
    frame[3] = math::apply(&ed60, t);
    write_matrix(part, l, &frame);
    if d[0] == 0. && d[1] == 0. {
        part.set_vec4(l + 0x70, frame[3]);
    } else {
        let n = d[0].hypot(d[1]);
        let axis = [d[0] / n, d[1] / n, 0., 0.];
        part.set_vec4(l + 0x40, axis);
        // The axis × z (the frame's +0x60, kept (0, 0, 1)).
        part.set_vec4(l + 0x50, [axis[1], -axis[0], 0., 0.]);
        part.set_vec4(l + 0x60, [0., 0., 1., 0.]);
        part.set_vec4(l + 0x70, frame[3]);
    }
    let depth = if infinite {
        part.f32(l + 0x38)
    } else {
        ed60[3][2] + at[2]
    };
    let s = depth / SCREEN;
    part.set_f32(l + 0x78, s);
    d[3] = 0.;
    let d = math::normalize(d);
    part.set_f32(l + 0x70, d[0] * s);
    part.set_f32(l + 0x74, d[1] * s);
    from = sub(from, light);
    let distance = (from[0] * from[0] + from[1] * from[1] + from[2] * from[2]).sqrt();
    part.set_f32(l + 0xa4, from[0].hypot(from[1]));
    from[3] = 0.;
    let toward = math::normalize(from);
    let fall = |x: f32| if x <= 1. { 1. - x } else { 0. };
    let strength = if infinite {
        // Facing the light: 1, falling to 0 at a third of a turn.
        let mut k = fall((1. - (1. - toward[2]) * 0.5) / 0.3);
        if part.f32(l + 0x9c) >= 0. {
            let fwd = vec3(part, l + 0x90);
            let c = fwd[0] * toward[0] + fwd[1] * toward[1] + fwd[2] * toward[2];
            k *= fall((1. - c) * 0.5 / part.f32(l + 0x9c));
        }
        let span = part.f32(l + 0xa8) - f32::from(part.u8(l + 0x8e));
        if span > 0. {
            let r = distance / span;
            k * if r < 1. { 1. - r } else { 0. }
        } else {
            0.
        }
    } else if part.f32(l + 0xa8) > 0. {
        fall(part.f32(l + 0xa4) / part.f32(l + 0xa8))
    } else {
        0.
    };
    part.set_f32(l + 0x7c, strength);
}

/// FUN_1404d60e0 → FUN_1404e1b30 (pppKeLnsClm's draw): the colour bytes
/// (+8) × the instance colour; with a lens block, a shape and a strength,
/// each sprite steps its flip-book (+0x200 a draw), moves `place ·
/// spacing` steps along the axis from the start and draws its frame at
/// size `place · +0xc`, its alpha `(place · 0.75 + 0.25) · fade · alpha ·
/// strength`; the fade is the light's screen distance past the fade start
/// over (+0x14 · depth / 512), at most 1, reversed when +0xe is set.
fn draw_column(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let c = use_val(ctx, p, 0);
    let (lens, shape) = (part.i32(c) - 1, part.i32(c + 4) - 1);
    if lens < 0 || shape < 0 {
        return;
    }
    let l = lens as usize;
    let Some(anim) = shape_anim(ctx, shape as u32) else {
        return;
    };
    let strength = part.f32(l + 0x7c);
    if strength == 0. {
        return;
    }
    let colour: [u8; 4] = std::array::from_fn(|i| {
        (f32::from(byte(ctx, cursor + 8 + i)) * ctx.colour[i]) as i32 as u8
    });
    let size = float(ctx, cursor + 0xc);
    let fade_scale = f32::from(i16_at(&ctx.dpd.bytes, cursor + 0x14).unwrap_or(0) as u16);
    let infinite = part.u8(l + 0x8f) != 0;
    let mut m: Mat = if infinite {
        read_matrix(part, l)
    } else {
        let mut m = math::IDENTITY;
        m[3][2] = ctx.ed60[3][2] + part.f32(l + 0x88);
        m
    };
    let (mut x, mut y) = if infinite {
        (0., 0.)
    } else {
        (ctx.ed60[3][0], ctx.ed60[3][1])
    };
    let step = [part.f32(l + 0x70), part.f32(l + 0x74)];
    let spacing = f32::from(part.i16(c + 0xa));
    let start = f32::from(part.i16(c + 0xc));
    x += start * step[0];
    y += start * step[1];
    let span = part.f32(l + 0xa4) - f32::from(part.u8(l + 0x8e));
    let mut fade = if span > 0. {
        (span / (fade_scale * part.f32(l + 0x78))).min(1.)
    } else {
        0.
    };
    if part.u8(c + 0xe) != 0 {
        fade = 1. - fade;
    }
    let (unit_x, unit_y) = (part.f32(l), part.f32(l + 0x14));
    let count = i16_at(&ctx.dpd.bytes, anim + 6).unwrap_or(0);
    for i in 0..usize::from(part.u8(c + 8)) {
        let s = c + 0x10 + 8 * i;
        let place = part.f32(s + 4);
        let frame = part.u8(s + 2);
        let b = &ctx.dpd.bytes;
        let at = anim + usize::from(frame) * 8;
        let mut t = part.i16(s).wrapping_add(0x200);
        let wait = i16_at(b, at + 0x12).unwrap_or(0);
        if wait <= t {
            t = t.wrapping_sub(wait);
            let next = frame.wrapping_add(1);
            part.set_u8(s + 2, next);
            if count <= i16::from(next) {
                let flag = b.get(at + 0x14).copied().unwrap_or(0) as i8;
                t = 0;
                part.set_u8(s + 2, if flag < 0 { 0 } else { frame });
            }
        }
        part.set_i16(s, t);
        x += place * spacing * step[0];
        y += place * spacing * step[1];
        let alpha = ((f64::from(place) * 0.75 + 0.25)
            * f64::from(fade * f32::from(colour[3]) * strength)) as i32 as u8;
        m[0][0] = place * size * unit_x * ctx.scales[0];
        m[1][1] = place * size * unit_y * ctx.scales[1];
        m[3][0] = x;
        m[3][1] = y;
        let d = math::mul(&ctx.ede0, &m);
        let packet = [colour[0], colour[1], colour[2], alpha];
        emit_sprites(ctx, anim, usize::from(part.u8(s + 2)), &d, packet);
    }
}
