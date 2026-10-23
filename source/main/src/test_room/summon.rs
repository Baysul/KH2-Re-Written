//! Summons from the second deck, recovered from the executable.
//!
//! Picking a summon (execute 72, FUN_140405440) plays Sora's motion 0x8a,
//! pays the command's Drive bars (FUN_1403c2780) and starts the summon
//! (FUN_1403de460: summon state 2, Sora held, a task). The task
//! (FUN_1403de610) sends the party away (FUN_1403e6fa0), waits SSTM +0x88
//! frames, puts the Drive gauge in summon mode (FUN_1403c2740), spawns the
//! `sumn` row's object (+4) 200–300 units from Sora (FUN_1403f3fa0 /
//! FUN_1403f4180) as a summon (+0x6c8 bit 22), plays Sora's 0x82 and starts
//! the object's act `appear_skip`. Dismiss (execute 316, FUN_1403de010) or
//! summon time at 0 once Dismiss could run (FUN_140404a40) starts the act
//! `leave` (FUN_1403de080); when the object has removed itself the party
//! comes back (FUN_140400440).
//!
//! [I] The first-time `appear` event (summon effect setting Full / Auto,
//! FUN_1402e4fa0) is not played: the room takes the setting Off path.
use super::{
    Game, SceneRoot,
    enemy_battle::{Clips, build_clips},
    enemy_catalog::{Kind, load_entry_kind, plain_entry},
    enemy_gallery::{EnemyGallery, sample},
    enemy_script::ScriptBrain,
    second_deck::SummonRow,
    spawn_model,
};
use bevy::prelude::*;
use data::objentry;
use enemy::friend::{Friend, Personality};
use player::{World, actor_physics, combat, shadow::ScriptRandom};
use std::sync::Arc;
use visual::area_fog::FogMaterial;

/// Sora's motions: the call (FUN_140405440, blend 8.0 at 0x140623bac) and
/// the one FUN_1403de610 plays as the summon appears (`appear_skip`).
pub const CALL_MOTION: u16 = 0x8a;
pub const APPEAR_MOTION: u16 = 0x82;
const CALL_BLEND: f32 = 8.;
/// FUN_1403f3fa0's distance range (0x140585efc, 0x140623bf8).
const NEAR: f32 = 200.;
const FAR: f32 = 300.;
/// The summon level the gauge counts from (DAT_1409acdd6, raised only by
/// FUN_14040d530) [I: a new game's 0; no save is loaded in the room].
const LEVEL: u8 = 0;

/// DAT_142a24edc.
pub mod state {
    pub const NONE: u8 = 0;
    pub const OUT: u8 = 2;
    pub const LEAVING: u8 = 3;
}

/// The summons' tables: the `sumn` rows and the SSTM wait.
pub struct Source {
    pub rows: Vec<SummonRow>,
    /// SSTM +0x88: the frames FUN_1403de610 waits before the summon comes.
    pub wait: f32,
    pub sstm: actor_physics::Sstm,
}

impl Source {
    /// `record` is 03system `pref` / `sstm`'s record.
    pub fn new(rows: Vec<SummonRow>, record: &[u8]) -> Option<Self> {
        let wait = f32::from_le_bytes(record.get(0x88..0x8c)?.try_into().ok()?);
        Some(Source {
            rows,
            wait,
            sstm: actor_physics::Sstm::read(record)?,
        })
    }
}

/// The summon object: its model and motions, its script and the pose drawn.
pub struct Actor {
    pub kind: Arc<Kind>,
    brain: ScriptBrain,
    clips: Arc<Clips>,
    rng: ScriptRandom,
    pub pose: Vec<Vec3>,
    pub revision: u64,
    reported: bool,
    /// The leaving fade (`sysobj_fadeout` in the act `leave`) has begun.
    fading: bool,
    /// Faults already logged.
    faults: std::collections::BTreeSet<String>,
    last_sora: Option<[f32; 3]>,
    sora_hp: Option<i32>,
    /// Sora's attack action last tick and the attacks noted so far.
    last_attack: Option<player::Action>,
    attack_serial: u32,
    /// Where a summon in camera space (Stitch) stands and faces this frame.
    camera_pose: Option<(Vec3, Quat)>,
    /// PRTY record 0x3e (Peter Pan's limit flight).
    flight_spec: Option<actor_physics::ObjParams>,
    /// Chicken Little's limit: a target is locked (+0xbf8), and it was
    /// locked this frame (the view turns to it, FUN_140178560).
    aim_lock: bool,
    aim_recenter: bool,
}

impl Actor {
    pub fn position(&self) -> Vec3 {
        Vec3::from_array(self.brain.position())
    }
    pub fn yaw(&self) -> f32 {
        self.brain.yaw()
    }
    /// The drawn heading: camera space's for Stitch, else the yaw.
    pub fn rotation(&self) -> Quat {
        self.camera_pose
            .map_or_else(|| Quat::from_rotation_y(self.yaw()), |(_, r)| r)
    }
    /// The object removed itself, or [I] its leaving fade has ended: the
    /// summon class's removal (FUN_1403dd850 clears DAT_142a24ee8 and the
    /// summon state) runs from the kn::Friend update, which is not traced;
    /// the act `leave` ends in `sysobj_fadeout` and a wait.
    /// Sora's position last frame (his velocity, +0xa38).
    fn sora_velocity(&mut self, now: [f32; 3]) -> [f32; 3] {
        let v = self
            .last_sora
            .map_or([0.; 3], |p| [0, 1, 2].map(|k| now[k] - p[k]));
        self.last_sora = Some(now);
        v
    }
    pub fn gone(&self) -> bool {
        self.brain.gone() || self.fading && self.brain.fade().0 <= 0.
    }
}

/// A summon from its call to its leaving.
pub struct Summoned {
    /// DAT_142a24edc.
    pub state: u8,
    pub command: u16,
    row: SummonRow,
    /// FUN_1403de610's countdown before the summon comes.
    wait: f32,
    /// Sora is held (player +0x120 bits 0x104) until the summon is out.
    pub holds_sora: bool,
    pub actor: Option<Actor>,
    /// The members FUN_1403e6fa0 sent away.
    pub stored: Vec<usize>,
    /// Render identity of this summon.
    pub serial: u64,
}

