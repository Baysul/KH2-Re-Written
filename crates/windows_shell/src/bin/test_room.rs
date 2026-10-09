//! Playable source-bound test room: Sora walks/runs, jumps/lands and attacks
//! with motions read from the selected Steam installation.
//!
//! Every launch rehashes the complete selected source once, then decodes the
//! room, Sora's model/motion set and his PRTY/SSTM/PTYA records from it. The
//! controller (`player_sim`) is a test-room profile: data-backed where a
//! public lead exists, explicit choices elsewhere. It is not a measured
//! reproduction of the installed game's movement or combat rules.
use bevy::{
    app::AppExit,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::view::{
        NoIndirectDrawing,
        screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use package_reader::{
    bar, model_inspection, model_inspection_textures::TextureMode, motion, objentry, original,
    preferences, probe::ProbeSource, ptya, sklt,
};
use player_sim::{Action, AttackStep, Clip, Clips, Input, Params, Player, TriangleWorld, World};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use windows_shell::room::{CollisionTriangle, LoadedRoom, load_rooms, mesh, rgba_image, upload};

#[derive(Deserialize)]
struct Locator {
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
}
#[derive(Deserialize)]
struct Config {
    schema_version: u32,
    source_content_identity: String,
    /// Paths relative to this configuration file.
    room_selection: PathBuf,
    actor_selection: PathBuf,
    motion_set: Locator,
    battle_mode: bool,
    has_weapon: bool,
    battle: Locator,
    ptya_pointer: usize,
    ground_combo_ids: Vec<u8>,
    air_combo_ids: Vec<u8>,
    system: Locator,
    prty_pointer: usize,
    /// 00objentry locator and the object model name whose WeaponJoint byte
    /// names the actor joint carrying the weapon. Weapon-type entries carry
    /// a hand-like joint while player entries carry 1; that reading is a
    /// test-room inference from installed records, not observed routing.
    objentry: Locator,
    object_model_name: String,
    weapon_selection: PathBuf,
    /// The weapon's own motion set (W_EX010.mset); its clips share Sora's
    /// slot numbers and pose the weapon in the hand joint's space.
    #[serde(default)]
    weapon_motion_set: Option<Locator>,
    /// Character id (public dictionary: 1 = Sora) whose SKLT primary bone
    /// in the system archive carries the weapon. Without it the objentry
    /// byte above is used.
    #[serde(default)]
    sklt_character_id: Option<u32>,
    /// Optional spawn (x, probe height, z) in scene coordinates.
    #[serde(default)]
    spawn: Option<[f32; 3]>,
}

/// Public MotionSet names for the locomotion IDs used here.
const LOCOMOTION: [(&str, u16); 6] = [
    ("IDLE", 0),
    ("WALK", 1),
    ("RUN", 2),
    ("JUMP", 3),
    ("FALL", 4),
    ("LAND", 5),
];
/// Test-room choices, recorded in the startup log.
const GRAVITY: f32 = 0.8;
const STEP_HEIGHT: f32 = 24.;
const WALK_THRESHOLD: f32 = 0.6;
const BODY_RADIUS: f32 = 30.;
const BODY_HEIGHT: f32 = 140.;
const FLOOR_MIN_NORMAL_Y: f32 = 0.5;
const BLEND_TICKS: u32 = 6;
/// Test-room choice: put the weapon away after 6 s without attacking.
const DISARM_TICKS: u32 = 360;
/// Presentation mirror. Raw source coordinates render with the objentry
/// weapon joint (25) as Sora's left hand; mirroring X puts the Kingdom Key
/// in his right hand. Inferred handedness, not an observed render matrix.
const MIRROR: Vec3 = Vec3::new(-1., 1., 1.);
const PARTY_STAND: u32 = 1 << 0;
const PARTY_COLLIDE: u32 = 1 << 4;

/// Precomputed skinned positions for every frame of one clip.
struct ClipPoses {
    frames: Vec<Vec<Vec3>>,
    /// Model-space matrix of the objentry weapon joint for each frame.
    weapon: Vec<Mat4>,
    /// Weapon vertices in hand-joint space per frame, from the weapon's own
    /// motion at the same slot and key time (empty when it has none).
    weapon_frames: Vec<Vec<Vec3>>,
    /// Root-position curve translation for each frame (model space).
    root: Vec<Vec3>,
}

#[derive(Resource)]
struct Game {
    params: Params,
    clips: Clips,
    world: TriangleWorld,
    player: Player,
    poses: HashMap<(Action, bool), usize>,
    clip_poses: Vec<ClipPoses>,
    part_indices: Vec<Vec<u32>>,
    tick: u64,
    log: Option<File>,
    limit: Option<u64>,
    autoplay: Option<PathBuf>,
    captures_taken: usize,
    capture_pending: bool,
    gamepad: bool,
    jump_queued: bool,
    attack_queued: bool,
    display: Vec<Vec3>,
    /// Pose, Keyblade transform and start tick of the active crossfade.
    blend_from: Option<(Vec<Vec3>, Transform, u64)>,
    display_hand: Transform,
    weapon_indices: Vec<Vec<u32>>,
    weapon_display: Vec<Vec3>,
    shown_action: Option<(Action, u64)>,
    action_started: u64,
    exit_code: Option<u8>,
    show_collision: bool,
    rendered_frames: u64,
    autoplay_failed: bool,
}

#[derive(Resource)]
struct CameraRig {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
}

#[derive(Component)]
struct SoraRoot;
#[derive(Component)]
struct SoraPart(usize);
#[derive(Component)]
struct WeaponPart(usize);
#[derive(Component)]
struct WeaponRoot;
#[derive(Component)]
struct CollisionOverlay;

impl Game {
    fn event(&mut self, value: serde_json::Value) {
        if let Some(log) = &mut self.log {
            serde_json::to_writer(&mut *log, &value).expect("test room log write");
            writeln!(log).expect("test room log newline");
            log.flush().expect("test room log flush");
        }
    }
}

fn read_limited(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(1_048_577).read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        return Err("selection exceeds 1 MiB".into());
    }
    Ok(bytes)
}

fn read_archive(
    source: &ProbeSource,
    locator: &Locator,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut checked = source.open_index(&locator.index)?;
    let entry = checked
        .inventory
        .entries
        .get(locator.ordinal)
        .filter(|e| e.name_hash == locator.name_hash)
        .ok_or("archive source entry mismatch")?;
    let original = original::read_original(&mut checked.reader, entry)?;
    checked.verify_unchanged()?;
    Ok(original.bytes)
}

fn bar_entry<'a>(bytes: &'a [u8], name: &[u8; 4]) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let table = bar::read_bar(bytes)?;
    let matches: Vec<_> = table
        .entries
        .iter()
        .filter(|e| &e.name_bytes == name && e.offset > 0 && e.size > 0)
        .collect();
    let [entry] = matches.as_slice() else {
        return Err(format!("expected one {} entry", String::from_utf8_lossy(name)).into());
    };
    bytes
        .get(entry.offset as usize..entry.offset as usize + entry.size as usize)
        .ok_or_else(|| "BAR entry exceeds archive".into())
}

