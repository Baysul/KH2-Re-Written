use super::*;
use skate_core::physics::{
    board_world::{
        WorldTriangle,
        query_metadata::{QueryMesh, QueryMetadata},
    },
    contact::RetailContactMaterial,
    drive_frames::RetailAffineTransform,
};

fn triangle(height: f32, fatness: f32) -> WorldTriangle {
    WorldTriangle::from_vertices(
        [
            Vector3::new(-2., height, -2.),
            Vector3::new(-2., height, 2.),
            Vector3::new(2., height, -2.),
        ],
        RetailContactMaterial {
            static_friction: 0.7,
            dynamic_friction: 0.4,
            restitution: 0.2,
        },
        0xdead_beef,
        0xe0,
        [1.; 3],
        fatness,
    )
    .unwrap()
}

fn world(entries: &[(f32, QueryPool, i32, u16)], flags: u32, fatness: f32) -> BoardWorld {
    let triangles: Vec<_> = entries.iter().map(|e| triangle(e.0, fatness)).collect();
    let meshes = entries
        .iter()
        .enumerate()
        .map(|(index, e)| QueryMesh {
            triangle_range: index..index + 1,
            local_to_world: RetailAffineTransform::IDENTITY,
            world_to_local: RetailAffineTransform::IDENTITY,
            local_bounds: Bounds::from_points(triangles[index].triangle.vertices).unwrap(),
            matching_group: e.2,
            rejection_flags: u32::MAX,
            geometry: 700,
            pool: e.1,
        })
        .collect();
    BoardWorld::with_query_metadata(
        triangles,
        QueryMetadata {
            packed_surfaces: entries.iter().map(|e| e.3).collect(),
            meshes,
            static_edges: vec![],
            island_flags: flags,
        },
    )
    .unwrap()
}

fn down() -> Probe {
    Probe {
        start: [-0.5, 1., -0.5, 0.],
        end: [-0.5, -1., -0.5, 0.],
        radius: 0.,
    }
}