/// The summon state the deck reads (FUN_1403de250).
pub fn state_of(game: &Game) -> u8 {
    game.summon.as_ref().map_or(state::NONE, |s| s.state)
}

/// FUN_140405440 (execute 72) with FUN_1403de460: start summon `command`.
pub fn start(game: &mut Game, command: u16, tick: u64) -> Result<(), String> {
    if game.summon.is_some() {
        return Err("a summon is out".into());
    }
    let row = game
        .summons
        .as_ref()
        .and_then(|s| s.rows.iter().find(|r| r.command == command).copied())
        .ok_or("no sumn row for the command")?;
    let cost = game
        .second_deck
        .as_ref()
        .and_then(|d| d.commands.get(&command))
        .map_or(0, |c| c.cost);
    // FUN_140405440: Sora's call motion, then the payment (FUN_1403c2780).
    game.player
        .play_motion(CALL_MOTION, CALL_BLEND, &game.clips);
    game.player.drive.pay(cost);
    // FUN_1403e6fa0: Donald and Goofy go.
    let mut stored = Vec::new();
    for (i, m) in game.party.iter_mut().enumerate() {
        if !m.stored {
            m.stored = true;
            stored.push(i);
        }
    }
    // FUN_1403de460: Genie takes one of Sora's owned forms (1, 2, 4, 5)
    // at random (DAT_140783ca0 × 0x10dcd); Peter Pan's is 1.
    if command == GENIE {
        let owned = game.drive.as_ref().map_or([false; 7], |d| d.forms.owned);
        let forms: Vec<u8> = [1u8, 2, 4, 5]
            .into_iter()
            .filter(|&f| owned[f as usize])
            .collect();
        if !forms.is_empty() {
            // [I] The generator's state is the process's; the room's starts
            // at 1 for each summon.
            let mut rng = ScriptRandom::default();
            game.summon_form = genie_pick(&mut rng, &forms);
        }
    } else if command == PETER_PAN {
        game.summon_form = 1;
    }
    let wait = game.summons.as_ref().map_or(0., |s| s.wait);
    game.summon_serial += 1;
    game.summon = Some(Summoned {
        state: state::OUT,
        command,
        row,
        wait,
        holds_sora: true,
        actor: None,
        stored: stored.clone(),
        serial: game.summon_serial,
    });
    game.event(
        serde_json::json!({"event":"summon_start","tick":tick,"command":command,
        "object":row.object,"cost":cost,"wait":wait,"stored":stored}),
    );
    Ok(())
}

/// The commands the summon's script offers this tick (`command_override`,
/// FUN_1403b2850 records) and their targets, while it is out.
pub fn offers(game: &Game) -> Option<[Option<u16>; 8]> {
    let a = game.summon.as_ref()?.actor.as_ref()?;
    a.brain
        .offers
        .iter()
        .any(Option::is_some)
        .then_some(a.brain.offers)
}

/// A deck row the summon's offer took (`record`), or the reaction button on
/// its record 1: outside its limit, FUN_1403d8b40 for the offered command
/// (the `limt` row whose second participant is the summon); inside, the
/// limit's follow-up (trigger 23 of the summon's script). [I] The motion
/// windows that decide a follow-up (FUN_1403b6950) are not tested; the
/// limit's cost (FUN_1403a9cf0 in summon mode) is not taken.
pub fn decide(game: &mut Game, tick: u64, record: Option<usize>) {
    let Some(record) = record else {
        return;
    };
    let player = sora_sys(game);
    let Some(s) = game.summon.as_mut() else {
        return;
    };
    let Some(a) = s.actor.as_mut() else {
        return;
    };
    // A special deck's row (Chicken Little's mode 5), else an offer.
    let special = record
        .checked_sub(super::party_limit::SPECIAL)
        .and_then(|k| Some(a.brain.special_deck()?.get(k)?.0));
    let Some(cmd) = special.or_else(|| a.brain.offers.get(record).copied().flatten()) else {
        return;
    };
    if a.brain.in_limit() {
        a.brain.limit_follow_up(cmd);
        game.event(serde_json::json!({"event":"summon_limit_follow_up","tick":tick,"command":cmd}));
        return;
    }
    let time = game
        .second_deck
        .as_ref()
        .and_then(|d| d.commands.get(&cmd))
        .map_or(180., |c| f32::from(c.time));
    let known = game.second_deck.as_ref().is_some_and(|d| {
        d.limits
            .iter()
            .any(|l| l.command == cmd && l.members[1].wrapping_add(0x38) < 5)
    });
    let result = if known {
        a.brain.begin_limit(cmd, time, player)
    } else {
        Err("no summon limit row for the command".into())
    };
    match result {
        Ok(()) => game.event(serde_json::json!({"event":"summon_limit_start","tick":tick,
            "command":cmd,"time":time})),
        Err(e) => game.event(
            serde_json::json!({"event":"summon_limit_refused","tick":tick,
            "command":cmd,"error":e}),
        ),
    }
}

/// What a running summon limit's HUD shows (FUN_1403d96d0 shows the
/// file's gauge and combo counter): the file among the room's limits, the
/// command, the gauge's time and the hits.
pub fn hud_values(game: &Game) -> Option<(usize, u16, f32, i32)> {
    let l = game.summon.as_ref()?.actor.as_ref()?.brain.limit_state()?;
    let k = game
        .party_limits
        .limits
        .iter()
        .position(|x| x.row.command == l.command)?;
    Some((k, l.command, l.time, l.hits))
}

/// Chicken Little's limit view (camera mode 9): his aim point (this frame's
/// coordinates), whether a target is locked, and whether it was locked
/// this frame.
pub fn aim_view(game: &Game) -> Option<([f32; 3], bool, bool)> {
    let a = game.summon.as_ref()?.actor.as_ref()?;
    a.brain.special_deck()?;
    Some((a.brain.aim()?, a.aim_lock, a.aim_recenter))
}