fn word_f32(bytes: &[u8], at: usize) -> Result<f32, Box<dyn std::error::Error>> {
    let value = f32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or("truncated parameter word")?
            .try_into()?,
    );
    if !value.is_finite() {
        return Err("nonfinite parameter word".into());
    }
    Ok(value)
}

/// Clip timing and honoured range triggers from one decoded motion.
/// Clip timing relative to FrameStart, plus the key time of FrameStart.
fn clip_for(selected: &motion::SelectedMotion) -> Result<(Clip, u32), Box<dyn std::error::Error>> {
    let (frame_start, frame_end, fps, frame_return) = selected.motion.frame_window()?;
    if !(1. ..=60.).contains(&fps)
        || !(0. ..frame_end).contains(&frame_start)
        || !(frame_start..=frame_end).contains(&frame_return)
    {
        return Err("motion frame window outside test-room profile".into());
    }
    let scale = 60. / fps;
    let start = (frame_start * scale).round() as u32;
    let end = (frame_end * scale).round() as u32 - start;
    let mut clip = Clip {
        end,
        loop_start: (frame_return * scale).round() as u32 - start,
        ..Clip::default()
    };
    for record in selected.triggers.iter().flat_map(|t| &t.records.ranges) {
        let start = record.start_frame.max(0) as u32;
        let stop = if record.end_frame < 0 {
            end
        } else {
            record.end_frame as u32
        };
        match record.trigger {
            11 => clip.combo_windows.push((start, stop)),
            2 => clip.blend_to_idle = Some(clip.blend_to_idle.map_or(start, |b: u32| b.min(start))),
            3 => clip.no_gravity.push((start, stop)),
            _ => {}
        }
    }
    Ok((clip, start))
}

struct Loaded {
    room: LoadedRoom,
    actor: model_inspection::InspectionModel,
    weapon: model_inspection::InspectionModel,
    params: Params,
    clips: Clips,
    actions: Vec<((Action, bool), usize)>,
    clip_poses: Vec<ClipPoses>,
    spawn: Vec3,
    world: TriangleWorld,
    summary: serde_json::Value,
}

/// Walkable collision levels (8-unit height bins) by XZ area, largest first,
/// each with the centroid of its largest triangle.
fn floor_levels(triangles: &[CollisionTriangle]) -> Vec<(f32, [f32; 3])> {
    let mut bins: HashMap<i32, (f32, f32, [f32; 3])> = HashMap::new();
    for [a, b, c] in triangles
        .iter()
        .filter(|t| t.surface_flags & PARTY_STAND != 0)
        .map(|t| &t.corners)
    {
        let normal = (*b - *a).cross(*c - *a);
        if normal.length() <= 1e-6 || normal.normalize().y.abs() < FLOOR_MIN_NORMAL_Y {
            continue;
        }
        let area = 0.5 * normal.y.abs();
        let centroid = (*a + *b + *c) / 3.;
        let bin = bins
            .entry((centroid.y / 8.).round() as i32)
            .or_insert((0., 0., [0.; 3]));
        bin.0 += area;
        if area > bin.1 {
            bin.1 = area;
            bin.2 = (centroid + Vec3::Y).to_array();
        }
    }
    let mut levels: Vec<_> = bins.into_values().map(|(a, _, p)| (a, p)).collect();
    levels.sort_by(|x, y| y.0.total_cmp(&x.0));
    levels
}

