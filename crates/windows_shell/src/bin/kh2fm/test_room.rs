//! Playable source-bound test room: Sora walks/runs, jumps/lands and attacks
//! with motions read from the selected Steam installation.
//!
//! Every launch checks the selected source (executable and indexes rehashed,
//! each package byte read verified against the full-scan record), then decodes the
//! room, Sora's model/motion set and his PRTY/SSTM/PTYA records from it. The
//! controller (`player_sim`) is a test-room profile: data-backed where a
//! public lead exists, explicit choices elsewhere. It is not a measured
//! reproduction of the installed game's movement or combat rules.
use bevy::{
    app::AppExit,
    asset::RenderAssetUsages,
    camera::visibility::RenderLayers,
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
    bar, effect, model_inspection, model_inspection_textures::TextureMode, motion,
    object_collision, objentry, original, preferences, probe::ProbeSource, ptya, sklt,
};
use player_sim::shadow::{self, ScriptRandom, Shadow};
use player_sim::{
    Action, AttackStep, Clip, Clips, Input, Params, Player, TriangleWorld, World,
    camera::{self, CameraInput, FollowCamera},
};
use player_sim::{abilities, combat, magic, physics};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use windows_shell::camp::journal::{self, Curtain, Journal};
use windows_shell::camp::{self, Camp};
use windows_shell::command::{self, CommandMenu, Decision, Item};
use windows_shell::gauge::{Canvas, Draw, Gauge, GaugeStats, Texture};
use windows_shell::hud::{self, Sheet, SystemFont};
use windows_shell::input;
use windows_shell::layout2d::{self, Menu2ld};
use windows_shell::pad::{self, Pad};
use windows_shell::room::{CollisionTriangle, LoadedRoom, load_rooms, mesh, rgba_image, upload};
use windows_shell::sound::{self, Banks};
use windows_shell::ui2d::{Ui2dFrame, Ui2dPlugin};

#[derive(Deserialize)]
struct Locator {
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
}
mod magic_room;

fn player_object_default() -> String {
    "P_EX100".into()
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
    /// Superseded: the attack is chosen from the whole PTYA set
    /// (player_sim::ptya); kept only to read older configurations.
    #[serde(default)]
    ground_combo_ids: Vec<u8>,
    #[serde(default)]
    air_combo_ids: Vec<u8>,
    system: Locator,
    /// The player's own 00objentry model name; its +0x4e PRTY index selects
    /// the movement record (FUN_1403da940 → FUN_14040a480).
    #[serde(default = "player_object_default")]
    player_object_name: String,
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
    /// Sound banks: `se/seNNN.win32.scd` entries, or object archives with
    /// the name of their `se/obj/seNNN` replacement asset.
    #[serde(default)]
    sounds: Vec<BankSource>,
    /// Effect sources: object archives holding the Keyblade's and the
    /// enemy's PAX, and `effect/texcommon.dpd` (the common atlas).
    #[serde(default)]
    effects: Option<EffectConfig>,
    /// A field NPC to talk to (needs `menu` for the reaction label and
    /// `system` for the text colour).
    #[serde(default)]
    npc: Option<NpcConfig>,
    /// The camp (pause) menu: menu/<lang>/camp.2ld (needs `menu`).
    #[serde(default)]
    camp: Option<Locator>,
    /// Jiminy's Journal (needs `camp`).
    #[serde(default)]
    journal: Option<JournalConfig>,
    /// The room's field and battle music (03system 'arif': tt08 set 0 =
    /// bgm/music118 and music119).
    #[serde(default)]
    music: Option<RoomMusic>,
}

/// The journal's files: menu/<lang>/jmemo.2ld (the book), jiminy.bar (the
/// tables), msg/<lang>/jm.bar (message slot 0x65), menu/jm_world.2ld (the
/// world icons), menu/jm_top_<code>.2ld by code and the menu-chain curtain
/// menu/<lang>/pause.2ld.
#[derive(Deserialize)]
struct JournalConfig {
    book: Locator,
    tables: Locator,
    messages: Locator,
    worlds: Locator,
    tops: std::collections::BTreeMap<String, Locator>,
    curtain: Locator,
}

#[derive(Deserialize)]
struct RoomMusic {
    field: Locator,
    battle: Locator,
}

/// Field-talk test NPC. In vanilla no field talk has Axel speaking, so
/// three values are composed (dialogue-01 §7B): the talk reaction (cmd 31
/// via the towns-person reaction sphere), the talk camera shape (the TARGET
/// shape of `target_model`) and the script (`script`, libretto op 1 per
/// message id).
#[derive(Deserialize)]
struct NpcConfig {
    /// Model selection (relative to this configuration): obj/B_EX110_RTN.mdlx.
    model_selection: PathBuf,
    /// Its motion set (direct slots; slot 0 is the idle 'R00A').
    motion_set: Locator,
    /// Archive whose object collision has the TARGET shape used for the
    /// camera (obj/B_EX110.mdlx shares the skeleton).
    target_model: Locator,
    /// Placement relative to Sora's spawn (scene units) and facing (rad).
    offset: [f32; 3],
    yaw: f32,
    /// Message archive with the script's lines (msg/us/hb.bar for Axel).
    messages: Locator,
    script: Vec<u32>,
    /// msg/<lang>/fontinfo.bar, fontimage.bar and the world message bar.
    font_info: Locator,
    font_image: Locator,
    world_messages: Locator,
    /// World index into the ftst colour table (tt = 2).
    world: usize,
    /// Composed talk-camera shape (the type-19 role) used instead of the
    /// TARGET fallback: the towns-person dimensions on the head bone.
    #[serde(default)]
    camera_shape: Option<ShapeConfig>,
}

#[derive(Deserialize)]
struct ShapeConfig {
    bone: usize,
    offset: [f32; 3],
    pos_h: f32,
    radius: f32,
    height: f32,
}

#[derive(Deserialize)]
struct EffectConfig {
    weapon: Locator,
    #[serde(default)]
    enemy: Option<Locator>,
    atlas: Locator,
    /// 00effect.bar (the system PAX: guard and clash sparks).
    #[serde(default)]
    system: Option<Locator>,
}

struct EffectSources {
    weapon: effect::Pax,
    enemy: Option<effect::Pax>,
    system: Option<effect::Pax>,
    atlas: windows_shell::effects::Atlas,
}

/// Effects raised by combat this tick.
enum EffectRequest {
    /// Keyblade hit spark (FUN_1403d1b40): ATKP hit effect and random
    /// count, contact point and hit direction in scene coordinates.
    Hit {
        effect: u8,
        random: u8,
        contact: Vec3,
        direction: Vec3,
    },
    /// The enemy's `dead` act: effect_start_bind(obj, 2).
    EnemyDead,
    /// A system (00effect) element at a point, unrotated
    /// (FUN_1403d0f00 clash effects).
    System { id: u16, at: Vec3 },
}

struct RunningEffect {
    instance: effect_vm::Instance,
    label: &'static str,
    /// Element transform kept for effects bound to the enemy.
    bound: Option<effect::Element>,
}

/// Running effect instances and the quads they drew this tick.
#[derive(Resource)]
struct Effects {
    weapon: Option<Arc<effect::Pax>>,
    enemy: Option<Arc<effect::Pax>>,
    system: Option<Arc<effect::Pax>>,
    running: Vec<RunningEffect>,
    rng: effect_vm::Rng,
    quads: Vec<effect_vm::Quad>,
    /// The view matrix the quads were drawn with, and its inverse.
    view_inverse: Mat4,
    uv_scale: [f32; 2],
    reported: Vec<u32>,
}

#[derive(Component)]
struct EffectMesh {
    additive: bool,
}

#[derive(Deserialize)]
struct BankSource {
    #[serde(flatten)]
    locator: Locator,
    #[serde(default)]
    replacement: Option<String>,
}

#[derive(Deserialize)]
struct ArenaConfig {
    half_size: f32,
    wall_height: f32,
    tile: f32,
}

fn standard_difficulty() -> u8 {
    1
}

#[derive(Deserialize)]
struct EnemyConfig {
    /// DAT_1409abd48: 0 Beginner, 1 Standard (default), 2 Proud, 3 Critical.
    #[serde(default = "standard_difficulty")]
    difficulty: u8,
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

/// Sora's damage motions (10..19, Aerial Recovery 40, DEAD 54), in
/// `Clips::hurt` order.
const SORA_HURT: [u16; 12] = player_sim::HURT_MOTIONS;

/// Guard (PTYA 0x0b) and its success recoil (guard-01).
const SORA_GUARD: [u16; 2] = [173, 42];

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
const STEP_HEIGHT: f32 = 24.;
/// Run at stick magnitude 0.8 (DAT_140585c7c).
const WALK_THRESHOLD: f32 = physics::RUN_MAGNITUDE;
const BODY_RADIUS: f32 = 30.;
const BODY_HEIGHT: f32 = 140.;
const FLOOR_MIN_NORMAL_Y: f32 = 0.5;
/// Test-room choice: put the weapon away after 6 s without attacking.
const DISARM_TICKS: u32 = 360;
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
    /// Weapon motion frame trigger 8 ("play SE"): (clip tick, sound id).
    sounds: Vec<(u32, u32)>,
}

/// Frame trigger 8 (FUN_14040dd70): id = p0·1000 + p1; with more than
/// three parameters and p3 set, only when p0 is the equipped weapon's bank.
fn sound_triggers(selected: &motion::SelectedMotion, weapon_bank: i16) -> Vec<(u32, u32)> {
    selected
        .triggers
        .iter()
        .flat_map(|t| &t.records.frames)
        .filter(|f| f.trigger == 8 && f.parameters.len() >= 2)
        .filter(|f| {
            f.parameters.len() <= 3 || f.parameters[3] == 0 || f.parameters[0] == weapon_bank
        })
        .filter(|f| f.parameters[0] >= 0 && f.parameters[1] >= 0)
        .map(|f| {
            (
                f.frame.max(0) as u32,
                f.parameters[0] as u32 * 1000 + f.parameters[1] as u32,
            )
        })
        .collect()
}

/// Kingdom Key sound bank (its SEB stub's bank number, sound-01 §1.1).
const WEAPON_BANK: i16 = 500;
/// System bank (se000, engine slot 2) sounds: cursor move, low-HP alarm.
const SE_CURSOR: u32 = 1;
const SE_LOW_HP: u32 = 9;

#[derive(Resource)]
struct Game {
    params: Params,
    /// Builds the player's ability record from equipped slots.
    ability_source: AbilitySource,
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
    /// The room's pad record (logical bits, pad mode 1 in the field) and
    /// this tick's PADDATA (stick bytes).
    pad: Pad,
    pad_data: pad::PadData,
    jump_queued: bool,
    attack_queued: bool,
    guard_queued: bool,
    /// L1-held shortcut presses of this tick (bit k = slot k) and the
    /// shortcut command ids (camp Customize).
    shortcut_queued: u8,
    shortcuts: [u16; 4],
    /// The back button (Circle, logical bit 3) pressed this tick.
    back_queued: bool,
    /// Test-room aid (F4): a blow-reaction hit with no damage, to try the
    /// blow, Aerial Recovery and Retaliating Slash without a blowing enemy.
    debug_blow: bool,
    /// Sora's magic: owned spells, running casts and their volumes.
    magic: Option<magic_room::MagicRt>,
    /// Guard's ability is equipped (the camp's Abilities screen sets it).
    guard_equipped: bool,
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
    /// Sound requests raised this tick: (id, cause).
    sound_queue: Vec<(u32, &'static str)>,
    /// Action start tick and clip tick already scanned for sound triggers.
    sound_frame: Option<(u64, u32)>,
    effect_queue: Vec<EffectRequest>,
    /// Field talk: control locked, and this tick's reaction / advance
    /// presses (Triangle; decide, back or Triangle).
    talking: bool,
    rc_queued: bool,
    advance_queued: bool,
    /// Unattended capture ticks and end for this room.
    captures: &'static [u64],
    autoplay_end: u64,
}

/// Sound output and banks; requests resolve by id (bank·1000 + sound).
#[derive(Resource)]
struct Audio {
    output: Option<audio_out::Output>,
    banks: Banks,
    alarm: bool,
}

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

/// Clip timing and honoured range triggers from one decoded motion.
/// Clip timing relative to FrameStart, plus the key time of FrameStart.
/// Loads the ATKP rows of `id` in sub-table `sub` into the runtime (and the
/// id's base row and hit sound for the clash rules) on first use.
fn ensure_attack_rows(rt: &mut EnemyRuntime, id: u16, sub: u8) -> bool {
    if rt.steps.contains_key(&(id, sub)) {
        return true;
    }
    match package_reader::battle_tables::attack_steps_sub(&rt.atkp, id, sub) {
        Ok(rows) => {
            let first = rows[0];
            rt.rows.entry(id).or_insert(first);
            rt.hit_sounds.entry(id).or_insert(first.hit_sound);
            rt.steps.insert((id, sub), rows);
            true
        }
        Err(_) => false,
    }
}

/// The damage routine's view of an ATKP row.
fn params_of(a: &package_reader::battle_tables::Attack) -> combat::AttackParams {
    combat::AttackParams {
        power: a.power,
        stat: a.stat,
        element: a.element,
        reaction: a.reaction,
        knockback: a.knockback,
        attack_type: a.kind,
        combo_kind: a.combo_kind,
        flags: a.flags,
    }
}

/// A PTYA record for the attack chooser (the 0x44 bytes as read).
fn ptya_record(e: &ptya::PtyaEntry) -> player_sim::ptya::Record {
    let f = |k: usize| f32::from_bits(e.float_bits[k]);
    player_sim::ptya::Record {
        id: e.id,
        kind: e.kind,
        alternate: e.sub,
        combo_offset: e.combo_offset,
        flags: e.flag,
        motion: e.motion_id,
        blend_arg: e.next_motion_id,
        jump: f(0),
        jump_max: f(1),
        jump_min: f(2),
        speed_min: f(3),
        speed_max: f(4),
        near: f(5),
        far: f(6),
        high: f(7),
        low: f(8),
        cos_min: f(9),
        cos_max: f(10),
        blend: f(11),
        reach: f(12),
        ability: e.ability,
        w42: e.score,
    }
}

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
            50 => clip.finisher_windows.push((start, stop)),
            2 => {
                clip.blend_to_idle = Some(clip.blend_to_idle.map_or(start, |b: u32| b.min(start)));
                clip.actionable.push((start, stop));
            }
            0 | 1 => clip.actionable.push((start, stop)),
            3 => clip.no_gravity.push((start, stop)),
            27 => clip.invulnerable.push((start, stop)),
            42 => clip.inertia.push((start, stop)),
            // 28 turns toward the lock-on target, 22 toward the command
            // target; one enemy here, so they are the same.
            28 | 22 => {
                if let Some(&deg) = record.parameters.first() {
                    clip.turn_to_target.push((start, stop, deg as f32));
                }
            }
            _ => {}
        }
    }
    for record in selected.triggers.iter().flat_map(|t| &t.records.frames) {
        let frame = record.frame.max(0) as u32;
        match record.trigger {
            0 => clip.takeoff = Some(clip.takeoff.map_or(frame, |f: u32| f.min(frame))),
            21 => {
                if let Some(&deg) = record.parameters.first() {
                    clip.face_target.push((frame, deg as f32));
                }
            }
            // Spell release (FUN_14040dd70 case 0x11), filtered by the
            // motion id when it has an argument.
            17 => clip
                .release
                .push((frame, record.parameters.first().map(|&m| m as u16))),
            _ => {}
        }
    }
    Ok((clip, start))
}

/// What builds the battle ability record (stats +0x1d0): the SSTM adders,
/// each ability item's bit (+4) and growth flag (+6 non-zero), and the
/// equipped weapon's ability item.
struct AbilitySource {
    rates: abilities::Rates,
    items: HashMap<u16, (u16, bool)>,
    weapon: u16,
}

impl AbilitySource {
    /// FUN_1403c1d90's ability part: reset (FUN_1404016b0), the weapon's
    /// stat-row ability (FUN_1403c1190), then the equipped slots
    /// (FUN_1404014a0).
    fn build(&self, slots: &[u16]) -> abilities::Abilities {
        let mut a = abilities::Abilities::default();
        if let Some(&(bit, _)) = self.items.get(&self.weapon) {
            a.add(self.weapon, bit, &self.rates);
        }
        a.equip(
            slots,
            None,
            |id| self.items.get(&id).copied(),
            &abilities::GROWTH,
            &self.rates,
        );
        a
    }
}

struct Loaded {
    room: LoadedRoom,
    arena_mesh: Option<windows_shell::room::ArenaMesh>,
    actor: model_inspection::InspectionModel,
    weapon: model_inspection::InspectionModel,
    params: Params,
    /// SSTM battle parameters (blow, hit-back, fall max).
    sstm: physics::Sstm,
    abilities: AbilitySource,
    magic: Option<magic_room::MagicSource>,
    clips: Clips,
    actions: Vec<((Action, bool), usize)>,
    clip_poses: Vec<ClipPoses>,
    spawn: Vec3,
    world: TriangleWorld,
    summary: serde_json::Value,
    hud: Option<HudSource>,
    enemy: Option<EnemySource>,
    banks: Banks,
    effects: Option<EffectSources>,
    npc: Option<NpcSource>,
    camp: Option<CampSource>,
    journal: Option<JournalSource>,
    /// Field and battle music SCD bytes.
    music: Option<[Vec<u8>; 2]>,
}

/// The journal's art, tables and messages.
struct JournalSource {
    book: Menu2ld,
    curtain: Menu2ld,
    world_icons: Vec<layout2d::Texture>,
    tops: Vec<(String, Vec<layout2d::Texture>)>,
    worlds: Vec<package_reader::journal_table::World>,
    stories: Vec<package_reader::journal_table::Story>,
    texts: HashMap<u16, Vec<u8>>,
}

fn load_journal(
    source: &ProbeSource,
    j: &JournalConfig,
    sys: &Locator,
) -> Result<JournalSource, Box<dyn std::error::Error>> {
    use package_reader::journal_table;
    use windows_shell::camp::journal;
    let two_ld = |l: &Locator| layout2d::load(source, &l.index, l.ordinal, l.name_hash);
    let images = |l: &Locator| layout2d::load_images(source, &l.index, l.ordinal, l.name_hash);
    let book = two_ld(&j.book)?;
    let curtain = two_ld(&j.curtain)?;
    let world_icons = images(&j.worlds)?;
    let mut tops = Vec::new();
    for (code, l) in &j.tops {
        tops.push((code.clone(), images(l)?));
    }
    let tables = read_archive(source, &j.tables)?;
    let worlds = journal_table::read_worlds(bar_entry(&tables, b"worl")?)?;
    let stories = journal_table::read_stories(bar_entry(&tables, b"stor")?)?;
    let mut ids: Vec<u32> = journal::MESSAGES.iter().map(|&i| i as u32).collect();
    for w in &worlds {
        ids.extend([w.names[0], w.alt_names[0]].map(u32::from));
    }
    for s in &stories {
        ids.extend([s.summary, s.objective].map(u32::from));
    }
    ids.retain(|&i| i != 0);
    ids.sort_unstable();
    ids.dedup();
    let found = hud::load_messages_optional(
        source,
        &j.messages.index,
        j.messages.ordinal,
        j.messages.name_hash,
        &ids,
    )?;
    let mut texts: HashMap<u16, Vec<u8>> = ids
        .iter()
        .zip(found)
        .filter_map(|(&id, m)| m.map(|m| (id as u16, m)))
        .collect();
    // 0x8adc is the system message 0xadc.
    let fake = hud::load_messages(
        source,
        &sys.index,
        sys.ordinal,
        sys.name_hash,
        &[journal::NO_STORY as u32],
    )?;
    texts.insert(
        journal::NO_STORY,
        fake.into_iter().next().unwrap_or_default(),
    );
    Ok(JournalSource {
        book,
        curtain,
        world_icons,
        tops,
        worlds,
        stories,
        texts,
    })
}

/// Camp (pause) menu sources: camp.2ld, the entries' labels and help
/// lines (sys.bar), and the system font.
struct CampSource {
    menu: Menu2ld,
    entries: Vec<camp::Entry>,
    font: SystemFont,
    /// "LV", "HP", "MP" and Sora's name (0xb9fc..0xb9fe, 0x851f).
    labels: [Vec<u8>; 3],
    name: Vec<u8>,
    /// Sora's Status help line and the 0x18 banner.
    help: Vec<u8>,
    banner: Vec<u8>,
    /// Status stats: bit, label, help, style.
    stats: Vec<(usize, Vec<u8>, Vec<u8>, usize)>,
    config: Vec<camp::ConfigRow>,
    numbers: SoraNumbers,
    /// The item table with its texts and the Items screens' messages.
    items: camp::items::ItemsData,
    /// The Abilities screens' messages and the Kingdom Key's ability.
    abilities: camp::abilities::AbilityTexts,
    weapon_ability: u16,
    party_help: [Vec<u8>; 3],
    customize: camp::customize::CustomizeTexts,
    equip_texts: camp::equip::EquipTexts,
    equip_stats: Vec<camp::equip::ItemStats>,
    /// The font image's icon plane (0x09 icons).
    icons: SystemFont,
}

/// Items screen messages (camp-05 §9).
const ITEM_MESSAGES: [u32; 19] = [
    0x8455, 0x845a, 0x8456, 0x8457, 0x8448, 0xb943, 0xb944, 0xbb86, 0xbb87, 0xb90c, 0xb90f, 0xb90d,
    0xb90e, 0xb910, 0xb94f, 0xbb90, 0x9fcf, 0x9fd0, 0x9fd1,
];