/// The summon's special deck (Chicken Little's limit, mode 5) while it is
/// up: (command, selector) rows.
pub fn special_deck(game: &Game) -> Option<[(u16, u16); 3]> {
    game.summon.as_ref()?.actor.as_ref()?.brain.special_deck()
}

/// Sora as a limit's user (FUN_1403d7fa0 takes him as he stands).
fn sora_sys(game: &Game) -> enemy::sysobj::Sysobj {
    let mut sora = enemy::sysobj::Sysobj::bare();
    sora.model = 1;
    sora.team = super::enemy_script::SORA_TEAM as i32;
    sora.entry_part = 1;
    sora.position = game.player.position;
    sora.yaw = game.player.yaw;
    sora
}

/// While the summon's limit runs Sora stands and turns where it has him
/// (the scripted stand-in); his own update does not run. He plays what the
/// summon's script captures for him (`obj_motion_capture`, FUN_1403c6cc0:
/// a motion of the summon's own set), else stands. Returns false when no
/// summon limit runs.
pub fn sora_in_limit(game: &mut Game) -> bool {
    let Some(s) = game.summon.as_ref() else {
        return false;
    };
    let Some(a) = s.actor.as_ref() else {
        return false;
    };
    let Some(p) = a.brain.limit_player() else {
        return false;
    };
    let row = game
        .summons
        .as_ref()
        .and_then(|t| t.rows.iter().position(|r| r.object == s.row.object));
    let captured = p.capture.and_then(|c| {
        // A slot Sora's kit does not hold (not of his skeleton) is not
        // played.
        let clip =
            capture_clip(row?, c.motion).filter(|&m| game.clips.scripted_clip(m).is_some())?;
        let length = enemy::env::Clips::length(&*a.clips, c.motion as usize);
        Some((clip, length.saturating_sub(c.remaining)))
    });
    let (position, yaw) = (p.position, p.yaw);
    game.player.position = position;
    game.player.yaw = yaw;
    match captured {
        Some((clip, frame)) => {
            game.player.action = player::Action::Scripted(clip);
            game.player.frame = frame;
        }
        None => game.player.action = player::Action::Idle,
    }
    true
}

/// The slots of a summon's own motion set its scripts capture for Sora
/// (`obj_motion_capture` in P_EX330 / P_EX340 / P_EX350: 252..256).
const CAPTURE_SLOTS: std::ops::RangeInclusive<usize> = 252..=256;

/// The id Sora's kit keeps a summon's captured motion under: the `sumn`
/// row above the limits' ids (0x4000..).
fn capture_clip(row: usize, slot: i32) -> Option<u16> {
    let slot = usize::try_from(slot)
        .ok()
        .filter(|s| CAPTURE_SLOTS.contains(s))?;
    Some(0x7800 | (row as u16) << 8 | (slot - 240) as u16)
}

/// A captured slot's motions for the victim (FUN_1403c6cc0 ->
/// FUN_1403c64a0): the slot is a set of its own, motion 0 for the body
/// (entries "0010"), 1 for the first weapon ("0011"), each in four variants
/// chosen as a set's are (FUN_1403c7c50's table) [I: in battle with the
/// Keyblade out]. The body's slot is made unique as the limits' are.
pub(super) fn capture_parts(
    anb: &[u8],
    clip: u16,
) -> Option<(
    data::motion::SelectedMotion,
    Option<data::motion::SelectedMotion>,
)> {
    let table = data::bar::read_bar(anb).ok()?;
    let part = |id: u16, tag: usize| {
        let present = |k: usize| {
            table
                .entries
                .get(k)
                .is_some_and(|e| e.offset > 0 && e.size > 0)
        };
        let k = data::motion::motion_set_index(present, id, true, true)?;
        let e = &table.entries[k];
        let bytes = anb.get(e.offset as usize..e.offset as usize + e.size as usize)?;
        data::motion::read_anb(bytes, tag).ok()
    };
    let body = part(0, 0x1_0000 + clip as usize)?;
    Some((body, part(1, 0x1_0000 + clip as usize)))
}

/// Sora's captured motions of every summon (the `sumn` rows' objects'
/// sets, [`CAPTURE_SLOTS`]) for his kit: (clip id, body motion, Keyblade
/// motion).
pub fn capture_motions(
    source: &data::probe_source::ProbeSource,
    battle: &[u8],
) -> Vec<(
    u16,
    data::motion::SelectedMotion,
    Option<data::motion::SelectedMotion>,
)> {
    let mut out = Vec::new();
    let Ok(mut library) = data::path_lookup::Library::open(source) else {
        return out;
    };
    let Some(objects) = library
        .read("00objentry.bin")
        .ok()
        .and_then(|b| objentry::read(&b).ok())
    else {
        return out;
    };
    let Ok(sumn) = super::bar_entry(battle, b"sumn") else {
        return out;
    };
    for (row, r) in super::second_deck::read_summons(sumn).iter().enumerate() {
        let Some(object) = objects.iter().find(|o| o.object_id == u32::from(r.object)) else {
            continue;
        };
        let name: Vec<u8> = object
            .animation_name
            .iter()
            .copied()
            .take_while(|&b| b != 0)
            .collect();
        let name = String::from_utf8_lossy(&name);
        let path = if name.ends_with(".mset") {
            format!("obj/{name}")
        } else {
            format!("obj/{name}.mset")
        };
        let Ok(bytes) = library.read(&path) else {
            continue;
        };
        let Ok(table) = data::bar::read_bar(&bytes) else {
            continue;
        };
        for slot in CAPTURE_SLOTS {
            let Some(clip) = capture_clip(row, slot as i32) else {
                continue;
            };
            let Some(anb) = table
                .entries
                .get(slot)
                .filter(|e| e.offset > 0 && e.size > 0)
                .and_then(|e| bytes.get(e.offset as usize..e.offset as usize + e.size as usize))
            else {
                continue;
            };
            let Some((body, weapon)) = capture_parts(anb, clip) else {
                eprintln!("{path}: slot {slot} holds no motion for Sora; left out");
                continue;
            };
            let usable = body
                .motion
                .frame_window()
                .is_ok_and(|(start, end, fps, _)| {
                    (1. ..=120.).contains(&fps) && end > start.max(0.)
                });
            if usable {
                out.push((clip, body, weapon));
            }
        }
    }
    out
}