fn load(
    source: &ProbeSource,
    config_path: &Path,
    basic_fk: bool,
) -> Result<Loaded, Box<dyn std::error::Error>> {
    let config: Config = serde_json::from_slice(&read_limited(config_path)?)?;
    if config.schema_version != 1
        || config.source_content_identity != source.content_identity()
        || config.ground_combo_ids.is_empty()
        || config.ground_combo_ids.len() > 8
        || config.air_combo_ids.len() > 8
    {
        return Err("test room configuration identity/schema invalid".into());
    }
    let base = config_path.parent().ok_or("configuration directory")?;
    // One complete selected-source scan admits this session.
    source.verify_full_source()?;
    let mut actors =
        model_inspection::read_selected_models(source, &base.join(&config.actor_selection))?;
    if actors.len() != 1 {
        return Err("test room requires exactly one character model".into());
    }
    let actor = actors.remove(0);
    let mut weapons =
        model_inspection::read_selected_models(source, &base.join(&config.weapon_selection))?;
    if weapons.len() != 1 {
        return Err("test room requires exactly one weapon model".into());
    }
    let weapon = weapons.remove(0);
    let objects = objentry::read(&read_archive(source, &config.objentry)?)?;
    let object = objects
        .iter()
        .find(|o| o.model_name == config.object_model_name.as_bytes())
        .ok_or("weapon object entry absent")?;
    let objentry_joint = object.weapon_joint as usize;
    let object_candidates: Vec<_> = objects
        .iter()
        .filter(|o| o.model_name.starts_with(b"P_EX100") || o.model_name.starts_with(b"W_EX010"))
        .map(|o| serde_json::json!({"id":o.object_id,"type":o.object_type,"joint":o.weapon_joint,
            "model":String::from_utf8_lossy(&o.model_name),"motion":String::from_utf8_lossy(&o.animation_name)}))
        .collect();
    // PRTY walk/run/jump/turn and SSTM fall max (public labels).
    let system = read_archive(source, &config.system)?;
    let weapon_joint = match config.sklt_character_id {
        Some(id) => {
            let entries = sklt::read(bar_entry(&system, b"sklt")?)?;
            let entry = entries
                .iter()
                .find(|e| e.character_id == id)
                .ok_or("SKLT entry for the character absent")?;
            usize::try_from(entry.primary_bone).map_err(|_| "SKLT primary bone negative")?
        }
        None => objentry_joint,
    };
    if weapon_joint >= actor.skinning.bones.len() {
        return Err("weapon joint outside the actor skeleton".into());
    }
    let room = load_rooms(source, &base.join(&config.room_selection))?;

    let pref = bar_entry(&system, b"pref")?;
    let prty_bytes = bar_entry(pref, b"prty")?;
    let prty = preferences::read_prty(prty_bytes)?;
    let offset = *prty
        .offsets
        .get(config.prty_pointer)
        .ok_or("PRTY pointer absent")?;
    let record = prty
        .records
        .iter()
        .find(|r| r.offset == offset && !r.overlaps_table)
        .ok_or("PRTY record absent or overlaps table")?;
    let prty_value = |i: usize| f32::from_bits(record.words[i]);
    let sstm = bar_entry(pref, b"sstm")?;
    let sstm_offset = u32::from_le_bytes(sstm.get(4..8).ok_or("SSTM pointer")?.try_into()?);
    let fall_max = word_f32(sstm, sstm_offset as usize + 20)?;
    let params = Params {
        walk_speed: prty_value(0),
        run_speed: prty_value(1),
        jump_height: prty_value(2),
        turn_speed: prty_value(3),
        fall_max,
        gravity: GRAVITY,
        step_height: STEP_HEIGHT,
        walk_threshold: WALK_THRESHOLD,
        radius: BODY_RADIUS,
        height: BODY_HEIGHT,
        disarm_ticks: DISARM_TICKS,
    };
    params.validate()?;

    // PTYA attack entries chosen by ID for the grounded and aerial combos.
    let battle = read_archive(source, &config.battle)?;
    let ptya_bytes = bar_entry(&battle, b"ptya")?;
    let table = ptya::read_table(ptya_bytes)?;
    let set = ptya::read_set(ptya_bytes, &table, config.ptya_pointer)?;
    let pick = |id: u8| {
        set.iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or_else(|| format!("PTYA entry {id} absent"))
    };
    let ground: Vec<_> = config
        .ground_combo_ids
        .iter()
        .map(|&id| pick(id))
        .collect::<Result<_, _>>()?;
    let air: Vec<_> = config
        .air_combo_ids
        .iter()
        .map(|&id| pick(id))
        .collect::<Result<_, _>>()?;

    let mut ids: Vec<u16> = LOCOMOTION.iter().map(|&(_, id)| id).collect();
    ids.extend(ground.iter().chain(&air).map(|e| e.motion_id));
    let selection = motion::MotionSetSelection {
        index: config.motion_set.index.clone(),
        ordinal: config.motion_set.ordinal,
        name_hash: config.motion_set.name_hash,
        battle: config.battle_mode,
        has_weapon: config.has_weapon,
    };
    let mut motions = motion::read_motion_ids(source, &selection, &ids)?;
    // Field-mode (weapon put away) idle/walk/run through the same public
    // chooser with battle=false, has_weapon=false.
    let field_selection = motion::MotionSetSelection {
        battle: false,
        has_weapon: false,
        ..selection
    };
    motions.extend(motion::read_motion_ids(
        source,
        &field_selection,
        &[0, 1, 2],
    )?);
    ids.extend([0, 1, 2]);
    let battle_count = ids.len() - 3;
    let mut weapon_motions: HashMap<usize, motion::SelectedMotion> = HashMap::new();
    if let Some(set) = &config.weapon_motion_set {
        let weapon_selection = motion::MotionSetSelection {
            index: set.index.clone(),
            ordinal: set.ordinal,
            name_hash: set.name_hash,
            battle: config.battle_mode,
            has_weapon: config.has_weapon,
        };
        let found = motion::read_motion_ids(source, &weapon_selection, &ids[..battle_count])?;
        for (selected, own) in found.into_iter().zip(&motions) {
            weapon_motions.insert(own.slot, selected);
        }
    }

    // Precompute every frame of every clip; undefined basic-FK frames reject
    // the session instead of substituting a pose.
    let mut clip_poses = Vec::new();
    let mut eval_totals = [0usize; 4];
    let mut clip_list = Vec::new();
    let mut by_slot: HashMap<usize, usize> = HashMap::new();
    let mut slots = Vec::new();
    for selected in &motions {
        let (clip, key_start) = clip_for(selected)?;
        let index = match by_slot.get(&selected.slot) {
            Some(&i) => i,
            None => {
                let mut frames = Vec::with_capacity(clip.end as usize);
                let mut hand = Vec::with_capacity(clip.end as usize);
                let mut root = Vec::with_capacity(clip.end as usize);
                let mut weapon_frames = Vec::new();
                for offset in 0..=clip.end {
                    let frame = key_start + offset;
                    if let Some(own) = weapon_motions.get(&selected.slot) {
                        let (pose, _) = weapon
                            .skinning
                            .sample_evaluated(&own.motion, frame as f32)?;
                        weapon_frames.push(
                            pose.position_bits
                                .iter()
                                .map(|p| Vec3::from_array(p.map(f32::from_bits)))
                                .collect(),
                        );
                    }
                    let pose = if basic_fk {
                        root.push(Vec3::ZERO);
                        actor.skinning.sample_fk(&selected.motion, frame)?
                    } else {
                        let (mut pose, evaluated) = actor
                            .skinning
                            .sample_evaluated(&selected.motion, frame as f32)?;
                        // Root motion moves the player; the rendered pose
                        // stays at the player origin.
                        let r = evaluated.root_translation;
                        root.push(Vec3::from_array(r));
                        for p in &mut pose.position_bits {
                            *p = std::array::from_fn(|i| (f32::from_bits(p[i]) - r[i]).to_bits());
                        }
                        for m in &mut pose.matrix_bits {
                            for i in 0..3 {
                                m[12 + i] = (f32::from_bits(m[12 + i]) - r[i]).to_bits();
                            }
                        }
                        let s = evaluated.stats;
                        eval_totals[0] += s.constraints_applied;
                        eval_totals[1] += s.ik_chains_solved;
                        eval_totals[2] += s.expressions_applied;
                        eval_totals[3] += s.unsupported_constraints;
                        pose
                    };
                    // Row-vector matrices: the row-major words are Bevy's
                    // column-major layout of the same transform.
                    hand.push(Mat4::from_cols_array(
                        &pose.matrix_bits[weapon_joint].map(f32::from_bits),
                    ));
                    frames.push(
                        pose.position_bits
                            .iter()
                            .map(|p| Vec3::from_array(p.map(f32::from_bits)))
                            .collect(),
                    );
                }
                clip_poses.push(ClipPoses {
                    frames,
                    weapon: hand,
                    weapon_frames,
                    root,
                });
                by_slot.insert(selected.slot, clip_poses.len() - 1);
                clip_poses.len() - 1
            }
        };
        slots.push(serde_json::json!({"motion_id":ids[clip_list.len()],"slot":selected.slot,"sha256":selected.sha256,
            "end_ticks":clip.end,"loop_start":clip.loop_start,"combo_windows":clip.combo_windows,
            "blend_to_idle":clip.blend_to_idle,"no_gravity":clip.no_gravity}));
        clip_list.push((clip, index));
    }
    let step = |entry: &ptya::PtyaEntry, (clip, index): &(Clip, usize)| AttackStep {
        clip: clip.clone(),
        hop: f32::from_bits(entry.float_bits[0]),
        root: clip_poses[*index]
            .root
            .iter()
            .map(|r| r.to_array())
            .collect(),
    };
    let n = LOCOMOTION.len();
    let field = n + ground.len() + air.len();
    let clips = Clips {
        idle: clip_list[0].0.clone(),
        walk: clip_list[1].0.clone(),
        run: clip_list[2].0.clone(),
        field_idle: clip_list[field].0.clone(),
        field_walk: clip_list[field + 1].0.clone(),
        field_run: clip_list[field + 2].0.clone(),
        jump: clip_list[3].0.clone(),
        fall: clip_list[4].0.clone(),
        land: clip_list[5].0.clone(),
        ground_combo: ground
            .iter()
            .zip(&clip_list[n..])
            .map(|(e, c)| step(e, c))
            .collect(),
        air_combo: air
            .iter()
            .zip(&clip_list[n + ground.len()..])
            .map(|(e, c)| step(e, c))
            .collect(),
    };
    // Pose clip per (action, weapon drawn).
    let mut actions = vec![
        ((Action::Idle, true), clip_list[0].1),
        ((Action::Walk, true), clip_list[1].1),
        ((Action::Run, true), clip_list[2].1),
        ((Action::Idle, false), clip_list[field].1),
        ((Action::Walk, false), clip_list[field + 1].1),
        ((Action::Run, false), clip_list[field + 2].1),
    ];
    let mut both = vec![
        (Action::Jump, clip_list[3].1),
        (Action::Fall, clip_list[4].1),
        (Action::Land, clip_list[5].1),
    ];
    for i in 0..ground.len() {
        both.push((Action::Attack(i), clip_list[n + i].1));
    }
    for i in 0..air.len() {
        both.push((Action::AirAttack(i), clip_list[n + ground.len() + i].1));
    }
    for (action, clip) in both {
        actions.push(((action, true), clip));
        actions.push(((action, false), clip));
    }

    // Floors must carry partyStand and walls partyCollide (public labels).
    let world = TriangleWorld::new(
        room.collision_triangles
            .iter()
            .filter(|t| t.surface_flags & (PARTY_STAND | PARTY_COLLIDE) != 0)
            .map(|t| {
                (
                    t.corners.map(|v| v.to_array()),
                    t.surface_flags & PARTY_STAND != 0,
                    t.surface_flags & PARTY_COLLIDE != 0,
                )
            }),
        FLOOR_MIN_NORMAL_Y,
    );
    let mut flag_counts: HashMap<u32, usize> = HashMap::new();
    for t in &room.collision_triangles {
        *flag_counts.entry(t.surface_flags).or_default() += 1;
    }
    let flag_counts: HashMap<String, usize> = flag_counts
        .into_iter()
        .map(|(f, n)| (format!("{f:#x}"), n))
        .collect();
    let low = room
        .frame_points
        .iter()
        .fold(Vec3::splat(f32::MAX), |a, &b| a.min(b));
    let high = room
        .frame_points
        .iter()
        .fold(Vec3::splat(f32::MIN), |a, &b| a.max(b));
    let levels = floor_levels(&room.collision_triangles);
    let weapon_points: Vec<Vec3> = weapon.poses[0]
        .position_bits
        .iter()
        .map(|p| Vec3::from_array(p.map(f32::from_bits)))
        .collect();
    let weapon_bounds = [
        weapon_points
            .iter()
            .fold(Vec3::splat(f32::MAX), |a, &b| a.min(b))
            .to_array(),
        weapon_points
            .iter()
            .fold(Vec3::splat(f32::MIN), |a, &b| a.max(b))
            .to_array(),
    ];
    let idle_hand = clip_poses[clip_list[0].1].weapon[0].to_cols_array();
    let weapon_root = weapon.poses[0].matrix_bits[0].map(f32::from_bits);
    let [sx, sy, sz] = match config.spawn {
        Some(spawn) => spawn,
        // Default: the walkable level with the most floor area.
        None => levels.first().map(|l| l.1).ok_or("no walkable floor")?,
    };
    let ground_height = world
        .floor(sx, sz, sy)
        .ok_or("no walkable collision floor below the spawn point")?;
    let spawn = Vec3::new(sx, ground_height, sz);
    let (floors, walls) = world.counts();
    let summary = serde_json::json!({"event":"admission","source_qualification":"quarantined_unqualified",
        "params":{"walk_speed":params.walk_speed,"run_speed":params.run_speed,"jump_height":params.jump_height,
            "turn_speed":params.turn_speed,"fall_max":params.fall_max},
        "test_room_choices":{"gravity":GRAVITY,"step_height":STEP_HEIGHT,"walk_threshold":WALK_THRESHOLD,
            "body_radius":BODY_RADIUS,"body_height":BODY_HEIGHT,"floor_min_normal_y":FLOOR_MIN_NORMAL_Y,
            "blend_ticks":BLEND_TICKS,"clock":"fixed 60 Hz, one motion tick per update"},
        "ptya":{"ground":ground.iter().map(|e| serde_json::json!({"id":e.id,"kind":e.kind,"flag":e.flag,"motion_id":e.motion_id})).collect::<Vec<_>>(),
            "air":air.iter().map(|e| serde_json::json!({"id":e.id,"kind":e.kind,"flag":e.flag,"motion_id":e.motion_id})).collect::<Vec<_>>()},
        "motions":slots,"collision":{"floors":floors,"walls":walls,"surface_flags":flag_counts,"bounds":[low.to_array(),high.to_array()],
            "largest_levels":levels.iter().take(8).map(|(area,p)| serde_json::json!({"area":area,"point":p})).collect::<Vec<_>>()},
        "spawn":spawn.to_array(),"pose_evaluator":if basic_fk {"basic public FK (T053)"} else {"motion_eval IK/constraints (T057)"},
        "evaluation_totals":{"constraints":eval_totals[0],"ik_chains":eval_totals[1],"expressions":eval_totals[2],"unsupported_constraints":eval_totals[3]},"weapon_joint":weapon_joint,"objentry_joint":objentry_joint,"weapon_bounds":weapon_bounds,"idle_hand_matrix":idle_hand,"weapon_root_matrix":weapon_root,"weapon_bones":weapon.skinning.bones.len(),"object_candidates":object_candidates,
        "object_count":objects.len(),
        "missing":["scale channels","up-vector/path/camera constraint types","limiter damping","root rotation","weapon constraint motion","hit detection","enemies","measured installed timing"],
        "inferred_rules":["direction constraints aim local +X","two-joint IK bends the middle joint about local Z","expression list/condition semantics","Hermite tangents scaled by key span","field/battle mode switch with 6 s disarm timeout"],
        "gameplay_verified":false});
    Ok(Loaded {
        room,
        actor,
        weapon,
        params,
        clips,
        actions,
        clip_poses,
        spawn,
        world,
        summary,
    })
}