/// The room's stock and Sora's equipment [test-room values: a new save's
/// inventory is not decoded]: 3 Potions, a Hi-Potion, 2 Ethers; the
/// Kingdom Key with one empty armor, accessory and two item slots.
const ROOM_STOCK: [(u16, u8); 9] = [
    (1, 3),
    (2, 1),
    (3, 2),
    // The element items (type 0x12): Fire, Blizzard, Thunder, Cure, Magnet
    // and Reflect at their top level [test-room values: a new save has
    // none].
    (0x15, 3),
    (0x16, 3),
    (0x17, 3),
    (0x18, 3),
    (0x57, 3),
    (0x58, 3),
];
/// The element items by magic type (Fire, Blizzard, Thunder, Cure,
/// Magnet, Reflect).
const ELEMENT_ITEMS: [u16; 6] = [0x15, 0x16, 0x17, 0x18, 0x57, 0x58];
/// Shortcut slots of a fresh room [test-room values]: Fire, Blizzard,
/// Thunder and Cure by their level-1 command ids; Customize changes them.
const ROOM_SHORTCUTS: [u16; 4] = [0x31, 0x33, 0x32, 0x34];

/// Equip messages (camp-09 §10): sections, None, no items, unequip help,
/// the two boxes, the info labels.
const EQUIP_MESSAGES: [u32; 22] = [
    0x845b, 0x845d, 0x8463, 0x8465, 0x8467, 0x8449, 0x844a, 0xcb8d, 0xcb8e, 0x8469, 0x846a, 0xc22e,
    0xc648, 0xc649, 0xc64a, 0xc64b, 0xc651, 0xc652, 0xc653, 0xc654, 0xc64c, 0,
];

/// Customize messages (camp-07 §1.3 / §7): help, category labels,
/// shortcut headings, remove help, MP Charge and Auto-Reload choices and
/// help, the Auto-Reload warning.
const CUSTOMIZE_MESSAGES: [u32; 19] = [
    0xbb89, 0xbb8a, 0x847d, 0x8481, 0x8482, 0x847d, 0x847e, 0x847f, 0x8480, 0x9df7, 0x9df9, 0x9df8,
    0x9dfb, 0x9dfa, 0x9dfd, 0x9dfc, 0x9dff, 0x9dfe, 0x846a,
];

/// Abilities screen messages (camp-06 §7).
const ABILITY_MESSAGES: [u32; 15] = [
    0xbb8b, 0xbb8c, 0xb2ee, 0xb2ef, 0xb2f0, 0xb2ed, 0xb372, 0xb629, 0xb627, 0xb62a, 0xb628, 0xb373,
    0xb619, 0xb61a, 0x8454,
];

/// Sora's abilities [test-room values: a new save's slots are not
/// decoded]: Guard (0x52) equipped, as the guard-01 recipe asks, and the
/// combo abilities unequipped so they can be tried from the camp: the
/// Square abilities, the combo finishers and starters (PTYA ability bits
/// 90–101, 177–180, 18), the combo supports, and the damage supports
/// (Defender, Second Chance, Once More, the element boosts, MP Rage).
const ROOM_ABILITIES: [u16; 35] = [
    0x8052, 0x809e, 0x0089, 0x010f, 0x010b, 0x0111, 0x010c, 0x0106, 0x0107, 0x022f, 0x0108, 0x0232,
    0x0109, 0x010a, 0x010d, 0x0230, 0x010e, 0x0110, 0x0231, 0x00a2, 0x00a3, 0x0186, 0x0187, 0x0188,
    0x0189, 0x018a, 0x018b, 0x021b, 0x019e, 0x019f, 0x01a0, 0x0198, 0x0199, 0x019a, 0x019c,
];
/// [test-room value] AP added to Sora's level-1 AP so the room's abilities
/// fit (the Critical start record's 50 plus Aerial Recovery's 2).
const ROOM_EXTRA_AP: i32 = 50;
/// The Kingdom Key (its stat record gives the weapon ability).
const KINGDOM_KEY: u16 = 0x29;

fn load_camp_abilities(
    system: &[u8],
    msg: impl Fn(&[u32]) -> Result<Vec<Option<Vec<u8>>>, Box<dyn std::error::Error>>,
) -> Result<(camp::abilities::AbilityTexts, u16), Box<dyn std::error::Error>> {
    let item = bar_entry(system, b"item")?;
    let table = package_reader::item_table::read(item)?;
    let stats = package_reader::item_table::read_stats(item)?;
    // FUN_1403e3ef0: weapons (types 2..13) use the stat entry at +6.
    let weapon_ability = table
        .iter()
        .find(|r| r.id == KINGDOM_KEY)
        .and_then(|r| stats.iter().find(|s| s.id == r.weapon_stats))
        .map_or(0, |s| s.ability);
    let mut t = msg(&ABILITY_MESSAGES)?
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect::<Vec<_>>()
        .into_iter();
    let mut take = || t.next().unwrap_or_default();
    let texts = camp::abilities::AbilityTexts {
        help: [take(), take()],
        categories: [take(), take(), take(), take()],
        empty: take(),
        choices: [take(), take()],
        choice_help: [take(), take()],
        items_banner: take(),
        abilities_banner: take(),
        switch: take(),
        triangle: take(),
    };
    Ok((texts, weapon_ability))
}

/// 03system `item` with names and descriptions, and the screens' texts.
fn load_camp_items(
    system: &[u8],
    msg: impl Fn(&[u32]) -> Result<Vec<Option<Vec<u8>>>, Box<dyn std::error::Error>>,
) -> Result<camp::items::ItemsData, Box<dyn std::error::Error>> {
    use camp::items::{ItemInfo, ItemTexts, ItemsData};
    let table = package_reader::item_table::read(bar_entry(system, b"item")?)?;
    // Consumables' commands (FUN_1403c41c0: type 0 → record +0x10) and the
    // commands' names (03system `cmd` +8).
    let commands = package_reader::command_table::read(bar_entry(system, b"cmd\0")?)?;
    let command_name = |r: &package_reader::item_table::Item| -> u32 {
        if r.kind != 0 {
            return 0;
        }
        commands
            .iter()
            .find(|c| c.id == r.command)
            .map_or(0, |c| c.name)
    };
    let ids: Vec<u32> = table
        .iter()
        .flat_map(|r| [r.name as u32, r.description as u32, command_name(r)])
        .collect();
    let texts = msg(&ids)?;
    let table = table
        .iter()
        .zip(texts.chunks(3))
        .map(|(r, t)| ItemInfo {
            command: if r.kind == 0 { r.command } else { 0 },
            command_name: t[2].clone().unwrap_or_default(),
            id: r.id,
            kind: r.kind,
            amount: r.amount,
            sell: r.sell,
            icon: r.icon,
            param: r.weapon_stats,
            name: t[0].clone().unwrap_or_default(),
            description: t[1].clone().unwrap_or_default(),
        })
        .collect();
    let fixed: Vec<u32> = ITEM_MESSAGES.to_vec();
    let mut f = msg(&fixed)?
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect::<Vec<_>>()
        .into_iter();
    let mut take = || f.next().unwrap_or_default();
    // Category glyph messages (0x1405ba790): 0xb9c7 + t for t ≤ 0x11,
    // 0xbe2b for type 0x14.
    let category_ids: Vec<u32> = (0..=0x14u32)
        .map(|t| match t {
            0..=0x11 => 0xb9c7 + t,
            0x14 => 0xbe2b,
            _ => 0,
        })
        .collect();
    let categories = msg(&category_ids)?
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect();
    let texts = ItemTexts {
        stock: take(),
        empty: take(),
        use_: take(),
        throw: take(),
        blank: take(),
        use_on: take(),
        all: take(),
        help: [take(), take()],
        tabs: [take(), take(), take(), take(), take()],
        throw_query: take(),
        no_effect: take(),
        ok: take(),
        yes: take(),
        no: take(),
        categories,
    };
    Ok(ItemsData {
        table,
        texts,
        ..ItemsData::default()
    })
}

/// Sora's Status numbers the room does not track elsewhere.
#[derive(Clone, Copy)]
struct SoraNumbers {
    next_exp: u32,
    ap: i32,
    strength: i32,
    magic: i32,
    defense: i32,
}

/// Status stats Sora shows with nothing obtained (camp-04 §2.4): bit,
/// label id, help id, style.
const CAMP_STATS: [(usize, u32, u32, usize); 14] = [
    (0, 0xc559, 0xc56e, 0),
    (1, 0xc55a, 0xc56f, 0),
    (2, 0xc55b, 0xc570, 0),
    (3, 0xc55c, 0xc571, 1),
    (4, 0xc55d, 0xc572, 1),
    (5, 0xc55e, 0xc573, 1),
    (7, 0xc567, 0xc57c, 4),
    (10, 0xc560, 0xc575, 2),
    (11, 0xc561, 0xc576, 2),
    (12, 0xc562, 0xc577, 2),
    (13, 0xc563, 0xc578, 3),
    (14, 0xc564, 0xc579, 3),
    (15, 0xc565, 0xc57a, 3),
    (16, 0xc566, 0xc57b, 3),
];

/// Config rows (table 0x1405bc340): label, choice labels, help per choice.
const CAMP_CONFIG: [(u32, &[u32], &[u32]); 8] = [
    (0xb717, &[0xb71e, 0xb71f], &[0xb720, 0xb721]),
    (0xc2f5, &[0xc2f8, 0xc2f9], &[0xc2fa, 0xc2fb]),
    (0xc2f6, &[0xc2fc, 0xc2fd], &[0xc2fe, 0xc2ff]),
    (0xc2f7, &[0xc302, 0xc300, 0xc301], &[0xc305, 0xc303, 0xc304]),
    (0xb719, &[0xb726, 0xb727], &[0xb728, 0xb729]),
    (0xb71a, &[0xb72a, 0xb752], &[0xb72c, 0xb72d]),
    (0xb71c, &[0xb734, 0xb735], &[0xb736, 0xb737]),
    (
        0xb71d,
        &[0xb738, 0xb739, 0xb73a, 0xce30],
        &[0xb73b, 0xb73c, 0xb73d, 0xce31],
    ),
];

/// Camp entries (table 0x1405badb0): id, label and help ids. The test
/// room offers Items, Abilities, Customize, Party, Status, Jiminy's
/// Journal and Config [test-room choice: the menu flags come from a save].
const CAMP_ENTRIES: [(u8, u32, u32); 7] = [
    (0, 0x844b, 0xb70d),
    (1, 0x844d, 0xb70f),
    (2, 0x8451, 0xb713),
    (3, 0x844e, 0xb710),
    (4, 0x844f, 0xb711),
    (5, 0x8450, 0xb712),
    (7, 0xb617, 0xb715),
];

/// One enemy clip: skinned positions, HIT-bone matrices and root
/// translation per tick, plus its loop start.
struct EnemyClip {
    frames: Vec<Vec<Vec3>>,
    bones: Vec<HashMap<usize, Mat4>>,
    root: Vec<Vec3>,
    loop_start: u32,
    /// Attack windows (range trigger 10): start, end, ATKP id, group.
    windows: Vec<(u32, u32, u16, u8)>,
    /// Frame trigger 8 sounds: (tick, sound id).
    sounds: Vec<(u32, u32)>,
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
    targets: Vec<(usize, Vec3, f32, f32)>,
    blade: HashMap<u8, Vec<(Vec3, f32)>>,
    /// Every ATKP step row by (id, sub-table); FUN_1403eb230 picks one per
    /// hit count. Spell rows are added when a spell first hits.
    steps: HashMap<(u16, u8), Vec<package_reader::battle_tables::Attack>>,
    /// The ATKP table, for spell rows by sub-table.
    atkp: Vec<u8>,
    sora: combat::AttackerStats,
    enemy: Shadow,
    rng: ScriptRandom,
    claws: HashMap<u8, Vec<(usize, Vec3, f32)>>,
    shadow_attacker: combat::AttackerStats,
    sora_target: combat::TargetStats,
    sora_body: Vec<(usize, Vec3, f32, bool)>,
    /// Summed TARGET radii for aerial hops (FUN_1403d7520).
    target_reach: f32,
    /// Sora's hit record (obj+0xc98): the claws' hit-once mask and the
    /// 30-tick group lock of a player-type object.
    sora_record: combat::HitRecord,
    /// Attack ids and combo groups of live attack instances.
    ids: combat::AttackIds,
    instance_ids: HashMap<u64, (u8, i32)>,
    next_group: i32,
    difficulty: combat::Difficulty,
    claw_previous: HashMap<(u64, usize), Vec3>,
    /// Sora's HP and KO timer (test room refills after a KO).
    sora_hp: i32,
    ko_ticks: u32,
    /// Last frame's attack sphere centres by (window instance, sphere).
    previous: HashMap<(u64, usize), Vec3>,
    indices: Vec<Vec<u32>>,
    /// ATKP HitSound by id (Keyblade, then claws).
    hit_sounds: HashMap<u16, i32>,
    /// ATKP hit effect and its random count by id (Keyblade).
    hit_effects: HashMap<u16, (u8, u8)>,
    /// Full ATKP rows (Keyblade, guard and claws) for the clash rules.
    rows: HashMap<u16, package_reader::battle_tables::Attack>,
    /// Sora's invulnerability after a guard (+0xd70), in ticks.
    sora_invulnerable: f32,
    /// Motion serial and tick already scanned for sound triggers.
    sound_tick: Option<(u32, u32)>,
}

/// A placeholder for a spell's hit volume: the spell effects (the `.mag`
/// PAX containers) are not drawn yet, so each live ATTACK_MAGIC volume shows
/// as a translucent shape tinted by its element.
#[derive(Component)]
struct SpellVolume(usize);

