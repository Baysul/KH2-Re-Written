//! Post-solver contact reports and physical/animation errors for the live skater.
//! Engine report storage is ours; spy arithmetic and report order are native.
use super::{GamePhysics, SkaterRuntime, solve::deck_frame};
use skate_core::physics::{
    assembly::BodySnapshot,
    board_step::CollisionBody,
    contact_feedback::spy_contact_jacobians,
    skeleton_animation_record::AnimationPartTransform,
    skeleton_board_frames::SkeletonBoardFrames,
    skeleton_body::{SkeletonCollisionInput, SkeletonContactBody, SkeletonContactReport},
    skeleton_root::SkeletonRootFrames,
};

pub(super) fn publish(
    physics: &GamePhysics,
    skater: &mut SkaterRuntime,
    request_partial_ragdoll: bool,
) {
    let reports = collect(physics, skater);
    let p = &skater.player_input.processed;
    let deck = deck_frame(&physics.board);
    skater.skeleton_output.correction.observe_board(
        deck[3],
        skater.animated_skeleton.roots.predicted_board_position,
    );
    let state = p.state_2508;
    let offboard = p.category_2512 == 500 && !matches!(state, 501 | 503);
    let mut point = p.vectors_464_480_496_512_528[if matches!(state, 601 | 602) { 1 } else { 2 }]
        .map(f32::from_bits);
    // Both82BD8280 and82BE2538 read Skeleton16424 ->Reckoning1216.
    let n = physics.riding.reckoning.ground_normal;
    let mut normal = [n.x, n.y, n.z, 0.0];
    if offboard && p.flags_2480 & 2 != 0 {
        point = p.vectors_880_896_912_928_944[0].map(f32::from_bits);
        normal = p.vectors_880_896_912_928_944[1].map(f32::from_bits);
    }
    let frames = skater.skeleton.part_transforms();
    skater.collision_feedback.update(
        &SkeletonCollisionInput {
            dt: physics.settings.step.simulation.time_step,
            plane_point: point,
            plane_normal: normal,
            reference_velocity: if state == 201 {
                skater.skeleton.record.velocities[23]
            } else {
                skater.skeleton_input.deck_velocity
            },
            com_velocity: skater.animated_skeleton.board_frames.com_velocity,
            ragdoll: skater.skeleton_collision.is_ragdoll,
            disable_ground_filter: p.flags_2472 & 0x8000 != 0,
            ai_collision_scalar: p.flags_2472 & 0x1000_0000 != 0,
            offboard,
            entering_offboard: matches!(state, 501 | 503),
            category_600: p.category_2512 == 600,
            request_partial_ragdoll,
            physical: &skater.skeleton.record,
            part_weights: &skater.skeleton.definition.animation_masses.part_weights,
            body_frames: &frames,
        },
        &reports,
    );
    let region_response = skater.collision_feedback.fill_response(
        &skater.skeleton.record,
        &skater.skeleton.definition.animation_masses.part_weights,
    );
    // SkeletonCollision Fill82BD60C8 -> Collision+80/+196/+200/+208/+214/+217.
    let collision = &mut skater.player_input.physical.collision;
    collision.contact_region_response_80 = region_response;
    collision.body_response_196 = skater.collision_feedback.wipeout_times[0];
    collision.force_response_200 = skater.collision_feedback.wipeout_times[1];
    collision.surface_height_208 = skater.collision_feedback.material_12_height;
    collision.flag_214 = u8::from(skater.collision_feedback.flags.material_6);
    collision.flag_217 = u8::from(skater.collision_feedback.flags.material_12);
    skater.skeleton_collision.finish_contact_frame();
    skater.pose_errors.targets = skater.skeleton_input.extra_target_positions;
    skater.pose_errors.update(
        &skater.skeleton.record,
        skater.animated_skeleton.roots.animation_to_world,
        &skater.skeleton_input.drive_frames,
    );
    let response = if skater.skeleton_collision.partial_ragdoll {
        skater.pose_errors.partial_response()
    } else {
        skater
            .pose_errors
            .normal_response(&skater.collision_feedback, [n.x, n.y, n.z, 0.0])
    };
    skater.collision_pose_error = response.impulse;
    skater.collision_extra_errors = response.extra;
    skater.collision_maximum_error = Some(response.maximum_error);
    publish_board_observations(
        &mut skater.animated_skeleton.board_frames,
        &skater.animated_skeleton.roots,
        &deck,
        skater.skeleton.record.centre_of_mass,
        physics.settings.step.simulation.time_step,
        p.flags_2472,
    );
}

fn publish_board_observations(
    frames: &mut SkeletonBoardFrames,
    roots: &SkeletonRootFrames,
    deck: &AnimationPartTransform,
    physical_com: [f32; 4],
    dt: f32,
    flags_2472: u32,
) {
    //82BD9F3C..9F70 transforms retained16144 into16208;9F90 uses the
    //current deck for16240. Only later82BDA024 replaces16144 with physical
    //COM10928. Contact/error processing above does not consume these locals.
    frames.publish_local_observations(roots, deck);
    frames.publish_centre_of_mass(physical_com, dt, flags_2472);
}

