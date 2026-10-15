//! First source-bound room/character scene. Camera inspection, no game rules.
use bevy::{
    app::AppExit,
    camera::ScalingMode,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
    render::view::{
        NoIndirectDrawing,
        screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use data::{
    model_inspection::{self, InspectionModel},
    model_inspection_textures::TextureMode,
    motion::{self, SelectedMotion},
    probe_source::ProbeSource,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};
use visual::room::{LoadedRoom, Room, load_rooms, mesh, rgba_image, upload};

#[derive(Resource)]
struct Scene {
    rooms: Vec<Room>,
    frame_points: Vec<Vec3>,
    actor: InspectionModel,
    log: File,
    capture: PathBuf,
    frames: u64,
    limit: Option<u64>,
    pending: bool,
    captured: bool,
    direction: Vec3,
    zoom: f32,
    motions: Vec<SelectedMotion>,
    motion_index: usize,
    motion_frame: u32,
    motion_paused: bool,
    actor_offset: Vec3,
    actor_scale: f32,
    motion_tour: bool,
    capture_count: usize,
    last_capture_frame: u64,
    displayed_motion_frame: u32,
    camera_target: Vec3,
    inspection_ground: Option<f32>,
}
#[derive(Component)]
struct ActorPart(usize);
impl Scene {
    fn event(&mut self, value: serde_json::Value) {
        serde_json::to_writer(&mut self.log, &value).expect("scene log write");
        writeln!(self.log).expect("scene log newline");
        self.log.flush().expect("scene log flush");
    }
}
fn setup(
    mut commands: Commands,
    mut scene: ResMut<Scene>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let points = &scene.frame_points;
    let low = points.iter().fold(Vec3::splat(f32::MAX), |a, &b| a.min(b));
    let high = points.iter().fold(Vec3::splat(f32::MIN), |a, &b| a.max(b));
    let center = (low + high) * 0.5;
    let radius = (high - low).max_element() * 0.5;
    let scale = 1. / radius;
    let mut room_triangles = 0;
    let mut texture_observations = Vec::new();
    for (room_ordinal, room) in scene.rooms.iter().enumerate() {
        let handles: Vec<_> = room
            .textures
            .iter()
            .map(|t| upload(t, &mut images))
            .collect();
        for t in &room.textures {
            texture_observations.push(serde_json::json!({"room":room_ordinal,"environment":t.environment_ordinal,
            "width":t.width,"height":t.height,"rgba_sha256":format!("{:x}",Sha256::digest(&t.pixels_rgba))}));
        }
        for chunk in &room.model.chunks {
            if chunk.indices.is_empty() {
                continue;
            }
            let positions = chunk
                .indices
                .iter()
                .map(|&i| {
                    ((Vec3::from_array(
                        chunk.vertices[i as usize].position_bits.map(f32::from_bits),
                    ) - center)
                        * scale)
                        .to_array()
                })
                .collect();
            let uvs = chunk
                .indices
                .iter()
                .map(|&i| chunk.vertices[i as usize].uv_bits.map(f32::from_bits))
                .collect();
            let colors = chunk
                .indices
                .iter()
                .map(|&i| chunk.vertices[i as usize].color_bits.map(f32::from_bits))
                .collect();
            let texture = room
                .textures
                .iter()
                .position(|t| t.environment_ordinal as i16 == chunk.texture_index)
                .map(|i| handles[i].clone());
            commands.spawn((
                Mesh3d(meshes.add(mesh(positions, uvs, colors))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: if texture.is_some() {
                        Color::WHITE
                    } else {
                        Color::srgb(1., 0., 1.)
                    },
                    base_color_texture: texture,
                    unlit: true,
                    cull_mode: None,
                    alpha_mode: if chunk.transparency_flag == 0 {
                        AlphaMode::Opaque
                    } else {
                        AlphaMode::Blend
                    },
                    ..default()
                })),
                Transform::default(),
            ));
            room_triangles += chunk.indices.len() / 3;
        }
    }
    let actor = &scene.actor;
    let pose = &actor.poses[0];
    let actor_points: Vec<Vec3> = pose
        .position_bits
        .iter()
        .map(|p| Vec3::from_array(p.map(f32::from_bits)))
        .collect();
    let actor_low = actor_points
        .iter()
        .fold(Vec3::splat(f32::MAX), |a, &b| a.min(b));
    let actor_high = actor_points
        .iter()
        .fold(Vec3::splat(f32::MIN), |a, &b| a.max(b));
    // Explicit inspection placement. This is not a decoded gameplay spawn.
    let actor_center = (actor_low + actor_high) * 0.5;
    let placement = Vec3::new(center.x, scene.inspection_ground.unwrap_or(low.y), center.z)
        - Vec3::new(actor_center.x, actor_low.y, actor_center.z);
    let mut actor_triangles = 0;
    for (ordinal, part) in actor.parts.iter().enumerate() {
        if part.indices.is_empty() {
            continue;
        }
        let positions = part
            .indices
            .iter()
            .map(|&i| ((actor_points[i as usize] + placement - center) * scale).to_array())
            .collect();
        let mut uvs = vec![[0., 0.]; part.indices.len()];
        let mut texture = None;
        if let Some(set) = &actor.textures {
            let route = &actor.texture_routes[0].parts[ordinal];
            uvs = route
                .expanded_uv_bits
                .iter()
                .map(|uv| uv.map(f32::from_bits))
                .collect();
            let binding = set
                .bind(route.environment_ordinal, TextureMode::OriginalIndexed)
                .expect("admitted actor binding");
            if let Some(image) = binding.image_index {
                let level = &set.images[image].levels[0];
                texture = Some(rgba_image(
                    level.width,
                    level.height,
                    level.pixels_rgba.clone(),
                    &mut images,
                ));
            }
        }
        commands.spawn((
            ActorPart(ordinal),
            Mesh3d(meshes.add(mesh(positions, uvs, vec![[1.; 4]; part.indices.len()]))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: texture,
                unlit: true,
                cull_mode: None,
                alpha_mode: AlphaMode::Opaque,
                ..default()
            })),
            Transform::default(),
        ));
        actor_triangles += part.indices.len() / 3;
    }
    scene.actor_offset = placement - center;
    scene.actor_scale = scale;
    scene.camera_target = (actor_center + placement - center) * scale;
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            near: 0.01,
            far: 1000.,
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 1.6,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(scene.camera_target + scene.direction * 0.35)
            .looking_at(scene.camera_target, Vec3::Y),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        NoIndirectDrawing,
    ));
    let inspection_ground = scene.inspection_ground;
    scene.event(serde_json::json!({"event":"scene","room_triangles":room_triangles,"actor_triangles":actor_triangles,
        "room_center":center.to_array(),"radius":radius,"actor_translation":placement.to_array(),"placement_is_gameplay_spawn":false,"inspection_ground_height":inspection_ground,
        "textures":texture_observations,"materials":"original indexed, nearest, decoded opaque/blend split, unlit inspection",
        "gameplay_verified":false}));
}
fn animate(
    mut scene: ResMut<Scene>,
    keys: Res<ButtonInput<KeyCode>>,
    actor_meshes: Query<(&ActorPart, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut exit: MessageWriter<AppExit>,
) {
    if scene.motions.is_empty() || scene.pending {
        return;
    }
    if scene.motion_tour {
        if scene.capture_count >= scene.motions.len() * 2 {
            return;
        }
        scene.motion_index = scene.capture_count / 2;
        scene.motion_frame = if scene.capture_count.is_multiple_of(2) {
            0
        } else {
            15
        };
    }
    if keys.just_pressed(KeyCode::Space) {
        scene.motion_paused = !scene.motion_paused;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        scene.motion_index = (scene.motion_index + 1) % scene.motions.len();
        scene.motion_frame = 0;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        scene.motion_frame = scene.motion_frame.saturating_sub(1);
    }
    let selected = &scene.motions[scene.motion_index];
    let pose = match scene
        .actor
        .skinning
        .sample_fk(&selected.motion, scene.motion_frame)
    {
        Ok(pose) => pose,
        Err(e) => {
            scene.event(serde_json::json!({"event":"motion_failure","error":e.to_string(),"gameplay_verified":false}));
            exit.write(AppExit::Error(std::num::NonZeroU8::new(1).unwrap()));
            return;
        }
    };
    for (part, handle) in &actor_meshes {
        let positions: Vec<[f32; 3]> = scene.actor.parts[part.0]
            .indices
            .iter()
            .map(|&i| {
                ((Vec3::from_array(pose.position_bits[i as usize].map(f32::from_bits))
                    + scene.actor_offset)
                    * scene.actor_scale)
                    .to_array()
            })
            .collect();
        meshes
            .get_mut(&handle.0)
            .expect("retained actor mesh")
            .insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    }
    if scene.frames.is_multiple_of(30) || keys.just_pressed(KeyCode::KeyM) || scene.motion_paused {
        let slot = selected.slot;
        let frame = scene.motion_frame;
        scene.event(serde_json::json!({"event":"motion_sample","slot":slot,"frame":frame,
            "profile":pose.arithmetic_profile,"positions_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&pose.position_bits).expect("pose words"))),
            "matrices_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&pose.matrix_bits).expect("matrix words"))),"gameplay_verified":false}));
    }
    scene.displayed_motion_frame = scene.motion_frame;
    if !scene.motion_tour && (!scene.motion_paused || keys.just_pressed(KeyCode::KeyN)) {
        // Preview stepping is one explicit frame per rendered update. It is not
        // an authoritative KH2 clock. Loop only over the basic public window.
        let end = f32::from_bits(
            scene.motions[scene.motion_index]
                .motion
                .interpolated_header_words[33],
        ) * 2.;
        scene.motion_frame += 1;
        if scene.motion_frame as f32 >= end {
            scene.motion_frame = 0;
        }
    }
}
fn update(
    mut commands: Commands,
    mut scene: ResMut<Scene>,
    keys: Res<ButtonInput<KeyCode>>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<Real>>,
) {
    scene.frames += 1;
    if keys.just_pressed(KeyCode::KeyJ) {
        scene.direction = Quat::from_rotation_y(-0.15) * scene.direction;
    }
    if keys.just_pressed(KeyCode::KeyL) {
        scene.direction = Quat::from_rotation_y(0.15) * scene.direction;
    }
    if keys.just_pressed(KeyCode::Equal) {
        scene.zoom = (scene.zoom * 0.8).max(0.125);
    }
    if keys.just_pressed(KeyCode::Minus) {
        scene.zoom = (scene.zoom * 1.25).min(8.);
    }
    if let Ok((mut transform, mut projection)) = camera.single_mut() {
        *transform = Transform::from_translation(scene.camera_target + scene.direction * 0.35)
            .looking_at(scene.camera_target, Vec3::Y);
        if let Projection::Orthographic(p) = &mut *projection {
            p.scale = scene.zoom;
        }
    }
    let capture_limit = if scene.motion_tour {
        scene.motions.len() * 2
    } else {
        1
    };
    if !scene.pending
        && scene.capture_count < capture_limit
        && scene.frames >= 180
        && time.elapsed_secs_f64() >= 5.
        && scene.frames >= scene.last_capture_frame + 6
    {
        scene.pending = true;
        let path = if scene.capture_count == 0 {
            scene.capture.clone()
        } else {
            let stem = scene
                .capture
                .file_stem()
                .expect("capture stem")
                .to_string_lossy();
            scene
                .capture
                .with_file_name(format!("{stem}-{:02}.png", scene.capture_count))
        };
        let motion_slot = scene.motions.get(scene.motion_index).map(|m| m.slot);
        let motion_frame = scene.displayed_motion_frame;
        commands.spawn(Screenshot::primary_window()).observe(move |event:On<ScreenshotCaptured>,mut scene:ResMut<Scene>| {
            let image=event.image.clone().try_into_dynamic().expect("scene capture");
            let rgba=image.to_rgba8();
            let mut file=OpenOptions::new().write(true).create_new(true).open(&path).expect("new private capture");
            image.write_to(&mut file,image::ImageFormat::Png).expect("capture encode");file.sync_all().expect("capture flush");
            scene.captured=true;scene.pending=false;scene.capture_count+=1;scene.last_capture_frame=scene.frames;
            scene.event(serde_json::json!({"event":"capture","slot":motion_slot,"motion_frame":motion_frame,"width":rgba.width(),"height":rgba.height(),"rgba_sha256":format!("{:x}",Sha256::digest(rgba.as_raw()))}));
        });
    }
    if keys.just_pressed(KeyCode::Escape) || scene.limit.is_some_and(|n| scene.frames >= n) {
        let captured = scene.captured && scene.capture_count == capture_limit;
        scene.event(serde_json::json!({"event":"exit","captured":captured}));
        exit.write(if captured {
            AppExit::Success
        } else {
            AppExit::Error(std::num::NonZeroU8::new(1).unwrap())
        });
    }
}
fn run() -> Result<AppExit, Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() < 6 || !(args.len() - 6).is_multiple_of(2) {
        return Err("usage: room_scene <Steam-root> <manifest> <room-selection> <actor-selection> <new-log> <new-capture.png> [--frames N] [--motion selection] [--tour motion]".into());
    }
    let mut limit = None;
    let mut motion_path = None;
    let mut motion_tour = false;
    for option in args[6..].as_chunks::<2>().0 {
        match option[0].to_str() {
            Some("--frames") if limit.is_none() => {
                limit = Some(
                    option[1]
                        .to_str()
                        .ok_or("invalid frame limit")?
                        .parse::<u64>()?,
                )
            }
            Some("--motion") if motion_path.is_none() => motion_path = Some(&option[1]),
            Some("--tour") if !motion_tour && option[1].as_os_str() == "motion" => {
                motion_tour = true
            }
            _ => return Err("unknown/duplicate scene option".into()),
        }
    }
    if motion_tour && motion_path.is_none() {
        return Err("motion tour requires motion selection".into());
    }
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let log_path = source.output_path(&args[4])?;
    let capture = source.output_path(&args[5])?;
    if capture.exists() || log_path.exists() {
        return Err("scene outputs must be new".into());
    }
    if motion_tour {
        let stem = capture
            .file_stem()
            .ok_or("capture stem absent")?
            .to_string_lossy();
        for i in 1..16 {
            if capture
                .with_file_name(format!("{stem}-{i:02}.png"))
                .exists()
            {
                return Err("tour capture outputs must be new".into());
            }
        }
    }
    // Existing character admission performs a complete selected-source scan.
    let mut actors = model_inspection::read_selected_models(&source, &args[3])?;
    if actors.len() != 1 {
        return Err("scene requires exactly one character model".into());
    }
    let LoadedRoom {
        rooms,
        frame_points,
        inspection_ground,
        ..
    } = load_rooms(&source, &args[2])?;
    let actor = actors.remove(0);
    let motions = if let Some(path) = motion_path {
        let motions = motion::read_selected(&source, path)?;
        if motions.iter().any(|m| m.motion.raw.is_some()) {
            return Err("legacy FK room viewer requires interpolated motion; use the enemy gallery for raw matrices".into());
        }
        motions
    } else {
        Vec::new()
    };
    for selected in &motions {
        let end = f32::from_bits(selected.motion.interpolated_header_words[33]) * 2.;
        if !end.is_finite() || !(1. ..=65_536.).contains(&end) {
            return Err("invalid public preview frame window".into());
        }
        // Validate every frame in the declared public preview window before
        // launching, so undefined basic-FK cases cannot silently substitute.
        for frame in 0..end.ceil() as u32 {
            actor.skinning.sample_fk(&selected.motion, frame)?;
        }
    }
    let points: Vec<_> = rooms
        .iter()
        .flat_map(|r| &r.model.chunks)
        .flat_map(|c| &c.vertices)
        .flat_map(|v| v.position_bits)
        .collect();
    if points.is_empty() || points.iter().any(|&b| !f32::from_bits(b).is_finite()) {
        return Err("room coordinates empty or nonfinite".into());
    }
    if !rooms
        .iter()
        .any(|r| r.model.header_words[1] == 0 && !r.model.chunks.is_empty())
    {
        return Err("field model required".into());
    }
    let field_points = &frame_points;
    let low = field_points
        .iter()
        .fold(Vec3::splat(f32::MAX), |a, &b| a.min(b));
    let high = field_points
        .iter()
        .fold(Vec3::splat(f32::MIN), |a, &b| a.max(b));
    let radius = (high - low).max_element() * 0.5;
    if !((low + high) * 0.5).is_finite() || !radius.is_finite() || radius <= 0. {
        return Err("field bounds cannot form a finite inspection frame".into());
    }
    let mut log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(log_path)?;
    serde_json::to_writer(
        &mut log,
        &serde_json::json!({"event":"motion_admission","motions":motions.iter().map(|m| serde_json::json!({"slot":m.slot,"sha256":m.sha256})).collect::<Vec<_>>(),
        "profile":"basic public FK only","clock":"explicit frame per rendered update","missing":["IK","inverse curves","scale","constraints","expressions","root motion","triggers","installed cadence"],"gameplay_verified":false}),
    )?;
    writeln!(log)?;
    Ok(App::new()
        .insert_resource(Scene {
            rooms,
            frame_points,
            actor,
            log,
            capture,
            frames: 0,
            limit,
            pending: false,
            captured: false,
            direction: Vec3::new(0.8, 0.6, 1.).normalize(),
            zoom: 0.6,
            motions,
            motion_index: 0,
            motion_frame: 0,
            motion_paused: false,
            actor_offset: Vec3::ZERO,
            actor_scale: 1.,
            motion_tour,
            capture_count: 0,
            last_capture_frame: 0,
            displayed_motion_frame: 0,
            camera_target: Vec3::ZERO,
            inspection_ground,
        })
        .insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.09)))
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title:
                        "Room/FK preview | Space pause; N/B step; M slot; J/L orbit; +/- zoom; Esc"
                            .into(),
                    resolution: (1280, 720).into(),
                    ..default()
                }),
                ..default()
            }),
        )
        .add_systems(Startup, setup)
        .add_systems(Update, (animate, update).chain())
        .run())
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(AppExit::Success) => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(e) => {
            eprintln!("Room scene rejected: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