/// Tints by ATKP id: fire, blizzard, thunder, cure, magnet, reflect.
fn spell_tint(atkp: u16) -> [f32; 4] {
    match atkp {
        502 => [1., 0.45, 0.1, 0.35],
        248 => [0.5, 0.85, 1., 0.45],
        249..=251 => [1., 0.95, 0.3, 0.35],
        708 | 709 => [0.3, 1., 0.5, 0.35],
        658 | 976 | 1440 => [0.7, 0.4, 1., 0.3],
        573 | 1247 => [0.9, 0.9, 1., 0.3],
        574..=578 | 667..=669 => [0.9, 0.5, 1., 0.4],
        _ => [1., 1., 1., 0.3],
    }
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
    /// TARGET shapes (type 3, enabled): bone, offset, radius, height.
    targets: Vec<(usize, Vec3, f32, f32)>,
    /// Keyblade attack spheres by collision group: offset, radius.
    blade: HashMap<u8, Vec<(Vec3, f32)>>,
    steps: HashMap<(u16, u8), Vec<package_reader::battle_tables::Attack>>,
    atkp: Vec<u8>,
    difficulty: combat::Difficulty,
    sora: combat::AttackerStats,
    stats: combat::TargetStats,
    offset: Vec3,
    claws: HashMap<u8, Vec<(usize, Vec3, f32)>>,
    shadow_attacker: combat::AttackerStats,
    sora_target: combat::TargetStats,
    sora_body: Vec<(usize, Vec3, f32, bool)>,
    /// Summed TARGET radii for aerial hops (FUN_1403d7520).
    target_reach: f32,
    hit_sounds: HashMap<u16, i32>,
    hit_effects: HashMap<u16, (u8, u8)>,
    rows: HashMap<u16, package_reader::battle_tables::Attack>,
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
    if config.schema_version != 1 || config.source_content_identity != source.content_identity() {
        return Err("test room configuration identity/schema invalid".into());
    }
    let base = config_path.parent().ok_or("configuration directory")?;
    // The session check (executable and indexes rehashed, every package
    // byte read verified by chunk) admits this session.
    source.verify_session()?;
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

    // Movement block (obj+0x128): FUN_14040a440 defaults with SSTM
    // FallMax, then FUN_14040a480 copies the PRTY record named by the
    // player's objentry +0x4e (FUN_1403e9590 lookup) when it is nonzero.
    let pref = bar_entry(&system, b"pref")?;
    let sstm_bytes = bar_entry(pref, b"sstm")?;
    let sstm_offset = u32::from_le_bytes(sstm_bytes.get(4..8).ok_or("SSTM pointer")?.try_into()?);
    let sstm = sstm_bytes
        .get(sstm_offset as usize..)
        .and_then(physics::Sstm::read)
        .ok_or("SSTM record truncated")?;
    let ability_source = {
        let rates = sstm_bytes
            .get(sstm_offset as usize..)
            .and_then(abilities::Rates::read)
            .ok_or("SSTM record truncated")?;
        let item = bar_entry(&system, b"item")?;
        let table = package_reader::item_table::read(item)?;
        let stats = package_reader::item_table::read_stats(item)?;
        // Ability items (types 0x12..0x14): bit at +4, growth flag at +6.
        let items = table
            .iter()
            .filter(|r| (0x12..=0x14).contains(&r.kind))
            .map(|r| (r.id, (r.amount, r.weapon_stats & 0xff != 0)))
            .collect();
        // FUN_1403e3ef0: the weapon's stat entry (+6) carries its ability.
        let weapon = table
            .iter()
            .find(|r| r.id == KINGDOM_KEY)
            .and_then(|r| stats.iter().find(|s| s.id == r.weapon_stats))
            .map_or(0, |s| s.ability);
        AbilitySource {
            rates,
            items,
            weapon,
        }
    };
    let mut obj_params = physics::ObjParams::defaults(sstm.fall_max);
    let player_object = objects
        .iter()
        .find(|o| o.model_name == config.player_object_name.as_bytes())
        .ok_or("player object entry absent")?;
    let prty_index = player_object.neo_moveset as i16 as i32;
    let mut prty_offset = None;
    if prty_index != 0 {
        let prty = preferences::read_prty(bar_entry(pref, b"prty")?)?;
        let offset = physics::prty_offset(&prty.offsets, prty_index).ok_or("PRTY record absent")?;
        let record = prty
            .records
            .iter()
            .find(|r| r.offset == offset && !r.overlaps_table)
            .ok_or("PRTY record absent or overlaps table")?;
        obj_params.apply_prty(&record.words);
        prty_offset = Some(offset);
    }
    let params = Params {
        walk_speed: obj_params.walk,
        run_speed: obj_params.run,
        jump_height: obj_params.jump,
        turn_speed: obj_params.turn,
        fall_max: obj_params.fall_max,
        land_inertia: obj_params.word10,
        land_drag: obj_params.word11,
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
    // Every record of the player's PTYA set: the attack command chooses
    // among them per press (player_sim::ptya). Each distinct motion loads
    // once.
    let records: Vec<player_sim::ptya::Record> = set.iter().map(ptya_record).collect();
    let mut attack_motions: Vec<u16> = Vec::new();
    for e in &set {
        if e.motion_id != 0 && !attack_motions.contains(&e.motion_id) {
            attack_motions.push(e.motion_id);
        }
    }
    // The six spells' tables and cast motions (magc, pref/magi, cmd).
    let magic_src = magic_room::load(&battle, pref, &system, 70.)
        .map_err(|e| format!("magic tables: {e}"))
        .ok();
    let spell_ids = magic_src
        .as_ref()
        .map(|m| magic_room::motion_ids(&m.records))
        .unwrap_or_default();
    for &m in &spell_ids {
        if !attack_motions.contains(&m) {
            attack_motions.push(m);
        }
    }
    if attack_motions.is_empty() {
        return Err("PTYA set has no attack motions".into());
    }

    let mut ids: Vec<u16> = LOCOMOTION.iter().map(|&(_, id)| id).collect();
    let selection = motion::MotionSetSelection {
        index: config.motion_set.index.clone(),
        ordinal: config.motion_set.ordinal,
        name_hash: config.motion_set.name_hash,
        battle: config.battle_mode,
        has_weapon: config.has_weapon,
    };
    let mut motions = motion::read_motion_ids(source, &selection, &ids)?;
    for chunk in attack_motions.chunks(32) {
        motions.extend(motion::read_motion_ids(source, &selection, chunk)?);
    }
    ids.extend(&attack_motions);
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
    // Guard (PTYA 0x0b → motion 173) and its success recoil (motion 42).
    let guard_selection = motion::MotionSetSelection {
        index: config.motion_set.index.clone(),
        ordinal: config.motion_set.ordinal,
        name_hash: config.motion_set.name_hash,
        battle: config.battle_mode,
        has_weapon: config.has_weapon,
    };
    motions.extend(motion::read_motion_ids(
        source,
        &guard_selection,
        &SORA_GUARD,
    )?);
    ids.extend(SORA_GUARD);
    // Sora's HIT spheres (object collision type 2), for enemy attacks.
    let mut sora_target_radius = 0f32;
    let mut sora_target_lift = 70f32;
    let sora_body: Vec<(usize, Vec3, f32, bool)> = match &config.enemy {
        Some(e) => {
            let shapes = object_collision::read(bar_entry_typed(
                &read_archive(source, &e.actor_model)?,
                23,
            )?)?;
            // FUN_1403d7520 reads the attacker's TARGET shape (kind 3) via
            // FUN_1403b5650(obj, 3, -1) / FUN_1403c9c00.
            sora_target_radius = shapes
                .iter()
                .find(|c| c.kind == object_collision::TARGET)
                .map_or(0., |c| c.radius as f32);
            sora_target_lift = shapes
                .iter()
                .find(|c| c.kind == object_collision::TARGET)
                .map_or(70., |c| c.position_height as f32);
            shapes
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
        let mut found = Vec::new();
        for chunk in ids[..battle_count].chunks(32) {
            found.extend(motion::read_motion_ids(source, &weapon_selection, chunk)?);
        }
        for (selected, own) in found.into_iter().zip(&motions) {
            weapon_motions.insert(own.slot, selected);
        }
        // The guard volumes (ATKP 161/162) live in the weapon's motions.
        let found = motion::read_motion_ids(source, &weapon_selection, &SORA_GUARD)?;
        for (selected, own) in found.into_iter().zip(&motions[motions.len() - 2..]) {
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
        let (mut clip, key_start) = clip_for(selected)?;
        // Range 35 opens the combo window only for its own motion id
        // (FUN_14040d810 case 0x23).
        let own_id = ids[clip_list.len()];
        for record in selected.triggers.iter().flat_map(|t| &t.records.ranges) {
            if record.trigger == 35 && record.parameters.first().map(|&m| m as u16) == Some(own_id)
            {
                let start = record.start_frame.max(0) as u32;
                let stop = if record.end_frame < 0 {
                    clip.end
                } else {
                    record.end_frame as u32
                };
                clip.combo_windows.push((start, stop));
            }
        }
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
                    sounds: weapon_motions
                        .get(&selected.slot)
                        .map(|own| sound_triggers(own, WEAPON_BANK))
                        .unwrap_or_default(),
                });
                by_slot.insert(selected.slot, clip_poses.len() - 1);
                clip_poses.len() - 1
            }
        };
        slots.push(serde_json::json!({"motion_id":ids[clip_list.len()],"slot":selected.slot,"sha256":selected.sha256,
            "end_ticks":clip.end,"loop_start":clip.loop_start,"combo_windows":clip.combo_windows,
            "blend_to_idle":clip.blend_to_idle,"no_gravity":clip.no_gravity,"actionable":clip.actionable,
            "inertia":clip.inertia,"turn_to_target":clip.turn_to_target,"face_target":clip.face_target,"takeoff":clip.takeoff,
            "root_start":clip_poses[index].root.first().map(|r| r.to_array()),
            "root_end":clip_poses[index].root.last().map(|r| r.to_array())}));
        clip_list.push((clip, index));
    }
    let step = |entry: &ptya::PtyaEntry, (clip, index): &(Clip, usize)| AttackStep {
        clip: clip.clone(),
        hop: f32::from_bits(entry.float_bits[0]),
        hop_speed: f32::from_bits(entry.float_bits[4]),
        hop_max: f32::from_bits(entry.float_bits[1]),
        hop_min: f32::from_bits(entry.float_bits[2]),
        hop_speed_min: f32::from_bits(entry.float_bits[3]),
        hop_reach: f32::from_bits(entry.float_bits[12]),
        blend: f32::from_bits(entry.float_bits[11]),
        root: clip_poses[*index]
            .root
            .iter()
            .map(|r| r.to_array())
            .collect(),
    };
    let n = LOCOMOTION.len();
    let field = n + attack_motions.len();
    let attack_clip = |motion: u16| {
        &clip_list[n + attack_motions
            .iter()
            .position(|&m| m == motion)
            .unwrap_or(0)]
    };
    let spells: Vec<player_sim::SpellClip> = spell_ids
        .iter()
        .map(|&m| {
            let (clip, index) = attack_clip(m);
            player_sim::SpellClip {
                motion: m,
                clip: clip.clone(),
                root: clip_poses[*index]
                    .root
                    .iter()
                    .map(|r| r.to_array())
                    .collect(),
            }
        })
        .collect();
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
        records: records.clone(),
        spells,
        attacks: set
            .iter()
            .map(|e| step(e, attack_clip(e.motion_id)))
            .collect(),
        hurt: clip_list[field + 3..field + 3 + hurt_count]
            .iter()
            .map(|c| c.0.clone())
            .collect(),
        guard: Some(clip_list[field + 3 + hurt_count].0.clone()),
        guard_recoil: Some(clip_list[field + 4 + hurt_count].0.clone()),
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
    for (i, e) in set.iter().enumerate() {
        let pose = attack_clip(e.motion_id).1;
        both.push((Action::Attack(i), pose));
        both.push((Action::AirAttack(i), pose));
    }
    for (i, &m) in spell_ids.iter().enumerate() {
        let pose = attack_clip(m).1;
        both.push((Action::Cast(i), pose));
        both.push((Action::AirCast(i), pose));
    }
    for i in 0..hurt_count {
        both.push((Action::Hurt(i), clip_list[field + 3 + i].1));
    }
    both.push((Action::Guard, clip_list[field + 3 + hurt_count].1));
    both.push((Action::GuardRecoil, clip_list[field + 4 + hurt_count].1));
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
            "turn_speed":params.turn_speed,"fall_max":params.fall_max,"gravity":physics::G,"run_magnitude":WALK_THRESHOLD,
            "prty_index":prty_index,"prty_offset":prty_offset,"land_inertia":params.land_inertia,"land_drag":params.land_drag,"sstm":format!("{sstm:?}")},
        "test_room_choices":{"step_height":STEP_HEIGHT,
            "body_radius":BODY_RADIUS,"body_height":BODY_HEIGHT,"floor_min_normal_y":FLOOR_MIN_NORMAL_Y,
            "blend_ticks":"per action (player_sim::blend_for, PTYA +0x38 after attacks)","clock":"fixed 60 Hz, one motion tick per update"},
        "ptya":{"records":set.iter().map(|e| serde_json::json!({"id":e.id,"kind":e.kind,"alternate":e.sub,"combo_offset":e.combo_offset,"flag":e.flag,"motion_id":e.motion_id,"ability":e.ability,"floats":e.float_bits.map(f32::from_bits)})).collect::<Vec<_>>(),
            "legacy_combo_ids":[&config.ground_combo_ids,&config.air_combo_ids]},
        "motions":slots,"collision":{"floors":floors,"walls":walls,"surface_flags":flag_counts,"bounds":[low.to_array(),high.to_array()],
            "largest_levels":levels.iter().take(8).map(|(area,p)| serde_json::json!({"area":area,"point":p})).collect::<Vec<_>>()},
        "spawn":spawn.to_array(),"pose_evaluator":if basic_fk {"basic public FK (T053)"} else {"motion_eval IK/constraints (T057)"},
        "evaluation_totals":{"constraints":eval_totals[0],"ik_chains":eval_totals[1],"expressions":eval_totals[2],"unsupported_constraints":eval_totals[3]},"weapon_joint":weapon_joint,"objentry_joint":objentry_joint,"weapon_bounds":weapon_bounds,"idle_hand_matrix":idle_hand,"weapon_root_matrix":weapon_root,"weapon_bones":weapon.skinning.bones.len(),"object_candidates":object_candidates,
        "object_count":objects.len(),
        "missing":["scale channels","up-vector/path/camera constraint types","limiter damping","root rotation","weapon constraint motion","hit detection","enemies","measured installed timing"],
        "inferred_rules":["direction constraints aim local +X","two-joint IK bends the middle joint about local Z","expression list/condition semantics","Hermite tangents scaled by key span","field/battle mode switch with 6 s disarm timeout"],
        "gameplay_verified":false});
    let mut banks = Banks::default();
    for b in &config.sounds {
        let l = &b.locator;
        banks.insert(sound::load_bank(
            source,
            &l.index,
            l.ordinal,
            l.name_hash,
            b.replacement.as_deref(),
        )?)?;
    }
    let npc = match &config.npc {
        Some(n) => Some(load_npc(source, base, &config, n)?),
        None => None,
    };
    let effects = match &config.effects {
        Some(e) => {
            use windows_shell::effects::{load_atlas, load_pax};
            let pax = |l: &Locator| load_pax(source, &l.index, l.ordinal, l.name_hash);
            Some(EffectSources {
                weapon: pax(&e.weapon)?,
                enemy: e.enemy.as_ref().map(pax).transpose()?,
                system: e.system.as_ref().map(pax).transpose()?,
                atlas: load_atlas(source, &e.atlas.index, e.atlas.ordinal, e.atlas.name_hash)?,
            })
        }
        None => None,
    };
    let camp = match (&config.camp, &config.menu) {
        (Some(l), Some(m)) => {
            let menu = layout2d::load(source, &l.index, l.ordinal, l.name_hash)?;
            let ids: Vec<u32> = CAMP_ENTRIES
                .iter()
                .flat_map(|&(_, label, help)| [label, help])
                .collect();
            let texts = hud::load_messages(
                source,
                &m.messages.index,
                m.messages.ordinal,
                m.messages.name_hash,
                &ids,
            )?;
            let entries = CAMP_ENTRIES
                .iter()
                .zip(texts.chunks(2))
                .map(|(&(id, _, _), t)| camp::Entry {
                    id,
                    label: t[0].clone(),
                    help: t[1].clone(),
                })
                .collect();
            let font = hud::load_system_font(
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
            )?;
            let icons = hud::load_font(
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
                b"icon",
            )?;
            let msg = |ids: &[u32]| {
                hud::load_messages(
                    source,
                    &m.messages.index,
                    m.messages.ordinal,
                    m.messages.name_hash,
                    ids,
                )
            };
            let mut status = msg(&[0xb9fc, 0xb9fd, 0xb9fe, 0x851f, 0xc5ae, 0xb61a])?;
            let banner = status.pop().unwrap_or_default();
            let status_help = status.pop().unwrap_or_default();
            let name = status.pop().unwrap_or_default();
            let labels = [status[0].clone(), status[1].clone(), status[2].clone()];
            // "<0d>´s Status.": code 0x0d takes the character's name.
            let mut help = Vec::new();
            for &b in &status_help {
                if b == 0x0d {
                    help.extend_from_slice(&name);
                } else {
                    help.push(b);
                }
            }
            let stat_ids: Vec<u32> = CAMP_STATS
                .iter()
                .flat_map(|&(_, label, help, _)| [label, help])
                .collect();
            let stat_texts = msg(&stat_ids)?;
            let stats = CAMP_STATS
                .iter()
                .zip(stat_texts.chunks(2))
                .map(|(&(bit, _, _, style), t)| (bit, t[0].clone(), t[1].clone(), style))
                .collect();
            let config_ids: Vec<u32> = CAMP_CONFIG
                .iter()
                .flat_map(|(label, choices, helps)| {
                    std::iter::once(*label)
                        .chain(choices.iter().copied())
                        .chain(helps.iter().copied())
                })
                .collect();
            let mut config_texts = msg(&config_ids)?.into_iter();
            let config = CAMP_CONFIG
                .iter()
                .map(|(_, choices, helps)| {
                    let mut take = |n: usize| -> Vec<Vec<u8>> {
                        (0..n).filter_map(|_| config_texts.next()).collect()
                    };
                    let label = take(1).pop().unwrap_or_default();
                    camp::ConfigRow {
                        label,
                        choices: take(choices.len()),
                        help: take(helps.len()),
                    }
                })
                .collect();
            let lvup = bar_entry(&battle, b"lvup")?;
            let plrp = bar_entry(&battle, b"plrp")?;
            let lv = package_reader::battle_tables::player_level(lvup, 1, 1)?;
            let byte = |at: usize| plrp.get(8 + at).copied().unwrap_or(0) as i32;
            // Sora at level 1 (LVUP row + PLRP id 0, Kingdom Key +3 Str /
            // +1 Mag as in the combat setup) [test-room values].
            let numbers = SoraNumbers {
                next_exp: lv.exp,
                ap: lv.ap as i32 + byte(5) + ROOM_EXTRA_AP,
                strength: lv.strength as i32 + byte(6) + 3,
                magic: lv.magic as i32 + byte(7) + 1,
                defense: lv.defense as i32 + byte(8),
            };
            let (abilities, weapon_ability) = load_camp_abilities(&system, |ids| {
                hud::load_messages_optional(
                    source,
                    &m.messages.index,
                    m.messages.ordinal,
                    m.messages.name_hash,
                    ids,
                )
            })?;
            // Customize texts (customize-spec §1.3 / §7).
            let mut cust = msg(&CUSTOMIZE_MESSAGES)?.into_iter();
            let mut take = || cust.next().unwrap_or_default();
            let customize = camp::customize::CustomizeTexts {
                help: [take(), take()],
                labels: [take(), take(), take()],
                shortcut_headings: [take(), take(), take(), take()],
                remove_help: take(),
                mp_choices: [take(), take()],
                mp_help: [take(), take()],
                reload_choices: [take(), take()],
                reload_help: [take(), take()],
                reload_warning: take(),
            };
            let mut eq = msg(&EQUIP_MESSAGES[..21])?.into_iter();
            let mut take = || eq.next().unwrap_or_default();
            let equip_texts = camp::equip::EquipTexts {
                sections: [take(), take(), take(), take(), take()],
                none: take(),
                no_items: take(),
                unequip_help: [take(), take(), take()],
                reload_box: take(),
                accessory_box: take(),
                stat_labels: [
                    take(),
                    take(),
                    take(),
                    take(),
                    take(),
                    take(),
                    take(),
                    take(),
                    take(),
                ],
            };
            let equip_stats = package_reader::item_table::read_stats(bar_entry(&system, b"item")?)?
                .into_iter()
                .map(|s| camp::equip::ItemStats {
                    id: s.id,
                    ability: s.ability,
                    strength: s.strength,
                    magic: s.magic,
                    defense: s.defense,
                    ap: s.ap,
                    resists: s.resists,
                })
                .collect();
            // Party help (party-spec §11).
            let mut party = msg(&[0xbb88, 0x846b, 0x846c])?.into_iter();
            let mut next = || party.next().unwrap_or_default();
            let party_help = [next(), next(), next()];
            let items = load_camp_items(&system, |ids| {
                hud::load_messages_optional(
                    source,
                    &m.messages.index,
                    m.messages.ordinal,
                    m.messages.name_hash,
                    ids,
                )
            })?;
            Some(CampSource {
                menu,
                entries,
                font,
                labels,
                name,
                help,
                banner,
                stats,
                config,
                numbers,
                items,
                icons,
                abilities,
                weapon_ability,
                party_help,
                customize,
                equip_texts,
                equip_stats,
            })
        }
        _ => None,
    };
    let journal = match (&config.journal, &config.menu, &camp) {
        (Some(j), Some(m), Some(_)) => Some(load_journal(source, j, &m.messages)?),
        _ => None,
    };
    let music = match &config.music {
        Some(m) => Some([
            sound::load_bank(
                source,
                &m.field.index,
                m.field.ordinal,
                m.field.name_hash,
                None,
            )?,
            sound::load_bank(
                source,
                &m.battle.index,
                m.battle.ordinal,
                m.battle.name_hash,
                None,
            )?,
        ]),
        None => None,
    };
    let magic_src = magic_src.map(|mut m| {
        m.target_lift = sora_target_lift;
        m
    });
    // The spells' names for the Magic submenu (cmd +8 message ids).
    let magic_src = match (magic_src, &config.menu) {
        (Some(mut m), Some(menu)) => {
            let wanted: Vec<(u16, u32)> = m
                .records
                .iter()
                .filter_map(|r| m.commands.get(&r.command).map(|c| (r.command, c.name)))
                .collect();
            let ids: Vec<u32> = wanted.iter().map(|w| w.1).collect();
            let texts = hud::load_messages_optional(
                source,
                &menu.messages.index,
                menu.messages.ordinal,
                menu.messages.name_hash,
                &ids,
            )?;
            for ((cmd, _), text) in wanted.into_iter().zip(texts) {
                if let Some(t) = text {
                    m.names.insert(cmd, t);
                }
            }
            Some(m)
        }
        (m, _) => m,
    };
    let enemy = match &config.enemy {
        Some(e) => Some(load_enemy(
            source,
            base,
            e,
            &battle,
            &clip_poses,
            sora_body,
            sora_target_radius,
        )?),
        None => None,
    };
    Ok(Loaded {
        room,
        arena_mesh,
        actor,
        weapon,
        params,
        sstm,
        abilities: ability_source,
        magic: magic_src,
        clips,
        actions,
        clip_poses,
        spawn,
        world,
        summary,
        hud,
        enemy,
        banks,
        effects,
        npc,
        camp,
        journal,
        music,
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
    sora_target_radius: f32,
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
    // FUN_1403d7520: Sora's TARGET radius plus the target's own shape
    // radius (FUN_1403be0f0) [I: the enemy's TARGET entry].
    let target_reach = sora_target_radius
        + collision
            .iter()
            .find(|c| c.kind == object_collision::TARGET)
            .map_or(0., |c| c.radius as f32);
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
    // TARGET entries on bones: what the attack chooser and the automatic
    // target measure (FUN_1403bdac0 / FUN_1403be4b0).
    let targets: Vec<_> = collision
        .iter()
        .filter(|e| e.kind == object_collision::TARGET && e.bone >= 0)
        .map(|e| {
            (
                e.bone as usize,
                Vec3::new(
                    e.position[0] as f32,
                    e.position[1] as f32,
                    e.position[2] as f32,
                ),
                e.radius as f32,
                e.height as f32,
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
                sounds: sound_triggers(sel, -1),
            },
        );
    }
    let atkp = bar_entry(battle, b"atkp")?;
    let mut steps = HashMap::new();
    let mut hit_sounds = HashMap::new();
    let mut hit_effects = HashMap::new();
    let mut rows = HashMap::new();
    for (_, _, id, _) in clip_poses.iter().flat_map(|c| &c.hits) {
        let all = battle_tables::attack_steps(atkp, *id)?;
        let a = all[0];
        steps.insert((*id, 0), all);
        rows.insert(*id, a);
        hit_sounds.insert(*id, a.hit_sound);
        hit_effects.insert(*id, (a.hit_effect, a.hit_effect_random));
    }
    for (_, _, id, _) in clips.values().flat_map(|c: &EnemyClip| &c.windows) {
        let all = battle_tables::attack_steps(atkp, *id)?;
        let a = all[0];
        steps.insert((*id, 0), all);
        hit_sounds.insert(*id, a.hit_sound);
        rows.insert(*id, a);
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
        ..combat::AttackerStats::default()
    };
    // Enemy as attacker: LVPM Str with its max/min clamps; Sora as target:
    // LVUP + PLRP defence, player clamps 99999/1, weaknesses 100.
    let shadow_attacker = combat::AttackerStats {
        strength: row.strength as i32,
        magic: row.strength as i32,
        max: row.max_strength as i32,
        min: row.min_strength as i32,
        ..combat::AttackerStats::default()
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
        targets,
        blade,
        steps,
        atkp: atkp.to_vec(),
        difficulty: match config.difficulty {
            0 => combat::Difficulty::Beginner,
            2 => combat::Difficulty::Proud,
            3 => combat::Difficulty::Critical,
            _ => combat::Difficulty::Standard,
        },
        sora,
        stats,
        offset: Vec3::from_array(config.offset),
        claws,
        shadow_attacker,
        sora_target,
        sora_body,
        target_reach,
        hit_sounds,
        hit_effects,
        rows,
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
    effect_atlas: Option<windows_shell::effects::Atlas>,
    npc_model: Option<model_inspection::InspectionModel>,
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

// Bevy systems take each resource as a parameter.
#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    mut startup: ResMut<Startup>,
    mut game: ResMut<Game>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    talk: Option<ResMut<Talk>>,
    mut frame: ResMut<Ui2dFrame>,
    windows: Query<&Window>,
) {
    let room = startup.room.take().expect("room retained for setup");
    let mut compose = Compose {
        scene: Handle::default(),
        scene_sheet: 0,
        snapshot_sheet: None,
        hud_sheet: None,
        stable_frames: 0,
    };
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
            mp_charge: None,
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
        // The 16:9 HUD canvas over the whole window, composed in gamma space
        // after the scene.
        compose.hud_sheet = Some(frame.add_image(
            image.clone(),
            [canvas.width as u32, canvas.height as u32],
            true,
        ));
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
                let mut menu = CommandMenu::new(&sheet.sequence, items);
                // The Magic row opens submenu 1 (command-01 section 0.6).
                menu.opens = vec![None, Some(1), None, None];
                MenuState { menu, sheet, font }
            }),
            stats,
            canvas,
            image,
            updated: true,
        });
    }
    // Scene data stays in source coordinates under one root. The exe maps
    // them to the screen without a mirror (model +X on screen right,
    // camp-03 §1.4), as the rotation from_world does.
    let world = commands
        .spawn((Transform::default(), Visibility::default()))
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
    // A pool of spell-volume placeholders (the most volumes live at once).
    let unit = meshes.add(Sphere::new(1.).mesh().ico(3).unwrap());
    for i in 0..16 {
        commands.spawn((
            SpellVolume(i),
            ChildOf(world),
            Mesh3d(unit.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(1., 1., 1., 0.3),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                ..default()
            })),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
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
    // Tz::MenuObj's member 0: Sora's walk (motion 0x8b = slot 6) rendered
    // off-screen on layer 3 and pasted into the camp (camp-03 spec). Menu
    // space: (mx, my, mz + 100) seen from (0, 50, 800) toward (0, 0, -55),
    // i.e. the game's view turned 180° about X (no mirror).
    let walk = game.poses[&(Action::Walk, false)];
    let walk_pose: Vec<Vec3> = {
        let c = &game.clip_poses[walk];
        c.frames[0].iter().map(|p| *p + c.root[0]).collect()
    };
    let menu_root = commands
        .spawn((
            MenuModelRoot,
            Transform::from_xyz(0., 0., 100.),
            Visibility::Hidden,
            RenderLayers::layer(MENU_LAYER),
        ))
        .id();
    for (ordinal, entity) in spawn_model(
        &mut commands,
        &actor,
        &walk_pose,
        menu_root,
        &mut meshes,
        &mut materials,
        &mut images,
    ) {
        if let Some(entity) = entity {
            commands
                .entity(entity)
                .insert((MenuModelPart(ordinal), RenderLayers::layer(MENU_LAYER)));
        }
    }
    let menu_target = images.add(Image::new_target_texture(
        MENU_TARGET[0],
        MENU_TARGET[1],
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -3,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        bevy::camera::RenderTarget::Image(menu_target.clone().into()),
        // A 640x480-style camera with horizontal FOV 40° (focal 879.19):
        // vertical 2·atan(240 / 879.19); squeezed to the layout, the
        // target's 16:9 square pixels match it (camp-03 §1).
        Projection::Perspective(PerspectiveProjection {
            fov: 2. * (240f32 / 879.1928).atan(),
            near: 1.,
            far: 100_000.,
            ..default()
        }),
        Transform::from_xyz(0., 50., 800.).looking_at(Vec3::new(0., 0., -55.), Vec3::Y),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        NoIndirectDrawing,
        RenderLayers::layer(MENU_LAYER),
        MenuCamera,
    ));
    commands.insert_resource(MenuModel {
        sheet: frame.add_image(menu_target, MENU_TARGET, true),
        clip: walk,
        indices: actor.parts.iter().map(|p| p.indices.clone()).collect(),
    });
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
    if let (Some(model), Some(mut talk)) = (startup.npc_model.take(), talk) {
        let root = commands
            .spawn((
                NpcRoot,
                ChildOf(world),
                Transform::from_translation(talk.position)
                    .with_rotation(Quat::from_rotation_y(talk.yaw)),
                Visibility::default(),
            ))
            .id();
        let mut indices = Vec::new();
        for (ordinal, entity) in spawn_model(
            &mut commands,
            &model,
            &talk.frames[0],
            root,
            &mut meshes,
            &mut materials,
            &mut images,
        ) {
            indices.push(model.parts[ordinal].indices.clone());
            if let Some(entity) = entity {
                commands.entity(entity).insert(NpcPart(ordinal));
            }
        }
        talk.indices = indices;
        // The cross-dissolve: a second camera renders the old view into an
        // image, drawn under the HUD while it fades.
        let snapshot = images.add(Image::new_target_texture(
            1280,
            720,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                is_active: false,
                ..default()
            },
            bevy::camera::RenderTarget::Image(snapshot.clone().into()),
            Transform::default(),
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            NoIndirectDrawing,
            SnapshotCamera,
        ));
        compose.snapshot_sheet = Some(frame.add_image(snapshot, [1280, 720], true));
    }
    game.weapon_display = weapon_pose;
    game.part_indices = part_indices;
    game.display = pose;
    // Effect quads: one mesh per blend (GS ALPHA 0x44 blend, 0x48 add).
    if let Some(atlas) = startup.effect_atlas.take() {
        let image = rgba_image(atlas.size[0], atlas.size[1], atlas.rgba, &mut images);
        if let Some(mut i) = images.get_mut(&image) {
            i.sampler = ImageSampler::linear();
        }
        for additive in [false, true] {
            commands.spawn((
                EffectMesh { additive },
                ChildOf(world),
                Mesh3d(meshes.add(mesh(vec![[0.; 3]; 3], vec![[0.; 2]; 3], vec![[0.; 4]; 3]))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: Some(image.clone()),
                    unlit: true,
                    cull_mode: None,
                    alpha_mode: if additive {
                        AlphaMode::Add
                    } else {
                        AlphaMode::Blend
                    },
                    ..default()
                })),
                Transform::default(),
            ));
        }
    }
    // The 3D scene renders into an image the 2D pass composes first, so
    // every 2D layer blends in gamma space as the exe's framebuffer does.
    let size = windows
        .single()
        .map(|w| [w.physical_width().max(1), w.physical_height().max(1)])
        .unwrap_or([1280, 720]);
    compose.scene = images.add(Image::new_target_texture(
        size[0],
        size[1],
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    compose.scene_sheet = frame.add_image(compose.scene.clone(), size, true);
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -2,
            ..default()
        },
        bevy::camera::RenderTarget::Image(compose.scene.clone().into()),
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
    commands.insert_resource(compose);
}