#[derive(Resource)]
struct Startup {
    room: Option<LoadedRoom>,
    actor: Option<model_inspection::InspectionModel>,
    weapon: Option<model_inspection::InspectionModel>,
}

/// Spawns one textured mesh per model part under `parent`, in part order.
fn spawn_model(
    commands: &mut Commands,
    model: &model_inspection::InspectionModel,
    pose: &[Vec3],
    parent: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Vec<(usize, Option<Entity>)> {
    let mut spawned = Vec::new();
    for (ordinal, part) in model.parts.iter().enumerate() {
        if part.indices.is_empty() {
            spawned.push((ordinal, None));
            continue;
        }
        let positions = part
            .indices
            .iter()
            .map(|&i| pose[i as usize].to_array())
            .collect();
        let mut uvs = vec![[0., 0.]; part.indices.len()];
        let mut texture = None;
        if let Some(set) = &model.textures {
            let route = &model.texture_routes[0].parts[ordinal];
            uvs = route
                .expanded_uv_bits
                .iter()
                .map(|uv| uv.map(f32::from_bits))
                .collect();
            let binding = set
                .bind(route.environment_ordinal, TextureMode::OriginalIndexed)
                .expect("admitted model binding");
            if let Some(image) = binding.image_index {
                let level = &set.images[image].levels[0];
                texture = Some(rgba_image(
                    level.width,
                    level.height,
                    level.pixels_rgba.clone(),
                    images,
                ));
            }
        }
        let entity = commands
            .spawn((
                ChildOf(parent),
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
            ))
            .id();
        spawned.push((ordinal, Some(entity)));
    }
    spawned
}

fn setup(
    mut commands: Commands,
    mut startup: ResMut<Startup>,
    mut game: ResMut<Game>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let room = startup.room.take().expect("room retained for setup");
    // Scene data stays in source coordinates under one X-mirrored root; see
    // MIRROR. The simulation never sees the mirror.
    let world = commands
        .spawn((Transform::from_scale(MIRROR), Visibility::default()))
        .id();
    for room in &room.rooms {
        let handles: Vec<_> = room
            .textures
            .iter()
            .map(|t| upload(t, &mut images))
            .collect();
        for chunk in &room.model.chunks {
            if chunk.indices.is_empty() {
                continue;
            }
            let vertex = |i: &u32| &chunk.vertices[*i as usize];
            let positions = chunk
                .indices
                .iter()
                .map(|i| vertex(i).position_bits.map(f32::from_bits))
                .collect();
            let uvs = chunk
                .indices
                .iter()
                .map(|i| vertex(i).uv_bits.map(f32::from_bits))
                .collect();
            let colors = chunk
                .indices
                .iter()
                .map(|i| vertex(i).color_bits.map(f32::from_bits))
                .collect();
            let texture = room
                .textures
                .iter()
                .position(|t| t.environment_ordinal as i16 == chunk.texture_index)
                .map(|i| handles[i].clone());
            commands.spawn((
                ChildOf(world),
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
        }
    }
    // Collision overlay (F1): walkable floors green, walls red.
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for triangle in &room.collision_triangles {
        let t = &triangle.corners;
        let normal = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        let stand = triangle.surface_flags & PARTY_STAND != 0;
        let collide = triangle.surface_flags & PARTY_COLLIDE != 0;
        let color = if normal.y.abs() >= FLOOR_MIN_NORMAL_Y && stand {
            [0.1, 1., 0.2, 0.35]
        } else if collide {
            [1., 0.15, 0.1, 0.35]
        } else {
            // Not used by the player controller.
            [0.2, 0.4, 1., 0.15]
        };
        for v in t {
            positions.push((*v + Vec3::Y * 2.).to_array());
            colors.push(color);
        }
    }
    let corners = positions.len();
    commands.spawn((
        CollisionOverlay,
        ChildOf(world),
        Mesh3d(meshes.add(mesh(positions, vec![[0., 0.]; corners], colors))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            cull_mode: None,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::default(),
        if game.show_collision {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    ));
    let actor = startup.actor.take().expect("actor retained for setup");
    let weapon = startup.weapon.take().expect("weapon retained for setup");
    let pose = game.clip_poses[game.poses[&(Action::Idle, false)]].frames[0].clone();
    let root = commands
        .spawn((
            SoraRoot,
            ChildOf(world),
            Transform::from_translation(Vec3::from_array(game.player.position)),
            Visibility::default(),
        ))
        .id();
    let mut part_indices = Vec::new();
    for (ordinal, entity) in spawn_model(
        &mut commands,
        &actor,
        &pose,
        root,
        &mut meshes,
        &mut materials,
        &mut images,
    ) {
        part_indices.push(actor.parts[ordinal].indices.clone());
        if let Some(entity) = entity {
            commands.entity(entity).insert(SoraPart(ordinal));
        }
    }
    let hand = commands
        .spawn((
            WeaponRoot,
            ChildOf(root),
            Transform::from_matrix(game.clip_poses[game.poses[&(Action::Idle, false)]].weapon[0]),
            Visibility::default(),
        ))
        .id();
    // The weapon's bind pose places it by its own root joint; express it in
    // that joint's space so the actor's weapon joint replaces it.
    let weapon_root = Mat4::from_cols_array(&weapon.poses[0].matrix_bits[0].map(f32::from_bits));
    let weapon_pose: Vec<Vec3> = weapon.poses[0]
        .position_bits
        .iter()
        .map(|p| {
            weapon_root
                .inverse()
                .transform_point3(Vec3::from_array(p.map(f32::from_bits)))
        })
        .collect();
    // With the weapon's own motion, its posed vertices are already in the
    // hand joint's space; otherwise fall back to the bind pose above.
    let battle_idle = &game.clip_poses[game.poses[&(Action::Idle, true)]];
    let animated = !battle_idle.weapon_frames.is_empty();
    let weapon_pose = if animated {
        battle_idle.weapon_frames[0].clone()
    } else {
        weapon_pose
    };
    let mut weapon_indices = Vec::new();
    for (ordinal, entity) in spawn_model(
        &mut commands,
        &weapon,
        &weapon_pose,
        hand,
        &mut meshes,
        &mut materials,
        &mut images,
    ) {
        weapon_indices.push(weapon.parts[ordinal].indices.clone());
        if let Some(entity) = entity {
            commands.entity(entity).insert(WeaponPart(ordinal));
        }
    }
    game.weapon_indices = weapon_indices;
    game.weapon_display = weapon_pose;
    game.part_indices = part_indices;
    game.display = pose;
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 50f32.to_radians(),
            near: 5.,
            far: 200_000.,
            ..default()
        }),
        Transform::default(),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        NoIndirectDrawing,
    ));
}

fn gather_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    mut game: ResMut<Game>,
) {
    if game.autoplay.is_some() {
        return;
    }
    let mut jump = keys.just_pressed(KeyCode::Space);
    let mut attack = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::KeyK)
        || mouse.just_pressed(MouseButton::Left);
    if game.gamepad {
        for pad in &pads {
            jump |= pad.just_pressed(GamepadButton::East);
            attack |= pad.just_pressed(GamepadButton::South);
        }
    }
    game.jump_queued |= jump;
    game.attack_queued |= attack;
}

