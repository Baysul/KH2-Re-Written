//! Source-bound untextured 3D research. No simulation or installed rendering claim.
use bevy::{
    app::AppExit,
    asset::RenderAssetUsages,
    camera::ScalingMode,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    mesh::VertexAttributeValues,
    prelude::*,
    render::{
        render_resource::PrimitiveTopology,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    window::PrimaryWindow,
};
use package_reader::{
    model_inspection::{InspectionModel, read_selected_models},
    probe::ProbeSource,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};

#[derive(Component)]
struct ModelPart;
#[derive(Resource)]
struct Inspector {
    models: Vec<InspectionModel>,
    item: usize,
    pose: usize,
    part: Option<usize>,
    direction: Vec3,
    up: Vec3,
    zoom: f32,
    dirty: bool,
    mesh_handles: Vec<Handle<Mesh>>,
    material_handles: Vec<Handle<StandardMaterial>>,
    log: File,
    frames: u64,
    dwell: u64,
    settled_at: f64,
    exit_after: Option<u64>,
    tour: bool,
    step: usize,
    paths: Vec<PathBuf>,
    pending: bool,
    captured: usize,
}
impl Inspector {
    fn event(&mut self, value: serde_json::Value) {
        serde_json::to_writer(&mut self.log, &value).expect("private model log write");
        writeln!(self.log).expect("private model log newline");
        self.log.flush().expect("private model log flush");
    }
    fn tour_state(&mut self) {
        let mut remaining = self.step;
        for (item, model) in self.models.iter().enumerate() {
            let count = model.poses.len() * 4;
            if remaining < count {
                self.item = item;
                self.pose = remaining / 4;
                self.part = (remaining % 4 == 3).then(|| {
                    model
                        .parts
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| !p.indices.is_empty())
                        .max_by_key(|(_, p)| p.cross_part_corners)
                        .unwrap()
                        .0
                });
                (self.direction, self.up) = axes(remaining % 4);
                self.zoom = 1.0;
                self.dirty = true;
                self.dwell = 0;
                return;
            }
            remaining -= count;
        }
        panic!("bounded model tour state");
    }
}
fn axes(view: usize) -> (Vec3, Vec3) {
    match view {
        1 => (Vec3::NEG_Y, Vec3::Z),
        2 => (Vec3::X, Vec3::Y),
        _ => (Vec3::Z, Vec3::Y),
    }
}
fn color(part: usize) -> [u8; 3] {
    [53, 97, 29].map(|n| (64 + part * n % 128) as u8)
}
/// Presentation-only common fit: both profiles share one center/radius, so
/// selecting a profile never silently changes the inspection normalization.
fn frame(model: &InspectionModel) -> ([f64; 3], f64) {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for position in model.poses.iter().flat_map(|pose| &pose.position_bits) {
        for axis in 0..3 {
            let value = f32::from_bits(position[axis]) as f64;
            low[axis] = low[axis].min(value);
            high[axis] = high[axis].max(value);
        }
    }
    let center = std::array::from_fn(|axis| (low[axis] + high[axis]) * 0.5);
    let radius = low
        .iter()
        .zip(high)
        .map(|(&l, h)| (h - l) * 0.5)
        .fold(0.0f64, f64::max)
        .max(f64::MIN_POSITIVE);
    (center, radius)
}
fn run() -> Result<AppExit, Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = PathBuf::from(args.next().ok_or("selected install root required")?);
    let manifest = PathBuf::from(args.next().ok_or("source manifest required")?);
    let selection = PathBuf::from(args.next().ok_or("explicit model selection required")?);
    let output = PathBuf::from(args.next().ok_or("new private JSONL log required")?);
    let mut tour = false;
    let mut capture_dir = None;
    let mut exit_after = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--tour") => tour = true,
            Some("--capture-dir") => {
                capture_dir = Some(PathBuf::from(
                    args.next().ok_or("capture directory required")?,
                ))
            }
            Some("--frames") => {
                let value = args
                    .next()
                    .ok_or("frame count required")?
                    .to_string_lossy()
                    .parse::<u64>()?;
                if !(180..=100000).contains(&value) {
                    return Err("frame count outside 180..100000".into());
                }
                exit_after = Some(value);
            }
            _ => return Err("unknown model inspector option".into()),
        }
    }
    if tour && capture_dir.is_none() {
        return Err("tour requires an existing private capture directory".into());
    }
    let source = ProbeSource::bind(&root, &manifest)?;
    let output = source.output_path(&output)?;
    if let Some(directory) = &capture_dir {
        source.output_path(&directory.join("capture-admission.png"))?;
    }
    eprintln!("Fresh full selected-source scan; model research remains quarantined.");
    let models = read_selected_models(&source, &selection)?;
    let count = if tour {
        models.iter().map(|model| model.poses.len() * 4).sum()
    } else {
        usize::from(capture_dir.is_some())
    };
    let mut paths = Vec::new();
    if let Some(directory) = capture_dir {
        for step in 0..count {
            paths.push(source.output_path(&directory.join(format!("model-{step:03}.png")))?);
        }
    }
    let mut state = Inspector {
        models,
        item: 0,
        pose: 0,
        part: None,
        direction: Vec3::Z,
        up: Vec3::Y,
        zoom: 1.0,
        dirty: true,
        mesh_handles: Vec::new(),
        material_handles: Vec::new(),
        log: OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?,
        frames: 0,
        dwell: 0,
        settled_at: 0.0,
        exit_after,
        tour,
        step: 0,
        paths,
        pending: false,
        captured: 0,
    };
    state.event(serde_json::json!({"event":"admitted","qualification":"quarantined_model_research_only",
        "source_content_identity":source.content_identity(),"fresh_full_scan":true,"models":state.models.len(),
        "tour":tour,"expected_captures":count,"gameplay_enabled":false,
        "presentation":"common fit; orthographic; unlit part colors; double-sided opaque triangles; no source material, normal or renderer claim"}));
    if tour {
        state.tour_state();
    }
    let mut app = App::new();
    app.insert_resource(state)
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "QUARANTINED KH2FM model research".into(),
                resolution:
                    bevy::window::WindowResolution::new(900, 900).with_scale_factor_override(1.0),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (controls, refresh, finish).chain());
    Ok(app.run())
}
fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            near: 0.1,
            far: 10.0,
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 2.6,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
    ));
}
fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<Inspector>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        let complete = !state.tour || state.captured == state.paths.len();
        state.event(serde_json::json!({"event":"escape","tour_complete":complete}));
        exit.write(if complete {
            AppExit::Success
        } else {
            AppExit::Error(std::num::NonZeroU8::new(1).unwrap())
        });
    }
    if state.tour || state.pending {
        return;
    }
    for (key, delta) in [(KeyCode::ArrowRight, 1isize), (KeyCode::ArrowLeft, -1)] {
        if keys.just_pressed(key) {
            state.item =
                (state.item as isize + delta).rem_euclid(state.models.len() as isize) as usize;
            state.pose = 0;
            state.part = None;
            state.dirty = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyP) {
        state.pose = (state.pose + 1) % state.models[state.item].poses.len();
        state.dirty = true;
    }
    if keys.just_pressed(KeyCode::Space) {
        state.part = match state.part {
            None => Some(0),
            Some(p) if p + 1 < state.models[state.item].parts.len() => Some(p + 1),
            _ => None,
        };
        state.dirty = true;
    }
    for (key, view) in [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
    ] {
        if keys.just_pressed(key) {
            (state.direction, state.up) = axes(view);
            state.dirty = true;
        }
    }
    for (key, axis, angle) in [
        (KeyCode::KeyJ, Vec3::Y, -0.15),
        (KeyCode::KeyL, Vec3::Y, 0.15),
        (KeyCode::KeyI, Vec3::X, -0.15),
        (KeyCode::KeyK, Vec3::X, 0.15),
    ] {
        if keys.just_pressed(key) {
            let rotation = Quat::from_axis_angle(axis, angle);
            state.direction = rotation * state.direction;
            state.up = rotation * state.up;
            state.dirty = true;
        }
    }
    for (key, factor) in [(KeyCode::Equal, 0.8), (KeyCode::Minus, 1.25)] {
        if keys.just_pressed(key) {
            state.zoom = (state.zoom * factor).clamp(0.125, 8.0);
            state.dirty = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyF) {
        state.zoom = 1.0;
        state.dirty = true;
    }
}
fn refresh(
    mut commands: Commands,
    mut state: ResMut<Inspector>,
    assets: (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    parts: Query<Entity, With<ModelPart>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    time: Res<Time<Real>>,
) {
    if !state.dirty {
        return;
    }
    let (mut meshes, mut materials) = assets;
    for entity in &parts {
        commands.entity(entity).despawn();
    }
    for handle in state.mesh_handles.drain(..) {
        meshes.remove(handle.id());
    }
    for handle in state.material_handles.drain(..) {
        materials.remove(handle.id());
    }
    let model = &state.models[state.item];
    let pose = &model.poses[state.pose];
    let (center, radius) = frame(model);
    let normalized: Vec<[f32; 3]> = pose
        .position_bits
        .iter()
        .map(|position| {
            std::array::from_fn(|axis| {
                ((f32::from_bits(position[axis]) as f64 - center[axis]) / radius) as f32
            })
        })
        .collect();
    let mut rendered = Vec::new();
    let mut mesh_handles = Vec::new();
    let mut material_handles = Vec::new();
    for (ordinal, part) in model.parts.iter().enumerate() {
        if part.indices.is_empty() || state.part.is_some_and(|p| p != ordinal) {
            continue;
        }
        let positions: Vec<_> = part
            .indices
            .iter()
            .map(|&i| normalized[i as usize])
            .collect();
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        let Some(VertexAttributeValues::Float32x3(actual_positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("mesh position attribute");
        };
        let words: Vec<[u32; 3]> = actual_positions
            .iter()
            .map(|p| p.map(f32::to_bits))
            .collect();
        let rgb = color(ordinal);
        rendered.push(
            serde_json::json!({"part":ordinal,"source":part,"rgb":rgb,"mesh_position_bits":words}),
        );
        let mesh = meshes.add(mesh);
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb_u8(rgb[0], rgb[1], rgb[2]),
            unlit: true,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            ModelPart,
        ));
        mesh_handles.push(mesh);
        material_handles.push(material);
    }
    let metadata = serde_json::json!({"event":"view","step":state.step,"item":state.item,"index":model.index,
        "ordinal":model.ordinal,"bar_ordinal":model.bar_ordinal,"pose_profile":pose.profile,
        "arithmetic_profile":pose.arithmetic_profile,"runtime_verified":false,"excluded_linked_groups":model.excluded_linked_groups,
        "source_position_bits":pose.position_bits,"center":center,"radius":radius,"part":state.part,
        "direction":state.direction.to_array(),"up":state.up.to_array(),"zoom":state.zoom,"rendered":rendered});
    let (mut transform, mut projection) = cameras.single_mut().expect("research model camera");
    *transform =
        Transform::from_translation(state.direction * 4.0).looking_at(Vec3::ZERO, state.up);
    if let Projection::Orthographic(p) = &mut *projection {
        p.scale = state.zoom;
    }
    windows.single_mut().expect("research model window").title = format!(
        "QUARANTINED model {}/{} | {:?} | part {:?} | arrows; P pose; Space part; 1/2/3 axes; I/J/K/L orbit; +/-; F fit; Esc",
        state.item + 1,
        state.models.len(),
        pose.profile,
        state.part
    );
    state.mesh_handles = mesh_handles;
    state.material_handles = material_handles;
    state.event(metadata);
    state.dirty = false;
    state.dwell = 0;
    state.settled_at = time.elapsed_secs_f64();
}
fn finish(
    mut commands: Commands,
    mut state: ResMut<Inspector>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<Real>>,
) {
    state.frames += 1;
    state.dwell += 1;
    if !state.pending
        && !state.dirty
        && state.captured < state.paths.len()
        && state.dwell >= if state.step == 0 { 180 } else { 30 }
        && time.elapsed_secs_f64() - state.settled_at >= if state.step == 0 { 5.0 } else { 1.0 }
    {
        let path = state.paths[state.step].clone();
        state.pending = true;
        commands.spawn(Screenshot::primary_window()).observe(move |event: On<ScreenshotCaptured>, mut state: ResMut<Inspector>| {
            let dynamic = event.image.clone().try_into_dynamic().expect("model GPU screenshot conversion");
            let rgba = dynamic.to_rgba8();
            let mut file = OpenOptions::new().write(true).create_new(true).open(&path).expect("new private model capture");
            dynamic.write_to(&mut file, image::ImageFormat::Png).expect("private model capture encode");
            file.sync_all().expect("private model capture flush");
            let frames = state.frames; let step = state.step;
            state.event(serde_json::json!({"event":"captured","step":step,"frames":frames,
                "file":path.file_name().and_then(|name|name.to_str()),"width":rgba.width(),"height":rgba.height(),
                "pixels_sha256":format!("{:x}",Sha256::digest(rgba.as_raw()))}));
            state.pending = false; state.captured += 1;
            if state.tour && state.captured < state.paths.len() { state.step += 1; state.tour_state(); }
        });
    }
    if (state.tour && !state.pending && state.captured == state.paths.len())
        || state.exit_after.is_some_and(|limit| state.frames >= limit)
    {
        let complete = !state.pending && state.captured == state.paths.len();
        let frames = state.frames;
        let captures = state.captured;
        state.event(serde_json::json!({"event":"exit","frames":frames,"captures":captures,"complete":complete}));
        exit.write(if complete {
            AppExit::Success
        } else {
            AppExit::Error(std::num::NonZeroU8::new(1).unwrap())
        });
    }
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(AppExit::Success) => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(error) => {
            eprintln!("Model research rejected: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use package_reader::model_pose::{ModelPose, PoseProfile};
    #[test]
    fn common_frame_preserves_profile_displacements_and_handles_extreme_finite_coordinates() {
        let poses = [PoseProfile::Legacy, PoseProfile::Modern]
            .into_iter()
            .enumerate()
            .map(|(i, profile)| ModelPose {
                profile,
                arithmetic_profile: "authored",
                matrix_bits: Vec::new(),
                position_bits: vec![
                    [if i == 0 { -f32::MAX } else { f32::MAX }; 3].map(f32::to_bits),
                ],
                runtime_verified: false,
            })
            .collect();
        let model = InspectionModel {
            index: String::new(),
            ordinal: 0,
            bar_ordinal: 0,
            excluded_linked_groups: 0,
            parts: Vec::new(),
            poses,
        };
        let (center, radius) = frame(&model);
        assert_eq!(center, [0.0; 3]);
        assert_eq!(radius, f32::MAX as f64);
        assert_eq!((f32::MAX as f64 / radius) as f32, 1.0);
        assert_eq!((-f32::MAX as f64 / radius) as f32, -1.0);
    }
}