/// The camp menu's member model (Tz::MenuObj) and its off-screen target.
const MENU_LAYER: usize = 3;
const MENU_TARGET: [u32; 2] = [1280, 720];

#[derive(Component)]
struct MenuModelRoot;
#[derive(Component)]
struct MenuModelPart(usize);
#[derive(Component)]
struct MenuCamera;

#[derive(Resource)]
struct MenuModel {
    sheet: usize,
    clip: usize,
    indices: Vec<Vec<u32>>,
}

/// While the camp shows its members, render Sora's walk at the menu time
/// (root motion kept, camp-03 §3).
fn menu_model_present(
    camp: Option<Res<CampRes>>,
    model: Option<Res<MenuModel>>,
    game: Res<Game>,
    mut cameras: Query<&mut Camera, With<MenuCamera>>,
    mut roots: Query<&mut Visibility, With<MenuModelRoot>>,
    parts: Query<(&MenuModelPart, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(model) = model else { return };
    let time = camp.and_then(|c| c.open.as_ref().and_then(|m| m.model_time));
    if let Ok(mut camera) = cameras.single_mut() {
        camera.is_active = time.is_some();
    }
    for mut v in &mut roots {
        let want = if time.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    let Some(t) = time else { return };
    let c = &game.clip_poses[model.clip];
    let f = (t as usize).min(c.frames.len() - 1);
    let root = c.root[f.min(c.root.len() - 1)];
    for (part, handle) in &parts {
        let positions: Vec<[f32; 3]> = model.indices[part.0]
            .iter()
            .map(|&i| (c.frames[f][i as usize] + root).to_array())
            .collect();
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
    }
}
/// The 2D composition's sheets: the rendered scene, the dissolve snapshot
/// and the HUD canvas.
#[derive(Resource)]
struct Compose {
    scene: Handle<Image>,
    scene_sheet: usize,
    snapshot_sheet: Option<usize>,
    hud_sheet: Option<usize>,
    /// Frames since the scene image last changed size (unattended
    /// captures wait for a settled target).
    stable_frames: u64,
}

/// The scene image follows the window's size.
fn resize_scene(
    compose: Option<ResMut<Compose>>,
    windows: Query<&Window>,
    mut images: ResMut<Assets<Image>>,
    mut frame: ResMut<Ui2dFrame>,
) {
    let (Some(mut c), Ok(w)) = (compose, windows.single()) else {
        return;
    };
    let size = [w.physical_width().max(1), w.physical_height().max(1)];
    c.stable_frames += 1;
    if frame.sheets[c.scene_sheet].size == size {
        return;
    }
    c.stable_frames = 0;
    if let Some(mut image) = images.get_mut(&c.scene) {
        image.resize(Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        });
    }
    frame.sheets[c.scene_sheet].size = size;
}

/// The scene, then the dissolve snapshot fading out over it, then the HUD
/// (hidden while the camp is open [inferred]); the camp draws after.
fn compose_2d(
    compose: Option<Res<Compose>>,
    talk: Option<Res<Talk>>,
    camp: Option<Res<CampRes>>,
    mut frame: ResMut<Ui2dFrame>,
) {
    let Some(c) = compose else { return };
    frame.full_screen(c.scene_sheet, [1.; 4]);
    if let (Some(s), Some(t)) = (c.snapshot_sheet, &talk)
        && t.dissolve > 0
    {
        frame.full_screen(s, [1., 1., 1., t.dissolve as f32 / DISSOLVE_FRAMES as f32]);
    }
    let camp_open = camp.is_some_and(|c| c.menu_up());
    if let Some(h) = c.hud_sheet
        && !camp_open
    {
        frame.full_screen(h, [1.; 4]);
    }
}

/// The exe's pad pipeline for the room (bgm-input-01 Part B): PADDATA,
/// raw word, logical bits through pad mode 1 (field) or 7 (a menu). New
/// presses queue the field actions: jump 0x100 (back button), decide 0x4,
/// guard 0x200 (Square), reaction 0x400 (Triangle), talk advance 0x40c and
/// the command menu's up/down 0x4000/0x8000.
fn read_pad(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<(&Gamepad, Option<&Name>)>,
    camp: Option<Res<CampRes>>,
    mut game: ResMut<Game>,
) {
    if game.autoplay.is_some() {
        return;
    }
    let (data, _, _) = input::read_devices(&keys, &mouse, 0., &pads);
    let menu = camp.is_some_and(|c| c.menu_up());
    let mode = if menu {
        pad::MODE_TITLE
    } else {
        pad::MODE_FIELD
    };
    game.pad_data = data;
    game.pad.update_raw(pad::raw(&data), mode, 1.);
    if menu {
        return;
    }
    let p = game.pad.pressed;
    // L1 held (logical bit 23) shows the shortcut menu: the face buttons
    // (logical bits 24-27) then pick its four slots instead of acting.
    if game.pad.held & 0x80_0000 != 0 {
        for k in 0..4 {
            if p & (1 << (24 + k)) != 0 {
                game.shortcut_queued |= 1 << k;
            }
        }
        return;
    }
    game.back_queued |= p & 0x8 != 0;
    game.jump_queued |= p & 0x100 != 0;
    game.attack_queued |= p & 0x4 != 0;
    game.guard_queued |= p & 0x200 != 0;
    game.rc_queued |= p & 0x400 != 0;
    game.advance_queued |= p & 0x40c != 0;
    game.menu_up_queued |= p & 0x4000 != 0;
    game.menu_down_queued |= p & 0x8000 != 0;
}
/// Scripted input for unattended verification runs.
fn autoplay_input(tick: u64) -> (Vec2, bool, bool) {
    let stick = match tick {
        60..150 => Vec2::new(0., 1.),
        150..240 => Vec2::new(0., 0.35),
        240..270 => Vec2::new(1., 0.),
        _ => Vec2::ZERO,
    };
    // (1520: Aerial Recovery out of the blow struck at 1500.)
    let jump = matches!(tick, 300 | 560 | 1520);
    // (spell slots are scripted in `simulate`)
    // (1720: Retaliating Slash out of the blow struck at 1700.)
    let attack = matches!(
        tick,
        400 | 410 | 440 | 470 | 500 | 575 | 590 | 610 | 630 | 1720
    );
    (stick, jump, attack)
}
const AUTOPLAY_CAPTURES: [u64; 85] = [
    90, 110, 200, 255, 312, 330, 352, 405, 450, 495, 585, 615, 760, 1010, 1100, 1198, 1300, 1500,
    1512, 1530, 1700, 1712, 1730, 1830, 1900, 1930, 1990, 2030, 2050, 2070, 2110, 2230, 2310, 2330,
    2390, 2420, 2450, 2520, 2690, 2715, 2760, 2780, 2840, 2880, 2920, 2970, 3080, 3200, 3220, 3270,
    3300, 3560, 3660, 3690, 3730, 3800, 3830, 3900, 4060, 4150, 4420, 4450, 4800, 4880, 5000, 5100,
    5500, 5560, 5650, 5690, 5800, 5840, 5940, 6000, 6210, 6270, 6300, 6390, 6460, 6530, 6600, 6670,
    6705, 6725, 6760,
];
/// The talk room: Talk at tick 60, one advance once the text is up, and
/// captures through the prompt, dissolve, window, close and return.
const AUTOPLAY_TALK: [u64; 1] = [60];
const AUTOPLAY_ADVANCE: [u64; 1] = [150];
const AUTOPLAY_TALK_CAPTURES: [u64; 10] = [40, 61, 68, 84, 100, 140, 156, 164, 172, 200];
const AUTOPLAY_TALK_END: u64 = 260;
/// Debug blow hits of the unattended run (see `Game::debug_blow`).
const AUTOPLAY_BLOWS: [u64; 2] = [1500, 1700];
/// Guard presses ahead of the Shadow's claws seen at ticks 1194 and 1361
/// in earlier unattended runs.
const AUTOPLAY_GUARDS: [u64; 2] = [1170, 1340];
/// The last stretch leaves Sora standing so an enemy can attack him.
const AUTOPLAY_END: u64 = 6860;
const AUTOPLAY_WARMUP_FRAMES: u64 = 30;

fn simulate(mut rig: ResMut<CameraRig>, mut game: ResMut<Game>, hud: Option<ResMut<Hud>>) {
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
        if game.autoplay_end == AUTOPLAY_TALK_END {
            // The talk room's unattended run stands still.
            (Vec2::ZERO, false, false)
        } else {
            autoplay_input(tick)
        }
    } else {
        // The left-stick vector past the game's 60-unit dead zone (up is
        // negative in PADDATA), magnitude capped at 1.
        let [x, y, magnitude] = pad::analog(game.pad_data.left);
        let stick = Vec2::new(x, -y).normalize_or_zero() * magnitude;
        let queued = (game.jump_queued, game.attack_queued);
        game.jump_queued = false;
        game.attack_queued = false;
        (stick, queued.0, queued.1)
    };
    // Field talk locks control (player+0xdc8 → +0x40 bit 0).
    let talking = game.talking;
    let (stick, jump, decide) = if talking {
        (Vec2::ZERO, false, false)
    } else {
        (stick, jump, decide)
    };
    let (up, down) = if talking { (false, false) } else { (up, down) };
    // Camera-relative stick: forward is the view direction on the ground,
    // right is screen right.
    let eye = from_world(rig.camera.eye);
    let look_at = from_world(rig.camera.look_at);
    let forward = (look_at - eye).xz().normalize_or(Vec2::NEG_Y);
    let right = Vec2::new(-forward.y, forward.x);
    let direction = forward * stick.y + right * stick.x;
    // The decide button executes the command under the cursor; Attack
    // (icon 1) is the only command this room implements.
    let mut cursor_moved = false;
    let back = std::mem::take(&mut game.back_queued) && !talking;
    let mut attack = decide;
    let mut sub_was_open = false;
    let mut menu_cast = None;
    let mut denied = false;
    if let Some(mut h) = hud
        && let Some(m) = h.menu.as_mut()
    {
        // Magic row colour (command-01 section 0.6: 5 without spells, 1
        // when every entry is unusable) and the open submenu's rows.
        if let Some(mg) = game.magic.as_ref() {
            let list = mg.menu_list(&game.player.mp);
            if let Some(row) = m.menu.items.get_mut(1) {
                row.colour = if list.is_empty() {
                    5
                } else if list.iter().all(|c| c.1 >= 4) {
                    1
                } else {
                    0
                };
            }
            if let Some(sub) = m.menu.sub.as_mut() {
                for (item, (_, c)) in sub.items.iter_mut().zip(&list) {
                    item.colour = *c;
                }
            }
        }
        sub_was_open = m.menu.sub.is_some();
        let before = (m.menu.cursor, m.menu.sub.as_ref().map(|s| s.cursor));
        let decision = m.menu.input_with_back(up, down, decide, back);
        cursor_moved = (m.menu.cursor, m.menu.sub.as_ref().map(|s| s.cursor)) != before;
        attack =
            decision == Decision::Execute(0) && m.menu.items.first().is_some_and(|i| i.icon == 1);
        match decision {
            Decision::Open(1) => {
                let mp = game.player.mp;
                if let Some(mg) = game.magic.as_mut() {
                    let list = mg.menu_list(&mp);
                    let items = list
                        .iter()
                        .map(|&(t, colour)| {
                            let cmd = mg.current(t).map(|r| r.command).unwrap_or(0);
                            Item {
                                label: mg.src.names.get(&cmd).cloned().unwrap_or_default(),
                                icon: mg.src.commands.get(&cmd).map_or(0, |c| c.icon),
                                colour,
                            }
                        })
                        .collect();
                    mg.menu_types = list.iter().map(|l| l.0).collect();
                    m.menu.open_sub(1, items);
                    cursor_moved = true;
                }
            }
            Decision::ExecuteSub { row: 1, index } => {
                menu_cast = game
                    .magic
                    .as_ref()
                    .and_then(|mg| mg.menu_types.get(index).copied());
            }
            Decision::Invalid => denied = true,
            _ => {}
        }
    }
    // Circle backs out of a submenu instead of jumping.
    let jump = jump && !sub_was_open;
    // Cursor moves play system sound 1 (FUN_1403bc640/FUN_1403b11e0).
    if cursor_moved {
        game.sound_queue.push((SE_CURSOR, "menu_cursor"));
    }
    if denied {
        game.sound_queue.push((5, "menu_denied"));
    }
    // Guard (Square): F or the pad's west button; unattended runs guard
    // just before two of the Shadow's claws.
    let guard = if game.autoplay.is_some() {
        AUTOPLAY_GUARDS.contains(&tick)
    } else {
        std::mem::take(&mut game.guard_queued)
    } && !talking
        && game.guard_equipped;
    // The current target: the enemy while it can be hit.
    let target = game
        .enemy
        .as_ref()
        .filter(|rt| rt.enemy.targetable())
        .map(|rt| {
            let p = game.player.position;
            [
                rt.enemy.position[0] - p[0],
                rt.enemy.position[1] - p[1],
                rt.enemy.position[2] - p[2],
                rt.target_reach,
            ]
        });
    // The enemy's TARGET shapes: candidates for the attack chooser, and the
    // current target while it can be hit [test room: one enemy, so the
    // automatic target is it whenever targetable].
    let shapes = game
        .enemy
        .as_ref()
        .filter(|rt| rt.enemy.targetable())
        .map(enemy_targets)
        .unwrap_or_default();
    let mut nearby = [None; 4];
    for (slot, t) in nearby.iter_mut().zip(&shapes) {
        *slot = Some(*t);
    }
    let input = Input {
        direction: direction.to_array(),
        jump,
        attack,
        guard,
        target: target.map(|t| [t[0], t[2]]),
        target_rise: target.map_or(0., |t| t[1]),
        target_reach: target.map_or(0., |t| t[3]),
        target_shape: shapes.first().copied(),
        nearby,
        probe_blocked: false,
    };
    // L1 + face button: the shortcut's command (a spell's level-1 command id
    // stands for the spell of its magic type, FUN_1403b68b0). Unattended
    // runs cast Fire, Thunder, Blizzard and Cure on fixed ticks.
    let slot = std::mem::take(&mut game.shortcut_queued);
    let mut cast = None;
    if slot != 0 && !talking {
        let k = slot.trailing_zeros() as usize;
        let cmd = game.shortcuts[k];
        cast = game
            .magic
            .as_ref()
            .and_then(|m| m.src.commands.get(&cmd))
            .filter(|c| c.execute == 2)
            .map(|c| c.argument as u8);
    }
    if game.autoplay.is_some() && game.autoplay_end != AUTOPLAY_TALK_END {
        cast = match tick {
            700 => Some(magic::FIRE),
            800 => Some(magic::BLIZZARD),
            900 => Some(magic::THUNDER),
            1000 => Some(magic::MAGNET),
            1100 => Some(magic::REFLECT),
            1250 => Some(magic::CURE),
            _ => None,
        };
    }
    let cast = cast.or(menu_cast);
    if let Some(t) = cast
        && !talking
    {
        magic_room::request(&mut game, t, tick);
    }
    let Game {
        player,
        params,
        clips,
        world,
        ..
    } = &mut *game;
    let step = player.tick(&input, params, clips, world);
    magic_room::tick(&mut game, tick);
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
        // Camera bits (mode "A", right stick): yaw right 21 / left 20 and
        // height down 19 / up 18 scaled by the right stick's magnitude;
        // R3 (bit 17) recentres. Height drives the follow camera's radius
        // here [test-room mapping of the original height adjust].
        let held = game.pad.held;
        let [_, _, magnitude] = pad::analog(game.pad_data.right);
        let axis = |pos: u64, neg: u64| {
            ((held & pos != 0) as i32 - (held & neg != 0) as i32) as f32 * magnitude
        };
        turn = axis(0x20_0000, 0x10_0000);
        zoom = axis(0x8_0000, 0x4_0000);
        if game.pad.pressed & 0x2_0000 != 0 {
            rig.recenter_queued = true;
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
        let motion = game.player.motion_id(&game.clips);
        let ptya_id = match action {
            Action::Attack(i) | Action::AirAttack(i) => Some(game.clips.records[i].id),
            _ => None,
        };
        let combo = [game.player.combo.step, game.player.combo.finish];
        game.event(
            serde_json::json!({"event":"action","tick":tick,"action":format!("{action:?}"),"armed":armed,
            "position":position,"landed":step.landed,"motion":motion,"ptya":ptya_id,"combo":combo}),
        );
    }
    sora_sounds(&mut game);
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

/// The camera's view matrix in game world space (Y down), in the effect
/// VM's convention: view x right, y down, z forward.
fn game_view(eye: [f32; 3], look_at: [f32; 3]) -> effect_vm::math::Mat {
    let e = Vec3::from_array(eye);
    let f = (Vec3::from_array(look_at) - e).normalize_or(Vec3::Z);
    let down = (Vec3::Y - f * Vec3::Y.dot(f)).normalize_or(Vec3::Y);
    let right = down.cross(f);
    [
        [right.x, down.x, f.x, 0.],
        [right.y, down.y, f.y, 0.],
        [right.z, down.z, f.z, 0.],
        [-right.dot(e), -down.dot(e), -f.dot(e), 1.],
    ]
}

/// The enemy's object matrix in game world space (scene yaw θ is a game
/// rotation of −θ about Y) composed with a bound element's transform.
fn bound_world(position: [f32; 3], yaw: f32, element: &effect::Element) -> effect_vm::math::Mat {
    let object = effect_vm::instance_matrix(to_world(position), [0., -yaw, 0.], [1.; 3]);
    let local = effect_vm::instance_matrix(element.translation, element.rotation, element.scale);
    effect_vm::math::mul(&object, &local)
}

/// Starts requested effects, steps every running instance one game frame
/// and keeps the quads they drew.
fn effects_tick(mut game: ResMut<Game>, rig: Res<CameraRig>, mut fx: ResMut<Effects>) {
    let tick = game.tick;
    let view = game_view(rig.camera.eye, rig.camera.look_at);
    for request in std::mem::take(&mut game.effect_queue) {
        let (pax, element, world, label, bound) = match request {
            EffectRequest::Hit {
                effect,
                random,
                contact,
                direction,
            } => {
                let Some(pax) = fx.weapon.clone() else {
                    continue;
                };
                // mask = (1 << (32 + hitEffect − 1)) | 1. A random count
                // adds an LCG pick (DAT_140783ca0 × 0x10dcd) whose seed is
                // untraced; Sora's rows have none, so those hits are exact.
                if random != 0 {
                    game.event(serde_json::json!({"event":"effect_random_unmodelled","tick":tick,"count":random}));
                }
                let b = effect as u64 - 1;
                let mask = (1u64 << (32 + b)) | 1;
                let Some(element) = pax.by_category(mask).cloned() else {
                    game.event(serde_json::json!({"event":"effect_missing","tick":tick,"mask":format!("{mask:#x}")}));
                    continue;
                };
                let d = to_world(direction.to_array());
                let pitch = (-d[1]).atan2((d[0] * d[0] + d[2] * d[2]).sqrt());
                let yaw = d[0].atan2(d[2]);
                let world = effect_vm::instance_matrix(
                    to_world(contact.to_array()),
                    [pitch, yaw, 0.],
                    element.scale,
                );
                (pax, element, world, "hit", None)
            }
            EffectRequest::EnemyDead => {
                let (Some(pax), Some(rt)) = (fx.enemy.clone(), game.enemy.as_ref()) else {
                    continue;
                };
                let Some(element) = pax.by_id(2).cloned() else {
                    continue;
                };
                let world = bound_world(rt.enemy.position, rt.enemy.yaw, &element);
                (pax, element.clone(), world, "enemy_dead", Some(element))
            }
            EffectRequest::System { id, at } => {
                let Some(pax) = fx.system.clone() else {
                    continue;
                };
                let Some(element) = pax.by_id(id).cloned() else {
                    continue;
                };
                let world = effect_vm::instance_matrix(
                    to_world(at.to_array()),
                    element.rotation,
                    element.scale,
                );
                (pax, element, world, "system", None)
            }
        };
        // The element's own sound [inferred: played as a plain request id].
        if element.sound >= 0 {
            game.sound_queue.push((element.sound as u32, "effect_se"));
        }
        let Some(dpx) = pax.effect(element.effect_number) else {
            continue;
        };
        let Some(instance) = effect_vm::Instance::new(
            pax.dpds[dpx.dpd].clone(),
            dpx.pdata,
            world,
            element.start_wait,
        ) else {
            continue;
        };
        // At most 8 hit effects live (FUN_1403d9be0 ring).
        if label == "hit"
            && fx.running.iter().filter(|r| r.label == "hit").count() >= 8
            && let Some(oldest) = fx.running.iter().position(|r| r.label == "hit")
        {
            fx.running.remove(oldest);
        }
        game.event(serde_json::json!({"event":"effect","tick":tick,"kind":label,
            "element":element.id,"effect_number":element.effect_number,"dpd":dpx.dpd,"pdata":dpx.pdata,
            "position":[world[3][0],world[3][1],world[3][2]]}));
        fx.running.push(RunningEffect {
            instance,
            label,
            bound,
        });
    }
    let enemy = game
        .enemy
        .as_ref()
        .map(|rt| (rt.enemy.position, rt.enemy.yaw));
    let Effects {
        running,
        rng,
        quads,
        view_inverse,
        reported,
        ..
    } = &mut *fx;
    let mut spread: Vec<(&'static str, f32)> = Vec::new();
    quads.clear();
    for r in running.iter_mut() {
        if let (Some(element), Some((position, yaw))) = (&r.bound, enemy) {
            r.instance.world = bound_world(position, yaw, element);
        }
        let drawn = r.instance.frame(&view, rng);
        // Farthest quad centre from the instance origin (view space).
        let o = effect_vm::math::apply(&view, r.instance.world[3]);
        let far = drawn
            .iter()
            .map(|q| {
                let c: [f32; 3] =
                    std::array::from_fn(|i| q.corners.iter().map(|p| p[i]).sum::<f32>() / 4.);
                ((c[0] - o[0]).powi(2) + (c[1] - o[1]).powi(2) + (c[2] - o[2]).powi(2)).sqrt()
            })
            .fold(0f32, f32::max);
        spread.push((r.label, far));
        quads.extend(drawn);
        for &kind in &r.instance.unsupported {
            if !reported.contains(&kind) {
                reported.push(kind);
                game.event(serde_json::json!({"event":"effect_unsupported","tick":tick,"program":kind,"effect":r.label}));
            }
        }
    }
    running.retain(|r| !r.instance.finished());
    *view_inverse = Mat4::from_cols_array_2d(&view).inverse();
    if tick.is_multiple_of(10) && !running.is_empty() {
        game.event(serde_json::json!({"event":"effects_state","tick":tick,"running":running.len(),
            "parts":running.iter().map(|r| r.instance.live_parts()).sum::<usize>(),"quads":quads.len(),
            "spread":spread}));
    }
}

/// Rebuilds the effect meshes from the last tick's quads: view space back
/// to game world through the inverse view, then to scene coordinates.
/// Colours are PS2 gamma-space multipliers, converted to linear.
fn present_effects(
    fx: Res<Effects>,
    targets: Query<(&EffectMesh, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (target, handle) in &targets {
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        for q in fx
            .quads
            .iter()
            .filter(|q| q.tbp == windows_shell::effects::ATLAS_TBP)
            .filter(|q| (q.blend == 0x48) == target.additive)
        {
            let corner = |i: usize| {
                let p = fx
                    .view_inverse
                    .transform_point3(Vec3::from_array(q.corners[i]));
                from_world(p.to_array()).to_array()
            };
            let colour = [
                q.colour[0].max(0.).powf(2.2),
                q.colour[1].max(0.).powf(2.2),
                q.colour[2].max(0.).powf(2.2),
                q.colour[3].clamp(0., 1.),
            ];
            for i in [0, 1, 2, 0, 2, 3] {
                positions.push(corner(i));
                uvs.push([q.uv[i][0] * fx.uv_scale[0], q.uv[i][1] * fx.uv_scale[1]]);
                colors.push(colour);
            }
        }
        if positions.is_empty() {
            positions = vec![[0.; 3]; 3];
            uvs = vec![[0.; 2]; 3];
            colors = vec![[0.; 4]; 3];
        }
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        }
    }
}

/// A collision shape for the talk camera: bone, offset, height offset,
/// radius and height (model space, Y up).
#[derive(Clone, Copy, Debug)]
struct Shape {
    bone: usize,
    offset: Vec3,
    pos_h: f32,
    radius: f32,
    height: f32,
}

struct NpcSource {
    model: model_inspection::InspectionModel,
    frames: Vec<Vec<Vec3>>,
    bones: Vec<HashMap<usize, Mat4>>,
    loop_start: u32,
    target: Shape,
    sheet: Sheet,
    font: SystemFont,
    lines: HashMap<u32, Vec<u8>>,
    colour: u32,
    label: Vec<u8>,
    offset: Vec3,
    yaw: f32,
    script: Vec<windows_shell::dialogue::Op>,
}

/// Sora's OBJ ellipsoid (P_EX100 type 23 entry 1): radius, height and its
/// centre above the root; the towns-person reaction sphere (composed for
/// Axel): centre 200 up, radius 200, height 200 (dialogue-01 §2.2).
const SORA_OBJ: [f32; 3] = [35., 55., 85.];
const REACTION_SPHERE: [f32; 3] = [200., 200., 200.];
/// cmd 31 "Talk": angle gate cos 135° (+0x10); message 0x81ea.
const TALK_DOT: f32 = -std::f32::consts::FRAC_1_SQRT_2;
const TALK_LABEL: u32 = 0x81ea;
/// Screen cross-dissolve length (FUN_140163040(8.0, 1): 2 × 8 frames) and
/// the talk camera's field of view (0x14071a290, 60°).
const DISSOLVE_FRAMES: u32 = 16;
const TALK_FOV: f32 = std::f32::consts::FRAC_PI_3;

/// The field-talk state: the NPC, the window art and font, the running
/// script and the talk camera (FUN_140401410 / FUN_140400f90).
#[derive(Resource)]
struct Talk {
    position: Vec3,
    yaw: f32,
    home_yaw: f32,
    tick: u32,
    frames: Vec<Vec<Vec3>>,
    bones: Vec<HashMap<usize, Mat4>>,
    loop_start: u32,
    target: Shape,
    indices: Vec<Vec<u32>>,
    sheet: Sheet,
    font: SystemFont,
    lines: HashMap<u32, Vec<u8>>,
    colour: u32,
    label: Vec<u8>,
    script: Vec<windows_shell::dialogue::Op>,
    /// Script position while talking.
    active: Option<usize>,
    current: Option<windows_shell::dialogue::TalkWindow>,
    closing: Vec<windows_shell::dialogue::TalkWindow>,
    wait: f32,
    /// Talk camera eye and target (scene coordinates).
    camera: Option<(Vec3, Vec3)>,
    /// Remaining cross-dissolve frames, and a snapshot to take this frame.
    dissolve: u32,
    snapshot: bool,
    /// Reaction candidate this frame: Some(dimmed).
    reaction: Option<bool>,
}

#[derive(Component)]
struct NpcRoot;
#[derive(Component)]
struct NpcPart(usize);
#[derive(Component)]
struct SnapshotCamera;

fn load_npc(
    source: &ProbeSource,
    base: &Path,
    config: &Config,
    npc: &NpcConfig,
) -> Result<NpcSource, Box<dyn std::error::Error>> {
    let menu = config
        .menu
        .as_ref()
        .ok_or("an NPC needs the menu sources")?;
    let mut models =
        model_inspection::read_selected_models(source, &base.join(&npc.model_selection))?;
    if models.len() != 1 {
        return Err("NPC selection requires exactly one model".into());
    }
    let model = models.remove(0);
    let collision = object_collision::read(bar_entry_typed(
        &read_archive(source, &npc.target_model)?,
        23,
    )?)?;
    let t = collision
        .iter()
        .find(|e| e.kind == object_collision::TARGET && e.bone >= 0)
        .ok_or("no TARGET shape for the talk camera")?;
    let target = match &npc.camera_shape {
        Some(c) => Shape {
            bone: c.bone,
            offset: Vec3::from_array(c.offset),
            pos_h: c.pos_h,
            radius: c.radius,
            height: c.height,
        },
        None => Shape {
            bone: t.bone as usize,
            offset: Vec3::new(
                t.position[0] as f32,
                t.position[1] as f32,
                t.position[2] as f32,
            ),
            pos_h: t.position_height as f32,
            radius: t.radius as f32,
            height: t.height as f32,
        },
    };
    let selected = motion::read_slots(
        source,
        &npc.motion_set.index,
        npc.motion_set.ordinal,
        npc.motion_set.name_hash,
        &[0],
    )?;
    let sel = selected.first().ok_or("NPC idle motion absent")?;
    let (clip, key_start) = clip_for(sel)?;
    let (mut frames, mut bones) = (Vec::new(), Vec::new());
    for offset in 0..=clip.end {
        let (pose, evaluated) = model
            .skinning
            .sample_evaluated(&sel.motion, (key_start + offset) as f32)?;
        let r = Vec3::from_array(evaluated.root_translation);
        frames.push(
            pose.position_bits
                .iter()
                .map(|p| Vec3::from_array(p.map(f32::from_bits)) - r)
                .collect(),
        );
        let mut b = HashMap::new();
        if let Some(m) = pose.matrix_bits.get(target.bone) {
            let mut m = Mat4::from_cols_array(&m.map(f32::from_bits));
            m.w_axis -= r.extend(0.);
            b.insert(target.bone, m);
        }
        bones.push(b);
    }
    fn loc(l: &Locator) -> (&str, usize, [u8; 16]) {
        (l.index.as_str(), l.ordinal, l.name_hash)
    }
    let sheet = hud::load_window_sheet(source, loc(&npc.font_info), loc(&npc.world_messages))?;
    let font = hud::load_font(source, loc(&npc.font_image), loc(&npc.font_info), b"evt\0")?;
    let texts = hud::load_messages(
        source,
        &npc.messages.index,
        npc.messages.ordinal,
        npc.messages.name_hash,
        &npc.script,
    )?;
    let lines = npc.script.iter().copied().zip(texts).collect();
    let colour = hud::ftst_colour(&read_archive(source, &config.system)?, 5, npc.world)?;
    let label = hud::load_messages(
        source,
        &menu.messages.index,
        menu.messages.ordinal,
        menu.messages.name_hash,
        &[TALK_LABEL],
    )?
    .remove(0);
    Ok(NpcSource {
        model,
        frames,
        bones,
        loop_start: clip.loop_start,
        target,
        sheet,
        font,
        lines,
        colour,
        label,
        offset: Vec3::from_array(npc.offset),
        yaw: npc.yaw,
        // Composed libretto script: op 1 per message id.
        script: npc
            .script
            .iter()
            .map(|&id| windows_shell::dialogue::Op::Message(id))
            .collect(),
    })
}

/// FUN_140175e50 (flag 1): the target is the shape centre; the eye sits
/// `radius` from it toward the player, raised by `height`.
fn talk_camera(talk: &Talk, player: Vec3) -> (Vec3, Vec3) {
    let s = talk.target;
    let m = talk.bones[(talk.tick as usize).min(talk.bones.len() - 1)]
        .get(&s.bone)
        .copied()
        .unwrap_or(Mat4::IDENTITY);
    let turn = Quat::from_rotation_y(talk.yaw);
    let centre = talk.position + turn * (m.transform_point3(s.offset) + Vec3::Y * s.pos_h);
    let toward = ((player - centre) * Vec3::new(1., 0., 1.)).normalize_or(Vec3::Z);
    (centre + toward * s.radius + Vec3::Y * s.height, centre)
}

/// One frame of field talk: the NPC's idle, the reaction-command test
/// (FUN_140407de0 / FUN_1404088f0), and while talking the script
/// (FUN_1403ecb60), its windows and Sora turning toward the NPC.
fn talk_tick(mut game: ResMut<Game>, talk: Option<ResMut<Talk>>, hud: Option<ResMut<Hud>>) {
    use windows_shell::dialogue::{Op, TalkWindow};
    let Some(mut talk) = talk else { return };
    let tick = game.tick;
    let autoplay = game.autoplay.is_some();
    let rc = std::mem::take(&mut game.rc_queued) || (autoplay && AUTOPLAY_TALK.contains(&tick));
    let advance =
        std::mem::take(&mut game.advance_queued) || (autoplay && AUTOPLAY_ADVANCE.contains(&tick));
    // The talk motion (0x32) is absent from Axel's set, so he keeps idling.
    talk.tick += 1;
    if talk.tick as usize >= talk.frames.len() {
        talk.tick = talk.loop_start;
    }
    talk.dissolve = talk.dissolve.saturating_sub(1);
    let sora = Vec3::from_array(game.player.position);
    if talk.active.is_none() {
        let sora_centre = sora + Vec3::Y * SORA_OBJ[2];
        let npc_centre = talk.position + Vec3::Y * REACTION_SPHERE[2];
        let d = npc_centre - sora_centre;
        let (r, h) = (
            REACTION_SPHERE[0] + SORA_OBJ[0],
            REACTION_SPHERE[1] + SORA_OBJ[1],
        );
        let inside = (d.x * d.x + d.z * d.z) / (r * r) + d.y * d.y / (h * h) < 1.;
        // cmd flags bit 4: no Talk in battle (an enemy is active).
        let battle = game.enemy.as_ref().is_some_and(|rt| rt.enemy.alive());
        let facing = Vec3::new(game.player.yaw.sin(), 0., game.player.yaw.cos());
        let dir = (d * Vec3::new(1., 0., 1.)).normalize_or_zero();
        talk.reaction = (inside && !battle && !game.player.action.airborne())
            .then(|| dir.dot(facing) < TALK_DOT);
        if rc && talk.reaction == Some(false) {
            // FUN_140401410 → FUN_140400f90: lock Sora, snap the NPC to face
            // him, cross-dissolve to the talk camera.
            talk.active = Some(0);
            game.talking = true;
            let to_sora = sora - talk.position;
            talk.yaw = to_sora.x.atan2(to_sora.z);
            talk.snapshot = true;
            talk.dissolve = DISSOLVE_FRAMES;
            let cam = talk_camera(&talk, sora);
            talk.camera = Some(cam);
            game.event(serde_json::json!({"event":"talk_start","tick":tick,
                "camera_eye":cam.0.to_array(),"camera_target":cam.1.to_array()}));
        }
    }
    if let Some(mut i) = talk.active {
        // Sora turns toward the NPC at his turn rate (FUN_1403b65c0).
        let v = talk.position - sora;
        let want = v.x.atan2(v.z);
        let step = game.params.turn_speed;
        let delta = (want - game.player.yaw + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        game.player.yaw += delta.clamp(-step, step);
        loop {
            let Some(op) = talk.script.get(i).cloned() else {
                // End: restore facing, camera (with a dissolve) and control.
                talk.active = None;
                talk.yaw = talk.home_yaw;
                talk.camera = None;
                talk.snapshot = true;
                talk.dissolve = DISSOLVE_FRAMES;
                game.talking = false;
                game.event(serde_json::json!({"event":"talk_end","tick":tick}));
                break;
            };
            match op {
                Op::Message(id) => match talk.current.as_ref().map(|w| w.state) {
                    None => {
                        let text = talk.lines.get(&id).cloned().unwrap_or_default();
                        let window = TalkWindow::new(
                            &talk.sheet.sequence,
                            text,
                            talk.colour,
                            &talk.font.widths,
                        );
                        game.event(serde_json::json!({"event":"talk_window","tick":tick,
                            "message":id,"size":window.size()}));
                        talk.current = Some(window);
                        break;
                    }
                    Some(3) => {
                        let mut w = talk.current.take().unwrap();
                        w.close();
                        talk.closing.push(w);
                        i += 1;
                    }
                    Some(_) => break,
                },
                Op::Wait(frames) => {
                    if talk.wait <= 0. {
                        talk.wait = frames;
                    }
                    talk.wait -= 1.;
                    if talk.wait > 0. {
                        break;
                    }
                    i += 1;
                }
                Op::Stop => i = usize::MAX,
                Op::Signal(_) | Op::Other(..) => i += 1,
            }
            if talk.active.is_some() {
                talk.active = Some(i);
            }
        }
    }
    let mut sounds = Vec::new();
    if let Some(w) = talk.current.as_mut() {
        sounds.extend(w.step(advance));
    }
    for w in &mut talk.closing {
        sounds.extend(w.step(false));
    }
    talk.closing.retain(|w| !w.deleted());
    for s in sounds {
        game.sound_queue.push((s, "talk"));
    }
    if let Some(mut hud) = hud
        && let Some(m) = hud.menu.as_mut()
    {
        let label = talk.label.clone();
        m.menu.reaction = if game.talking {
            None
        } else {
            talk.reaction
                .map(|dimmed| command::Reaction { label, dimmed })
        };
    }
}

fn present_npc(
    talk: Option<Res<Talk>>,
    mut roots: Query<&mut Transform, With<NpcRoot>>,
    parts: Query<(&NpcPart, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(talk) = talk else { return };
    if let Ok(mut t) = roots.single_mut() {
        t.translation = talk.position;
        t.rotation = Quat::from_rotation_y(talk.yaw);
    }
    let frame = &talk.frames[(talk.tick as usize).min(talk.frames.len() - 1)];
    for (part, handle) in &parts {
        let positions: Vec<[f32; 3]> = talk.indices[part.0]
            .iter()
            .map(|&i| frame[i as usize].to_array())
            .collect();
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
    }
}

/// FUN_140163040 / FUN_1401635a0: on a camera cut the old frame is
/// snapshotted and drawn over the new view at alpha falling 1 → 0 over 16
/// frames. The snapshot renders the old view with a second camera before
/// `camera_control` moves the main one.
#[allow(clippy::type_complexity)]
fn dissolve_present(
    talk: Option<ResMut<Talk>>,
    main: Query<(&Transform, &Projection), CameraOnly>,
    mut snapshot: Query<
        (&mut Camera, &mut Transform, &mut Projection),
        (With<SnapshotCamera>, Without<SoraRoot>, Without<WeaponRoot>),
    >,
) {
    let Some(mut talk) = talk else { return };
    let Ok((mut camera, mut transform, mut projection)) = snapshot.single_mut() else {
        return;
    };
    camera.is_active = false;
    if std::mem::take(&mut talk.snapshot)
        && let Ok((t, p)) = main.single()
    {
        *transform = *t;
        *projection = p.clone();
        camera.is_active = true;
    }
}

/// Frame-8 sound triggers of Sora's current clip (the Keyblade's motion)
/// reached since the last scan; a new action or a loop rescans from 0.
fn sora_sounds(game: &mut Game) {
    let clip = game.poses[&(game.player.action, game.player.armed)];
    let frame = game.player.motion_frame(&game.clips);
    let started = game.action_started;
    let from = match game.sound_frame {
        Some((s, f)) if s == started && f <= frame => Some(f),
        _ => None,
    };
    for &(at, id) in &game.clip_poses[clip].sounds {
        if from.is_none_or(|f| at > f) && at <= frame {
            game.sound_queue.push((id, "swing"));
        }
    }
    game.sound_frame = Some((started, frame));
}

/// Plays this tick's requests and keeps the low-HP alarm (system sound 9,
/// looped) in step with the gauge's low-HP state: started when HP <= max/4
/// and stopped when HP recovers or Sora is KO (FUN_14017f050).
fn play_sounds(mut game: ResMut<Game>, mut audio: ResMut<Audio>, hud: Option<Res<Hud>>) {
    let tick = game.tick;
    let mut requests = std::mem::take(&mut game.sound_queue);
    if let Some(hud) = hud {
        let s = hud.stats;
        let low = s.hp > 0 && s.hp <= s.max_hp / 4;
        if low && !audio.alarm {
            requests.push((SE_LOW_HP, "low_hp_alarm"));
        } else if !low && audio.alarm {
            if let Some(output) = &audio.output
                && let Ok(mut mixer) = output.mixer.lock()
            {
                mixer.stop(SE_LOW_HP);
            }
            game.event(serde_json::json!({"event":"sound_stop","tick":tick,"id":SE_LOW_HP}));
        }
        audio.alarm = low;
    }
    for (id, cause) in requests {
        let Audio { output, banks, .. } = &mut *audio;
        let resolved = match banks.request(id) {
            Ok(r) => r,
            Err(error) => {
                eprintln!("Sound {id} failed to decode: {error}");
                None
            }
        };
        game.event(serde_json::json!({"event":"sound","tick":tick,"id":id,"cause":cause,
            "layers":resolved.as_ref().map(|r| r.layers.len()),
            "volume":resolved.as_ref().map(|r| r.volume),"looped":resolved.as_ref().map(|r| r.looped)}));
        if let (Some(output), Some(r)) = (output.as_ref(), resolved)
            && let Ok(mut mixer) = output.mixer.lock()
        {
            for layer in r.layers {
                mixer.play(layer, id, r.volume, 1.);
            }
        }
    }
}

/// HUD frame: test-room debug keys (F4 blow hit, F5/F6 HP -/+2, F7/F8 MP -/+10, F9 drive +10)
/// or the unattended script change stats, then the gauge logic runs.
fn hud_tick(keys: Res<ButtonInput<KeyCode>>, mut game: ResMut<Game>, hud: Option<ResMut<Hud>>) {
    if keys.just_pressed(KeyCode::F4) && game.autoplay.is_none() {
        game.debug_blow = true;
    }
    let Some(mut hud) = hud else { return };
    let mut s = hud.stats;
    if let Some(rt) = game.enemy.as_mut() {
        // Combat drives HP; G refills it.
        if keys.just_pressed(KeyCode::F6) {
            rt.sora_hp = rt.sora_target.max_hp;
        }
        s.hp = rt.sora_hp;
        s.max_hp = rt.sora_target.max_hp;
    } else if game.autoplay.is_some() {
        match game.tick {
            420 => s.hp = s.max_hp * 7 / 10,
            640 => s.hp = s.max_hp / 5,
            900 => s.hp = s.max_hp,
            _ => {}
        }
    } else {
        if keys.just_pressed(KeyCode::F5) {
            s.hp = (s.hp - 2).max(0);
        }
        if keys.just_pressed(KeyCode::F6) {
            s.hp = (s.hp + 2).min(s.max_hp);
        }
        if keys.just_pressed(KeyCode::F7) {
            game.player.mp.add(-10);
        }
        if keys.just_pressed(KeyCode::F8) {
            game.player.mp.add(10);
        }
        if keys.just_pressed(KeyCode::F9) {
            s.drive = (s.drive + 10).min(100);
        }
    }
    // MP is Sora's (casts spend it, MP Charge refills it).
    s.mp = game.player.mp.cur;
    s.max_mp = game.player.mp.max;
    let mp = game.player.mp;
    s.mp_charge = mp.charging().then_some((mp.remaining, mp.total));
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
    talk: Option<Res<Talk>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut hud) = hud else { return };
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
                pad: 1,
            },
            first,
            first + 1,
        );
    }
    draws.extend(gauge.draw(&sheets.iter().map(|s| &s.sequence).collect::<Vec<_>>()));
    // Message windows (task priority 25000) draw above the HUD.
    if let Some(t) = &talk {
        let first = textures.len();
        textures.push(Texture {
            rgba: &t.sheet.texture_rgba,
            size: t.sheet.texture_size.map(|v| v as usize),
            texel_scale: t.sheet.texel_scale as f32,
        });
        textures.push(Texture {
            rgba: &t.font.rgba,
            size: t.font.size.map(|v| v as usize),
            texel_scale: t.font.texel_scale as f32,
        });
        for w in t.closing.iter().chain(&t.current) {
            draws.extend(w.draw(first, first + 1));
        }
    }
    canvas.render(&draws, &textures);
    if let Some(mut target) = images.get_mut(image.id()) {
        target.data = Some(canvas.rgba.clone());
    }
}