/// Scripted input for unattended verification runs.
fn autoplay_input(tick: u64) -> (Vec2, bool, bool) {
    let stick = match tick {
        60..150 => Vec2::new(0., 1.),
        150..240 => Vec2::new(0., 0.35),
        240..270 => Vec2::new(1., 0.),
        _ => Vec2::ZERO,
    };
    let jump = matches!(tick, 300 | 560);
    let attack = matches!(tick, 400 | 410 | 440 | 470 | 500 | 575 | 590 | 610 | 630);
    (stick, jump, attack)
}
const AUTOPLAY_CAPTURES: [u64; 15] = [
    90, 110, 200, 255, 312, 330, 352, 405, 450, 495, 585, 615, 760, 1010, 1100,
];
const AUTOPLAY_END: u64 = 1140;
const AUTOPLAY_WARMUP_FRAMES: u64 = 30;

fn simulate(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    rig: Res<CameraRig>,
    mut game: ResMut<Game>,
) {
    let tick = game.tick;
    let (stick, jump, attack) = if game.autoplay.is_some() {
        autoplay_input(tick)
    } else {
        let mut stick = Vec2::ZERO;
        for (key, delta) in [
            (KeyCode::KeyW, Vec2::Y),
            (KeyCode::ArrowUp, Vec2::Y),
            (KeyCode::KeyS, Vec2::NEG_Y),
            (KeyCode::ArrowDown, Vec2::NEG_Y),
            (KeyCode::KeyA, Vec2::NEG_X),
            (KeyCode::ArrowLeft, Vec2::NEG_X),
            (KeyCode::KeyD, Vec2::X),
            (KeyCode::ArrowRight, Vec2::X),
        ] {
            if keys.pressed(key) {
                stick += delta;
            }
        }
        stick = stick.clamp_length_max(1.);
        if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            stick *= 0.4;
        }
        if game.gamepad {
            for pad in &pads {
                let s = pad.left_stick();
                if s.length() > 0.2 {
                    stick = s.clamp_length_max(1.);
                }
            }
        }
        let queued = (game.jump_queued, game.attack_queued);
        game.jump_queued = false;
        game.attack_queued = false;
        (stick, queued.0, queued.1)
    };
    // Camera-relative stick: forward points from the camera toward Sora.
    let forward = Vec2::new(-rig.yaw.sin(), -rig.yaw.cos());
    let right = Vec2::new(rig.yaw.cos(), -rig.yaw.sin());
    // The camera works in displayed (mirrored) space; map back to source.
    let direction = (forward * stick.y + right * stick.x) * MIRROR.xz();
    let input = Input {
        direction: direction.to_array(),
        jump,
        attack,
    };
    let Game {
        player,
        params,
        clips,
        world,
        ..
    } = &mut *game;
    let step = player.tick(&input, params, clips, world);
    game.tick += 1;
    if let Some(action) = step.started {
        let position = game.player.position;
        let armed = game.player.armed;
        game.action_started = game.tick;
        game.event(
            serde_json::json!({"event":"action","tick":tick,"action":format!("{action:?}"),"armed":armed,
            "position":position,"landed":step.landed}),
        );
    }
    if tick.is_multiple_of(60) {
        let (position, yaw, action) = (game.player.position, game.player.yaw, game.player.action);
        game.event(
            serde_json::json!({"event":"state","tick":tick,"position":position,"yaw":yaw,
            "action":format!("{action:?}")}),
        );
    }
}