/// Genie's and Peter Pan's summon commands (FUN_1403de460).
pub const GENIE: u16 = 0x59;
pub const PETER_PAN: u16 = 0x132;

/// FUN_1403de460's pick: the 69069 generator, `state / ((n − 1 + 2³²) /
/// n)`.
fn genie_pick(rng: &mut ScriptRandom, forms: &[u8]) -> u8 {
    rng.0 = rng.0.wrapping_mul(0x10dcd);
    let n = forms.len() as u64;
    let k = u64::from(rng.0) / (1u64 << 32).div_ceil(n);
    forms[(k as usize).min(forms.len() - 1)]
}

/// FUN_1403de220: Genie's form command by form number (0x1405c83a6).
pub fn genie_form_command(form: u8) -> u16 {
    match form {
        1 => 0x217,
        2 => 0x218,
        4 => 0x219,
        5 => 0x21a,
        _ => 0,
    }
}

/// FUN_1403de010: the summon leaves (state 3, act `leave`); the gauge
/// goes back to Drive mode (FUN_140405440 / FUN_140404a40 set +0x1af 1).
pub fn dismiss(game: &mut Game, tick: u64) {
    let Some(s) = game.summon.as_mut() else {
        return;
    };
    if s.state == state::LEAVING {
        return;
    }
    s.state = state::LEAVING;
    if let Some(a) = s.actor.as_mut() {
        a.brain.start_act(b"leave");
    }
    game.player.drive.mode = player::drive::mode::DRIVE;
    game.event(serde_json::json!({"event":"summon_dismiss","tick":tick}));
}

/// FUN_1403f3fa0 / FUN_1403f4180: a point `d` (random in 200..300) from
/// Sora, trying eight directions π/4 apart from a random base whose octant
/// steps each call (DAT_142ae5d78), on the floor below and with no wall on
/// the way; 100 nearer after each round. [I] The wall test samples the
/// room's wall push along the line (FUN_14040c2b0 is not ported), and the
/// last fallback (FUN_1403f3dd0) is Sora's own point.
fn placement(
    world: &impl World,
    from: [f32; 3],
    rng: &mut ScriptRandom,
    octant: &mut u32,
) -> [f32; 3] {
    // The scripts' generator (×0x10dcd): the distance, then the angle.
    let mut d = rng.range(NEAR, FAR);
    let n = *octant;
    let base = ((n >> 2 & 1) | n << 1) as f32 * std::f32::consts::FRAC_PI_4
        + rng.getf(std::f32::consts::FRAC_PI_4);
    *octant = (n + 1) & 7;
    while NEAR <= d {
        for k in 1..=8 {
            let angle = k as f32 * std::f32::consts::FRAC_PI_4 + base;
            let (s, c) = angle.sin_cos();
            let at = [from[0] + s * d, from[1], from[2] + c * d];
            let Some(floor) = world.floor(at[0], at[2], from[1] + 200.) else {
                continue;
            };
            let clear = (1..=8).all(|i| {
                let t = i as f32 / 8. * (d + 20.);
                let p = [from[0] + s * t, from[2] + c * t];
                let pushed = world.resolve_walls(p, 1., floor + 100., floor + 100.);
                (pushed[0] - p[0]).hypot(pushed[1] - p[1]) < 1.0e-3
            });
            if clear {
                return [at[0], floor, at[2]];
            }
        }
        d -= 100.;
    }
    from
}

