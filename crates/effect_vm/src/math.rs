//! The exe's vector unit conventions (PS2 libvu0 kept on PC): a matrix is
//! four columns (x, y, z axes, translation), `mul(a, b)` = a·b applies b
//! first, `apply(m, v)` = m·v. Angles are integers with 0x10000 = 360°
//! looked up in a 4096-entry sine table (index = angle >> 4).
pub type Vec4 = [f32; 4];
pub type Mat = [Vec4; 4];

pub const IDENTITY: Mat = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

pub fn apply(m: &Mat, v: Vec4) -> Vec4 {
    std::array::from_fn(|r| m[0][r] * v[0] + m[1][r] * v[1] + m[2][r] * v[2] + m[3][r] * v[3])
}

pub fn mul(a: &Mat, b: &Mat) -> Mat {
    std::array::from_fn(|c| apply(a, b[c]))
}

pub fn scale(v: Vec4, s: f32) -> Vec4 {
    v.map(|x| x * s)
}

/// FUN_140478d80: xyz normalised (w kept).
pub fn normalize(v: Vec4) -> Vec4 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0. {
        [v[0] / l, v[1] / l, v[2] / l, v[3]]
    } else {
        v
    }
}

fn table_sin(angle: i32) -> f32 {
    let index = ((angle as i64 >> 4) & 0xfff) as f32;
    (index * std::f32::consts::TAU / 4096.).sin()
}

/// (sin, cos) of an integer angle (FUN_1404dc860 and siblings).
pub fn sin_cos(angle: i32) -> (f32, f32) {
    (table_sin(angle), table_sin(angle.wrapping_add(0x4000)))
}

pub fn rot_x(angle: i32) -> Mat {
    let (s, c) = sin_cos(angle);
    [
        [1., 0., 0., 0.],
        [0., c, s, 0.],
        [0., -s, c, 0.],
        [0., 0., 0., 1.],
    ]
}

pub fn rot_y(angle: i32) -> Mat {
    let (s, c) = sin_cos(angle);
    [
        [c, 0., -s, 0.],
        [0., 1., 0., 0.],
        [s, 0., c, 0.],
        [0., 0., 0., 1.],
    ]
}

pub fn rot_z(angle: i32) -> Mat {
    let (s, c) = sin_cos(angle);
    [
        [c, s, 0., 0.],
        [-s, c, 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ]
}

/// FUN_1404be820: Rz·Ry·Rx from integer angles.
pub fn rot_xyz(a: [i32; 3]) -> Mat {
    mul(&mul(&rot_z(a[2]), &rot_y(a[1])), &rot_x(a[0]))
}

/// FUN_1404e01f0: Ry·Rx·Rz from integer angles (Z applied first).
pub fn rot_zxy(a: [i32; 3]) -> Mat {
    mul(&mul(&rot_y(a[1]), &rot_x(a[0])), &rot_z(a[2]))
}

/// Float-radian rotations (instance matrix, FUN_1404b1280 → libvu0 RotMatrixX/Y/Z).
pub fn rot_radians(m: &Mat, r: [f32; 3]) -> Mat {
    let rx = {
        let (s, c) = r[0].sin_cos();
        [
            [1., 0., 0., 0.],
            [0., c, s, 0.],
            [0., -s, c, 0.],
            [0., 0., 0., 1.],
        ]
    };
    let ry = {
        let (s, c) = r[1].sin_cos();
        [
            [c, 0., -s, 0.],
            [0., 1., 0., 0.],
            [s, 0., c, 0.],
            [0., 0., 0., 1.],
        ]
    };
    let rz = {
        let (s, c) = r[2].sin_cos();
        [
            [c, s, 0., 0.],
            [-s, c, 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ]
    };
    mul(&rz, &mul(&ry, &mul(&rx, m)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_convention_and_rotations() {
        let t = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [5., 6., 7., 1.],
        ];
        assert_eq!(apply(&t, [1., 1., 1., 1.]), [6., 7., 8., 1.]);
        // A quarter turn about Z takes +X to +Y.
        let p = apply(&rot_z(0x4000), [1., 0., 0., 1.]);
        assert!((p[0]).abs() < 1e-6 && (p[1] - 1.).abs() < 1e-6);
        // mul applies the right operand first.
        let m = mul(&t, &rot_z(0x4000));
        let q = apply(&m, [1., 0., 0., 1.]);
        assert!((q[0] - 5.).abs() < 1e-6 && (q[1] - 7.).abs() < 1e-6);
    }
}