/// The weapon root, distinct from Sora's root transform.
type WeaponOnly = (With<WeaponRoot>, Without<SoraRoot>);
/// The camera never shares a transform with Sora's hierarchy.
type CameraOnly = (With<Camera3d>, Without<SoraRoot>, Without<WeaponRoot>);

// Bevy systems take each resource as a parameter.
#[allow(clippy::too_many_arguments)]
fn camera_control(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    pads: Query<&Gamepad>,
    time: Res<Time<Real>>,
    game: Res<Game>,
    mut rig: ResMut<CameraRig>,
    mut camera: Query<&mut Transform, CameraOnly>,
) {
    let dt = time.delta_secs().min(0.1);
    let mut turn = 0.;
    if keys.pressed(KeyCode::KeyQ) || keys.pressed(KeyCode::KeyJ) {
        turn -= 2.;
    }
    if keys.pressed(KeyCode::KeyE) || keys.pressed(KeyCode::KeyL) {
        turn += 2.;
    }
    // Unattended runs swing the camera to Sora's front for a facing check.
    if game.autoplay.is_some() && (660..760).contains(&game.tick) {
        rig.yaw = std::f32::consts::PI * (1. - (game.tick - 660) as f32 / 100.);
    }
    rig.yaw += turn * dt;
    if mouse.pressed(MouseButton::Right) {
        rig.yaw -= motion.delta.x * 0.005;
        rig.pitch = (rig.pitch + motion.delta.y * 0.003).clamp(-0.2, 1.2);
    }
    if game.gamepad {
        for pad in &pads {
            let s = pad.right_stick();
            if s.length() > 0.2 {
                rig.yaw += s.x * 2.5 * dt;
                rig.pitch = (rig.pitch - s.y * 1.5 * dt).clamp(-0.2, 1.2);
            }
        }
    }
    if keys.just_pressed(KeyCode::Equal) {
        rig.distance = (rig.distance * 0.85).max(150.);
    }
    if keys.just_pressed(KeyCode::Minus) {
        rig.distance = (rig.distance * 1.18).min(3000.);
    }
    let focus = Vec3::from_array(game.player.position) * MIRROR + Vec3::Y * 90.;
    let follow = 1. - (-dt * 10.).exp();
    rig.target = rig.target.lerp(focus, follow);
    let offset = Vec3::new(
        rig.yaw.sin() * rig.pitch.cos(),
        rig.pitch.sin(),
        rig.yaw.cos() * rig.pitch.cos(),
    ) * rig.distance;
    if let Ok(mut transform) = camera.single_mut() {
        *transform =
            Transform::from_translation(rig.target + offset).looking_at(rig.target, Vec3::Y);
    }
}

