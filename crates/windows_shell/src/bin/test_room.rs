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
    asset::RenderAssetUsages,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    image::ImageSampler,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    render::view::{
        NoIndirectDrawing,
        screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use package_reader::{
    bar, model_inspection, model_inspection_textures::TextureMode, motion, object_collision,
    objentry, original, preferences, probe::ProbeSource, ptya, sklt,
};
use player_sim::combat;
use player_sim::shadow::{self, ScriptRandom, Shadow};
use player_sim::{
    Action, AttackStep, Clip, Clips, Input, Params, Player, TriangleWorld, World,
    camera::{self, CameraInput, FollowCamera},
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use windows_shell::command::{self, CommandMenu, Decision, Item};
use windows_shell::gauge::{Canvas, Draw, Gauge, GaugeStats, Texture};
use windows_shell::hud::{self, Sheet, SystemFont};
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
    /// A game room (map) selection; omitted when `arena` is used.
    #[serde(default)]
    room_selection: Option<PathBuf>,
    /// A generated test arena instead of a game room.
    #[serde(default)]
    arena: Option<ArenaConfig>,
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
    /// Field 2D sequence archive holding the 'gaug' HUD sequence
    /// (field2d/<lang>/<world>0field.2dd).
    #[serde(default)]
    hud: Option<Locator>,
    /// Character archive holding the 'face' sequence (obj/P_EX100.a.<lang>).
    #[serde(default)]
    face: Option<Locator>,
    /// Command menu sources.
    #[serde(default)]
    menu: Option<MenuSources>,
    /// Target enemy (test-room choice of placement).
    #[serde(default)]
    enemy: Option<EnemyConfig>,
    /// Optional spawn (x, probe height, z) in scene coordinates.
    #[serde(default)]
    spawn: Option<[f32; 3]>,
}

#[derive(Deserialize)]
struct ArenaConfig {
    half_size: f32,
    wall_height: f32,
    tile: f32,
}

#[derive(Deserialize)]
struct EnemyConfig {
    /// Model selection (relative to this configuration), e.g. M_EX020.mdlx.
    model_selection: PathBuf,
    /// The same mdlx as a locator, for its object-collision table.
    model: Locator,
    /// Its motion set (direct slot = motion id).
    motion_set: Locator,
    /// Weapon mdlx (W_EX010) for the Keyblade's attack spheres.
    weapon_model: Locator,
    /// Sora's mdlx (P_EX100) for his HIT spheres.
    actor_model: Locator,
    enmp_id: u16,
    /// World battle level used when the ENMP level is 0 (test-room choice).
    battle_level: u16,
    /// Placement relative to Sora's spawn, in scene units.
    offset: [f32; 3],
}

#[derive(Deserialize)]
struct MenuSources {
    /// field2d/<lang>/<world><n>command.2dd ('comw' sequence).
    command: Locator,
    /// msg/<lang>/fontimage.bar and msg/<lang>/fontinfo.bar.
    font_image: Locator,
    font_info: Locator,
    /// msg/<lang>/sys.bar (command labels).
    messages: Locator,
}

/// HUD sources loaded with the session.
struct HudSource {
    sheets: Vec<Sheet>,
    stats: (i32, i32),
    menu: Option<(Sheet, SystemFont, Vec<Vec<u8>>)>,
}

/// Top-menu labels: Attack, Magic, Items, Drive (03system 'cmd' message ids).
const MENU_MESSAGES: [u32; 4] = [0x81e0, 0x81e1, 0x81e2, 0x81e3];

/// Sora's grounded damage motions: front-high, front-low, back-high,
/// back-low (motion ids 10..13).
const SORA_HURT: [u16; 4] = [10, 11, 12, 13];

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
    /// Keyblade bone 2 matrix in hand-joint space per frame (attack spheres).
    blade: Vec<Mat4>,
    /// Attack windows from the weapon motion's range trigger 10:
    /// (start, end, ATKP id, collision group), in clip ticks.
    hits: Vec<(u32, u32, u16, u8)>,
    /// Sora's HIT-bone matrices per frame (model space, root removed).
    body: Vec<HashMap<usize, Mat4>>,
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
    menu_up_queued: bool,
    menu_down_queued: bool,
    enemy: Option<EnemyRuntime>,
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

#[derive(Component)]
struct HudOverlay;

struct MenuState {
    menu: CommandMenu,
    sheet: Sheet,
    font: SystemFont,
}

/// Sora's field gauge: the translated GAUGE_PLAYER logic, drawn into a
/// software canvas shown as a full-window overlay.
#[derive(Resource)]
struct Hud {
    /// Gauge sheet, then the face sheet when configured.
    sheets: Vec<Sheet>,
    menu: Option<MenuState>,
    gauge: Gauge,
    stats: GaugeStats,
    canvas: Canvas,
    image: Handle<Image>,
    updated: bool,
}

/// The original's default follow camera, ticked with the simulation in the
/// game's own world space (Y down: scene (x, -y, -z)).
#[derive(Resource)]
struct CameraRig {
    camera: FollowCamera,
    previous_position: [f32; 3],
    /// Right-drag turn not yet consumed by a tick, in manual-turn units.
    pending_turn: f32,
    recenter_queued: bool,
    /// Map camera collision triangles in game world space.
    collision: Vec<[[f32; 3]; 3]>,
    collision_hits: u64,
}

/// Scene (Y up) to game world (Y down) and back; the map is converted with
/// (x, -y, -z).
fn to_world(p: [f32; 3]) -> [f32; 3] {
    [p[0], -p[1], -p[2]]
}
fn from_world(p: [f32; 3]) -> Vec3 {
    Vec3::new(p[0], -p[1], -p[2])
}

fn camera_input(player: &Player, previous: [f32; 3], speed: f32) -> CameraInput {
    let [s, c] = [player.yaw.sin(), player.yaw.cos()];
    CameraInput {
        player_position: to_world(player.position),
        player_forward: to_world([s, 0., c]),
        moved: player.position[0] != previous[0] || player.position[2] != previous[2],
        speed,
        ..Default::default()
    }
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
    arena_mesh: Option<windows_shell::room::ArenaMesh>,
    actor: model_inspection::InspectionModel,
    weapon: model_inspection::InspectionModel,
    params: Params,
    clips: Clips,
    actions: Vec<((Action, bool), usize)>,
    clip_poses: Vec<ClipPoses>,
    spawn: Vec3,
    world: TriangleWorld,
    summary: serde_json::Value,
    hud: Option<HudSource>,
    enemy: Option<EnemySource>,
}

/// One enemy clip: skinned positions, HIT-bone matrices and root
/// translation per tick, plus its loop start.
struct EnemyClip {
    frames: Vec<Vec<Vec3>>,
    bones: Vec<HashMap<usize, Mat4>>,
    root: Vec<Vec3>,
    loop_start: u32,
    /// Attack windows (range trigger 10): start, end, ATKP id, group.
    windows: Vec<(u32, u32, u16, u8)>,
}

struct EnemyClips(HashMap<usize, EnemyClip>);
impl shadow::Clips for EnemyClips {
    fn length(&self, slot: usize) -> u32 {
        self.0.get(&slot).map_or(1, |c| c.frames.len() as u32)
    }
    fn loop_start(&self, slot: usize) -> u32 {
        self.0.get(&slot).map_or(0, |c| c.loop_start)
    }
    fn root(&self, slot: usize, tick: u32) -> [f32; 3] {
        self.0
            .get(&slot)
            .and_then(|c| c.root.get(tick as usize))
            .map_or([0.; 3], |r| r.to_array())
    }
}

struct RoomFloor<'a>(&'a TriangleWorld);
impl shadow::Env for RoomFloor<'_> {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        self.0.floor(x, z, top)
    }
}

