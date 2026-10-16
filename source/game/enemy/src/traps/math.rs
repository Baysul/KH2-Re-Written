//! Table 0 traps for vectors, numbers and the shared random generator,
//! translated from the executable (Steam build 15194255).
//!
//! Vectors are four consecutive f32 lanes (x, y, z, w). The vector traps
//! first replace NaN lanes of their operand by zero in place (`_isnan` loops);
//! those that return a vector write it to a function-local `static` buffer and
//! return its address, which here is a [`World::static_cell`](crate::world::World::static_cell).

use crate::script::{Fault, Memory, Slot};
use crate::world::{Ctx, TrapFn};

type Args = [Slot; 16];
pub(crate) type Vec4 = [f32; 4];

pub fn lookup(table: u16, index: u16) -> Option<TrapFn> {
    if table != 0 {
        return None;
    }
    Some(match index {
        3 => frame_time,
        4 => vector_add,
        5 => vector_sub,
        6 => vector_len,
        7 => vector_normalize,
        13 => vector_roty,
        16 => random_get,
        17 => random_getf,
        18 => random_range,
        35 => vector_scale,
        36 => vector_mul,
        42 => vector_atan_xz,
        43 => fix_rad,
        _ => return None,
    })
}

/// Read a vector, zeroing NaN lanes in the source first (as the traps do).
pub(crate) fn read_sanitized(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    handle: u32,
) -> Result<Vec4, Fault> {
    let mut v = [0.0; 4];
    for (k, lane) in v.iter_mut().enumerate() {
        let at = handle + 4 * k as u32;
        let x = ctx.read_f32(m, at).ok_or(Fault::Memory(at))?;
        *lane = if x.is_nan() {
            ctx.write_f32(m, at, 0.0);
            0.0
        } else {
            x
        };
    }
    Ok(v)
}

pub(crate) fn read_vec(ctx: &Ctx<'_>, m: &Memory<'_>, handle: u32) -> Result<Vec4, Fault> {
    let mut v = [0.0; 4];
    for (k, lane) in v.iter_mut().enumerate() {
        let at = handle + 4 * k as u32;
        *lane = ctx.read_f32(m, at).ok_or(Fault::Memory(at))?;
    }
    Ok(v)
}

/// Store into the trap's own static cell and return its address.
fn return_vector(
    ctx: &mut Ctx<'_>,
    m: &mut Memory<'_>,
    index: u16,
    v: Vec4,
    args: &mut Args,
) -> Result<(), Fault> {
    let cell = ctx.world.static_cell(0, index, 0);
    for (k, lane) in v.iter().enumerate() {
        ctx.write_f32(m, cell + 4 * k as u32, *lane);
    }
    args[0] = Slot::address(cell);
    Ok(())
}

/// 0:3 FUN_14041ca50 (`DAT_140717480`).
fn frame_time(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    args[0] = Slot::float(ctx.world.frame_time);
    Ok(())
}

/// 0:4 FUN_14041cab0: lane-wise sum (FUN_14013fce0).
fn vector_add(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let a = read_sanitized(ctx, m, args[0].value)?;
    let b = read_sanitized(ctx, m, args[1].value)?;
    return_vector(ctx, m, 4, [0, 1, 2, 3].map(|k| b[k] + a[k]), args)
}

/// 0:5 FUN_14041cc00: lane-wise difference (FUN_14013fd20).
fn vector_sub(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let a = read_sanitized(ctx, m, args[0].value)?;
    let b = read_sanitized(ctx, m, args[1].value)?;
    return_vector(ctx, m, 5, [0, 1, 2, 3].map(|k| a[k] - b[k]), args)
}

/// FUN_140140f50: sqrtf(x*x + y*y + z*z), summed left to right in f32.
pub fn length3(v: &Vec4) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// 0:6 FUN_14041cf70.
fn vector_len(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let v = read_vec(ctx, m, args[0].value)?;
    args[0] = Slot::float(length3(&v));
    Ok(())
}

/// 0:7 FUN_14041cfa0 / FUN_140142080: scale xyz by 1/length unless the length
/// is zero; the length is written to w and returned.
fn vector_normalize(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let handle = args[0].value;
    let v = read_vec(ctx, m, handle)?;
    let len = length3(&v);
    if len != 0.0 {
        let inv = 1.0 / len;
        for (k, lane) in v.iter().take(3).enumerate() {
            ctx.write_f32(m, handle + 4 * k as u32, lane * inv);
        }
    }
    ctx.write_f32(m, handle + 12, len);
    args[0] = Slot::float(len);
    Ok(())
}

