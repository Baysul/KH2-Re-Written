//! Native-matrix conversion shared by simulation publications. The skin
//! binding/presentation half of skate-game's animation module is removed.
use bevy::prelude::*;
use skate_core::animation::output::NativeMatrix;

pub(crate) fn native_matrix(matrix: NativeMatrix) -> Mat4 {
    Mat4::from_cols(
        Vec3::from_array(matrix[0][..3].try_into().unwrap()).extend(0.0),
        Vec3::from_array(matrix[1][..3].try_into().unwrap()).extend(0.0),
        Vec3::from_array(matrix[2][..3].try_into().unwrap()).extend(0.0),
        Vec3::from_array(matrix[3][..3].try_into().unwrap()).extend(1.0),
    )
}

/// skate-game's skin basis: the shipped GLB's bone-local axes are rotated
/// -90 degrees about X relative to the native frames. Both are Y-up in world.
pub fn render_basis() -> Mat4 {
    Mat4::from_cols(Vec4::X, -Vec4::Z, Vec4::Y, Vec4::W)
}