fn collect(physics: &GamePhysics, skater: &SkaterRuntime) -> Vec<SkeletonContactReport> {
    let rows = physics.board.solved_contacts();
    let mut storage: Vec<_> = rows
        .iter()
        .flat_map(|row| row.words().iter().copied())
        .collect();
    let mut count = 0;
    spy_contact_jacobians(
        &mut storage,
        rows.len() as u32,
        &mut count,
        physics.settings.step.simulation.frequency,
        |id| {
            resolve(id, physics, skater).map_or([0; 4], |body| {
                let p = body.rates.position;
                [p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), 0]
            })
        },
    );
    let mut reports = Vec::with_capacity(16);
    let mut mod_reports = Vec::new();
    for spy in storage[..count as usize * 28].chunks_exact(28) {
        let a = CollisionBody::from_contact_id(spy[24]);
        let b = CollisionBody::from_contact_id(spy[25]);
        //82768418 rejects the same logical Body and equal nonnull Body+32
        //owners. Board82C067F4 and Skeleton82BE4620 both store this actor.
        //Their eligible cross contacts still participate in the shared solve.
        let remote_base =
            skater.skeleton.bodies().len() + skater.skeleton_drives.targets.bodies.len();
        let external = |body| {
            matches!(body, CollisionBody::StaticWorld)
                || matches!(body, CollisionBody::Attached(index) if index >= remote_base)
        };
        if !external(a) && !external(b) {
            continue;
        }
        for (own, other, side_a) in [(a, b, true), (b, a, false)] {
            let CollisionBody::Attached(part) = own else {
                continue;
            };
            if part >= 24 {
                continue;
            }
            let vector = |offset| std::array::from_fn(|i| f32::from_bits(spy[offset + i]));
            let point = vector(12);
            let normal = vector(0).map(|v| v * if side_a { 1.0 } else { -1.0 });
            let other_group = match other {
                CollisionBody::StaticWorld => 0,
                CollisionBody::Board(_) => physics.board.collision_group(),
                CollisionBody::Attached(index) => physics
                    .network_proxies
                    .groups
                    .get(&index)
                    .copied()
                    .unwrap_or(skater.skeleton_collision.assembly_group),
            };
            let destination = if matches!(other,CollisionBody::Attached(index)
                if physics.network_proxies.solids.iter().any(|(i,_)|*i==index))
            {
                &mut mod_reports
            } else {
                &mut reports
            };
            let vehicle = other_group == 8;
            destination.push(SkeletonContactReport {
                part,
                normal,
                point,
                tag: if side_a {
                    spy[26] & 0xffff
                } else {
                    spy[26] >> 16
                },
                other_group,
                other_entity: None,
                body_a: contact_body(
                    resolve(spy[24], physics, skater),
                    point,
                    vehicle && is_mod_solid_contact(spy[24], physics),
                ),
                body_b: contact_body(
                    resolve(spy[25], physics, skater),
                    point,
                    vehicle && is_mod_solid_contact(spy[25], physics),
                ),
                side_a,
                solved_vector: vector(20),
            });
        }
    }
    // A large set of floor contacts must not crowd a moving prop impact out
    // of the native 16-report observation budget. Preserve order within each
    // group; the physical solve above still receives EVERY contact.
    mod_reports.extend(reports);
    mod_reports.truncate(16);
    mod_reports
}
fn is_mod_solid_contact(id: u32, physics: &GamePhysics) -> bool {
    matches!(
        CollisionBody::from_contact_id(id),
        CollisionBody::Attached(index)
            if physics.network_proxies.solids.iter().any(|(i, _)| *i == index)
    )
}

fn resolve<'a>(
    id: u32,
    physics: &'a GamePhysics,
    skater: &'a SkaterRuntime,
) -> Option<&'a BodySnapshot> {
    match CollisionBody::from_contact_id(id) {
        CollisionBody::StaticWorld => None,
        CollisionBody::Board(id) => Some(&physics.board.bodies()[id.index()]),
        CollisionBody::Attached(part) => skater
            .skeleton
            .bodies()
            .iter()
            .chain(skater.skeleton_drives.targets.bodies.iter())
            .chain(physics.network_proxies.bodies.iter())
            .nth(part),
    }
}
fn contact_body(
    body: Option<&BodySnapshot>,
    point: [f32; 4],
    vehicle: bool,
) -> SkeletonContactBody {
    match body {
        Some(body) => {
            let v = if vehicle {
                linear_velocity_at_point(body, point)
            } else {
                let v = body.rates.linear_velocity;
                [v.x, v.y, v.z, 0.0]
            };
            SkeletonContactBody {
                state_flags: body.state_flags,
                inverse_mass: body.inertia.inverse_mass,
                linear_velocity: v,
            }
        }
        None => SkeletonContactBody {
            state_flags: 1,
            inverse_mass: 0.0,
            linear_velocity: [0.0; 4],
        },
    }
}

/// Group-8 contacts use the surface point velocity (v + ω×r), matching the
/// solver contact row and keeping mVehicleContact low when riding a moving car.
fn linear_velocity_at_point(body: &BodySnapshot, point: [f32; 4]) -> [f32; 4] {
    let r = [
        point[0] - body.rates.position.x,
        point[1] - body.rates.position.y,
        point[2] - body.rates.position.z,
        0.0,
    ];
    let w = body.rates.angular_velocity;
    let v = body.rates.linear_velocity;
    [
        v.x + w.y * r[2] - w.z * r[1],
        v.y + w.z * r[0] - w.x * r[2],
        v.z + w.x * r[1] - w.y * r[0],
        0.0,
    ]
}