/// Live enemy state for the session.
struct EnemyRuntime {
    clips: EnemyClips,
    body: Vec<(usize, Vec3, f32, bool)>,
    blade: HashMap<u8, Vec<(Vec3, f32)>>,
    attacks: HashMap<u16, combat::AttackParams>,
    sora: combat::AttackerStats,
    enemy: Shadow,
    rng: ScriptRandom,
    claws: HashMap<u8, Vec<(usize, Vec3, f32)>>,
    claw_attacks: HashMap<u16, combat::AttackParams>,
    shadow_attacker: combat::AttackerStats,
    sora_target: combat::TargetStats,
    sora_body: Vec<(usize, Vec3, f32, bool)>,
    /// Claw instances that already hit Sora, and last claw centres.
    sora_hit_by: Vec<u64>,
    claw_previous: HashMap<(u64, usize), Vec3>,
    /// Sora's HP and KO timer (test room refills after a KO).
    sora_hp: i32,
    ko_ticks: u32,
    /// Last frame's attack sphere centres by (window instance, sphere).
    previous: HashMap<(u64, usize), Vec3>,
    indices: Vec<Vec<u32>>,
}

#[derive(Component)]
struct EnemyRoot;
#[derive(Component)]
struct EnemyPart(usize);

struct EnemySource {
    model: model_inspection::InspectionModel,
    clips: EnemyClips,
    /// HIT spheres: bone, offset, radius, high-flinch argument.
    body: Vec<(usize, Vec3, f32, bool)>,
    /// Keyblade attack spheres by collision group: offset, radius.
    blade: HashMap<u8, Vec<(Vec3, f32)>>,
    attacks: HashMap<u16, combat::AttackParams>,
    sora: combat::AttackerStats,
    stats: combat::TargetStats,
    offset: Vec3,
    claws: HashMap<u8, Vec<(usize, Vec3, f32)>>,
    claw_attacks: HashMap<u16, combat::AttackParams>,
    shadow_attacker: combat::AttackerStats,
    sora_target: combat::TargetStats,
    sora_body: Vec<(usize, Vec3, f32, bool)>,
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
    let (room, arena_mesh) = match (&config.arena, &config.room_selection) {
        (Some(a), _) => {
            let (room, mesh) = windows_shell::room::arena(a.half_size, a.wall_height, a.tile);
            (room, Some(mesh))
        }
        (None, Some(path)) => (load_rooms(source, &base.join(path))?, None),
        (None, None) => return Err("configuration needs a room selection or an arena".into()),
    };

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
    let hud = match &config.hud {
        Some(l) => {
            let mut sheets = vec![hud::load(
                source,
                &l.index,
                l.ordinal,
                l.name_hash,
                b"gaug",
            )?];
            if let Some(f) = &config.face {
                sheets.push(hud::load(
                    source,
                    &f.index,
                    f.ordinal,
                    f.name_hash,
                    b"face",
                )?);
            }
            let menu = match &config.menu {
                Some(m) => Some((
                    hud::load(
                        source,
                        &m.command.index,
                        m.command.ordinal,
                        m.command.name_hash,
                        b"comw",
                    )?,
                    hud::load_system_font(
                        source,
                        (
                            &m.font_image.index,
                            m.font_image.ordinal,
                            m.font_image.name_hash,
                        ),
                        (
                            &m.font_info.index,
                            m.font_info.ordinal,
                            m.font_info.name_hash,
                        ),
                    )?,
                    hud::load_messages(
                        source,
                        &m.messages.index,
                        m.messages.ordinal,
                        m.messages.name_hash,
                        &MENU_MESSAGES,
                    )?,
                )),
                None => None,
            };
            Some(HudSource {
                sheets,
                stats: hud::starting_stats(bar_entry(&battle, b"plrp")?)?,
                menu,
            })
        }
        None => None,
    };
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
    // Damage reactions (motion ids 10..13) when an enemy can hit Sora.
    let hurt_count = if config.enemy.is_some() {
        let hurt_selection = motion::MotionSetSelection {
            index: config.motion_set.index.clone(),
            ordinal: config.motion_set.ordinal,
            name_hash: config.motion_set.name_hash,
            battle: config.battle_mode,
            has_weapon: config.has_weapon,
        };
        motions.extend(motion::read_motion_ids(
            source,
            &hurt_selection,
            &SORA_HURT,
        )?);
        ids.extend(SORA_HURT);
        SORA_HURT.len()
    } else {
        0
    };
    // Sora's HIT spheres (object collision type 2), for enemy attacks.
    let sora_body: Vec<(usize, Vec3, f32, bool)> = match &config.enemy {
        Some(e) => {
            object_collision::read(bar_entry_typed(&read_archive(source, &e.actor_model)?, 23)?)?
                .iter()
                .filter(|c| c.kind == object_collision::HIT && c.group == 0 && c.bone >= 0)
                .map(|c| {
                    (
                        c.bone as usize,
                        Vec3::new(
                            c.position[0] as f32,
                            c.position[1] as f32,
                            c.position[2] as f32,
                        ),
                        c.radius as f32,
                        matches!(c.argument, 0 | 2 | 3 | 4 | 5),
                    )
                })
                .collect()
        }
        None => Vec::new(),
    };
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
                let mut blade = Vec::new();
                let mut body = Vec::new();
                let hits = weapon_motions
                    .get(&selected.slot)
                    .map(|own| {
                        own.triggers
                            .iter()
                            .flat_map(|t| &t.records.ranges)
                            .filter(|r| r.trigger == 10 && r.parameters.len() >= 2)
                            .map(|r| {
                                (
                                    r.start_frame.max(0) as u32,
                                    if r.end_frame < 0 {
                                        clip.end
                                    } else {
                                        r.end_frame as u32
                                    },
                                    r.parameters[0] as u16,
                                    r.parameters[1] as u8,
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
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
                        if let Some(m) = pose.matrix_bits.get(2) {
                            blade.push(Mat4::from_cols_array(&m.map(f32::from_bits)));
                        }
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
                    body.push(
                        sora_body
                            .iter()
                            .filter_map(|&(b, ..)| {
                                let m = pose.matrix_bits.get(b)?;
                                Some((b, Mat4::from_cols_array(&m.map(f32::from_bits))))
                            })
                            .collect(),
                    );
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
                    blade,
                    hits,
                    body,
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
        hurt: clip_list[field + 3..field + 3 + hurt_count]
            .iter()
            .map(|c| c.0.clone())
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
    for i in 0..hurt_count {
        both.push((Action::Hurt(i), clip_list[field + 3 + i].1));
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
    let enemy = match &config.enemy {
        Some(e) => Some(load_enemy(
            source,
            base,
            e,
            &battle,
            &clip_poses,
            sora_body,
        )?),
        None => None,
    };
    Ok(Loaded {
        room,
        arena_mesh,
        actor,
        weapon,
        params,
        clips,
        actions,
        clip_poses,
        spawn,
        world,
        summary,
        hud,
        enemy,
    })
}

/// Shadow (or another enemy) for the test room: model, direct-slot clips,
/// HIT spheres, the Keyblade's attack spheres and the ATKP/ENMP/LVPM/LVUP
/// rows the damage routine reads.
fn load_enemy(
    source: &ProbeSource,
    base: &Path,
    config: &EnemyConfig,
    battle: &[u8],
    clip_poses: &[ClipPoses],
    sora_body: Vec<(usize, Vec3, f32, bool)>,
) -> Result<EnemySource, Box<dyn std::error::Error>> {
    use package_reader::battle_tables;
    let mut models =
        model_inspection::read_selected_models(source, &base.join(&config.model_selection))?;
    if models.len() != 1 {
        return Err("enemy selection requires exactly one model".into());
    }
    let model = models.remove(0);
    let collision =
        object_collision::read(bar_entry_typed(&read_archive(source, &config.model)?, 23)?)?;
    let mut claws: HashMap<u8, Vec<(usize, Vec3, f32)>> = HashMap::new();
    for e in collision
        .iter()
        .filter(|e| e.kind == object_collision::ATTACK && e.bone >= 0)
    {
        claws.entry(e.group).or_default().push((
            e.bone as usize,
            Vec3::new(
                e.position[0] as f32,
                e.position[1] as f32,
                e.position[2] as f32,
            ),
            e.radius as f32,
        ));
    }
    let body: Vec<_> = collision
        .iter()
        .filter(|e| e.kind == object_collision::HIT && e.group == 0 && e.bone >= 0)
        .map(|e| {
            (
                e.bone as usize,
                Vec3::new(
                    e.position[0] as f32,
                    e.position[1] as f32,
                    e.position[2] as f32,
                ),
                e.radius as f32,
                matches!(e.argument, 0 | 2 | 3 | 4 | 5),
            )
        })
        .collect();
    let weapon_collision = object_collision::read(bar_entry_typed(
        &read_archive(source, &config.weapon_model)?,
        23,
    )?)?;
    let mut blade: HashMap<u8, Vec<(Vec3, f32)>> = HashMap::new();
    for e in weapon_collision
        .iter()
        .filter(|e| e.kind == object_collision::ATTACK && e.bone == 2)
    {
        blade.entry(e.group).or_default().push((
            Vec3::new(
                e.position[0] as f32,
                e.position[1] as f32,
                e.position[2] as f32,
            ),
            e.radius as f32,
        ));
    }
    let slots = shadow::motion::ALL;
    let selected = motion::read_slots(
        source,
        &config.motion_set.index,
        config.motion_set.ordinal,
        config.motion_set.name_hash,
        &slots,
    )?;
    let mut clips = HashMap::new();
    for sel in &selected {
        let (clip, key_start) = clip_for(sel)?;
        let mut frames = Vec::new();
        let mut bones = Vec::new();
        let mut root = Vec::new();
        let windows: Vec<(u32, u32, u16, u8)> = sel
            .triggers
            .iter()
            .flat_map(|t| &t.records.ranges)
            .filter(|r| r.trigger == 10 && r.parameters.len() >= 2)
            .map(|r| {
                (
                    r.start_frame.max(0) as u32,
                    if r.end_frame < 0 {
                        clip.end
                    } else {
                        r.end_frame as u32
                    },
                    r.parameters[0] as u16,
                    r.parameters[1] as u8,
                )
            })
            .collect();
        let bone_list: Vec<usize> = body
            .iter()
            .map(|b| b.0)
            .chain(claws.values().flatten().map(|c| c.0))
            .collect();
        for offset in 0..=clip.end {
            let (pose, evaluated) = model
                .skinning
                .sample_evaluated(&sel.motion, (key_start + offset) as f32)?;
            let r = Vec3::from_array(evaluated.root_translation);
            root.push(r);
            frames.push(
                pose.position_bits
                    .iter()
                    .map(|p| Vec3::from_array(p.map(f32::from_bits)) - r)
                    .collect(),
            );
            bones.push(
                bone_list
                    .iter()
                    .filter_map(|&b| {
                        let m = pose.matrix_bits.get(b)?;
                        let mut m = Mat4::from_cols_array(&m.map(f32::from_bits));
                        m.w_axis -= r.extend(0.);
                        Some((b, m))
                    })
                    .collect(),
            );
        }
        clips.insert(
            sel.slot,
            EnemyClip {
                frames,
                bones,
                root,
                loop_start: clip.loop_start,
                windows,
            },
        );
    }
    let atkp = bar_entry(battle, b"atkp")?;
    let mut attacks = HashMap::new();
    for (_, _, id, _) in clip_poses.iter().flat_map(|c| &c.hits) {
        let a = battle_tables::attack(atkp, *id)?;
        attacks.insert(
            *id,
            combat::AttackParams {
                power: a.power,
                stat: a.stat,
                element: a.element,
                reaction: a.reaction,
                knockback: a.knockback,
            },
        );
    }
    let mut claw_attacks = HashMap::new();
    for (_, _, id, _) in clips.values().flat_map(|c: &EnemyClip| &c.windows) {
        let a = battle_tables::attack(atkp, *id)?;
        claw_attacks.insert(
            *id,
            combat::AttackParams {
                power: a.power,
                stat: a.stat,
                element: a.element,
                reaction: a.reaction,
                knockback: a.knockback,
            },
        );
    }
    let enemy = battle_tables::enemy(bar_entry(battle, b"enmp")?, config.enmp_id)?;
    let level = if enemy.level == 0 {
        config.battle_level
    } else {
        enemy.level
    };
    let row = battle_tables::enemy_level(bar_entry(battle, b"lvpm")?, level)?;
    let stats = combat::TargetStats::enemy(
        enemy.hp_percent,
        enemy.damage_cap,
        enemy.damage_min,
        enemy.weakness,
        row.hp,
        row.defense,
    );
    // Sora at level 1: LVUP row + PLRP (id 0, char 1, Str byte 6) +
    // Kingdom Key item status +3 Str / +1 Mag (03system item row 80,
    // combat-01; test-room constant).
    let lv = battle_tables::player_level(bar_entry(battle, b"lvup")?, 1, 1)?;
    let plrp = bar_entry(battle, b"plrp")?;
    let plrp_str = plrp.get(8 + 6).copied().unwrap_or(0) as i32;
    let plrp_mag = plrp.get(8 + 7).copied().unwrap_or(0) as i32;
    let sora = combat::AttackerStats {
        strength: lv.strength as i32 + plrp_str + 3,
        magic: lv.magic as i32 + plrp_mag + 1,
        max: 99999,
        min: 1,
    };
    // Enemy as attacker: LVPM Str with its max/min clamps; Sora as target:
    // LVUP + PLRP defence, player clamps 99999/1, weaknesses 100.
    let shadow_attacker = combat::AttackerStats {
        strength: row.strength as i32,
        magic: row.strength as i32,
        max: row.max_strength as i32,
        min: row.min_strength as i32,
    };
    let plrp_hp = plrp.get(8 + 3).copied().unwrap_or(20) as i32;
    let plrp_def = plrp.get(8 + 8).copied().unwrap_or(0) as i32;
    let sora_target = combat::TargetStats {
        max_hp: plrp_hp,
        defense: lv.defense as i32 + plrp_def,
        cap: 99999,
        min: 1,
        weakness: [100; 7],
    };
    Ok(EnemySource {
        model,
        clips: EnemyClips(clips),
        body,
        blade,
        attacks,
        sora,
        stats,
        offset: Vec3::from_array(config.offset),
        claws,
        claw_attacks,
        shadow_attacker,
        sora_target,
        sora_body,
    })
}

/// The single BAR entry of a type.
fn bar_entry_typed(bytes: &[u8], kind: u16) -> Result<&[u8], Box<dyn std::error::Error>> {
    let table = bar::read_bar(bytes)?;
    let e = table
        .entries
        .iter()
        .find(|e| e.entry_type == kind && e.offset > 0 && e.size > 0)
        .ok_or("BAR entry type absent")?;
    bytes
        .get(e.offset as usize..e.offset as usize + e.size as usize)
        .ok_or_else(|| "BAR entry exceeds archive".into())
}

#[derive(Resource)]
struct Startup {
    room: Option<LoadedRoom>,
    arena_mesh: Option<windows_shell::room::ArenaMesh>,
    actor: Option<model_inspection::InspectionModel>,
    weapon: Option<model_inspection::InspectionModel>,
    hud: Option<HudSource>,
    enemy_model: Option<model_inspection::InspectionModel>,
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
    let enemy_model = startup.enemy_model.take();
    if let Some(HudSource {
        sheets,
        stats: (hp, mp),
        menu,
    }) = startup.hud.take()
    {
        // Test-room choices for values the opening room does not define:
        // full HP/MP from PLRP, normal drive mode with three full bars.
        let stats = GaugeStats {
            hp,
            max_hp: hp,
            mp,
            max_mp: mp,
            drive_mode: 1,
            drive: 0,
            drive_level: 3,
            drive_max: 3,
        };
        let canvas = Canvas::new(sheets[0].texel_scale as f32);
        let mut image = Image::new(
            Extent3d {
                width: canvas.width as u32,
                height: canvas.height as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            canvas.rgba.clone(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::linear();
        let image = images.add(image);
        // The 16:9 HUD canvas stretched over the window by a 2D camera drawn
        // after the scene.
        commands.spawn((
            Camera2d,
            Camera {
                order: 1,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
        ));
        commands.spawn((Sprite::from_image(image.clone()), HudOverlay));
        commands.insert_resource(Hud {
            gauge: Gauge::new(
                &sheets.iter().map(|s| &s.sequence).collect::<Vec<_>>(),
                &stats,
            ),
            sheets,
            menu: menu.map(|(sheet, font, labels)| {
                // Test-room choices for the opening state: Magic and Drive
                // have no entries yet (colour 5), Items is usable.
                let items = labels
                    .into_iter()
                    .zip([(1, 0), (2, 5), (3, 0), (4, 5)])
                    .map(|(label, (icon, colour))| Item {
                        label,
                        icon,
                        colour,
                    })
                    .collect();
                MenuState {
                    menu: CommandMenu::new(&sheet.sequence, items),
                    sheet,
                    font,
                }
            }),
            stats,
            canvas,
            image,
            updated: true,
        });
    }
    // Scene data stays in source coordinates under one X-mirrored root; see
    // MIRROR. The simulation never sees the mirror.
    let world = commands
        .spawn((Transform::from_scale(MIRROR), Visibility::default()))
        .id();
    if let Some(arena) = startup.arena_mesh.take() {
        let (w, h, rgba) = arena.texture;
        let texture = rgba_image(w, h, rgba, &mut images);
        commands.spawn((
            ChildOf(world),
            Mesh3d(meshes.add(mesh(arena.positions, arena.uvs, arena.colors))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(texture),
                unlit: true,
                cull_mode: None,
                ..default()
            })),
            Transform::default(),
        ));
    }
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
    if let (Some(model), Some(rt)) = (&enemy_model, game.enemy.as_mut()) {
        let e = &rt.enemy;
        let pose = &rt.clips.0[&e.motion].frames[0];
        let root = commands
            .spawn((
                EnemyRoot,
                ChildOf(world),
                Transform::from_translation(Vec3::from_array(e.position))
                    .with_rotation(Quat::from_rotation_y(e.yaw)),
                Visibility::default(),
            ))
            .id();
        let mut indices = Vec::new();
        for (ordinal, entity) in spawn_model(
            &mut commands,
            model,
            pose,
            root,
            &mut meshes,
            &mut materials,
            &mut images,
        ) {
            indices.push(model.parts[ordinal].indices.clone());
            if let Some(entity) = entity {
                commands.entity(entity).insert(EnemyPart(ordinal));
            }
        }
        rt.indices = indices;
    }
    game.weapon_display = weapon_pose;
    game.part_indices = part_indices;
    game.display = pose;
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: FollowCamera::new(camera::OUTDOOR, &CameraInput::default()).vertical_fov(),
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
    let mut up = keys.just_pressed(KeyCode::ArrowUp);
    let mut down = keys.just_pressed(KeyCode::ArrowDown);
    if game.gamepad {
        for pad in &pads {
            up |= pad.just_pressed(GamepadButton::DPadUp);
            down |= pad.just_pressed(GamepadButton::DPadDown);
        }
    }
    game.jump_queued |= jump;
    game.attack_queued |= attack;
    game.menu_up_queued |= up;
    game.menu_down_queued |= down;
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
const AUTOPLAY_CAPTURES: [u64; 18] = [
    90, 110, 200, 255, 312, 330, 352, 405, 450, 495, 585, 615, 760, 1010, 1100, 1300, 1500, 1700,
];
/// The last stretch leaves Sora standing so an enemy can attack him.
const AUTOPLAY_END: u64 = 1800;
const AUTOPLAY_WARMUP_FRAMES: u64 = 30;

fn simulate(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut rig: ResMut<CameraRig>,
    mut game: ResMut<Game>,
    hud: Option<ResMut<Hud>>,
) {
    let tick = game.tick;
    let (up, down) = if game.autoplay.is_some() {
        (false, tick == 980)
    } else {
        let queued = (game.menu_up_queued, game.menu_down_queued);
        game.menu_up_queued = false;
        game.menu_down_queued = false;
        queued
    };
    let (stick, jump, decide) = if game.autoplay.is_some() {
        autoplay_input(tick)
    } else {
        let mut stick = Vec2::ZERO;
        for (key, delta) in [
            (KeyCode::KeyW, Vec2::Y),
            (KeyCode::KeyS, Vec2::NEG_Y),
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
    // Camera-relative stick in displayed (mirrored) space: forward is the
    // view direction on the ground, right is screen right.
    let eye = from_world(rig.camera.eye) * MIRROR;
    let look_at = from_world(rig.camera.look_at) * MIRROR;
    let forward = (look_at - eye).xz().normalize_or(Vec2::NEG_Y);
    let right = Vec2::new(-forward.y, forward.x);
    let direction = (forward * stick.y + right * stick.x) * MIRROR.xz();
    // The decide button executes the command under the cursor; Attack
    // (icon 1) is the only command this room implements.
    let attack = match hud.and_then(|h| {
        h.into_inner().menu.as_mut().map(|m| {
            m.menu.input(up, down, decide) == Decision::Execute(0)
                && m.menu.items.first().is_some_and(|i| i.icon == 1)
        })
    }) {
        Some(execute) => execute,
        None => decide,
    };
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
    combat_tick(&mut game, tick);
    // Camera inputs: Q/E or J/L and the right stick turn, -/= zoom, C
    // recenters (test-room bindings for the original's camera buttons).
    let mut turn = 0f32;
    let mut zoom = 0f32;
    if game.autoplay.is_some() {
        // Unattended runs orbit 180 degrees to check Sora from the front.
        if (660..720).contains(&tick) {
            turn = 1.;
        }
    } else {
        if keys.pressed(KeyCode::KeyQ) || keys.pressed(KeyCode::KeyJ) {
            turn -= 1.;
        }
        if keys.pressed(KeyCode::KeyE) || keys.pressed(KeyCode::KeyL) {
            turn += 1.;
        }
        if keys.pressed(KeyCode::Minus) {
            zoom += 1.;
        }
        if keys.pressed(KeyCode::Equal) {
            zoom -= 1.;
        }
        if game.gamepad {
            for pad in &pads {
                let s = pad.right_stick();
                if s.length() > 0.2 {
                    turn = s.x;
                    zoom = -s.y;
                }
            }
        }
        let pending = rig.pending_turn.clamp(-1., 1.);
        if turn == 0. && pending != 0. {
            turn = pending;
            rig.pending_turn -= pending;
        }
    }
    let mut camera = camera_input(&game.player, rig.previous_position, game.params.run_speed);
    camera.stick = [stick.x, -stick.y];
    camera.magnitude = stick.length().min(1.);
    camera.turn = turn.clamp(-1., 1.);
    camera.zoom = zoom.clamp(-1., 1.);
    camera.recenter = std::mem::take(&mut rig.recenter_queued);
    let CameraRig {
        camera: follow,
        collision,
        collision_hits,
        ..
    } = &mut *rig;
    let hits = std::cell::Cell::new(0);
    follow.tick(&camera, &|a, b| {
        let hit = camera::segment_hit(collision, a, b);
        hits.set(hits.get() + hit.is_some() as u64);
        hit
    });
    *collision_hits += hits.get();
    rig.previous_position = game.player.position;
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
        let enemy = game.enemy.as_ref().map(|e| {
            serde_json::json!({"position":e.enemy.position,"yaw":e.enemy.yaw,
                "hp":e.enemy.hp,"act":format!("{:?}", e.enemy.current_act()),"motion":e.enemy.motion})
        });
        game.event(
            serde_json::json!({"event":"state","tick":tick,"position":position,"yaw":yaw,
            "action":format!("{action:?}"),"camera_eye":rig.camera.eye,"camera_yaw":rig.camera.yaw,
            "camera_collision_triangles":rig.collision.len(),"camera_collision_hits":rig.collision_hits,
            "enemy":enemy}),
        );
    }
}

/// HUD frame: test-room debug keys (H/G HP -/+2, M/N MP -/+10, B drive +10)
/// or the unattended script change stats, then the gauge logic runs.
fn hud_tick(keys: Res<ButtonInput<KeyCode>>, mut game: ResMut<Game>, hud: Option<ResMut<Hud>>) {
    let Some(mut hud) = hud else { return };
    let mut s = hud.stats;
    if let Some(rt) = game.enemy.as_mut() {
        // Combat drives HP; G refills it.
        if keys.just_pressed(KeyCode::KeyG) {
            rt.sora_hp = rt.sora_target.max_hp;
        }
        s.hp = rt.sora_hp;
        s.max_hp = rt.sora_target.max_hp;
        if game.autoplay.is_some() && game.tick == 500 {
            s.mp = s.max_mp / 2;
        }
    } else if game.autoplay.is_some() {
        match game.tick {
            420 => s.hp = s.max_hp * 7 / 10,
            500 => s.mp = s.max_mp / 2,
            640 => s.hp = s.max_hp / 5,
            900 => (s.hp, s.mp) = (s.max_hp, s.max_mp),
            _ => {}
        }
    } else {
        if keys.just_pressed(KeyCode::KeyH) {
            s.hp = (s.hp - 2).max(0);
        }
        if keys.just_pressed(KeyCode::KeyG) {
            s.hp = (s.hp + 2).min(s.max_hp);
        }
        if keys.just_pressed(KeyCode::KeyM) {
            s.mp = (s.mp - 10).max(0);
        }
        if keys.just_pressed(KeyCode::KeyN) {
            s.mp = (s.mp + 10).min(s.max_mp);
        }
        if keys.just_pressed(KeyCode::KeyB) {
            s.drive = (s.drive + 10).min(100);
        }
    }
    hud.stats = s;
    let Hud {
        sheets,
        gauge,
        menu,
        ..
    } = &mut *hud;
    if let Some(m) = menu {
        m.menu.update(&m.sheet.sequence);
    }
    gauge.update(&sheets.iter().map(|s| &s.sequence).collect::<Vec<_>>(), &s);
    hud.updated = true;
}

fn hud_present(
    hud: Option<ResMut<Hud>>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window>,
    mut overlay: Query<&mut Sprite, With<HudOverlay>>,
) {
    let Some(mut hud) = hud else { return };
    if let (Ok(window), Ok(mut sprite)) = (windows.single(), overlay.single_mut()) {
        let size = Vec2::new(window.width(), window.height());
        if sprite.custom_size != Some(size) {
            sprite.custom_size = Some(size);
        }
    }
    if !std::mem::take(&mut hud.updated) {
        return;
    }
    let Hud {
        sheets,
        gauge,
        canvas,
        image,
        menu,
        ..
    } = &mut *hud;
    let mut textures: Vec<Texture> = sheets
        .iter()
        .map(|s| Texture {
            rgba: &s.texture_rgba,
            size: s.texture_size.map(|v| v as usize),
            texel_scale: s.texel_scale as f32,
        })
        .collect();
    // The command menu (priorities 0x14b5..0x14b7) draws before the gauge
    // (7000).
    let mut draws: Vec<Draw> = Vec::new();
    if let Some(m) = menu {
        let first = textures.len();
        textures.push(Texture {
            rgba: &m.sheet.texture_rgba,
            size: m.sheet.texture_size.map(|v| v as usize),
            texel_scale: m.sheet.texel_scale as f32,
        });
        textures.push(Texture {
            rgba: &m.font.rgba,
            size: m.font.size.map(|v| v as usize),
            texel_scale: m.font.texel_scale as f32,
        });
        draws = m.menu.draw(
            &m.sheet.sequence,
            &command::Font {
                widths: &m.font.widths,
            },
            first,
            first + 1,
        );
    }
    draws.extend(gauge.draw(&sheets.iter().map(|s| &s.sequence).collect::<Vec<_>>()));
    canvas.render(&draws, &textures);
    if let Some(mut target) = images.get_mut(image.id()) {
        target.data = Some(canvas.rgba.clone());
    }
}

/// One tick of the Keyblade's attack windows against the enemy, then the
/// enemy's own tick. Attack spheres: weapon bone 2 entries of the window's
/// collision group, placed through Sora's hand and swept from last frame;
/// target spheres: the enemy's HIT entries on its animated bones.
fn combat_tick(game: &mut Game, tick: u64) {
    let Game {
        player,
        clips,
        clip_poses,
        poses,
        enemy,
        params,
        world,
        action_started,
        autoplay,
        log,
        ..
    } = game;
    let Some(rt) = enemy.as_mut() else { return };
    if autoplay.is_some() && tick == 380 {
        // Unattended runs place the target just ahead of Sora.
        let f = Vec3::new(player.yaw.sin(), 0., player.yaw.cos());
        let at = Vec3::from_array(player.position) + f * 110.;
        rt.enemy.position = at.to_array();
        rt.enemy.spawn = at.to_array();
    }
    let sora_at = Vec3::from_array(player.position);
    let sora_turn = Quat::from_rotation_y(player.yaw);
    let clip = poses[&(player.action, player.armed)];
    let frame = player.motion_frame(clips) as usize;
    let cp = &clip_poses[clip];
    let e = &rt.enemy;
    let targetable = e.targetable();
    let enemy_turn = Quat::from_rotation_y(e.yaw);
    let enemy_at = Vec3::from_array(e.position);
    let ec = &rt.clips.0[&e.motion];
    let bones = &ec.bones[(e.motion_tick as usize).min(ec.bones.len() - 1)];
    let body: Vec<(Vec3, f32, bool)> = rt
        .body
        .iter()
        .filter_map(|&(b, offset, r, high)| {
            let m = bones.get(&b)?;
            Some((enemy_at + enemy_turn * m.transform_point3(offset), r, high))
        })
        .collect();
    let mut hit = None;
    if targetable && !cp.blade.is_empty() {
        let f = frame.min(cp.blade.len() - 1);
        let to_model = cp.weapon[f.min(cp.weapon.len() - 1)] * cp.blade[f];
        for (w, &(start, end, atkp, group)) in cp.hits.iter().enumerate() {
            if (frame as u32) < start || frame as u32 > end {
                continue;
            }
            let instance = (*action_started << 4) | w as u64;
            for (i, &(offset, radius)) in rt.blade.get(&group).into_iter().flatten().enumerate() {
                let centre = sora_at + sora_turn * to_model.transform_point3(offset);
                let previous = rt.previous.insert((instance, i), centre);
                for c in combat::swept(centre.to_array(), previous.map(|p| p.to_array()), radius) {
                    if hit.is_none()
                        && let Some(&(_, _, high)) = body
                            .iter()
                            .find(|(b, r, _)| combat::spheres_overlap(c, radius, b.to_array(), *r))
                    {
                        hit = Some((instance as u32, atkp, high));
                    }
                }
            }
        }
    }
    if let Some((instance, atkp, high)) = hit
        && let Some(attack) = rt.attacks.get(&atkp)
    {
        let d = (enemy_at - sora_at) * Vec3::new(1., 0., 1.);
        let dir = d.normalize_or(Vec3::new(player.yaw.sin(), 0., player.yaw.cos()));
        if let Some(damage) = rt
            .enemy
            .hit(instance, dir.to_array(), high, attack, &rt.sora)
        {
            let record = serde_json::json!({"event":"hit","tick":tick,"atkp":atkp,"damage":damage,
                "enemy_hp":rt.enemy.hp,"reaction":format!("{:?}", rt.enemy.reaction),
                "act":format!("{:?}", rt.enemy.current_act()),"high":high});
            if let Some(file) = log.as_mut() {
                let _ = serde_json::to_writer(&mut *file, &record);
                let _ = std::io::Write::write_all(file, b"\n");
            }
        }
    }
    let _ = params;
    // The Shadow's claws against Sora's HIT spheres.
    let e = &rt.enemy;
    let ec = &rt.clips.0[&e.motion];
    let tick_in = e.motion_tick;
    let mut sora_hit = None;
    if e.alive() && !e.sunk {
        let bones = &ec.bones[(tick_in as usize).min(ec.bones.len() - 1)];
        let sora_bones = &cp.body[frame.min(cp.body.len().saturating_sub(1))];
        let sora_spheres: Vec<(Vec3, f32, bool)> = rt
            .sora_body
            .iter()
            .filter_map(|&(b, offset, r, high)| {
                let m = sora_bones.get(&b)?;
                Some((sora_at + sora_turn * m.transform_point3(offset), r, high))
            })
            .collect();
        for (w, &(start, end, atkp, group)) in ec.windows.iter().enumerate() {
            if tick_in < start || tick_in > end {
                continue;
            }
            let instance = ((e.motion_serial as u64) << 4) | w as u64;
            if rt.sora_hit_by.contains(&instance) {
                continue;
            }
            for (i, &(b, offset, radius)) in rt.claws.get(&group).into_iter().flatten().enumerate()
            {
                let Some(m) = bones.get(&b) else { continue };
                let centre = enemy_at + enemy_turn * m.transform_point3(offset);
                let previous = rt.claw_previous.insert((instance, i), centre);
                for c in combat::swept(centre.to_array(), previous.map(|p| p.to_array()), radius) {
                    if sora_hit.is_none()
                        && let Some(&(_, _, high)) = sora_spheres
                            .iter()
                            .find(|(b, r, _)| combat::spheres_overlap(c, radius, b.to_array(), *r))
                    {
                        sora_hit = Some((instance, atkp, high));
                    }
                }
            }
        }
    }
    if let Some((instance, atkp, high)) = sora_hit
        && let Some(attack) = rt.claw_attacks.get(&atkp)
        && rt.sora_hp > 0
    {
        rt.sora_hit_by.push(instance);
        let damage = combat::damage(&rt.shadow_attacker, attack, &rt.sora_target);
        rt.sora_hp = (rt.sora_hp - damage).max(0);
        let d = (sora_at - enemy_at) * Vec3::new(1., 0., 1.);
        let dir = d.normalize_or(Vec3::Z);
        let facing = Vec3::new(player.yaw.sin(), 0., player.yaw.cos());
        let back = facing.dot(dir) >= 0.;
        // Knock-back: KB1 0x7fff selects SSTM HitBackSmall (200), x 0.1.
        let kb = if attack.knockback[0] == 0x7fff {
            200.
        } else {
            attack.knockback[0] as f32
        } * 0.1;
        let index = (back as usize) * 2 + (!high) as usize;
        let started = player.hurt(index, [dir.x * kb, dir.z * kb], clips);
        if started.is_some() {
            *action_started = tick;
        }
        let record = serde_json::json!({"event":"sora_hit","tick":tick,"atkp":atkp,"damage":damage,
            "sora_hp":rt.sora_hp,"reaction":format!("{started:?}")});
        if let Some(file) = log.as_mut() {
            let _ = serde_json::to_writer(&mut *file, &record);
            let _ = std::io::Write::write_all(file, b"\n");
        }
    }
    if rt.sora_hp == 0 {
        rt.ko_ticks += 1;
        if rt.ko_ticks >= 120 {
            rt.sora_hp = rt.sora_target.max_hp;
            rt.ko_ticks = 0;
        }
    }
    let before = rt.enemy.current_act();
    rt.enemy.tick(
        &mut rt.rng,
        &rt.clips,
        &RoomFloor(world),
        player.position,
        player.position,
    );
    let after = rt.enemy.current_act();
    if before != after
        && let Some(file) = log.as_mut()
    {
        let record = serde_json::json!({"event":"enemy_act","tick":tick,"act":format!("{after:?}"),
            "motion":rt.enemy.motion,"position":rt.enemy.position});
        let _ = serde_json::to_writer(&mut *file, &record);
        let _ = std::io::Write::write_all(file, b"\n");
    }
}

fn present_enemy(
    game: Res<Game>,
    mut roots: Query<(&mut Transform, &mut Visibility), With<EnemyRoot>>,
    parts: Query<(&EnemyPart, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(rt) = &game.enemy else { return };
    let e = &rt.enemy;
    if let Ok((mut t, mut v)) = roots.single_mut() {
        t.translation = Vec3::from_array(e.position);
        t.rotation = Quat::from_rotation_y(e.yaw);
        t.scale = Vec3::new(1., e.scale_y.max(0.01), 1.);
        // Alpha fades are shown as visible/hidden at half alpha.
        *v = if e.gone || e.alpha < 0.5 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    let clip = &rt.clips.0[&e.motion];
    let pose = &clip.frames[(e.motion_tick as usize).min(clip.frames.len() - 1)];
    for (part, handle) in &parts {
        let positions: Vec<[f32; 3]> = rt.indices[part.0]
            .iter()
            .map(|&i| pose[i as usize].to_array())
            .collect();
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
    }
}

/// The weapon root, distinct from Sora's root transform.
type WeaponOnly = (With<WeaponRoot>, Without<SoraRoot>);
/// The camera never shares a transform with Sora's hierarchy.
type CameraOnly = (With<Camera3d>, Without<SoraRoot>, Without<WeaponRoot>);

fn camera_control(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut rig: ResMut<CameraRig>,
    mut camera: Query<&mut Transform, CameraOnly>,
) {
    if mouse.pressed(MouseButton::Right) {
        rig.pending_turn = (rig.pending_turn + motion.delta.x * 0.1).clamp(-30., 30.);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        rig.recenter_queued = true;
    }
    let eye = from_world(rig.camera.eye) * MIRROR;
    let look_at = from_world(rig.camera.look_at) * MIRROR;
    if let Ok(mut transform) = camera.single_mut() {
        *transform = Transform::from_translation(eye).looking_at(look_at, Vec3::Y);
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
        menu_up_queued: false,
        menu_down_queued: false,
        enemy: None,
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
    let mut enemy_model = None;
    if let Some(e) = loaded.enemy {
        let spawn = loaded.spawn + e.offset;
        let to_sora = loaded.spawn - spawn;
        let enemy = Shadow::new(spawn.to_array(), to_sora.x.atan2(to_sora.z), e.stats);
        game.event(serde_json::json!({"event":"enemy","hp":e.stats.max_hp,"defense":e.stats.defense,
            "damage_cap":e.stats.cap,"damage_min":e.stats.min,"sora_strength":e.sora.strength,
            "hit_spheres":e.body.len(),"blade_groups":e.blade.keys().collect::<Vec<_>>(),"position":spawn,
            "claw_groups":e.claws.iter().map(|(g, v)| (g.to_string(), v.len())).collect::<HashMap<_, _>>(),
            "claw_windows":e.clips.0.iter().filter(|(_, c)| !c.windows.is_empty()).map(|(s, c)| (s.to_string(), c.windows.clone())).collect::<HashMap<_, _>>(),
            "sora_hit_spheres":e.sora_body.len(),"sora_target":format!("{:?}", e.sora_target),"shadow_attacker":format!("{:?}", e.shadow_attacker)}));
        enemy_model = Some(e.model);
        game.enemy = Some(EnemyRuntime {
            clips: e.clips,
            body: e.body,
            blade: e.blade,
            attacks: e.attacks,
            sora: e.sora,
            enemy,
            rng: ScriptRandom::default(),
            claws: e.claws,
            claw_attacks: e.claw_attacks,
            shadow_attacker: e.shadow_attacker,
            sora_target: e.sora_target,
            sora_body: e.sora_body,
            sora_hit_by: Vec::new(),
            claw_previous: HashMap::new(),
            sora_hp: e.sora_target.max_hp,
            ko_ticks: 0,
            previous: HashMap::new(),
            indices: Vec::new(),
        });
    }
    let rig = CameraRig {
        camera: FollowCamera::new(
            camera::OUTDOOR,
            &camera_input(&game.player, game.player.position, game.params.run_speed),
        ),
        previous_position: game.player.position,
        pending_turn: 0.,
        recenter_queued: false,
        collision_hits: 0,
        collision: loaded
            .room
            .camera_triangles
            .iter()
            .map(|t| t.map(|p| to_world(p.to_array())))
            .collect(),
    };
    eprintln!(
        "Test room ready. WASD moves (Shift walks), Up/Down choose a command, Space jumps, Enter/K/left-click attacks, Q/E or right-drag turns the camera, -/= zoom, C recenters, Esc exits."
    );
    Ok(App::new()
        .insert_resource(game)
        .insert_resource(Startup {
            room: Some(loaded.room),
            arena_mesh: loaded.arena_mesh,
            actor: Some(loaded.actor),
            weapon: Some(loaded.weapon),
            hud: loaded.hud,
            enemy_model,
        })
        .insert_resource(rig)
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
        .add_systems(FixedUpdate, (simulate, hud_tick).chain())
        .add_systems(
            Update,
            (camera_control, present, present_enemy, hud_present, finish).chain(),
        )
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
