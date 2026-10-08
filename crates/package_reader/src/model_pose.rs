//! Explicit research bind routes; no installed transform convention selected.
//! OpenKH route formulas and .NET 10 FMA ordering are attributed in NOTICE.
use crate::model_skeleton::BoneRecord;
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum PoseProfile {
    #[serde(rename = "openkh_legacy_bind_fma_v1")]
    Legacy,
    #[serde(rename = "openkh_modern_bind_fma_v1")]
    Modern,
}
#[derive(Serialize)]
pub struct ModelPose {
    pub profile: PoseProfile,
    pub arithmetic_profile: &'static str,
    pub matrix_bits: Vec<[u32; 16]>,
    pub position_bits: Vec<[u32; 3]>,
    pub runtime_verified: bool,
}
type Matrix = [[f32; 4]; 4];
const IDENTITY: Matrix = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];
const MAX_ITEMS: usize = 262_144;
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn finite<const N: usize>(values: [f32; N]) -> io::Result<[f32; N]> {
    if values.iter().all(|n| n.is_finite()) {
        Ok(values)
    } else {
        Err(invalid("nonfinite pose input/output"))
    }
}
fn transform4(v: [f32; 4], m: &Matrix) -> [f32; 4] {
    std::array::from_fn(|i| {
        let a = m[0][i] * v[0];
        let b = m[1][i].mul_add(v[1], a);
        let c = m[2][i].mul_add(v[2], b);
        m[3][i].mul_add(v[3], c)
    })
}
fn transform3(v: [f32; 3], m: &Matrix) -> [f32; 3] {
    std::array::from_fn(|i| {
        let a = m[0][i] * v[0];
        let b = m[1][i].mul_add(v[1], a);
        m[2][i].mul_add(v[2], b) + m[3][i]
    })
}
fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    a.map(|row| transform4(row, b))
}
fn translation(t: [f32; 3]) -> Matrix {
    let mut result = IDENTITY;
    result[3][..3].copy_from_slice(&t);
    result
}
fn axis_rotation(axis: usize, angle: f32) -> Matrix {
    let (s, c) = angle.sin_cos();
    match axis {
        0 => [
            [1., 0., 0., 0.],
            [0., c, s, 0.],
            [0., -s, c, 0.],
            [0., 0., 0., 1.],
        ],
        1 => [
            [c, 0., -s, 0.],
            [0., 1., 0., 0.],
            [s, 0., c, 0.],
            [0., 0., 0., 1.],
        ],
        _ => [
            [c, s, 0., 0.],
            [-s, c, 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ],
    }
}
fn axis_quaternion(axis: usize, angle: f32) -> [f32; 4] {
    let (s, c) = (angle * 0.5).sin_cos();
    // Multiplication retains the signs of zero axis components.
    std::array::from_fn(|i| if i == 3 { c } else { f32::from(i == axis) * s })
}
fn quaternion_product(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let permutations = [[3, 2, 1, 0], [2, 3, 0, 1], [1, 0, 3, 2]];
    let signs = [[1., -1., 1., -1.], [1., 1., -1., -1.], [-1., 1., 1., -1.]];
    let mut result = b.map(|n| n * a[3]);
    for axis in 0..3 {
        for i in 0..4 {
            result[i] = (b[permutations[axis][i]] * a[axis]).mul_add(signs[axis][i], result[i]);
        }
    }
    result
}
fn quaternion_matrix([x, y, z, w]: [f32; 4]) -> Matrix {
    let (xx, yy, zz, xy, xz, wz, wy, yz, wx) = (
        x * x,
        y * y,
        z * z,
        x * y,
        x * z,
        z * w,
        y * w,
        y * z,
        x * w,
    );
    [
        [1. - 2. * (yy + zz), 2. * (xy + wz), 2. * (xz - wy), 0.],
        [2. * (xy - wz), 1. - 2. * (zz + xx), 2. * (yz + wx), 0.],
        [2. * (xz + wy), 2. * (yz - wx), 1. - 2. * (yy + xx), 0.],
        [0., 0., 0., 1.],
    ]
}
fn bind_matrices(bones: &[BoneRecord], profile: PoseProfile) -> io::Result<Vec<Matrix>> {
    if bones.len() > 2048 {
        return Err(invalid("too many pose bones"));
    }
    let mut result: Vec<Matrix> = Vec::new();
    let mut rotations = Vec::new();
    let mut translations: Vec<[f32; 3]> = Vec::new();
    for (ordinal, bone) in bones.iter().enumerate() {
        let t = finite(bone.transform_bits.map(f32::from_bits))?;
        let parent = match profile {
            PoseProfile::Legacy => bone.header_i32[1],
            PoseProfile::Modern => i32::from(bone.header_i16[2]),
        };
        if parent >= 0 && parent as usize >= ordinal {
            return Err(invalid("pose parent must precede child"));
        }
        let position = [t[8], t[9], t[10]];
        let matrix = match profile {
            PoseProfile::Legacy => {
                let (parent_rotation, parent_translation) = if parent < 0 {
                    ([0., 0., 0., 1.], [0.; 3])
                } else {
                    (rotations[parent as usize], translations[parent as usize])
                };
                let rotated = transform3(position, &quaternion_matrix(parent_rotation));
                let absolute_translation =
                    std::array::from_fn(|i| parent_translation[i] + rotated[i]);
                let mut local_rotation = [0., 0., 0., 1.];
                for axis in [2, 1, 0] {
                    if t[4 + axis] != 0. {
                        local_rotation =
                            quaternion_product(local_rotation, axis_quaternion(axis, t[4 + axis]));
                    }
                }
                let absolute_rotation =
                    finite(quaternion_product(parent_rotation, local_rotation))?;
                rotations.push(absolute_rotation);
                translations.push(finite(absolute_translation)?);
                multiply(
                    &multiply(&IDENTITY, &quaternion_matrix(absolute_rotation)),
                    &translation(absolute_translation),
                )
            }
            PoseProfile::Modern => {
                let mut matrix = IDENTITY;
                for axis in 0..3 {
                    matrix[axis][axis] = t[axis];
                }
                for axis in 0..3 {
                    matrix = multiply(&matrix, &axis_rotation(axis, t[4 + axis]));
                }
                matrix = multiply(&matrix, &translation(position));
                if parent >= 0 {
                    matrix = multiply(&matrix, &result[parent as usize]);
                }
                matrix
            }
        };
        for row in matrix {
            finite(row)?;
        }
        result.push(matrix);
    }
    Ok(result)
}
pub fn read_pose(bytes: &[u8], profile: PoseProfile) -> io::Result<ModelPose> {
    let structure = crate::model_skeleton::read_model_structure(bytes)?;
    let geometry = crate::model_geometry::read_geometry(bytes)?;
    let groups = crate::model_packets::read_packet_groups(bytes)?;
    let matrices = bind_matrices(&structure.blocks[0].bones, profile)?;
    let mut positions = Vec::new();
    let mut bindings = 0;
    for part in &geometry.parts {
        let group = groups
            .iter()
            .find(|g| {
                g.block_offset == geometry.block_offset && g.group_ordinal == part.group_ordinal
            })
            .ok_or_else(|| invalid("pose group missing"))?;
        let data = group
            .packets
            .get(part.packet_ordinal)
            .and_then(|p| p.vpu_data.as_ref())
            .ok_or_else(|| invalid("pose type-1 packet missing"))?;
        for assignment in &part.assignments {
            bindings += assignment.len();
            if assignment.is_empty()
                || assignment.len() > 32
                || bindings > MAX_ITEMS
                || positions.len() == MAX_ITEMS
            {
                return Err(invalid("pose assignments exceed bounded profile"));
            }
            let mut position = [0.; 3];
            for binding in assignment {
                let matrix = usize::try_from(binding.matrix_index)
                    .ok()
                    .and_then(|i| matrices.get(i))
                    .ok_or_else(|| invalid("pose matrix index out of range"))?;
                let vertex = finite(
                    data.vertex_bits
                        .get(binding.vertex_index)
                        .ok_or_else(|| invalid("pose coordinate index out of range"))?
                        .map(f32::from_bits),
                )?;
                if assignment.len() == 1 {
                    position = transform3([vertex[0], vertex[1], vertex[2]], matrix);
                } else {
                    let p = transform4(vertex, matrix);
                    for i in 0..3 {
                        position[i] += p[i];
                    }
                }
            }
            positions.push(finite(position)?.map(f32::to_bits));
        }
    }
    Ok(ModelPose {
        profile,
        arithmetic_profile: "dotnet10_fma_order_std_sincos_candidate_v1",
        matrix_bits: matrices
            .iter()
            .map(|m| std::array::from_fn(|i| m[i / 4][i % 4].to_bits()))
            .collect(),
        position_bits: positions,
        runtime_verified: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bone(parent: i32, scale: f32, x: f32) -> BoneRecord {
        BoneRecord {
            header_i32: [0, parent, 0, 0],
            header_i16: [0, 0, parent as i16, 0, 0, 0, 0, 0],
            transform_bits: [scale, scale, scale, 0., 0., 0., 0., 0., x, 0., 0., 0.]
                .map(f32::to_bits),
        }
    }
    #[test]
    fn distinct_parent_scale_routes_and_homogeneous_weights() {
        let b = [bone(-1, 2., 10.), bone(0, 1., 1.)];
        let legacy = bind_matrices(&b, PoseProfile::Legacy).unwrap();
        let modern = bind_matrices(&b, PoseProfile::Modern).unwrap();
        assert_eq!(legacy[1][3][0], 11.);
        assert_eq!(modern[1][3][0], 12.);
        assert_eq!(transform3([2., 0., 0.], &legacy[1])[0], 13.);
        assert_eq!(transform3([2., 0., 0.], &modern[1])[0], 16.);
        assert_eq!(
            transform4([1., 0., 0., 0.5], &legacy[0])[0]
                + transform4([2., 0., 0., 0.5], &legacy[1])[0],
            13.5
        );
        assert_eq!(
            transform4([1., 0., 0., 0.5], &modern[0])[0]
                + transform4([2., 0., 0., 0.5], &modern[1])[0],
            17.
        );
    }
    #[test]
    fn fused_vector_order_retains_small_cancellation_result() {
        let mut m = [[0.; 4]; 4];
        m[0][0] = -f32::from_bits(0x3f800002);
        m[1][0] = f32::from_bits(0x3f800001);
        let result = transform4([1., f32::from_bits(0x3f800001), 0., 0.], &m);
        assert_eq!(result[0].to_bits(), 0x28800000);
    }
    #[test]
    fn quaternion_order_and_matrix_preserve_authored_axis_permutation() {
        let a = [0.5; 4];
        let b = [0., 0., 1., 0.];
        assert_eq!(quaternion_product(a, b), [0.5, -0.5, 0.5, -0.5]);
        assert_eq!(quaternion_product(b, a), [-0.5, 0.5, 0.5, -0.5]);
        assert_eq!(
            transform3([1., 2., 3.], &quaternion_matrix(a)),
            [3., 1., 2.]
        );
    }
    #[test]
    fn rejects_overflow_and_unknown_profiles_without_substitution() {
        assert!(
            bind_matrices(
                &[bone(-1, 1., f32::MAX), bone(0, 1., f32::MAX)],
                PoseProfile::Legacy
            )
            .is_err()
        );
        assert!(
            bind_matrices(
                &[bone(-1, f32::MAX, 0.), bone(0, f32::MAX, 0.)],
                PoseProfile::Modern
            )
            .is_err()
        );
        assert!(serde_json::from_str::<PoseProfile>("\"unknown\"").is_err());
        assert!(read_pose(&[], PoseProfile::Legacy).is_err());
    }
    #[test]
    fn rejects_forward_parents_nonfinite_and_oversized_bone_arrays() {
        for profile in [PoseProfile::Legacy, PoseProfile::Modern] {
            assert!(bind_matrices(&[bone(0, 1., 0.)], profile).is_err());
            assert!(bind_matrices(&[bone(-1, f32::NAN, 0.)], profile).is_err());
            let bones: Vec<_> = (0..2049).map(|_| bone(-1, 1., 0.)).collect();
            assert!(bind_matrices(&bones, profile).is_err());
        }
    }
}
