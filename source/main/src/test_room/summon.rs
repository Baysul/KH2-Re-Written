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
}

impl Actor {
    pub fn position(&self) -> Vec3 {
        Vec3::from_array(self.brain.position())
    }
    pub fn yaw(&self) -> f32 {
        self.brain.yaw()
    }
    /// The object removed itself, or [I] its leaving fade has ended: the
    /// summon class's removal (FUN_1403dd850 clears DAT_142a24ee8 and the
    /// summon state) runs from the kn::Friend update, which is not traced;
    /// the act `leave` ends in `sysobj_fadeout` and a wait.
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
pub fn tick(mut game: ResMut<Game>, gallery: Res<EnemyGallery>) {
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
        a.brain.tick(&game.world, game.player.position, &mut a.rng);
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
        if !trace.is_empty() && (s.state == state::LEAVING || tick.is_multiple_of(30)) {
            let calls: Vec<String> = trace.iter().map(|(t, i)| format!("{t}:{i}")).collect();
            game.event(serde_json::json!({"event":"summon_trace","tick":tick,"state":s.state,"calls":calls}));
        }
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
                "missing":missing,"fade":a.brain.fade().0}),
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
    )?;
    brain.make_summon();
    if std::env::var_os("KH2_SUMMON_TRACE").is_some() {
        brain.trace();
    }
    brain.player_yaw = game.player.yaw;
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
    })
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
    mut roots: Query<(Entity, &SummonRoot, &mut Transform)>,
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
    for (entity, root, mut transform) in &mut roots {
        match actor {
            Some((serial, a)) if serial == root.0 => {
                live = true;
                transform.translation = a.position();
                transform.rotation = Quat::from_rotation_y(a.yaw());
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
                Transform::from_translation(a.position())
                    .with_rotation(Quat::from_rotation_y(a.yaw())),
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