/// FUN_1403b6f20: wrap an angle into (-pi, pi].
pub fn wrap_angle(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    if a > 0.0 {
        (a + PI) % TAU - PI
    } else {
        PI - (PI - a) % TAU
    }
}

/// FUN_1401a90c0: bring an angle into [-pi, pi] by whole turns.
fn reduce_angle(mut a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    if a <= 0.0 {
        while a < -PI {
            a += TAU;
        }
    } else {
        while PI < a {
            a -= TAU;
        }
    }
    a
}

/// FUN_14013f900: a 4x4 matrix (sixteen floats) applied to a vector,
/// `out[k] = m[k]*x + m[4+k]*y + m[8+k]*z + m[12+k]*w`.
fn transform(m: &[f32; 16], v: &Vec4) -> Vec4 {
    [0, 1, 2, 3].map(|k| m[k] * v[0] + m[4 + k] * v[1] + m[8 + k] * v[2] + m[12 + k] * v[3])
}

/// FUN_1403b7680: rotate about Y by `angle`. The matrix is the identity for a
/// wrapped angle of zero, else FUN_140141270's rotation (cos, -sin / sin, cos).
pub fn rotate_y(v: &Vec4, angle: f32) -> Vec4 {
    let a = wrap_angle(angle);
    let mut matrix = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    if a != 0.0 {
        let a = reduce_angle(a);
        let (c, s) = (a.cos(), a.sin());
        matrix[0] = c;
        matrix[2] = -s;
        matrix[8] = s;
        matrix[10] = c;
        // FUN_14013f850 multiplies by the identity: a -0.0 entry comes out +0.0.
        for e in matrix.iter_mut() {
            if *e == 0.0 {
                *e = 0.0;
            }
        }
    }
    transform(&matrix, v)
}

/// 0:13 FUN_14041cfe0.
fn vector_roty(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let angle = args[1].as_float();
    let v = read_sanitized(ctx, m, args[0].value)?;
    return_vector(ctx, m, 13, rotate_y(&v, angle), args)
}

/// 0:16 FUN_14041dbf0: `state / ((2^32 + n - 1) / n)`.
fn random_get(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let n = args[0].value;
    let state = ctx.world.rng.step();
    if n == 0 {
        return Err(Fault::DivideByZero);
    }
    let step = (u64::from(n.wrapping_sub(1)) + 0x1_0000_0000) / u64::from(n);
    args[0] = Slot::int((u64::from(state) / step) as u32 as i32);
    Ok(())
}

/// DAT_1405b4d30: 2^-32.
const RANDOM_SCALE: f32 = 2.328_306_4e-10;

/// 0:17 FUN_14041dc40: `((float)state * 2^-32) * x`.
fn random_getf(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let state = ctx.world.rng.step();
    args[0] = Slot::float(state as f32 * RANDOM_SCALE * args[0].as_float());
    Ok(())
}

/// 0:18 FUN_14041dc80: `(((b - a) * (float)state) * 2^-32) + a`.
fn random_range(ctx: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let (a, b) = (args[0].as_float(), args[1].as_float());
    let state = ctx.world.rng.step();
    args[0] = Slot::float((b - a) * state as f32 * RANDOM_SCALE + a);
    Ok(())
}

/// 0:35 FUN_14041d220 (`method_vector_scale`): scale all four lanes in place.
fn vector_scale(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let handle = args[0].value;
    let s = args[1].as_float();
    let v = read_vec(ctx, m, handle)?;
    for (k, lane) in v.iter().enumerate() {
        ctx.write_f32(m, handle + 4 * k as u32, s * lane);
    }
    Ok(())
}

/// 0:36 FUN_14041cd50: the vector times a scalar, all four lanes.
fn vector_mul(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let v = read_sanitized(ctx, m, args[0].value)?;
    let s = args[1].as_float();
    return_vector(ctx, m, 36, v.map(|lane| s * lane), args)
}

/// 0:42 FUN_14041d250: `atan2f(x, z)`.
fn vector_atan_xz(ctx: &mut Ctx<'_>, m: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let v = read_sanitized(ctx, m, args[0].value)?;
    args[0] = Slot::float(v[0].atan2(v[2]));
    Ok(())
}

/// 0:43 FUN_14041e5e0: a non-finite angle is zero, else wrapped.
fn fix_rad(_: &mut Ctx<'_>, _: &mut Memory<'_>, args: &mut Args) -> Result<(), Fault> {
    let a = args[0].as_float();
    args[0] = Slot::float(if a.is_finite() { wrap_angle(a) } else { 0.0 });
    Ok(())
}

#[cfg(test)]
mod tests;