/// One tick of the Keyblade's attack windows against the enemy, then the
/// enemy's own tick. Attack spheres: weapon bone 2 entries of the window's
/// collision group, placed through Sora's hand and swept from last frame;
/// target spheres: the enemy's HIT entries on its animated bones.
/// The enemy's TARGET shapes this tick, in simulation space.
fn enemy_targets(rt: &EnemyRuntime) -> Vec<player_sim::ptya::Target> {
    let e = &rt.enemy;
    let turn = Quat::from_rotation_y(e.yaw);
    let at = Vec3::from_array(e.position);
    let Some(ec) = rt.clips.0.get(&e.motion) else {
        return Vec::new();
    };
    let Some(bones) = ec
        .bones
        .get((e.motion_tick as usize).min(ec.bones.len().saturating_sub(1)))
    else {
        return Vec::new();
    };
    rt.targets
        .iter()
        .filter_map(|&(b, offset, radius, height)| {
            let m = bones.get(&b)?;
            Some(player_sim::ptya::Target {
                centre: (at + turn * m.transform_point3(offset)).to_array(),
                radius,
                height,
            })
        })
        .collect()
}

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
        sound_queue,
        effect_queue,
        magic,
        debug_blow,
        ..
    } = game;
    let Some(rt) = enemy.as_mut() else { return };
    let debug_blow =
        std::mem::take(debug_blow) || (autoplay.is_some() && AUTOPLAY_BLOWS.contains(&tick));
    let act_at_start = rt.enemy.current_act();
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
    // Live guard (ATKP Type 2/3) spheres: centre, radius, ATKP.
    let mut guards: Vec<(Vec3, f32, u16, Option<u64>)> = Vec::new();
    if !cp.blade.is_empty() {
        let f = frame.min(cp.blade.len() - 1);
        let to_model = cp.weapon[f.min(cp.weapon.len() - 1)] * cp.blade[f];
        for (w, &(start, end, atkp, group)) in cp.hits.iter().enumerate() {
            if (frame as u32) < start || frame as u32 > end {
                continue;
            }
            let instance = (*action_started << 4) | w as u64;
            // Guard types never test bodies (FUN_1403cff40).
            let guard = rt.rows.get(&atkp).is_some_and(|r| matches!(r.kind, 2 | 3));
            // FUN_1403cf400 / FUN_1403cf490: the attack's id and combo
            // group (ATKP ComboGroup, else a fresh one per attack).
            let (aid, _) = *rt.instance_ids.entry(instance).or_insert_with(|| {
                let combo = rt
                    .steps
                    .get(&(atkp, 0))
                    .map_or(0, |r| r[0].combo_group as i32);
                let group = if combo != 0 {
                    combo
                } else {
                    rt.next_group += 1;
                    rt.next_group
                };
                (rt.ids.allocate(), group)
            });
            // FUN_1403d2060: an attack hits a target once per record bit.
            if !guard && rt.enemy.record.has(aid) {
                continue;
            }
            for (i, &(offset, radius)) in rt.blade.get(&group).into_iter().flatten().enumerate() {
                let centre = sora_at + sora_turn * to_model.transform_point3(offset);
                let previous = rt.previous.insert((instance, i), centre);
                if guard {
                    guards.push((centre, radius, atkp, None));
                    continue;
                }
                if !targetable {
                    continue;
                }
                for c in combat::swept(centre.to_array(), previous.map(|p| p.to_array()), radius) {
                    if hit.is_none()
                        && let Some(&(target, target_radius, high)) = body
                            .iter()
                            .find(|(b, r, _)| combat::spheres_overlap(c, radius, b.to_array(), *r))
                    {
                        let at = (Vec3::from_array(c), target, target_radius);
                        hit = Some((instance, atkp, 0u8, high, at, None::<Vec3>, None::<u64>));
                    }
                }
            }
        }
    }
    // Spell volumes (YS::ATTACK_MAGIC): guards join the clash list, damaging
    // volumes are tested against the Shadow's HIT spheres after the weapon
    // (one body hit per target per frame, FUN_1403cff40).
    if let Some(m) = magic.as_mut() {
        let volumes = m.volumes.clone();
        for v in &volumes {
            ensure_attack_rows(rt, v.atkp, v.sub);
        }
        for v in &volumes {
            let centre = Vec3::from_array(v.centre);
            match v.role {
                player_sim::magic::Role::Guard => {
                    guards.push((centre, v.radius, v.atkp, Some(v.key)));
                }
                player_sim::magic::Role::Damage => {
                    if !targetable || hit.is_some() {
                        continue;
                    }
                    let key = magic_room::KEY_BASE | v.key;
                    let (aid, _) = *rt.instance_ids.entry(key).or_insert_with(|| {
                        let combo = rt
                            .steps
                            .get(&(v.atkp, v.sub))
                            .map_or(0, |r| r[0].combo_group as i32);
                        let group = if combo != 0 {
                            combo
                        } else {
                            rt.next_group += 1;
                            rt.next_group
                        };
                        (rt.ids.allocate(), group)
                    });
                    if rt.enemy.record.has(aid) {
                        continue;
                    }
                    for c in combat::swept_volume(v.centre, v.previous, v.radius, v.height) {
                        if hit.is_none()
                            && let Some(&(target, target_radius, high)) =
                                body.iter().find(|(b, r, _)| {
                                    combat::volume_hits_sphere(
                                        c,
                                        v.radius,
                                        v.height,
                                        b.to_array(),
                                        *r,
                                    )
                                })
                        {
                            let at = (Vec3::from_array(c), target, target_radius);
                            let moving = v.moving.map(Vec3::from_array);
                            hit = Some((key, v.atkp, v.sub, high, at, moving, Some(v.key)));
                        }
                    }
                }
            }
        }
    }
    // The enemy's record advances once a tick (FUN_1403cf4b0), like Sora's.
    let in_damage_motion = (10..=39).contains(&player.motion_id(clips));
    rt.sora_record.tick(1., in_damage_motion);
    rt.enemy.record.tick(1., false);
    if let Some((instance, atkp, sub, high, (attack_centre, target, target_radius), moving, key)) =
        hit
        && let Some(rows) = rt.steps.get(&(atkp, sub))
        && let Some(&(aid, group)) = rt.instance_ids.get(&instance)
    {
        // The hit direction (FUN_1404373e0): a moving volume's direction,
        // else toward the target from the volume's centre; the weapon uses
        // Sora's facing toward the Shadow.
        let d = match (moving, key) {
            (Some(m), _) => m,
            (None, Some(_)) => enemy_at - attack_centre,
            (None, None) => enemy_at - sora_at,
        } * Vec3::new(1., 0., 1.);
        let dir = d.normalize_or(Vec3::new(player.yaw.sin(), 0., player.yaw.cos()));
        // FUN_1403cf600: record the hit; the consecutive-hit count of its
        // combo group picks the ATKP level row (FUN_1403eb230).
        let cure = matches!(rows[0].kind, 5 | 6);
        let (counted, row) = rt.enemy.record.hit(aid, group, cure, |count| {
            let r = package_reader::battle_tables::step_index(rows, count);
            (r, rows[r].interval)
        });
        let attack = params_of(&rows[row]);
        // The attacker's ability terms (FUN_1403c12f0).
        let ab = &player.abilities;
        rt.sora.mods = combat::AttackerMods {
            player_type: true,
            mp_charging: player.mp_charging,
            berserk_strength: ab.berserk_strength as i32,
            element_percent: ab.element.map(i32::from),
            combo_step: player.combo.step as i32,
            ground_limit: ab.ground_combo as i32,
            air_limit: ab.air_combo as i32,
            combo_boost: ab.combo_boost as i32,
            air_combo_boost: ab.air_combo_boost as i32,
            reaction_boost: ab.reaction_boost as i32,
        };
        if !counted {
            // Recorded but not counted: no damage and no reaction.
        } else if let Some(damage) =
            rt.enemy
                .hit(dir.to_array(), high, &attack, &rt.sora, rt.difficulty)
        {
            // FUN_1403d12c0: the attacker's obj+0x120 bit 25, which lets
            // the next press chain the combo.
            player.connected = true;
            // ATKP HitSound (FUN_1403d12c0 → FUN_1403dadf0); 500xxx ids
            // resolve to the equipped weapon's bank, 500 for this Keyblade.
            if let Some(&id) = rt.hit_sounds.get(&atkp)
                && id >= 0
            {
                sound_queue.push((id as u32, "hit"));
            }
            // Spark at the contact point on the target's hit shape
            // (FUN_1403d1b40; a sphere here, so height = radius).
            if let Some(&(effect, random)) = rt.hit_effects.get(&atkp)
                && effect != 0
            {
                let v = attack_centre - target;
                let h = target_radius;
                let y = v.y.clamp(-h, h);
                let across = Vec3::new(v.x, 0., v.z).normalize_or_zero()
                    * target_radius
                    * (1. - (y / h) * (y / h)).max(0.).sqrt();
                effect_queue.push(EffectRequest::Hit {
                    effect,
                    random,
                    contact: target + across + Vec3::Y * y,
                    direction: dir,
                });
            }
            if let (Some(k), Some(m)) = (key, magic.as_mut()) {
                m.feedback.hit.push(k);
                // Reaction 5 (Magnet): held at a radius drawn between KB1
                // and KB2 (DAT_140783ca0 LCG), spinning by ATKP +0x10.
                if combat::reaction_class(attack.reaction) == 5
                    && let Some(v) = m.volumes.iter().find(|v| v.key == k)
                {
                    let r = &rows[row];
                    let hold = rt.rng.range(r.knockback[0] as f32, r.knockback[1] as f32);
                    rt.enemy.capture(
                        k,
                        Vec3::from_array(v.centre).to_array(),
                        hold,
                        r.spin as f32,
                    );
                }
            }
            let record = serde_json::json!({"event":"hit","tick":tick,"atkp":atkp,"sub":sub,"damage":damage,
                "step":row,"combo":player.combo.step,"enemy_hp":rt.enemy.hp,"reaction":format!("{:?}", rt.enemy.reaction),
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
    let mut clash = None;
    // FUN_1403d1f40: R = ReflectRange (0: no test); the attacker of the
    // other attack must lie within R° of this attack's owner's facing.
    let cone = |row: &package_reader::battle_tables::Attack, facing: Vec3, v: Vec3| {
        if row.reflect_range == 0 {
            return true;
        }
        let turn = Quat::from_rotation_y((row.reflect_angle as f32 * 2.).to_radians());
        let f = turn * facing;
        v.dot(f) < -(row.reflect_range as f32).to_radians().cos()
    };
    let sora_facing = Vec3::new(player.yaw.sin(), 0., player.yaw.cos());
    let shadow_facing = Vec3::new(e.yaw.sin(), 0., e.yaw.cos());
    let from_shadow = ((sora_at - enemy_at) * Vec3::new(1., 0., 1.)).normalize_or_zero();
    let clashes = |guard: u16, claw: u16| {
        let (Some(g), Some(c)) = (rt.rows.get(&guard), rt.rows.get(&claw)) else {
            return false;
        };
        // DAT_1405c5e98: the symmetric clash pairs 0-0, 0-2, 0-3, 1-3.
        let pair = (g.kind.min(c.kind), g.kind.max(c.kind));
        matches!(pair, (0, 0) | (0, 2) | (0, 3) | (1, 3))
            && cone(g, sora_facing, from_shadow)
            && cone(c, shadow_facing, -from_shadow)
    };
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
            let (aid, _) = *rt.instance_ids.entry(instance).or_insert_with(|| {
                let combo = rt
                    .steps
                    .get(&(atkp, 0))
                    .map_or(0, |r| r[0].combo_group as i32);
                let group = if combo != 0 {
                    combo
                } else {
                    rt.next_group += 1;
                    rt.next_group
                };
                (rt.ids.allocate(), group)
            });
            if rt.sora_record.has(aid) {
                continue;
            }
            for (i, &(b, offset, radius)) in rt.claws.get(&group).into_iter().flatten().enumerate()
            {
                let Some(m) = bones.get(&b) else { continue };
                let centre = enemy_at + enemy_turn * m.transform_point3(offset);
                let previous = rt.claw_previous.insert((instance, i), centre);
                for c in combat::swept(centre.to_array(), previous.map(|p| p.to_array()), radius) {
                    // Attack-vs-attack clashes run before the body pass.
                    if clash.is_none()
                        && let Some(&(g, gr, guard, gkey)) =
                            guards.iter().find(|(g, gr, guard, _)| {
                                combat::spheres_overlap(c, radius, g.to_array(), *gr)
                                    && clashes(*guard, atkp)
                            })
                    {
                        clash = Some((instance, atkp, guard, Vec3::from_array(c), g, gr, gkey));
                    }
                    if sora_hit.is_none()
                        && clash.is_none_or(|k| k.0 != instance)
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
    // A guarded claw: FUN_1403d2140 both ways, Sora's FUN_1403d61d0 and the
    // Shadow's FUN_140411060.
    if let Some((instance, claw, guard, claw_at, g, gr, gkey)) = clash
        && let (Some(&gr_row), Some(&claw_row)) = (rt.rows.get(&guard), rt.rows.get(&claw))
    {
        // attack_get_reflect_count (+0xb0): each clash bumps the guard's
        // counter, which Reflect's counter tiers read.
        if let (Some(k), Some(m)) = (gkey, magic.as_mut()) {
            *m.reflected.entry(k).or_insert(0) += 1;
        }
        if let Some(&(aid, _)) = rt.instance_ids.get(&instance) {
            rt.sora_record.mark(aid);
        }
        // FUN_1403d61d0 first sets obj+0x120 bit 25 ("connected").
        player.connected = true;
        // FUN_1403d0f00: at the guard sphere's surface toward the claw.
        let contact = g + (claw_at - g).normalize_or_zero() * gr;
        let (s, t) = (gr_row.refact_self, claw_row.refact_other);
        let outcome = if s == 2 || t == 2 {
            effect_queue.push(EffectRequest::System {
                id: 13,
                at: contact,
            });
            "pass"
        } else if !player.action.airborne() && (s != 0 || t != 0) {
            // Sora is the player character (0x6c8 bit 7): effect 11.
            effect_queue.push(EffectRequest::System {
                id: 11,
                at: contact,
            });
            if player
                .guard_recoil([from_shadow.x, from_shadow.z], clips)
                .is_some()
            {
                *action_started = tick;
            }
            rt.sora_invulnerable = 80.;
            "guard"
        } else {
            // Bounce (motion 41 + slide 50) is not loaded in this room.
            effect_queue.push(EffectRequest::System {
                id: 13,
                at: contact,
            });
            "bounce"
        };
        if claw_row.refact_self != 2 && claw_row.reflect_motion != 0 {
            rt.enemy.reflect(
                claw_row.reflect_motion as usize,
                claw_row.reflect_hit_back as f32,
            );
        }
        let record = serde_json::json!({"event":"guard","tick":tick,"outcome":outcome,
            "guard_atkp":guard,"claw_atkp":claw,"reflect_motion":claw_row.reflect_motion,
            "contact":contact.to_array()});
        if let Some(file) = log.as_mut() {
            let _ = serde_json::to_writer(&mut *file, &record);
            let _ = std::io::Write::write_all(file, b"\n");
        }
    }
    rt.sora_invulnerable = (rt.sora_invulnerable - 1.).max(0.);
    if debug_blow && rt.sora_hp > 0 {
        // Reaction 4 with KB1 and KB2 both 0 (the SSTM blow), away from
        // the enemy; no damage.
        let d = ((sora_at - enemy_at) * Vec3::new(1., 0., 1.)).normalize_or(Vec3::Z);
        let hit = player_sim::Hit {
            reaction: 4,
            original: 4,
            direction: [d.x, d.z],
            high: true,
            knockback: [0, 0],
        };
        let started = player.react(&hit, &rt.enemy.sstm, clips);
        if started.is_some() {
            *action_started = tick;
        }
        let record = serde_json::json!({"event":"sora_blow_debug","tick":tick,
            "reaction":format!("{started:?}")});
        if let Some(file) = log.as_mut() {
            let _ = serde_json::to_writer(&mut *file, &record);
            let _ = std::io::Write::write_all(file, b"\n");
        }
    }
    // Aerial Recovery's range 27 (obj+0x18c bit 22) takes no damage.
    if player.range_invulnerable(clips) {
        rt.sora_invulnerable = rt.sora_invulnerable.max(1.);
    }
    if let Some((instance, atkp, _)) = sora_hit
        && rt.sora_invulnerable > 0.
    {
        // FUN_1403d65a0: no damage while +0xd70 runs (the hit is still
        // recorded).
        if let Some(&(aid, group)) = rt.instance_ids.get(&instance) {
            rt.sora_record.hit(aid, group, false, |_| (0, 0));
        }
        let record = serde_json::json!({"event":"sora_hit_invulnerable","tick":tick,"atkp":atkp});
        if let Some(file) = log.as_mut() {
            let _ = serde_json::to_writer(&mut *file, &record);
            let _ = std::io::Write::write_all(file, b"\n");
        }
    } else if let Some((instance, atkp, high)) = sora_hit
        && let Some(rows) = rt.steps.get(&(atkp, 0))
        && let Some(&(aid, group)) = rt.instance_ids.get(&instance)
        && rt.sora_hp > 0
    {
        // FUN_1403cf600 on Sora's record (obj+0xc98): a player-type object
        // locks out other attack groups for 30 ticks.
        let cure = matches!(rows[0].kind, 5 | 6);
        let (counted, row) = rt.sora_record.hit(aid, group, cure, |count| {
            let r = package_reader::battle_tables::step_index(rows, count);
            (r, rows[r].interval)
        });
        let attack = params_of(&rows[row]);
        if let Some(&id) = rt.hit_sounds.get(&atkp)
            && id >= 0
        {
            sound_queue.push((id as u32, "sora_hit"));
        }
        let ab = &player.abilities;
        let state = combat::TargetState {
            hp: rt.sora_hp,
            player_type: true,
            defender: ab.defender as i32,
            damage_control: ab.damage_control,
            damage_control_ratio: 50,
            difficulty: rt.difficulty,
        };
        let damage = if counted {
            combat::damage_with(&rt.shadow_attacker, &attack, &rt.sora_target, &state)
        } else {
            0
        };
        // FUN_1403d6670: Second Chance (ability 127) and Once More (128).
        let veto = combat::sora_veto(ab.has(127), ab.has(128), rt.sora_hp, 0, in_damage_motion);
        rt.sora_hp = combat::apply_damage(
            rt.sora_hp,
            0,
            rt.sora_target.max_hp,
            damage,
            veto && counted,
        );

        let d = (sora_at - enemy_at) * Vec3::new(1., 0., 1.);
        let dir = d.normalize_or(Vec3::Z);
        // FUN_1403d2fe0: the reaction class picks the motion and physics
        // (flinch and slide, air flinch and hop, blow).
        let hit = player_sim::Hit {
            reaction: attack.reaction,
            original: attack.reaction,
            direction: [dir.x, dir.z],
            high,
            knockback: attack.knockback,
        };
        let started = if counted {
            player.react(&hit, &rt.enemy.sstm, clips)
        } else {
            None
        };
        if started.is_some() {
            *action_started = tick;
        }
        // MP Rage (FUN_1403d5e50): damage taken outside MP Charge gives
        // MP by its share of max HP × the ability's value.
        if damage > 0 && !player.mp.charging() && player.abilities.mp_rage > 0. {
            let gain =
                (damage as f32 / rt.sora_target.max_hp as f32 * player.abilities.mp_rage) as i32;
            if gain > 0 {
                player.mp.add(gain);
            }
        }
        // FUN_140404d60: HP 0 plays DEAD.
        if rt.sora_hp == 0 && player.die(clips).is_some() {
            *action_started = tick;
        }
        let record = serde_json::json!({"event":"sora_hit","tick":tick,"atkp":atkp,"damage":damage,
            "counted":counted,"step":row,"sora_hp":rt.sora_hp,"reaction":format!("{started:?}")});
        if let Some(file) = log.as_mut() {
            let _ = serde_json::to_writer(&mut *file, &record);
            let _ = std::io::Write::write_all(file, b"\n");
        }
    }
    if rt.sora_hp == 0 {
        rt.ko_ticks += 1;
        if rt.ko_ticks >= 120 {
            // [test-room] No game-over flow: stand up with full HP.
            rt.sora_hp = rt.sora_target.max_hp;
            rt.ko_ticks = 0;
        }
    }
    // Healed while dead (the camp, a Cure): FUN_1403c3c30 stands him up.
    if player.dead() && rt.sora_hp > 0 {
        player.revive();
        *action_started = tick;
    }
    // A Magnet's hold follows its volume and ends with it.
    if let Some(h) = rt.enemy.held {
        match magic
            .as_ref()
            .and_then(|m| m.volumes.iter().find(|v| v.key == h.key))
        {
            Some(v) => {
                if let Some(held) = rt.enemy.held.as_mut() {
                    held.centre = v.centre;
                }
            }
            None => rt.enemy.release_held(),
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
    // The enemy motion's frame-8 sounds reached this tick.
    let e = &rt.enemy;
    let from = match rt.sound_tick {
        Some((serial, t)) if serial == e.motion_serial && t <= e.motion_tick => Some(t),
        _ => None,
    };
    if !e.gone
        && let Some(c) = rt.clips.0.get(&e.motion)
    {
        for &(at, id) in &c.sounds {
            if from.is_none_or(|t| at > t) && at <= e.motion_tick {
                sound_queue.push((id, "enemy"));
            }
        }
    }
    rt.sound_tick = Some((e.motion_serial, e.motion_tick));
    let after = rt.enemy.current_act();
    if act_at_start != after && after == Some(shadow::ActKind::Dead) {
        effect_queue.push(EffectRequest::EnemyDead);
    }
    if before != after
        && let Some(file) = log.as_mut()
    {
        let record = serde_json::json!({"event":"enemy_act","tick":tick,"act":format!("{after:?}"),
            "motion":rt.enemy.motion,"position":rt.enemy.position});
        let _ = serde_json::to_writer(&mut *file, &record);
        let _ = std::io::Write::write_all(file, b"\n");
    }
}

/// Places the spell-volume placeholders on this tick's volumes.
fn present_spells(
    game: Res<Game>,
    mut volumes: Query<(
        &SpellVolume,
        &mut Transform,
        &mut Visibility,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let live: Vec<_> = game
        .magic
        .as_ref()
        .map(|m| m.volumes.clone())
        .unwrap_or_default();
    for (slot, mut transform, mut visibility, material) in &mut volumes {
        match live.get(slot.0) {
            Some(v) => {
                *visibility = Visibility::Visible;
                transform.translation = Vec3::from_array(v.centre);
                // Horizontal radius r, vertical half-extent h.
                transform.scale = Vec3::new(v.radius, v.height, v.radius);
                if let Some(mut m) = materials.get_mut(&material.0) {
                    let c = spell_tint(v.atkp);
                    m.base_color = Color::srgba(c[0], c[1], c[2], c[3]);
                }
            }
            None => *visibility = Visibility::Hidden,
        }
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
type CameraOnly = (
    With<Camera3d>,
    Without<SoraRoot>,
    Without<WeaponRoot>,
    Without<SnapshotCamera>,
    Without<MenuCamera>,
);

fn camera_control(
    motion: Res<AccumulatedMouseMotion>,
    mut rig: ResMut<CameraRig>,
    mut camera: Query<(&mut Transform, &mut Projection), CameraOnly>,
    talk: Option<Res<Talk>>,
) {
    // Mouse-look turns the camera (the PC layer feeds mouse motion to the
    // right stick; its sensitivity is unresolved).
    rig.pending_turn = (rig.pending_turn + motion.delta.x * 0.1).clamp(-30., 30.);
    let (eye, look_at, fov) = match talk.as_ref().and_then(|t| t.camera) {
        // The talk camera (FUN_140175e50): 60° in the game's horizontal
        // 4:3 sense, converted like the follow camera's.
        Some((eye, target)) => (
            eye,
            target,
            2. * (240. / (320. / (TALK_FOV / 2.).tan())).atan(),
        ),
        None => (
            from_world(rig.camera.eye),
            from_world(rig.camera.look_at),
            rig.camera.vertical_fov(),
        ),
    };
    if let Ok((mut transform, mut projection)) = camera.single_mut() {
        *transform = Transform::from_translation(eye).looking_at(look_at, Vec3::Y);
        if let Projection::Perspective(p) = projection.as_mut()
            && p.fov != fov
        {
            p.fov = fov;
        }
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
        let blend = game.player.blend;
        let weight = if blend > 0. {
            (elapsed / blend).clamp(0., 1.)
        } else {
            1.
        };
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
    music: Option<Res<MusicRes>>,
    compose: Option<Res<Compose>>,
) {
    // Unattended runs warm up once the music decodes have finished (they
    // compete with the first pipeline compiles).
    let music_ready = music.is_none_or(|m| {
        m.pending
            .iter()
            .flatten()
            .all(|slot| slot.lock().map_or(true, |r| r.is_some()))
    });
    let settled = compose.is_none_or(|c| c.stable_frames >= AUTOPLAY_WARMUP_FRAMES);
    if music_ready && settled {
        game.rendered_frames += 1;
    }
    if game.autoplay.is_some()
        && time.is_paused()
        && !game.capture_pending
        && game.rendered_frames >= AUTOPLAY_WARMUP_FRAMES
    {
        time.unpause();
    }
    if let Some(dir) = game.autoplay.clone()
        && !game.capture_pending
        && let Some(&at) = game.captures.get(game.captures_taken)
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
        && game.tick >= game.autoplay_end
        && game.captures_taken == game.captures.len();
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

/// The camp (pause) menu: its sources, the open menu and the menu pad.
#[derive(Resource)]
struct CampRes {
    source: CampSource,
    open: Option<Camp>,
    /// Cursor memory and the save's config bits / game mode.
    memory: camp::Memory,
    config_flags: u32,
    game_mode: u32,
    pad: Pad,
    /// Sheet slots: camp art per glyph variant, then the font.
    art: [Option<usize>; 4],
    font_sheet: usize,
    icon_sheet: usize,
    glyph: usize,
    keyboard_last: bool,
    /// The room's stock (ROOM_STOCK, then what the camp leaves) and the
    /// stock list's seen marks.
    stock: std::collections::BTreeMap<u16, u8>,
    seen: Vec<u16>,
    /// Sora's 80 ability slots (ROOM_ABILITIES, then what the camp leaves).
    ability_slots: Vec<u16>,
    /// Shortcut command ids (save+0x36f8; empty in a new save [test-room
    /// value]) and the MP-charge bits (save+0x3744).
    shortcuts: [u16; 4],
    no_charge: u8,
    /// Sora's equipment [test-room values]: the Kingdom Key, one empty
    /// armor and accessory slot, two empty item slots.
    equipment: camp::items::Equipment,
    /// The menu chain past the camp: the journal and the curtain between.
    journal_source: Option<JournalSource>,
    journal: Option<Journal>,
    curtain: Option<Curtain>,
    journal_sheets: JournalSheets,
}

/// Sheet slots of the journal's images.
#[derive(Default)]
struct JournalSheets {
    book: usize,
    curtain: usize,
    worlds: Vec<usize>,
    tops: HashMap<String, Vec<usize>>,
}

impl CampRes {
    /// A menu (camp, curtain or journal) holds the screen and the pad.
    fn menu_up(&self) -> bool {
        self.open.is_some() || self.journal.is_some() || self.curtain.is_some()
    }
}

/// Unattended camp check after the combat plan: open, down to Status,
/// the character list, the stat browser (column switch, move), back out,
/// down to Config, change Field Camera and Summon Effects, jump to the
/// bottom with R1, back out, close with START.
const AUTOPLAY_CAMP: [(u64, u64); 108] = [
    (1810, 0x2),
    (1840, 0x20),
    (1850, 0x20),
    (1860, 0x20),
    (1870, 0x20),
    (1890, 0x4),
    (1940, 0x4),
    (2040, 0x80),
    (2060, 0x20),
    (2090, 0x8),
    (2140, 0x8),
    (2240, 0x20),
    (2250, 0x20),
    (2270, 0x4),
    (2320, 0x80),
    (2340, 0x20),
    (2350, 0x20),
    (2360, 0x20),
    (2380, 0x80),
    (2400, 0x4000),
    (2430, 0x8),
    (2480, 0x2),
    // Items: reopen, wrap to Items, Stock, the consumables tab, use an
    // Ether, use another (MP full: the no-effect box), throw it away.
    (2600, 0x2),
    (2640, 0x20),
    (2660, 0x4),
    (2700, 0x20),
    (2730, 0x4),
    (2770, 0x80),
    (2790, 0x1_0000),
    (2830, 0x20),
    (2850, 0x4),
    (2890, 0x4),
    (2930, 0x4),
    (2980, 0x4),
    (3010, 0x4),
    (3050, 0x4),
    (3090, 0x4),
    (3120, 0x8),
    (3150, 0x20),
    (3170, 0x4),
    (3210, 0x40),
    (3230, 0x4),
    (3280, 0x8),
    (3320, 0x8),
    (3400, 0x2),
    // Abilities: reopen, down to Abilities, Sora, the list; unequip Guard
    // with TRIANGLE, re-equip it in the popup, back out, SQUARE to Items
    // and back, close.
    (3460, 0x2),
    (3500, 0x20),
    (3520, 0x4),
    (3580, 0x4),
    (3680, 0x20),
    (3700, 0x10),
    (3720, 0x1000),
    (3750, 0x4),
    (3820, 0x20),
    (3850, 0x4),
    (3920, 0x8),
    (3990, 0x800),
    (4080, 0x800),
    (4170, 0x8),
    (4240, 0x2),
    // Party: reopen, down to Party, the card (nothing to switch), back.
    (4300, 0x2),
    (4340, 0x20),
    (4350, 0x20),
    (4370, 0x4),
    (4440, 0x80),
    (4460, 0x8),
    (4540, 0x2),
    // Customize: reopen, up to Customize, Sora, shortcut 1 → Potion, back
    // out, close.
    (4600, 0x2),
    (4640, 0x10),
    (4660, 0x4),
    (4720, 0x4),
    (4830, 0x4),
    (4900, 0x20),
    (4920, 0x4),
    (5020, 0x8),
    (5120, 0x8),
    (5190, 0x2),
    // Equip: reopen, up to Items, Sora, item slot 1 → Potion, auto-reload
    // on with TRIANGLE, back to Items, close.
    (5260, 0x2),
    (5300, 0x10),
    (5310, 0x10),
    (5330, 0x4),
    (5410, 0x4),
    (5520, 0x20),
    (5530, 0x20),
    (5540, 0x20),
    (5580, 0x4),
    (5670, 0x20),
    (5710, 0x4),
    (5820, 0x1000),
    (5860, 0x8),
    (5960, 0x8),
    (6030, 0x2),
    // Journal: reopen, down to Jiminy's Journal (curtain, Notes of the
    // world), a row down, decide (no category page), R1 Collection, R1
    // Select World, L1 Collection, L1 Notes, cancel to Select World,
    // cancel out (the camp comes back as the curtain lifts), close.
    (6100, 0x2),
    (6140, 0x20),
    (6150, 0x20),
    (6160, 0x20),
    (6170, 0x20),
    (6180, 0x20),
    (6200, 0x4),
    (6290, 0x20),
    (6320, 0x4),
    (6340, 0x4000),
    (6410, 0x4000),
    (6480, 0x8000),
    (6550, 0x8000),
    (6620, 0x8),
    (6690, 0x8),
    (6780, 0x2),
];
fn camp_setup(
    camp: Option<ResMut<CampRes>>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut c) = camp else { return };
    let c = &mut *c;
    for n in 0..4 {
        if let Some((t, scale)) = c.source.menu.texture_for(0, n) {
            let (rgba, size) = (t.rgba.clone(), t.size);
            c.art[n] = Some(frame.add_sheet(&mut images, rgba, size, scale));
        }
    }
    c.font_sheet = frame.add_sheet(
        &mut images,
        std::mem::take(&mut c.source.font.rgba),
        c.source.font.size,
        c.source.font.texel_scale as f32,
    );
    c.icon_sheet = frame.add_sheet(
        &mut images,
        std::mem::take(&mut c.source.icons.rgba),
        c.source.icons.size,
        c.source.icons.texel_scale as f32,
    );
    if let Some(j) = c.journal_source.as_mut() {
        let mut add = |t: &mut layout2d::Texture| {
            frame.add_sheet(
                &mut images,
                std::mem::take(&mut t.rgba),
                t.size,
                t.texel_scale,
            )
        };
        let s = &mut c.journal_sheets;
        let book = j.book.texture_for(0, 0).map(|(_, k)| k);
        if let (Some(t), Some(k)) = (j.book.replacements[0].as_mut(), book) {
            t.texel_scale = k;
            s.book = add(t);
        }
        let curtain = j.curtain.texture_for(0, 0).map(|(_, k)| k);
        if let (Some(t), Some(k)) = (j.curtain.replacements[0].as_mut(), curtain) {
            t.texel_scale = k;
            s.curtain = add(t);
        }
        s.worlds = j.world_icons.iter_mut().map(&mut add).collect();
        for (code, t) in &mut j.tops {
            s.tops
                .insert(code.clone(), t.iter_mut().map(&mut add).collect());
        }
    }
}

fn field_running(camp: Option<Res<CampRes>>) -> bool {
    camp.is_none_or(|c| !c.menu_up())
}

/// START (logical 0x2 in the field's pad mode) opens the camp
/// (FUN_1401572b0 → FUN_1402e4950(0)); while it runs the field is not
/// stepped, the menu reads pad mode 7 and the frame counter keeps going.
fn camp_tick(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<(&Gamepad, Option<&Name>)>,
    camp: Option<ResMut<CampRes>>,
    mut game: ResMut<Game>,
    mut audio: ResMut<Audio>,
    mut hud: Option<ResMut<Hud>>,
) {
    let Some(mut c) = camp else { return };
    let c = &mut *c;
    let (data, keyboard, glyph) = input::read_devices(&keys, &mouse, 0., &pads);
    if keyboard {
        c.keyboard_last = true;
    } else if glyph.is_some() {
        c.keyboard_last = false;
    }
    let g = if c.keyboard_last {
        0
    } else {
        glyph.map_or(c.glyph, usize::from)
    };
    if c.art[g].is_some() {
        c.glyph = g;
    }
    let menu = c.menu_up();
    let mode = if menu {
        pad::MODE_TITLE
    } else {
        pad::MODE_FIELD
    };
    let tick = game.tick;
    if game.autoplay.is_some() {
        let logical = AUTOPLAY_CAMP
            .iter()
            .filter(|(at, _)| *at == tick)
            .fold(0, |acc, &(_, b)| {
                // Menu START is 0x100 in mode 7.
                acc | if menu && b == 0x2 { 0x100 } else { b }
            });
        c.pad.update(logical, 1.);
    } else {
        c.pad.update_raw(pad::raw(&data), mode, 1.);
    }
    let mut sounds = Vec::new();
    let layout = &c.source.menu.layout;
    // Test-room values: no munny, level 1, the room's HP/MP; the clock is
    // the room's frame count.
    let (hp, max_hp, mp, max_mp) = match (&game.enemy, &hud) {
        (Some(rt), Some(_)) => (
            rt.sora_hp,
            rt.sora_target.max_hp,
            game.player.mp.cur,
            game.player.mp.max,
        ),
        (None, Some(h)) => (
            h.stats.hp,
            h.stats.max_hp,
            game.player.mp.cur,
            game.player.mp.max,
        ),
        _ => (1, 1, 0, 1),
    };
    let panels = camp::Panels {
        munny: 0,
        time_frames: tick as u32,
        members: vec![camp::Member {
            name: c.source.name.clone(),
            level: 1,
            hp,
            max_hp,
            mp,
            max_mp,
        }],
        labels: c.source.labels.clone(),
    };
    // Status values are built when the camp opens (FUN_140354340).
    let camp_data = |c: &CampRes, panels: camp::Panels| -> camp::Data {
        let n = c.source.numbers;
        let sora_abilities = camp::abilities::MemberAbilities {
            slots: c.ability_slots.clone(),
            ap_max: n.ap,
            weapon_ability: c.source.weapon_ability,
        };
        let ap_used = camp::abilities::used(&camp::abilities::entries(
            &c.source.items.table,
            &sora_abilities,
        ));
        let value = |bit: usize| -> String {
            match bit {
                0 => "1".into(),
                1 | 7 => "0".into(),
                2 => format!("{}", n.next_exp),
                3 => format!("{hp}/{max_hp}"),
                4 => format!("{mp}/{max_mp}"),
                5 => format!("{ap_used}/{}", n.ap),
                10 => format!("{}", n.strength),
                11 => format!("{}", n.magic),
                12 => format!("{}", n.defense),
                _ => "0%".into(),
            }
        };
        let mut stats = vec![None; 20];
        for (bit, label, help, style) in &c.source.stats {
            stats[*bit] = Some(camp::Stat {
                label: label.clone(),
                help: help.clone(),
                style: *style,
                value: camp::encode_number(&value(*bit)),
            });
        }
        camp::Data {
            entries: c.source.entries.clone(),
            panels,
            status: vec![camp::StatusEntry {
                bit: 0,
                label: c.source.name.clone(),
                help: c.source.help.clone(),
                stats,
            }],
            banner: c.source.banner.clone(),
            config: c.source.config.clone(),
            config_flags: c.config_flags,
            game_mode: c.game_mode,
            items: camp::items::ItemsData {
                counts: c.stock.clone(),
                equipment: vec![c.equipment.clone()],
                seen: c.seen.clone(),
                ..c.source.items.clone()
            },
            abilities: camp::abilities::AbilitiesData {
                members: vec![sora_abilities.clone()],
                texts: c.source.abilities.clone(),
            },
            party_help: c.source.party_help.clone(),
            equip: camp::equip::EquipData {
                stats: c.source.equip_stats.clone(),
                totals: vec![camp::equip::Totals {
                    ap: n.ap,
                    strength: n.strength,
                    magic: n.magic,
                    defense: n.defense,
                    resists: [100; 4],
                }],
                texts: c.source.equip_texts.clone(),
            },
            customize: camp::customize::CustomizeData {
                shortcuts: c.shortcuts,
                no_charge: c.no_charge,
                texts: c.source.customize.clone(),
            },
        }
    };
    // The menu chain (menu mode 10, FUN_14031aac0): the camp; deciding a
    // journal brings the curtain down over the camp's close; the journal
    // runs; then the camp is rebuilt as the curtain lifts.
    if let Some(menu) = c.open.as_mut() {
        sounds = menu.frame(layout, c.pad.pressed, c.pad.repeat, tick as u32);
        game.tick += 1;
        if menu.journal.is_some()
            && c.curtain.is_none()
            && let Some(j) = &c.journal_source
        {
            c.curtain = Some(Curtain::cover(&j.curtain.layout));
        }
        // FUN_140305480 once the curtain holds black.
        if menu.journal.is_some()
            && !menu.closed
            && c.curtain.as_ref().is_some_and(Curtain::covered)
        {
            menu.kill_all();
        }
        if menu.closed {
            c.memory = menu.memory;
            c.config_flags = menu.data.config_flags;
            c.stock = menu.data.items.counts.clone();
            c.seen = menu.data.items.seen.clone();
            c.shortcuts = menu.data.customize.shortcuts;
            if let Some(e) = menu.data.items.equipment.first() {
                c.equipment = e.clone();
            }
            c.no_charge = menu.data.customize.no_charge;
            if let Some(m) = menu.data.abilities.members.first() {
                c.ability_slots = m.slots.clone();
            }
            // The battle ability record follows the equipped slots;
            // Guard's ability bit gates PTYA 0x0b (FUN_140402750).
            game.player.abilities = game.ability_source.build(&c.ability_slots);
            game.guard_equipped = game.player.abilities.has(abilities::bit::GUARD);
            // Owned spell levels (the element items), the shortcut slots
            // and the MP-charge toggles follow the camp.
            game.shortcuts = c.shortcuts;
            if let Some(m) = game.magic.as_mut() {
                for (t, item) in ELEMENT_ITEMS.iter().enumerate() {
                    m.levels[t] = c.stock.get(item).copied().unwrap_or(0);
                }
                m.dont_charge = c.no_charge;
            }
            // Items used in the camp change the room's HP / MP.
            if let Some(sora) = menu.data.panels.members.first() {
                if let Some(rt) = game.enemy.as_mut() {
                    rt.sora_hp = sora.hp;
                }
                if let Some(h) = hud.as_mut() {
                    h.stats.hp = sora.hp;
                }
                // Items used in the camp restore MP through the charge
                // rules (FUN_1403c0890).
                let delta = sora.mp - game.player.mp.cur;
                if delta != 0 {
                    game.player.mp.add(delta);
                }
            }
            let kind = menu.journal;
            c.open = None;
            game.event(serde_json::json!({"event":"camp_close","tick":tick}));
            if let (Some(kind), Some(j)) = (kind, &c.journal_source) {
                let data = journal::JournalData {
                    kind,
                    // The room stands in for Twilight Town (world 2) and a
                    // save with no story flags [test-room values].
                    world: 2,
                    worlds: j.worlds.clone(),
                    stories: j.stories.clone(),
                    flags: Vec::new(),
                    // FUN_140339500 counts each `limi` command's limit in
                    // save+0x3d48 (FUN_1403e7c30 / FUN_1403e8150): none
                    // in this save; no reports, pieces, missions, minigames
                    // or synthesis either [test-room values].
                    unlocks: journal::Unlocks::default(),
                    texts: j.texts.clone(),
                    tops: j.tops.iter().map(|(code, _)| code.clone()).collect(),
                    world_icons: !j.world_icons.is_empty(),
                };
                let (opened, s) = Journal::open(&j.book.layout, data);
                game.event(serde_json::json!({"event":"journal_open","tick":tick,
                    "kind":kind,"state":opened.state}));
                c.journal = Some(opened);
                sounds.extend(s);
            }
        }
    } else if let Some(open) = c.journal.as_mut() {
        let j = c.journal_source.as_ref().expect("journal source");
        sounds = open.frame(&j.book.layout, c.pad.pressed, c.pad.repeat);
        game.tick += 1;
        if open.closed {
            c.journal = None;
            game.event(serde_json::json!({"event":"journal_close","tick":tick}));
            // FUN_140304ea0: the camp is rebuilt (its open sound) and the
            // curtain lifts (FUN_14031a690).
            let (menu, s) = Camp::open(layout, camp_data(c, panels), c.memory);
            c.open = Some(menu);
            sounds.extend(s);
            if let Some(cur) = c.curtain.as_mut() {
                cur.uncover(&j.curtain.layout);
            }
            game.event(serde_json::json!({"event":"camp_open","tick":tick}));
        }
    } else if c.curtain.is_none() && c.pad.pressed & 0x2 != 0 && !game.talking {
        let (menu, s) = Camp::open(layout, camp_data(c, panels), c.memory);
        c.open = Some(menu);
        sounds = s;
        game.tick += 1;
        game.event(serde_json::json!({"event":"camp_open","tick":tick}));
    }
    if let (Some(cur), Some(j)) = (c.curtain.as_mut(), c.journal_source.as_ref()) {
        cur.update(&j.curtain.layout);
        if cur.dead {
            c.curtain = None;
        }
    }
    for n in sounds {
        game.event(serde_json::json!({"event":"sound","tick":tick,"id":n,"cause":"camp"}));
        let Audio { output, banks, .. } = &mut *audio;
        if let (Some(output), Ok(Some(r))) = (output.as_ref(), banks.request(n))
            && let Ok(mut mixer) = output.mixer.lock()
        {
            for layer in r.layers {
                mixer.play(layer, n, r.volume, 1.);
            }
        }
    }
}

/// The camp over the frozen field (the field HUD is hidden meanwhile
/// [inferred]), the curtain (0x7554) over it, the journal (0x7557 up) over
/// that.
fn camp_present(
    camp: Option<ResMut<CampRes>>,
    model: Option<Res<MenuModel>>,
    mut frame: ResMut<Ui2dFrame>,
) {
    let Some(mut c) = camp else { return };
    let c = &mut *c;
    let font = command::Font {
        widths: &c.source.font.widths,
        // Keyboard prompts (key caps) are not translated: Xbox icons.
        pad: if c.glyph == 0 { 1 } else { c.glyph as u8 },
    };
    if let Some(menu) = c.open.as_mut() {
        let art = c.art[c.glyph].or(c.art[0]).expect("camp art");
        for (sheet, mut quads) in menu.draw(&c.source.menu.layout, &font) {
            let slot = match sheet {
                camp::Sheet::Art => art,
                camp::Sheet::Font => c.font_sheet,
                camp::Sheet::Icon => c.icon_sheet,
                // Only Sora's model is rendered in this room.
                camp::Sheet::Model(i) => {
                    let Some(m) = model.as_ref().filter(|_| i == 0) else {
                        continue;
                    };
                    for q in &mut quads {
                        q.uv = [0, 0, MENU_TARGET[0] as i32, MENU_TARGET[1] as i32];
                    }
                    m.sheet
                }
            };
            frame.draws.push((slot, quads));
        }
    }
    let Some(j) = &c.journal_source else { return };
    let s = &c.journal_sheets;
    if let Some(cur) = &c.curtain {
        frame.draws.push((s.curtain, cur.quads(&j.curtain.layout)));
    }
    if let Some(open) = &c.journal {
        let top = open.top.as_ref().and_then(|code| s.tops.get(code));
        for (sheet, quads) in open.draw(&j.book.layout, &font) {
            let slot = match sheet {
                journal::Sheet::Art => Some(s.book),
                journal::Sheet::Font => Some(c.font_sheet),
                journal::Sheet::Icon => Some(c.icon_sheet),
                journal::Sheet::World(i) => s.worlds.get(i).copied(),
                journal::Sheet::Top(i) => top.and_then(|t| t.get(i).copied()),
            };
            if let Some(slot) = slot {
                frame.draws.push((slot, quads));
            }
        }
    }
}
/// Field (slot 0) and battle (slot 1) music (FUN_140155840 / FUN_140155c30)
/// and the battle state (FUN_1403ab6f0, DAT_142a11404) driving the switch.
#[derive(Resource)]
struct MusicRes {
    pending: [Option<sound::MusicSlot>; 2],
    clips: [Option<sound::Music>; 2],
    started: bool,
    battle: bool,
    counter: u32,
    since_reset: u32,
}

const MUSIC_KEYS: [u32; 2] = [9_000_000, 9_000_001];
/// BGM slot level 0x3000 / 16383.
const BGM_LEVEL: f32 = 0x3000 as f32 / 16383.;
/// Fade units are 1/240 s: 0xf0 = 1 s, 0x1e0 = 2 s, 0x2d0 = 3 s.
const BATTLE_IN: f32 = 1.;
const FIELD_PAUSE: f32 = 2.;
const BATTLE_OUT: f32 = 3.;
const FIELD_RESUME: f32 = 2.;
/// Battle when an enemy is within this horizontal distance; switch after
/// the target differs 45 frames (to battle) or 120 (to field), at once
/// in the first 30 frames after a reset.
const BATTLE_DISTANCE: f32 = 1200.;

fn music_tick(music: Option<ResMut<MusicRes>>, mut game: ResMut<Game>, audio: Res<Audio>) {
    let Some(mut m) = music else { return };
    let m = &mut *m;
    let tick = game.tick;
    for k in 0..2 {
        if let Some(slot) = &m.pending[k]
            && let Some(result) = slot.lock().ok().and_then(|mut r| r.take())
        {
            m.pending[k] = None;
            match result {
                Ok(music) => {
                    game.event(serde_json::json!({"event":"music_decoded","slot":k,"tick":tick,"stats":music.stats}));
                    m.clips[k] = Some(music);
                }
                Err(e) => game.event(
                    serde_json::json!({"event":"music_failed","slot":k,"tick":tick,"error":e}),
                ),
            }
        }
    }
    let Some(output) = &audio.output else { return };
    // Room entry: the field track at full level, no fade.
    if !m.started
        && let Some(field) = &m.clips[0]
    {
        if let Ok(mut mixer) = output.mixer.lock() {
            mixer.play(
                field.clip.clone(),
                MUSIC_KEYS[0],
                BGM_LEVEL * field.volume,
                1.,
            );
        }
        m.started = true;
        game.event(serde_json::json!({"event":"bgm_start","slot":0,"tick":tick}));
    }
    // FUN_1403ab6f0: the target state and its hysteresis.
    let p = game.player.position;
    let target = game.enemy.as_ref().is_some_and(|rt| {
        let e = rt.enemy.position;
        !matches!(
            rt.enemy.current_act(),
            Some(shadow::ActKind::Leave | shadow::ActKind::Dead)
        ) && ((e[0] - p[0]).powi(2) + (e[2] - p[2]).powi(2)).sqrt() < BATTLE_DISTANCE
    });
    m.since_reset = m.since_reset.saturating_add(1);
    if target != m.battle {
        m.counter += 1;
        let need = if target { 45 } else { 120 };
        if m.since_reset <= 30 || m.counter >= need {
            m.battle = target;
            m.counter = 0;
            let (Some(field), Some(battle)) = (&m.clips[0], &m.clips[1]) else {
                return;
            };
            if let Ok(mut mixer) = output.mixer.lock() {
                if target {
                    mixer.fade(MUSIC_KEYS[0], 0., FIELD_PAUSE, audio_out::FadeEnd::Pause);
                    mixer.stop(MUSIC_KEYS[1]);
                    mixer.play(battle.clip.clone(), MUSIC_KEYS[1], 0., 1.);
                    mixer.fade(
                        MUSIC_KEYS[1],
                        BGM_LEVEL * battle.volume,
                        BATTLE_IN,
                        audio_out::FadeEnd::Hold,
                    );
                } else {
                    mixer.fade(MUSIC_KEYS[1], 0., BATTLE_OUT, audio_out::FadeEnd::Stop);
                    mixer.resume(MUSIC_KEYS[0], BGM_LEVEL * field.volume, FIELD_RESUME);
                }
            }
            game.event(serde_json::json!({"event":"battle_music","tick":tick,"battle":target}));
        }
    } else {
        m.counter = 0;
    }
}
/// Double-click launch: with no arguments, read the three required paths
/// (and optional extra options) from a private launch file outside Git.
fn launch_file_args(name: &str) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
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
            candidates.push(up.join(format!(".local/test-room/{name}.json")));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates
            .push(PathBuf::from(local).join(format!("KH2FinalRewrite/test-room-{name}.json")));
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

fn run(mut args: Vec<PathBuf>) -> Result<AppExit, Box<dyn std::error::Error>> {
    // No arguments: the private launch file; `talk`: the talk-room one.
    if args.is_empty() {
        args = launch_file_args("launch")?;
    } else if args.len() == 1 && args[0].as_os_str() == "talk" {
        args = launch_file_args("launch-talk")?;
    }
    if args.len() < 3 {
        return Err("usage: test_room <Steam-root> <manifest> <test-room-config> [--log new.jsonl] [--frames N] [--autoplay <new-capture-dir>] [--gamepad] [--show-collision] [--basic-fk]".into());
    }
    let mut log_path = None;
    let mut limit = None;
    let mut autoplay = None;
    let mut show_collision = false;
    let mut basic_fk = false;
    let mut options = args[3..].iter();
    while let Some(option) = options.next() {
        match option.to_str() {
            // Pads are always read now; the flag is accepted for old launch files.
            Some("--gamepad") => {}
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
    eprintln!("Validating the selected KH2FM source, then decoding the test room...");
    let loaded = load(&source, &args[2], basic_fk)?;
    let log = match log_path {
        Some(path) => Some(OpenOptions::new().write(true).create_new(true).open(path)?),
        None => None,
    };
    let mut poses = HashMap::new();
    for (action, clip) in &loaded.actions {
        poses.insert(*action, *clip);
    }
    let talk_room = loaded.npc.is_some();
    let mut player = Player::new(loaded.spawn.to_array(), 0.);
    player.abilities = loaded.abilities.build(&ROOM_ABILITIES);
    if autoplay.is_some() {
        // The unattended run strikes Sora with blows and recovers with
        // Retaliating Slash (ability bit 101), which is unequipped by
        // default.
        player.abilities.set(101);
    }
    if let Some(h) = &loaded.hud {
        player.mp = magic::Mp::new(h.stats.1);
    }
    let guard_equipped = player.abilities.has(abilities::bit::GUARD);
    let mut game = Game {
        ability_source: loaded.abilities,
        params: loaded.params,
        clips: loaded.clips,
        world: loaded.world,
        player,
        poses,
        clip_poses: loaded.clip_poses,
        part_indices: Vec::new(),
        tick: 0,
        log,
        limit,
        autoplay,
        captures_taken: 0,
        capture_pending: false,
        pad: Pad::default(),
        pad_data: pad::PadData::default(),
        jump_queued: false,
        attack_queued: false,
        guard_queued: false,
        shortcut_queued: 0,
        back_queued: false,
        debug_blow: false,
        shortcuts: ROOM_SHORTCUTS,
        magic: loaded.magic.map(|src| {
            let mut levels = [0u8; 6];
            for (t, item) in ELEMENT_ITEMS.iter().enumerate() {
                levels[t] = ROOM_STOCK
                    .iter()
                    .find(|(i, _)| i == item)
                    .map_or(0, |&(_, n)| n);
            }
            magic_room::MagicRt::new(src, levels)
        }),
        guard_equipped,
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
        sound_queue: Vec::new(),
        sound_frame: None,
        effect_queue: Vec::new(),
        talking: false,
        rc_queued: false,
        advance_queued: false,
        captures: if talk_room {
            &AUTOPLAY_TALK_CAPTURES
        } else {
            &AUTOPLAY_CAPTURES
        },
        autoplay_end: if talk_room {
            AUTOPLAY_TALK_END
        } else {
            AUTOPLAY_END
        },
    };
    game.event(loaded.summary);
    let output = if loaded.banks.numbers().is_empty() {
        None
    } else {
        match audio_out::Output::open() {
            Ok(output) => Some(output),
            Err(error) => {
                eprintln!("Sound output unavailable: {error}");
                None
            }
        }
    };
    game.event(
        serde_json::json!({"event":"audio","banks":loaded.banks.numbers(),
        "output":output.is_some(),"rate":audio_out::RATE}),
    );
    let audio = Audio {
        output,
        banks: loaded.banks,
        alarm: false,
    };
    let (talk, npc_model) = match loaded.npc {
        Some(n) => {
            let position = loaded.spawn + n.offset;
            game.event(serde_json::json!({"event":"npc","position":position.to_array(),"yaw":n.yaw,
                "frames":n.frames.len(),"target_shape":format!("{:?}", n.target),
                "lines":n.lines.iter().map(|(k, v)| (format!("{k:#x}"), v.len())).collect::<HashMap<_, _>>(),
                "text_colour":format!("{:#010x}", n.colour),"window_texel_scale":n.sheet.texel_scale,
                "font_texel_scale":n.font.texel_scale}));
            (
                Some(Talk {
                    position,
                    yaw: n.yaw,
                    home_yaw: n.yaw,
                    tick: 0,
                    frames: n.frames,
                    bones: n.bones,
                    loop_start: n.loop_start,
                    target: n.target,
                    indices: Vec::new(),
                    sheet: n.sheet,
                    font: n.font,
                    lines: n.lines,
                    colour: n.colour,
                    label: n.label,
                    script: n.script,
                    active: None,
                    current: None,
                    closing: Vec::new(),
                    wait: 0.,
                    camera: None,
                    dissolve: 0,
                    snapshot: false,
                    reaction: None,
                }),
                Some(n.model),
            )
        }
        None => (None, None),
    };
    let (effects, effect_atlas) = match loaded.effects {
        Some(e) => {
            game.event(serde_json::json!({"event":"effects","weapon_pax":e.weapon.name,
                "weapon_elements":e.weapon.elements.len(),"enemy_pax":e.enemy.as_ref().map(|p| p.name.clone()),
                "atlas_size":e.atlas.size,"atlas_texels":e.atlas.texel_size,"atlas_hd":e.atlas.hd,
                "atlas_alignment":e.atlas.alignment}));
            let size = if e.atlas.hd {
                e.atlas.size
            } else {
                e.atlas.texel_size
            };
            (
                Effects {
                    weapon: Some(Arc::new(e.weapon)),
                    enemy: e.enemy.map(Arc::new),
                    system: e.system.map(Arc::new),
                    running: Vec::new(),
                    rng: effect_vm::Rng::default(),
                    quads: Vec::new(),
                    view_inverse: Mat4::IDENTITY,
                    uv_scale: [1. / size[0] as f32, 1. / size[1] as f32],
                    reported: Vec::new(),
                },
                Some(e.atlas),
            )
        }
        None => (
            Effects {
                weapon: None,
                enemy: None,
                system: None,
                running: Vec::new(),
                rng: effect_vm::Rng::default(),
                quads: Vec::new(),
                view_inverse: Mat4::IDENTITY,
                uv_scale: [1.; 2],
                reported: Vec::new(),
            },
            None,
        ),
    };
    let mut enemy_model = None;
    if let Some(e) = loaded.enemy {
        let spawn = loaded.spawn + e.offset;
        let to_sora = loaded.spawn - spawn;
        let enemy = Shadow::new(
            spawn.to_array(),
            to_sora.x.atan2(to_sora.z),
            e.stats,
            loaded.sstm,
        );
        game.event(serde_json::json!({"event":"enemy","hp":e.stats.max_hp,"defense":e.stats.defense,
            "damage_cap":e.stats.cap,"damage_min":e.stats.min,"sora_strength":e.sora.strength,
            "hit_spheres":e.body.len(),"blade_groups":e.blade.keys().collect::<Vec<_>>(),"position":spawn,
            "claw_groups":e.claws.iter().map(|(g, v)| (g.to_string(), v.len())).collect::<HashMap<_, _>>(),
            "claw_windows":e.clips.0.iter().filter(|(_, c)| !c.windows.is_empty()).map(|(s, c)| (s.to_string(), c.windows.clone())).collect::<HashMap<_, _>>(),
            "sora_hit_spheres":e.sora_body.len(),"target_reach":e.target_reach,"sora_target":format!("{:?}", e.sora_target),"shadow_attacker":format!("{:?}", e.shadow_attacker)}));
        enemy_model = Some(e.model);
        game.enemy = Some(EnemyRuntime {
            clips: e.clips,
            body: e.body,
            targets: e.targets,
            blade: e.blade,
            steps: e.steps,
            atkp: e.atkp,
            difficulty: e.difficulty,
            sora: e.sora,
            enemy,
            rng: ScriptRandom::default(),
            claws: e.claws,
            shadow_attacker: e.shadow_attacker,
            sora_target: e.sora_target,
            sora_body: e.sora_body,
            target_reach: e.target_reach,
            sora_record: combat::HitRecord::new(true),
            ids: combat::AttackIds::default(),
            instance_ids: HashMap::new(),
            next_group: 0x100,
            claw_previous: HashMap::new(),
            sora_hp: e.sora_target.max_hp,
            ko_ticks: 0,
            previous: HashMap::new(),
            indices: Vec::new(),
            hit_sounds: e.hit_sounds,
            hit_effects: e.hit_effects,
            rows: e.rows,
            sora_invulnerable: 0.,
            sound_tick: None,
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
        "Test room ready (the original PC bindings). WASD moves (Left Alt walks), Space/Backspace jump, Enter/left-click decides (Attack), Up/Down choose a command, right-click/Right Shift guards, F/Right Ctrl reacts (Talk), M opens the menu, the mouse turns the camera, middle-click/Right Alt recentres, Esc exits."
    );
    let mut app = App::new();
    if let Some(talk) = talk {
        app.insert_resource(talk);
    }
    if let Some(music) = loaded.music {
        let pending = music.map(|bytes| Some(sound::decode_music_in_background(bytes)));
        app.insert_resource(MusicRes {
            pending,
            clips: [None, None],
            started: false,
            battle: false,
            counter: 0,
            since_reset: 0,
        });
    }
    if let Some(c) = loaded.camp {
        game.event(serde_json::json!({"event":"camp_loaded","props":c.menu.layout.props.len(),
            "scenes":c.menu.layout.scenes.len(),"sequences":c.menu.layout.sequences.len(),
            "replacements":c.menu.replacements.iter().map(|r| r.as_ref().map(|t| t.size)).collect::<Vec<_>>(),
            "entries":c.entries.len(),"stats":c.stats.len(),"config_rows":c.config.len()}));
        app.insert_resource(CampRes {
            source: c,
            open: None,
            memory: camp::Memory::default(),
            // A new game's config (FUN_1402e4fc0) and Standard mode
            // (FUN_1401523a0) [test-room save].
            config_flags: 0x40b,
            game_mode: 1,
            pad: Pad::default(),
            art: [None; 4],
            font_sheet: 0,
            icon_sheet: 0,
            glyph: 0,
            keyboard_last: true,
            stock: ROOM_STOCK.into_iter().collect(),
            seen: Vec::new(),
            shortcuts: ROOM_SHORTCUTS,
            no_charge: 0,
            equipment: camp::items::Equipment {
                weapon: KINGDOM_KEY,
                armor: vec![0],
                accessories: vec![0],
                items: vec![0, 0],
                originals: vec![0, 0],
                ..Default::default()
            },
            ability_slots: {
                let mut s = ROOM_ABILITIES.to_vec();
                s.resize(80, 0);
                s
            },
            journal_source: loaded.journal,
            journal: None,
            curtain: None,
            journal_sheets: JournalSheets::default(),
        });
    }
    Ok(app
        .insert_resource(game)
        .insert_resource(Startup {
            room: Some(loaded.room),
            arena_mesh: loaded.arena_mesh,
            actor: Some(loaded.actor),
            weapon: Some(loaded.weapon),
            hud: loaded.hud,
            enemy_model,
            effect_atlas,
            npc_model,
        })
        .insert_resource(rig)
        .insert_resource(audio)
        .insert_resource(effects)
        .insert_resource(Time::<Fixed>::from_hz(60.))
        .insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.09)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "KH2FM test room (quarantined source) | WASD move, Space jump, Enter/LMB attack, RMB guard, F talk, M menu, Esc".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            // Closing the window goes through finish() so the exit code and
            // log record reflect an unfinished unattended run.
            exit_condition: bevy::window::ExitCondition::DontExit,
            close_when_requested: false,
            ..default()
        }))
        .add_plugins(Ui2dPlugin {
            order: 0,
            overlay: false,
        })
        .add_systems(Startup, (setup, pause_for_autoplay, camp_setup))
        .add_systems(
            FixedUpdate,
            (
                read_pad,
                camp_tick,
                (simulate, talk_tick, effects_tick, hud_tick, play_sounds, music_tick)
                    .chain()
                    .run_if(field_running),
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                dissolve_present,
                camera_control,
                present,
                present_enemy,
                present_spells,
                present_npc,
                present_effects,
                hud_present,
                resize_scene,
                compose_2d,
                menu_model_present,
                camp_present,
                finish,
            )
                .chain(),
        )
        .run())
}

/// Runs this program with its arguments; `interactive` = started from the
/// launcher menu (keep the console open on an error).
pub fn main(args: Vec<PathBuf>, interactive: bool) -> std::process::ExitCode {
    match run(args) {
        Ok(AppExit::Success) => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(e) => {
            eprintln!("Test room rejected: {e}");
            // Keep a double-clicked console open long enough to read the error.
            if interactive {
                eprintln!("Press Enter to close.");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
            std::process::ExitCode::FAILURE
        }
    }
}