/// One frame of the summon: FUN_1403de610's wait and spawn, the object's
/// script, the summon time (FUN_1403a92c0) and its end (FUN_140404a40),
/// and the leaving (FUN_1403de080 / FUN_140400440).
pub fn tick(mut game: ResMut<Game>, gallery: Res<EnemyGallery>, rig: Res<super::CameraRig>) {
    let game = &mut *game;
    let tick = game.tick.saturating_sub(1);
    // [Tour setup] Each summon of the tour starts from a full gauge.
    if game.summon_tour {
        if let Some(k) = super::summon_tour::request(tick)
            && let Some(row) = game.summons.as_ref().and_then(|s| s.rows.get(k)).copied()
        {
            game.player.drive.level = game.player.drive.max;
            game.summon_request = Some(row.command);
        }
        if super::summon_tour::dismiss(tick) {
            game.summon_dismiss = true;
        }
    }
    if let Some(cmd) = game.summon_request.take()
        && let Err(e) = start(game, cmd, tick)
    {
        game.event(
            serde_json::json!({"event":"summon_refused","tick":tick,"command":cmd,"error":e}),
        );
    }
    if std::mem::take(&mut game.summon_dismiss) {
        dismiss(game, tick);
    }
    let Some(mut s) = game.summon.take() else {
        return;
    };
    if s.actor.is_none() && s.state == state::OUT {
        s.wait -= 1.;
        if s.wait <= 0. {
            match spawn(game, &s, &gallery) {
                Ok(actor) => {
                    s.actor = Some(actor);
                    s.holds_sora = false;
                    // FUN_1403c2740: summon mode; Sora's appear motion.
                    game.player.drive.enter_summon(LEVEL);
                    game.player.play_motion(APPEAR_MOTION, 0., &game.clips);
                    game.player.queue_motion(0, 0.);
                    let at = s.actor.as_ref().map(|a| a.position().to_array());
                    game.event(serde_json::json!({"event":"summon_appear","tick":tick,
                        "command":s.command,"at":at}));
                }
                Err(e) => {
                    game.event(serde_json::json!({"event":"summon_failed","tick":tick,
                        "command":s.command,"error":e}));
                    finish(game, s, tick);
                    return;
                }
            }
        }
    }
    if let Some(a) = s.actor.as_mut() {
        a.brain.player_yaw = game.player.yaw;
        let velocity = a.sora_velocity(game.player.position);
        a.brain.facts = super::enemy_script::HostFacts {
            battle: super::second_deck::battle(game),
            drive_time: game.player.drive.time,
            // FUN_1403de460 sets DAT_1409acdd7 for Peter Pan (0x132).
            tink: s.command == 0x132,
            player_hp: game
                .enemy
                .as_ref()
                .map(|rt| (rt.sora_hp, rt.sora_target.max_hp)),
            player_mp: Some((game.player.mp.cur, game.player.mp.max)),
            player_velocity: velocity,
            stick: game.summon_stick,
            // [I] The projection's scale (+0x40) read as the follow
            // camera's focal length on the 640-wide screen.
            camera: Some((
                super::from_world(rig.camera.eye).to_array(),
                super::from_world(rig.camera.look_at).to_array(),
                320. / (rig.camera.mode.fov * 0.5).tan(),
            )),
            flight_spec: a.flight_spec,
            enemy_radius: super::party_limit::room_enemy_radius(game).unwrap_or(0.),
            player_star: false,
            player_centre: super::SORA_OBJ[2],
        };
        let enemy = super::party_limit::room_enemy(game);
        a.brain.set_enemy(enemy);
        // Sora's attacks notify the friends (FUN_1401b56e0 mode 1); each
        // attack step is a new +0x180 [I: one per Attack/AirAttack action
        // change; it aims at the room's enemy while one is there].
        let action = game.player.action;
        let attacking = matches!(
            action,
            player::Action::Attack(_) | player::Action::AirAttack(_)
        );
        if attacking && a.last_attack != Some(action) {
            a.attack_serial += 1;
            a.brain
                .note_player_attack(a.attack_serial, enemy.map(|_| super::enemy_script::ENEMY));
        }
        a.last_attack = attacking.then_some(action);
        a.brain
            .age_player_notify(attacking.then_some(a.attack_serial));
        // [I] A drop in Sora's HP while the room's enemy is there is its hit.
        let hp = game.enemy.as_ref().map(|rt| rt.sora_hp);
        if let (Some(now), Some(before)) = (hp, a.sora_hp)
            && now < before
            && enemy.is_some()
        {
            a.brain.note_player_attacker(super::enemy_script::ENEMY);
        }
        a.sora_hp = hp;
        // A form picked from the deck goes to Genie (trigger 11).
        if let Some(cmd) = game.genie_command.take() {
            a.brain.command(cmd);
            game.event(serde_json::json!({"event":"genie_command","tick":tick,"command":cmd}));
        }
        a.brain.tick(&game.world, game.player.position, &mut a.rng);
        if let Some(form) = a.brain.genie_form.take() {
            game.summon_form = form;
            game.event(serde_json::json!({"event":"genie_form","tick":tick,"form":form}));
        }
        // FUN_1403c2040: Stitch's MP special fills Sora's MP and ends a
        // charge.
        for _ in 0..std::mem::take(&mut a.brain.mp_refills) {
            game.player.mp = player::magic::Mp::new(game.player.mp.max);
        }
        // Stitch is drawn in camera space (FUN_1401cad30, FUN_1403cbfc0):
        // his screen point at depth 1024, scaled by 0.503644, in front of
        // the follow camera; his heading turns about the view's vertical.
        // [I] View x right, y down, z forward, as the engine's; the motion
        // root's heading is not subtracted.
        a.camera_pose = a.brain.stitch_screen().map(|v| {
            let s = enemy::friend::stitch::SCALE;
            let eye = super::from_world(rig.camera.eye);
            let look = super::from_world(rig.camera.look_at);
            let cam = Transform::from_translation(eye).looking_at(look, Vec3::Y);
            let at = eye + cam.rotation * Vec3::new(v[0] * s, -v[1] * s, -v[2] * s);
            let turn = cam.rotation * Quat::from_rotation_y(std::f32::consts::PI - v[3]);
            (at, turn)
        });
        if let Some((at, _)) = a.camera_pose {
            a.brain.set_position(at.to_array());
        }
        // FUN_1401cb830, Chicken Little's limit frame: the lock-on button
        // (pressed bit 11) takes a target, a held right-stick bit
        // (0x3c0000) lets it go; the aim point is the target, else 300
        // along the camera from its eye. [I] The room has one enemy: the
        // cycling through his target list (+0xde8) takes it each time; the
        // refusal sound (FUN_1401dd170(7)) is not played.
        a.aim_recenter = false;
        if a.brain.special_deck().is_some() {
            // The target's position (FUN_1403bdde0): its first TARGET
            // shape's centre.
            let enemy = super::party_limit::room_enemy(game).map(|feet| {
                game.enemy
                    .as_ref()
                    .and_then(|rt| super::enemy_targets(rt).first().map(|t| t.centre))
                    .unwrap_or(feet)
            });
            let lock =
                game.pad.pressed & 0x800 != 0 || game.summon_tour && super::summon_tour::lock(tick);
            if lock {
                a.aim_lock = enemy.is_some();
                a.aim_recenter = a.aim_lock;
            } else if game.pad.held & 0x3c_0000 != 0 {
                a.aim_lock = false;
            }
            let aim = match enemy.filter(|_| a.aim_lock) {
                Some(e) => Some(Vec3::from_array(e)),
                None => {
                    a.aim_lock = false;
                    // The limit's first frame keeps the aim his think left
                    // (the target, or 200 ahead): the view starts on it
                    // (FUN_1401cb6c0 -> FUN_140178760).
                    rig.camera.aiming.then(|| {
                        let eye = super::from_world(rig.camera.eye);
                        let look = super::from_world(rig.camera.look_at);
                        eye + (look - eye).normalize_or_zero() * enemy::friend::CHICKEN_LITTLE_AIM
                    })
                }
            };
            if let Some(aim) = aim {
                a.brain.set_aim(aim.to_array());
            }
        } else {
            a.aim_lock = false;
        }
        let blending = a.brain.blend() > 0.;
        match a
            .clips
            .pose_key(a.brain.motion(), a.brain.elapsed(), blending)
        {
            Some((_, motion, key)) => match sample(&a.kind, &motion, key, false, false) {
                Ok(p) => {
                    a.pose = p.points;
                    a.revision += 1;
                }
                Err(e) if !a.reported => {
                    a.reported = true;
                    game.event(
                        serde_json::json!({"event":"summon_pose_failed","tick":tick,"error":e}),
                    );
                }
                Err(_) => {}
            },
            None if !a.reported => {
                a.reported = true;
                let motion = a.brain.motion();
                game.event(
                    serde_json::json!({"event":"summon_motion_absent","tick":tick,"motion":motion}),
                );
            }
            None => {}
        }
        if s.state == state::LEAVING && a.brain.fade().1 {
            a.fading = true;
        }
        let trace = a.brain.take_trace();
        if !trace.is_empty() {
            let calls: Vec<String> = trace.iter().map(|(t, i)| format!("{t}:{i}")).collect();
            game.event(
                serde_json::json!({"event":"summon_trace","tick":tick,"state":s.state,"calls":calls,
                "y":a.brain.position()[1],"mode":a.brain.mode_flags(),"motion":a.brain.motion(),
                "frame":a.brain.elapsed(),
                "target":a.brain.friend_state().map(|f| f.4),
                "screen":a.brain.stitch_screen(),
                "sora_motion":a.brain.limit_player().map(|p| (p.motion.slot, p.capture.map(|c| (c.motion, c.remaining)))),
                "sora_action":format!("{:?}", game.player.action),
                "eye":super::from_world(rig.camera.eye).to_array(),
                "look":super::from_world(rig.camera.look_at).to_array(),
                "sora":game.player.position}),
            );
        }
        land_attacks(game, a, tick);
        let reports = a.brain.take_reports();
        if !reports.unsupported.is_empty() || !reports.services.is_empty() {
            game.event(
                serde_json::json!({"event":"summon_script_reports","tick":tick,
                "unsupported":reports.unsupported,"services":reports.services,
                "sounds":reports.sounds.iter().map(|s| s.0).collect::<Vec<_>>()}),
            );
        }
        if let Some(f) = a.brain.fault.take()
            && a.faults.insert(f.clone())
        {
            game.event(serde_json::json!({"event":"summon_fault","tick":tick,"fault":f}));
        }
        if tick.is_multiple_of(30) {
            let (motion, act) = (a.brain.motion(), a.brain.description());
            let time = game.player.drive.time;
            let missing: Vec<String> = a
                .brain
                .missing_traps()
                .iter()
                .map(|(t, i)| format!("{t}:{i}"))
                .collect();
            game.event(
                serde_json::json!({"event":"summon_tick","tick":tick,"state":s.state,
                "position":a.brain.position(),"motion":motion,"act":act,"time":time,
                "missing":missing,"fade":a.brain.fade().0,"friend":a.brain.friend_state(),
                "control":a.brain.motion_flags(),"sora":game.player.position,
                "limit":a.brain.limit_state().map(|l| (l.time, l.exec_rc, l.pending)),"offers":a.brain.offers}),
            );
        }
    }
    // FUN_1403a92c0: the summon time drains while it is out; at 0,
    // FUN_140404a40 dismisses once Sora could run Dismiss.
    if s.state == state::OUT && s.actor.is_some() {
        let (battle, boost) = (
            super::second_deck::battle(game),
            game.player.abilities.summon_boost,
        );
        game.player.drive.drain_frame(battle, 1., boost);
        if game.player.drive.time <= 0. && game.player.can_command(&game.clips) {
            game.summon = Some(s);
            dismiss(game, tick);
            s = game.summon.take().expect("summon kept");
        }
    }
    if s.state == state::LEAVING && s.actor.as_ref().is_none_or(Actor::gone) {
        finish(game, s, tick);
        return;
    }
    game.summon = Some(s);
}

