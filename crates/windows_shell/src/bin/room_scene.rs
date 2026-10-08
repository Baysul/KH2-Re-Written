//! First source-bound room/character scene. Camera inspection, no game rules.
use bevy::{
    app::AppExit,
    asset::RenderAssetUsages,
    camera::ScalingMode,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::ImageSampler,
    prelude::*,
    render::{
        render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
        view::{
            NoIndirectDrawing,
            screenshot::{Screenshot, ScreenshotCaptured},
        },
    },
};
use package_reader::{
    bar,
    model_inspection::{self, InspectionModel},
    model_inspection_textures::TextureMode,
    model_texture_pixels::{self, DecodedTexture},
    original,
    probe::ProbeSource,
    room_collision,
    room_geometry::{self, RoomModel},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
};

struct Room {
    model: RoomModel,
    textures: Vec<DecodedTexture>,
}
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
}
impl Scene {
    fn event(&mut self, value: serde_json::Value) {
        serde_json::to_writer(&mut self.log, &value).expect("scene log write");
        writeln!(self.log).expect("scene log newline");
        self.log.flush().expect("scene log flush");
    }
}
fn load_rooms(
    source: &ProbeSource,
    selection: &PathBuf,
) -> Result<(Vec<Room>, Vec<Vec3>), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(selection)?
        .take(1048577)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1048576 {
        return Err("room selection too large".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let items = value["selections"]
        .as_array()
        .ok_or("room selection absent")?;
    if value["schema_version"] != 1
        || value["source_content_identity"] != source.content_identity()
        || items.len() != 1
    {
        return Err("one source-bound room selection required".into());
    }
    let item = &items[0];
    let mut index = source.open_index(item["index"].as_str().ok_or("room index absent")?)?;
    let ordinal = usize::try_from(item["ordinal"].as_u64().ok_or("room ordinal absent")?)?;
    let entry = index
        .inventory
        .entries
        .get(ordinal)
        .ok_or("room entry absent")?;
    if serde_json::to_value(entry.name_hash)? != item["name_hash"] {
        return Err("room identity mismatch".into());
    }
    let archive = original::read_original(&mut index.reader, entry)?;
    let table = bar::read_bar(&archive.bytes)?;
    let mut rooms = Vec::new();
    let mut frame_points = Vec::new();
    for entry in table
        .entries
        .iter()
        .filter(|e| e.entry_type == 6 && e.offset > 0 && e.size > 0)
    {
        let collision = room_collision::read(
            &archive.bytes[entry.offset as usize..entry.offset as usize + entry.size as usize],
        )?;
        // Pinned MapStudio CollisionModel.GenerateVertex display conversion.
        frame_points.extend(collision.vertex_bits.iter().map(|p| {
            let [x, y, z, _] = p.map(f32::from_bits);
            Vec3::new(x, -y, -z)
        }));
    }
    let mut rgba_bytes = 0;
    for entry in table
        .entries
        .iter()
        .filter(|e| e.entry_type == 4 && e.offset > 0 && e.size > 0)
    {
        let payload =
            &archive.bytes[entry.offset as usize..entry.offset as usize + entry.size as usize];
        if payload.get(0x90..0x94) != Some(2u32.to_le_bytes().as_slice()) {
            continue;
        }
        let model = room_geometry::read(payload)?;
        let textures: Vec<_> = table
            .entries
            .iter()
            .filter(|e| {
                e.entry_type == 7 && e.name_bytes == entry.name_bytes && e.offset > 0 && e.size > 0
            })
            .collect();
        if textures.len() != 1 {
            return Err("room requires unique same-name texture block".into());
        }
        let entry = textures[0];
        let textures = model_texture_pixels::decode_images(
            &archive.bytes[entry.offset as usize..entry.offset as usize + entry.size as usize],
        )?;
        rgba_bytes += textures.iter().map(|t| t.pixels_rgba.len()).sum::<usize>();
        if rgba_bytes > 128 * 1024 * 1024 {
            return Err("room textures exceed cumulative limit".into());
        }
        rooms.push(Room { model, textures });
    }
    index.verify_unchanged()?;
    if rooms.is_empty() {
        return Err("no decoded room model".into());
    }
    if frame_points.is_empty() || frame_points.iter().any(|p| !p.is_finite()) {
        return Err("finite collision area required for scene framing".into());
    }
    Ok((rooms, frame_points))
}
fn upload(texture: &DecodedTexture, images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new(
        Extent3d {
            width: texture.width,
            height: texture.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texture.pixels_rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    images.add(image)
}
fn mesh(positions: Vec<[f32; 3]>, uvs: Vec<[f32; 2]>, colors: Vec<[f32; 4]>) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
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
    let placement = Vec3::new(center.x, low.y, center.z)
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
                let mut image = Image::new(
                    Extent3d {
                        width: level.width,
                        height: level.height,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    level.pixels_rgba.clone(),
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                );
                image.sampler = ImageSampler::nearest();
                texture = Some(images.add(image));
            }
        }
        commands.spawn((
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
        Transform::from_translation(scene.direction * 0.6).looking_at(Vec3::ZERO, Vec3::Y),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        NoIndirectDrawing,
    ));
    scene.event(serde_json::json!({"event":"scene","room_triangles":room_triangles,"actor_triangles":actor_triangles,
        "room_center":center.to_array(),"radius":radius,"actor_translation":placement.to_array(),"placement_is_gameplay_spawn":false,
        "textures":texture_observations,"materials":"original indexed, nearest, decoded opaque/blend split, unlit inspection",
        "gameplay_verified":false}));
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
        *transform =
            Transform::from_translation(scene.direction * 0.6).looking_at(Vec3::ZERO, Vec3::Y);
        if let Projection::Orthographic(p) = &mut *projection {
            p.scale = scene.zoom;
        }
    }
    if !scene.pending && !scene.captured && scene.frames >= 180 && time.elapsed_secs_f64() >= 5. {
        scene.pending = true;
        let path = scene.capture.clone();
        commands.spawn(Screenshot::primary_window()).observe(move |event:On<ScreenshotCaptured>,mut scene:ResMut<Scene>| {
            let image=event.image.clone().try_into_dynamic().expect("scene capture");
            let rgba=image.to_rgba8();
            let mut file=OpenOptions::new().write(true).create_new(true).open(&path).expect("new private capture");
            image.write_to(&mut file,image::ImageFormat::Png).expect("capture encode");file.sync_all().expect("capture flush");
            scene.captured=true;scene.pending=false;
            scene.event(serde_json::json!({"event":"capture","width":rgba.width(),"height":rgba.height(),"rgba_sha256":format!("{:x}",Sha256::digest(rgba.as_raw()))}));
        });
    }
    if keys.just_pressed(KeyCode::Escape) || scene.limit.is_some_and(|n| scene.frames >= n) {
        let captured = scene.captured;
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
    if args.len() != 6 && !(args.len() == 8 && args[6].as_os_str() == "--frames") {
        return Err("usage: room_scene <Steam-root> <manifest> <room-selection> <actor-selection> <new-log> <new-capture.png> [--frames N]".into());
    }
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let log_path = source.output_path(&args[4])?;
    let capture = source.output_path(&args[5])?;
    if capture.exists() || log_path.exists() {
        return Err("scene outputs must be new".into());
    }
    // Existing character admission performs a complete selected-source scan.
    let mut actors = model_inspection::read_selected_models(&source, &args[3])?;
    if actors.len() != 1 {
        return Err("scene requires exactly one character model".into());
    }
    let (rooms, frame_points) = load_rooms(&source, &args[2])?;
    let actor = actors.remove(0);
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
    let limit = if args.len() == 8 {
        Some(
            args[7]
                .to_str()
                .ok_or("invalid frame limit")?
                .parse::<u64>()?,
        )
    } else {
        None
    };
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(log_path)?;
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
            zoom: 1.,
        })
        .insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.09)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Room and character preview | J/L orbit; +/- zoom; Esc exit".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, update)
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
