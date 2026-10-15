//! CPU-only, generated-data comparison of animated vertex-buffer updates.
//! Run with the same feature/profile as the build being investigated:
//! cargo run -p main --example mesh_update_bench --release --features skate3 --locked
use bevy::{mesh::VertexAttributeValues, prelude::*};
use std::{hint::black_box, time::Instant};
use visual::room::{mesh, update_positions};

fn main() {
    const VERTICES: usize = 12_288;
    const UPDATES: usize = 20_000;
    let pose: Vec<Vec3> = (0..4_096)
        .map(|i| Vec3::new(i as f32 * 0.25, -(i as f32), (i % 17) as f32))
        .collect();
    let indices: Vec<usize> = (0..VERTICES).map(|i| (i * 17) % pose.len()).collect();
    let initial = vec![[0.; 3]; VERTICES];
    let mut replaced = mesh(
        initial.clone(),
        vec![[0.; 2]; VERTICES],
        vec![[1.; 4]; VERTICES],
    );
    let mut reused = mesh(initial, vec![[0.; 2]; VERTICES], vec![[1.; 4]; VERTICES]);
    let mut samples = [Vec::new(), Vec::new()];
    for round in 0..9 {
        // Alternate order so neither path always benefits from going first.
        for path in [round % 2, 1 - round % 2] {
            let mesh = if path == 0 {
                &mut replaced
            } else {
                &mut reused
            };
            let start = Instant::now();
            for _ in 0..UPDATES {
                let pose = black_box(&pose);
                let positions = black_box(&indices).iter().map(|&i| pose[i].to_array());
                if path == 0 {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.collect::<Vec<_>>());
                } else {
                    update_positions(mesh, positions);
                }
                black_box(mesh.attribute(Mesh::ATTRIBUTE_POSITION));
            }
            if round != 0 {
                samples[path].push(start.elapsed().as_secs_f64());
            }
        }
    }
    let expected = replaced.create_packed_vertex_buffer_data();
    assert_eq!(expected.len(), VERTICES * 36);
    assert_eq!(expected, reused.create_packed_vertex_buffer_data());
    let capacity = match reused.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
        VertexAttributeValues::Float32x3(values) => values.capacity(),
        _ => unreachable!(),
    };
    for sample in &mut samples {
        sample.sort_by(f64::total_cmp);
    }
    let median = |sample: &[f64]| (sample[3] + sample[4]) * 0.5;
    println!(
        "{}",
        serde_json::json!({
            "fixture":"generated-indexed-positions-v1",
            "vertices":VERTICES,"updates_per_sample":UPDATES,
            "samples":samples,"replacement_median_seconds":median(&samples[0]),
            "reuse_median_seconds":median(&samples[1]),"identical_vertex_bytes":true,
            "retained_capacity":capacity,
            "scope":"CPU vertex update only; not a gameplay FPS benchmark"
        })
    );
}