fn present(
    fixed: Res<Time<Fixed>>,
    mut game: ResMut<Game>,
    mut roots: Query<&mut Transform, (With<SoraRoot>, Without<WeaponRoot>)>,
    mut hands: Query<(&mut Transform, &mut Visibility), WeaponOnly>,
    parts: Query<(&SoraPart, &Mesh3d)>,
    weapon_parts: Query<(&WeaponPart, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let action = game.player.action;
    let started = game.action_started;
    if game.shown_action != Some((action, started)) {
        if game.shown_action.is_some() {
            game.blend_from = Some((game.display.clone(), game.display_hand, game.action_started));
        }
        game.shown_action = Some((action, started));
    }
    let frame = game.player.motion_frame(&game.clips) as usize;
    let clip = game.poses[&(action, game.player.armed)];
    let target = &game.clip_poses[clip].frames;
    let target = &target[frame.min(target.len() - 1)];
    let mut display = target.clone();
    let weapons = &game.clip_poses[clip].weapon;
    let mut hand = Transform::from_matrix(weapons[frame.min(weapons.len() - 1)]);
    // Crossfade measured in simulation ticks (plus the fixed-step overstep),
    // applied to the body and the Keyblade alike.
    if let Some((from, from_hand, start)) = &game.blend_from {
        let elapsed = (game.tick - start) as f32 + fixed.overstep_fraction();
        let weight = (elapsed / BLEND_TICKS as f32).clamp(0., 1.);
        if weight >= 1. {
            game.blend_from = None;
        } else {
            for (d, f) in display.iter_mut().zip(from) {
                *d = f.lerp(*d, weight);
            }
            hand = Transform {
                translation: from_hand.translation.lerp(hand.translation, weight),
                rotation: from_hand.rotation.slerp(hand.rotation, weight),
                scale: from_hand.scale.lerp(hand.scale, weight),
            };
        }
    }
    game.display_hand = hand;
    if let Ok((mut transform, mut visibility)) = hands.single_mut() {
        *transform = hand;
        // The Keyblade is drawn only in battle mode.
        *visibility = if game.player.armed {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (part, handle) in &parts {
        let positions: Vec<[f32; 3]> = game.part_indices[part.0]
            .iter()
            .map(|&i| display[i as usize].to_array())
            .collect();
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
    }
    game.display = display;
    // The Keyblade's own motion (key chain and blade) at the same frame.
    let weapon_frames = &game.clip_poses[clip].weapon_frames;
    if !weapon_frames.is_empty() {
        let pose = weapon_frames[frame.min(weapon_frames.len() - 1)].clone();
        for (part, handle) in &weapon_parts {
            let positions: Vec<[f32; 3]> = game.weapon_indices[part.0]
                .iter()
                .map(|&i| pose[i as usize].to_array())
                .collect();
            if let Some(mut mesh) = meshes.get_mut(&handle.0) {
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            }
        }
        game.weapon_display = pose;
    }
    if let Ok(mut root) = roots.single_mut() {
        root.translation = Vec3::from_array(game.player.position);
        root.rotation = Quat::from_rotation_y(game.player.yaw);
    }
}

/// Unattended runs start with simulation time paused until rendering has
/// warmed up, so the first capture is not a blank frame.
fn pause_for_autoplay(game: Res<Game>, mut time: ResMut<Time<Virtual>>) {
    if game.autoplay.is_some() {
        time.pause();
    }
}

// Bevy systems take each resource as a parameter.
#[allow(clippy::too_many_arguments)]
fn finish(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: Query<&mut Visibility, With<CollisionOverlay>>,
    mut game: ResMut<Game>,
    mut time: ResMut<Time<Virtual>>,
    mut closed: MessageReader<bevy::window::WindowCloseRequested>,
    mut exit: MessageWriter<AppExit>,
) {
    game.rendered_frames += 1;
    if game.autoplay.is_some()
        && time.is_paused()
        && !game.capture_pending
        && game.rendered_frames >= AUTOPLAY_WARMUP_FRAMES
    {
        time.unpause();
    }
    if let Some(dir) = game.autoplay.clone()
        && !game.capture_pending
        && let Some(&at) = AUTOPLAY_CAPTURES.get(game.captures_taken)
        && game.tick >= at
    {
        game.capture_pending = true;
        // Hold simulation time so the capture shows exactly this tick.
        time.pause();
        let index = game.captures_taken;
        let path = dir.join(format!("test-room-{index:02}.png"));
        let action = format!("{:?}", game.player.action);
        let tick = game.tick;
        commands.spawn(Screenshot::primary_window()).observe(
            move |event: On<ScreenshotCaptured>,
                  mut game: ResMut<Game>,
                  mut time: ResMut<Time<Virtual>>| {
                let image = event
                    .image
                    .clone()
                    .try_into_dynamic()
                    .expect("test room capture");
                let luma = image.to_luma8();
                let (low, high) = luma.pixels().fold((u8::MAX, u8::MIN), |(l, h), p| {
                    (l.min(p.0[0]), h.max(p.0[0]))
                });
                let uniform = high.saturating_sub(low) < 8;
                if uniform {
                    game.autoplay_failed = true;
                }
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .expect("new private capture");
                image
                    .write_to(&mut file, image::ImageFormat::Png)
                    .expect("capture encode");
                file.sync_all().expect("capture flush");
                game.capture_pending = false;
                game.captures_taken += 1;
                time.unpause();
                game.event(
                    serde_json::json!({"event":"capture","index":index,"tick":tick,"action":action,
                    "luma_range":[low,high],"uniform":uniform}),
                );
            },
        );
    }
    if keys.just_pressed(KeyCode::F1) {
        game.show_collision = !game.show_collision;
        if let Ok(mut visibility) = overlay.single_mut() {
            *visibility = if game.show_collision {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    let autoplay_done = game.autoplay.is_some()
        && game.tick >= AUTOPLAY_END
        && game.captures_taken == AUTOPLAY_CAPTURES.len();
    let window_closed = closed.read().count() > 0;
    if keys.just_pressed(KeyCode::Escape)
        || window_closed
        || autoplay_done
        || game.limit.is_some_and(|n| game.tick >= n)
    {
        let code = if game.autoplay.is_some() && (!autoplay_done || game.autoplay_failed) {
            1
        } else {
            0
        };
        if game.exit_code.is_none() {
            game.exit_code = Some(code);
            let tick = game.tick;
            game.event(serde_json::json!({"event":"exit","tick":tick,"code":code,"window_closed":window_closed}));
            exit.write(if code == 0 {
                AppExit::Success
            } else {
                AppExit::Error(std::num::NonZeroU8::new(code).unwrap())
            });
        }
    }
}

/// Double-click launch: with no arguments, read the three required paths
/// (and optional extra options) from a private launch file outside Git.
fn launch_file_args() -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    #[derive(Deserialize)]
    struct Launch {
        steam_root: PathBuf,
        manifest: PathBuf,
        config: PathBuf,
        #[serde(default)]
        options: Vec<String>,
    }
    // Search next to the checkout first (`.local/test-room/launch.json`, found
    // from the executable in `target/<profile>/`), then LOCALAPPDATA.
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        for up in exe.ancestors().skip(1).take(4) {
            candidates.push(up.join(".local/test-room/launch.json"));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("KH2FinalRewrite/test-room-launch.json"));
    }
    let path = candidates.iter().find(|p| p.is_file()).ok_or_else(|| {
        format!(
            "no arguments and no launch file; looked in: {}",
            candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("; ")
        )
    })?;
    let launch: Launch = serde_json::from_slice(&read_limited(path)?)?;
    let mut args = vec![launch.steam_root, launch.manifest, launch.config];
    args.extend(launch.options.into_iter().map(PathBuf::from));
    Ok(args)
}

fn run() -> Result<AppExit, Box<dyn std::error::Error>> {
    let mut args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.is_empty() {
        args = launch_file_args()?;
    }
    if args.len() < 3 {
        return Err("usage: test_room <Steam-root> <manifest> <test-room-config> [--log new.jsonl] [--frames N] [--autoplay <new-capture-dir>] [--gamepad] [--show-collision] [--basic-fk]".into());
    }
    let mut log_path = None;
    let mut limit = None;
    let mut autoplay = None;
    let mut gamepad = false;
    let mut show_collision = false;
    let mut basic_fk = false;
    let mut options = args[3..].iter();
    while let Some(option) = options.next() {
        match option.to_str() {
            Some("--gamepad") => gamepad = true,
            Some("--show-collision") => show_collision = true,
            Some("--basic-fk") => basic_fk = true,
            Some("--log") if log_path.is_none() => log_path = options.next().cloned(),
            Some("--frames") if limit.is_none() => {
                limit = Some(
                    options
                        .next()
                        .and_then(|v| v.to_str())
                        .ok_or("frame limit absent")?
                        .parse::<u64>()?,
                )
            }
            Some("--autoplay") if autoplay.is_none() => autoplay = options.next().cloned(),
            _ => return Err("unknown/duplicate test room option".into()),
        }
    }
    let source = ProbeSource::bind_session(&args[0], &args[1])?;
    // Validate the log path now; the file is created only after admission.
    let log_path = match log_path {
        Some(path) => {
            let path = source.output_path(&path)?;
            if path.exists() {
                return Err("test room log must be new".into());
            }
            Some(path)
        }
        None => None,
    };
    let autoplay = match autoplay {
        Some(dir) => {
            // Guard a file inside the directory: the directory itself exists.
            source.output_path(&dir.join("test-room-00.png"))?;
            if !dir.is_dir() || std::fs::read_dir(&dir)?.next().is_some() {
                return Err("autoplay capture directory must exist and be empty".into());
            }
            Some(dir)
        }
        None => None,
    };
    eprintln!("Validating the selected KH2FM source (full rehash), then decoding the test room...");
    let loaded = load(&source, &args[2], basic_fk)?;
    let log = match log_path {
        Some(path) => Some(OpenOptions::new().write(true).create_new(true).open(path)?),
        None => None,
    };
    let mut poses = HashMap::new();
    for (action, clip) in &loaded.actions {
        poses.insert(*action, *clip);
    }
    let mut game = Game {
        params: loaded.params,
        clips: loaded.clips,
        world: loaded.world,
        player: Player::new(loaded.spawn.to_array(), 0.),
        poses,
        clip_poses: loaded.clip_poses,
        part_indices: Vec::new(),
        tick: 0,
        log,
        limit,
        autoplay,
        captures_taken: 0,
        capture_pending: false,
        gamepad,
        jump_queued: false,
        attack_queued: false,
        display: Vec::new(),
        blend_from: None,
        display_hand: Transform::IDENTITY,
        weapon_indices: Vec::new(),
        weapon_display: Vec::new(),
        shown_action: None,
        action_started: 0,
        exit_code: None,
        show_collision,
        rendered_frames: 0,
        autoplay_failed: false,
    };
    game.event(loaded.summary);
    eprintln!(
        "Test room ready. WASD/arrows move (Shift walks), Space jumps, Enter/K/left-click attacks, Q/E or right-drag turns the camera, Esc exits."
    );
    Ok(App::new()
        .insert_resource(game)
        .insert_resource(Startup {
            room: Some(loaded.room),
            actor: Some(loaded.actor),
            weapon: Some(loaded.weapon),
        })
        .insert_resource(CameraRig {
            yaw: std::f32::consts::PI,
            pitch: 0.35,
            distance: 520.,
            target: loaded.spawn * MIRROR + Vec3::Y * 90.,
        })
        .insert_resource(Time::<Fixed>::from_hz(60.))
        .insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.09)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "KH2FM test room (quarantined source) | WASD move, Shift walk, Space jump, Enter/K/LMB attack, Q/E camera, Esc".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            // Closing the window goes through finish() so the exit code and
            // log record reflect an unfinished unattended run.
            exit_condition: bevy::window::ExitCondition::DontExit,
            close_when_requested: false,
            ..default()
        }))
        .add_systems(Startup, (setup, pause_for_autoplay))
        .add_systems(PreUpdate, gather_input.after(bevy::input::InputSystems))
        .add_systems(FixedUpdate, simulate)
        .add_systems(Update, (camera_control, present, finish).chain())
        .run())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(AppExit::Success) => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(e) => {
            eprintln!("Test room rejected: {e}");
            // Keep a double-clicked console open long enough to read the error.
            if std::env::args_os().len() == 1 {
                eprintln!("Press Enter to close.");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
            std::process::ExitCode::FAILURE
        }
    }
}