/// FUN_1403de610's spawn: the object's kind, clips and script at the
/// placement, a summon on Sora's side, act `appear_skip`.
fn spawn(game: &mut Game, s: &Summoned, gallery: &EnemyGallery) -> Result<Actor, String> {
    let source = gallery.source.clone();
    let mut library = data::path_lookup::Library::open(&source).map_err(|e| e.to_string())?;
    let objects = objentry::read(&library.read("00objentry.bin").map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let object = objects
        .iter()
        .find(|o| o.object_id == u32::from(s.row.object))
        .ok_or("summon object absent from objentry")?;
    let entry = plain_entry(&source, object).map_err(|e| e.to_string())?;
    let kind = Arc::new(load_entry_kind(&source, &entry).map_err(|e| e.to_string())?);
    let clips = Arc::new(build_clips(&kind, |_| Ok(Vec::new()))?);
    let sstm = game
        .summons
        .as_ref()
        .map(|s| s.sstm)
        .ok_or("summon tables absent")?;
    let mut rng = ScriptRandom::default();
    let at = placement(
        &game.world,
        game.player.position,
        &mut rng,
        &mut game.summon_octant,
    );
    let yaw = (game.player.position[0] - at[0]).atan2(game.player.position[2] - at[2]);
    // [I] A summon takes no damage in this room; its HP is nominal.
    let stats = combat::TargetStats::enemy(100, 10000, 0, [100; 7], 100, 1);
    let mut brain = ScriptBrain::load(
        &source,
        &entry.model_name,
        at,
        yaw,
        stats,
        sstm,
        clips.clone(),
        super::enemy_script::ScriptSetup {
            object_type: object.object_type,
            entry_id: object.object_id,
            ..Default::default()
        },
    )?;
    let friend = Personality::of_character(object.character).map(|p| {
        let (walk, run) = prty_speeds(&mut library, object.neo_moveset).unwrap_or((2., 8.));
        Friend::new(p, walk, run)
    });
    brain.make_summon(object.object_id, friend);
    // The motions its script captures for Sora last as long as his kit's
    // clips of them.
    let row = game
        .summons
        .as_ref()
        .and_then(|t| t.rows.iter().position(|r| r.object == s.row.object));
    if let Some(row) = row {
        brain.set_captured(
            CAPTURE_SLOTS
                .filter_map(|slot| {
                    let clip = capture_clip(row, slot as i32)?;
                    Some((slot, game.clips.scripted_clip(clip)?.1.end))
                })
                .collect(),
        );
    }
    if let Some(d) = game.second_deck.as_ref() {
        brain.set_commands(
            d.commands
                .iter()
                .map(|(&id, c)| (id, (c.chain, c.decider, c.flags)))
                .collect(),
        );
    }
    if s.command == GENIE {
        brain.set_genie_form(genie_form_command(game.summon_form));
    }
    if std::env::var_os("KH2_SUMMON_TRACE").is_some() {
        brain.trace();
    }
    brain.player_yaw = game.player.yaw;
    brain.set_spawner(Box::new(Spawner::new(source.clone(), &objects)));
    brain.start_act(b"appear_skip");
    Ok(Actor {
        pose: Vec::new(),
        kind,
        brain,
        clips,
        rng,
        revision: 0,
        reported: false,
        fading: false,
        faults: Default::default(),
        last_sora: None,
        sora_hp: None,
        last_attack: None,
        attack_serial: 0,
        camera_pose: None,
        aim_lock: false,
        aim_recenter: false,
        flight_spec: prty_words(&mut library, 0x3e).map(|w| {
            let mut p = actor_physics::ObjParams::defaults(sstm.fall_max);
            p.apply_prty(&w);
            p
        }),
    })
}

/// +0x128 / +0x12c: walk and run speed, PRTY words 0 and 1 of the record
/// objentry +0x4e names (FUN_1403d5840 keeps it at +0xd50) [I: as the party
/// members' read].
fn prty_speeds(library: &mut data::path_lookup::Library, moveset: u16) -> Option<(f32, f32)> {
    let w = prty_words(library, moveset as i16 as i32)?;
    Some((f32::from_bits(w[0]), f32::from_bits(w[1])))
}

/// The PRTY record (03system pref/prty) of `index` (FUN_1403e9590).
fn prty_words(library: &mut data::path_lookup::Library, index: i32) -> Option<[u32; 13]> {
    let system = library.read("03system.bin").ok()?;
    let pref = super::bar_entry(&system, b"pref").ok()?;
    let prty = data::party_records::read_prty(super::bar_entry(pref, b"prty").ok()?).ok()?;
    let offset = actor_physics::prty_offset(&prty.offsets, index)?;
    prty.records
        .iter()
        .find(|r| r.offset == offset && !r.overlaps_table)
        .map(|r| r.words)
}

/// The host side of the summon's `sysobj_appear` (FUN_1403df930): the
/// entry's model script, a bare object otherwise. [Open] The made objects'
/// models are not drawn.
/// The host side of `sysobj_appear` (FUN_1403df930): the object entry's
/// model script, and its class from the entry's type byte.
pub(super) struct Spawner {
    source: Arc<data::probe_source::ProbeSource>,
    models: std::collections::HashMap<u32, (String, u8)>,
    scripts: std::collections::HashMap<u32, Option<Vec<u8>>>,
}

impl Spawner {
    pub(super) fn new(
        source: Arc<data::probe_source::ProbeSource>,
        objects: &[objentry::ObjectEntry],
    ) -> Self {
        Spawner {
            source,
            models: objects
                .iter()
                .map(|o| {
                    (
                        o.object_id,
                        (
                            String::from_utf8_lossy(&o.model_name).into_owned(),
                            o.object_type,
                        ),
                    )
                })
                .collect(),
            scripts: Default::default(),
        }
    }
}

impl enemy::world::Spawner for Spawner {
    fn appear(&mut self, request: &enemy::world::AppearRequest) -> Option<enemy::world::Appeared> {
        let entry = request.entry & 0x0fff_ffff;
        let payload = self
            .scripts
            .entry(entry)
            .or_insert_with(|| {
                let (model, _) = self.models.get(&entry)?;
                super::enemy_script::script_payload(&self.source, model).ok()
            })
            .clone();
        let mut sys = enemy::sysobj::Sysobj::bare();
        sys.appear_way = 2;
        sys.model = entry;
        if let Some(&(_, kind)) = self.models.get(&entry) {
            sys.set_class(kind);
        }
        Some(enemy::world::Appeared { payload, sys })
    }
}

/// The summon's attack objects (its own and its made objects') land on
/// the room's enemy (FUN_1403cf830 hit tests through `party_room`).
/// [I] With the attacker's stats Sora's: the summon sheet's are not traced.
fn land_attacks(game: &mut Game, a: &mut Actor, tick: u64) {
    let strikes = a.brain.take_enemy_strikes();
    let Some(enemy_at) = super::party_limit::room_enemy(game) else {
        return;
    };
    // A direct strike (`attack_strike`, 2:17) lands where the enemy stands.
    let struck =
        strikes.into_iter().map(
            |(slot, [id, extra], serial)| super::enemy_script::ScriptAttack {
                slot,
                id,
                extra,
                serial,
                position: enemy_at,
                previous: None,
                radius: 1.,
                hits_sora: false,
            },
        );
    let volumes: Vec<_> = a
        .brain
        .attacks()
        .into_iter()
        .filter(|x| !x.hits_sora)
        .chain(struck)
        .collect();
    for at in volumes {
        let Some(rt) = game.enemy.as_mut() else {
            return;
        };
        let key = 1 << 59 | 1 << 57 | at.serial;
        let v = player::magic::Volume {
            key,
            atkp: at.id as u16,
            sub: at.extra as u8,
            centre: at.position,
            radius: at.radius,
            height: at.radius,
            previous: at.previous,
            moving: None,
            role: player::magic::Role::Damage,
        };
        let yaw = a.brain.yaw();
        let facing = bevy::math::Vec3::new(yaw.sin(), 0., yaw.cos());
        let stats = rt.sora;
        let landed = super::party_room::land_volume(
            rt,
            usize::MAX,
            "summon",
            &stats,
            &v,
            key,
            facing,
            tick,
            &mut game.sound_queue,
            &mut game.log,
        );
        if landed.is_some() {
            a.brain.count_limit_hits(1);
            a.brain
                .mark_attack_hit_on(at.slot, super::enemy_script::ENEMY);
            game.event(serde_json::json!({"event":"summon_hit","tick":tick,
                "atkp":at.id,"owner":a.brain.attack_owner(at.slot)}));
        }
    }
}

/// FUN_140400440: the members come back; the summon state clears.
fn finish(game: &mut Game, s: Summoned, tick: u64) {
    for &i in &s.stored {
        if let Some(m) = game.party.get_mut(i) {
            m.stored = false;
        }
    }
    if game.player.drive.mode == player::drive::mode::SUMMON {
        game.player.drive.mode = player::drive::mode::DRIVE;
    }
    game.event(serde_json::json!({"event":"summon_end","tick":tick,"command":s.command}));
}

// --- Drawing ---------------------------------------------------------------

#[derive(Component)]
pub struct SummonRoot(u64);

#[derive(Component)]
pub struct SummonPart {
    ordinal: usize,
    revision: u64,
}

type PartQuery<'a> = (
    &'a mut SummonPart,
    &'a Mesh3d,
    &'a MeshMaterial3d<FogMaterial>,
);

