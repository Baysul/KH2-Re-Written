use super::*;
use skate_core::{
    math::Vector3,
    physics::{
        board::BodyId, collision::Sphere, contact::RetailContactMaterial,
        skeleton_body::SkeletonCollisionSettings, world_contact::ContactPrimitive,
    },
};

fn mode() -> SkeletonCollisionMode {
    let mut mode = SkeletonCollisionMode::new_normal(
        SkeletonCollisionSettings {
            enabled: true,
            normal_material: material(),
            compliant: [false; 24],
            priority: [0.0; 24],
            effect_time: 0.0,
        },
        true,
    );
    mode.select_driven(3).unwrap();
    mode
}
fn material() -> RetailContactMaterial {
    RetailContactMaterial {
        static_friction: 0.5,
        dynamic_friction: 0.3,
        restitution: 0.0,
    }
}
fn volume(body: CollisionBody, x: f32) -> BoardWorldVolume {
    BoardWorldVolume {
        collision_group: 0,
        body,
        primitive: ContactPrimitive::Sphere(Sphere {
            center: Vector3::new(x, 0.0, 0.0),
            radius: 0.5,
        }),
        motion: Default::default(),
        material: material(),
    }
}
