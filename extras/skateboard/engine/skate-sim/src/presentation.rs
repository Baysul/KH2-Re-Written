//! Render-side conversions of completed fixed ticks (skate-game's
//! presentation snapshot math, without its Bevy resource and replay history).
//! Interpolated values never feed back into simulation.
use bevy::prelude::*;
use skate_core::camera::CameraFrame;

/// Native camera frame to a Bevy camera transform (looks along local -Z).
pub fn camera_transform(frame: CameraFrame) -> Transform {
    let [right, up, at] = frame.basis.columns.map(Vec3::from_array);
    Transform {
        translation: Vec3::new(frame.position[0], frame.position[1], frame.position[2]),
        // Native At is forward; Bevy cameras look along local -Z.
        rotation: Quat::from_mat3(&Mat3::from_cols(-right, up, -at)).normalize(),
        ..default()
    }
}

pub fn blend(a: Transform, b: Transform, alpha: f32) -> Transform {
    Transform {
        translation: a.translation.lerp(b.translation, alpha),
        rotation: a.rotation.normalize().slerp(b.rotation.normalize(), alpha),
        scale: a.scale.lerp(b.scale, alpha),
    }
}