/// Spawns the summon's model when it comes, poses it each frame and frees
/// it when it has gone.
#[allow(clippy::too_many_arguments)]
pub fn present(
    mut commands: Commands,
    game: Option<Res<Game>>,
    world: Option<Res<SceneRoot>>,
    mut roots: Query<(Entity, &SummonRoot, &mut Transform, &mut Visibility)>,
    mut parts: Query<PartQuery>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let actor = game
        .as_ref()
        .and_then(|g| g.summon.as_ref())
        .and_then(|s| Some((s.serial, s.actor.as_ref()?)))
        .filter(|(_, a)| !a.gone() && !a.pose.is_empty());
    let mut live = false;
    for (entity, root, mut transform, mut visibility) in &mut roots {
        match actor {
            Some((serial, a)) if serial == root.0 => {
                live = true;
                transform.translation = a.position();
                transform.rotation = a.rotation();
                // FUN_1401cb6c0 fades Chicken Little out (+0xa10) for his
                // limit, FUN_1401cb180 back in.
                let shown = if a.brain.special_deck().is_some() {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
                if *visibility != shown {
                    *visibility = shown;
                }
            }
            _ => {
                for (_, mesh, material) in &parts {
                    meshes.remove(mesh.0.id());
                    materials.remove(material.0.id());
                }
                commands.entity(entity).despawn();
            }
        }
    }
    let Some((serial, a)) = actor else {
        return;
    };
    if !live {
        let Some(world) = world else {
            return;
        };
        let root = commands
            .spawn((
                SummonRoot(serial),
                ChildOf(world.0),
                Transform::from_translation(a.position()).with_rotation(a.rotation()),
                Visibility::default(),
            ))
            .id();
        for (ordinal, entity) in spawn_model(
            &mut commands,
            &a.kind.model,
            &a.pose,
            root,
            &mut meshes,
            &mut materials,
            &mut images,
        ) {
            if let Some(entity) = entity {
                commands.entity(entity).insert(SummonPart {
                    ordinal,
                    revision: a.revision,
                });
            }
        }
        return;
    }
    for (mut part, mesh, _) in &mut parts {
        if part.revision == a.revision {
            continue;
        }
        if let Some(mut m) = meshes.get_mut(mesh.0.id()) {
            let positions = a.kind.model.parts[part.ordinal]
                .indices
                .iter()
                .map(|&i| a.pose[i as usize].to_array());
            visual::room::update_positions(&mut m, positions);
        }
        part.revision = a.revision;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_clips_sit_above_the_limits_and_apart_per_summon() {
        assert_eq!(capture_clip(3, 256), Some(0x7b10));
        assert_eq!(capture_clip(0, 252), Some(0x780c));
        assert_eq!(capture_clip(0, 251), None);
        assert_eq!(capture_clip(0, -1), None);
        // The limits' ids (0x4000 | index << 9 | motion) stay below.
        assert!(super::super::party_limit::clip_id(15, 0x1ff) < 0x7800);
    }

    #[test]
    fn genies_form_is_a_uniform_pick_of_the_owned_ones() {
        let mut rng = ScriptRandom::default();
        // 69069 · 1 = 69069: the first quarter of four.
        assert_eq!(genie_pick(&mut rng, &[1, 2, 4, 5]), 1);
        let picks: Vec<u8> = (0..400)
            .map(|_| genie_pick(&mut rng, &[1, 2, 4, 5]))
            .collect();
        for f in [1, 2, 4, 5] {
            assert!(picks.contains(&f), "{f}");
        }
        assert_eq!(genie_pick(&mut rng, &[4]), 4);
        assert_eq!(
            [1, 2, 3, 4, 5].map(genie_form_command),
            [0x217, 0x218, 0, 0x219, 0x21a]
        );
    }

    struct Flat;
    impl World for Flat {
        fn floor(&self, _: f32, _: f32, _: f32) -> Option<f32> {
            Some(0.)
        }
        fn resolve_walls(&self, p: [f32; 2], _: f32, _: f32, _: f32) -> [f32; 2] {
            p
        }
        fn collide(
            &self,
            start: player::Vec3,
            displacement: player::Vec3,
            _: bool,
            _: &player::Body,
        ) -> player::Collided {
            player::Collided {
                position: [0, 1, 2].map(|i| start[i] + displacement[i]),
                on_floor: true,
            }
        }
    }

    #[test]
    fn a_summon_lands_200_to_300_away_on_the_floor_and_the_octant_steps() {
        let mut rng = ScriptRandom::default();
        let mut octant = 7;
        let at = placement(&Flat, [10., 50., -20.], &mut rng, &mut octant);
        let d = (at[0] - 10.).hypot(at[2] + 20.);
        assert!((NEAR..=FAR).contains(&d), "{d}");
        assert_eq!(at[1], 0.);
        assert_eq!(octant, 0);
    }
}
