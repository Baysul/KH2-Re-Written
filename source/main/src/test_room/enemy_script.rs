//! Script-driven enemy brain: the enemy's own object script, loaded from the
//! installed game and executed by the translated interpreter in the `enemy`
//! crate, replaces the hand-written decision ports. The motion data, collision
//! world and Sora's position come from the surrounding host.
use super::{RoomWorld, enemy_battle::Clips};
use data::{bar, path_lookup::Library, probe_source::ProbeSource};
use enemy::{
    env::{Clips as EnemyClips, Env},
    sysobj::Sysobj,
    world::{Object, PLAYER_OBJECT, World as EnemyWorld},
};
use player::{World, actor_physics, combat, shadow::ScriptRandom};
use std::sync::Arc;

type Vec3 = [f32; 3];

/// Sora's team number (`sysobj+0x4dc`): attacks with her team in their mask
/// can hit her.
pub(super) const SORA_TEAM: u8 = 1;

/// The host view the engine reads each tick.
struct BrainEnv<'a> {
    room: Option<&'a RoomWorld>,
    clips: ClipView<'a>,
    player: Vec3,
    player_yaw: f32,
    facts: HostFacts,
    /// Each command's (+0x22 chain, +0x1f decider, +0xc flags).
    commands: &'a CommandWindows,
}

/// The command records the motion-window test reads (FUN_1403b6950).
pub type CommandWindows = std::collections::HashMap<u16, (u16, u8, u32)>;

/// The object's clips, and the lengths of the motions of its set its
/// script captures for Sora (`obj_motion_capture`), which only his kit
/// holds.
struct ClipView<'a> {
    base: &'a Clips,
    captured: &'a std::collections::HashMap<usize, u32>,
}

impl EnemyClips for ClipView<'_> {
    fn length(&self, slot: usize) -> u32 {
        match EnemyClips::length(self.base, slot) {
            0 => self.captured.get(&slot).copied().unwrap_or(0),
            n => n,
        }
    }
    fn loop_start(&self, slot: usize) -> u32 {
        EnemyClips::loop_start(self.base, slot)
    }
    fn root_delta(&self, slot: usize, tick: u32) -> Vec3 {
        EnemyClips::root_delta(self.base, slot, tick)
    }
    fn advance(&self, slot: usize, tick: u32) -> u32 {
        EnemyClips::advance(self.base, slot, tick)
    }
    fn events(&self, slot: usize) -> Option<&enemy::motion::MotionEvents> {
        EnemyClips::events(self.base, slot)
    }
}

/// The clips of an object with no motion set.
struct NoClips;
impl EnemyClips for NoClips {
    fn length(&self, _: usize) -> u32 {
        0
    }
    fn loop_start(&self, _: usize) -> u32 {
        0
    }
    fn root_delta(&self, _: usize, _: u32) -> Vec3 {
        [0.; 3]
    }
    fn events(&self, _: usize) -> Option<&enemy::motion::MotionEvents> {
        None
    }
}

/// What the host tells a script about the game around it each tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostFacts {
    /// The battle flag (DAT_142a11404).
    pub battle: bool,
    /// Sora's form or summon time (sheet +0x1b4).
    pub drive_time: f32,
    /// Peter Pan is out with Tinker Bell (FUN_1403de320).
    pub tink: bool,
    /// Sora's HP and maximum HP, and his velocity (+0xa38).
    pub player_hp: Option<(i32, i32)>,
    /// Sora's MP and maximum MP.
    pub player_mp: Option<(i32, i32)>,
    pub player_velocity: Vec3,
    /// The stick's movement request (`player_get_movement`).
    pub stick: Vec3,
    /// The camera's eye and look-at (this frame's coordinates) and the
    /// projection's scale.
    pub camera: Option<(Vec3, Vec3, f32)>,
    /// PRTY record 0x3e, the movement block of Peter Pan's limit flight
    /// (`obj_limit_peterpan_idle`).
    pub flight_spec: Option<player::actor_physics::ObjParams>,
    /// The TARGET radius of the room's enemy (its stand-in, [`ENEMY`]).
    pub enemy_radius: f32,
    /// Sora cannot be damaged now (`obj_is_star`).
    pub player_star: bool,
    /// How far above his feet the centre of Sora's OBJ shape is (his
    /// bone 0x4001).
    pub player_centre: f32,
}

/// The id of the room's enemy as a stand-in in a script's world.
pub const ENEMY: u32 = 1;

impl Env for BrainEnv<'_> {
    fn battle(&self) -> bool {
        self.facts.battle
    }
    fn drive_time(&self) -> f32 {
        self.facts.drive_time
    }
    fn summon_with_tink(&self) -> bool {
        self.facts.tink
    }
    fn player_hp(&self) -> Option<(i32, i32)> {
        self.facts.player_hp
    }
    fn player_mp(&self) -> Option<(i32, i32)> {
        self.facts.player_mp
    }
    fn player_move_input(&self) -> Vec3 {
        self.facts.stick
    }
    fn camera_view(&self, p: Vec3) -> Option<[f32; 4]> {
        let (eye, look, scale) = self.facts.camera?;
        let cam = bevy::prelude::Transform::from_translation(eye.into())
            .looking_at(look.into(), bevy::prelude::Vec3::Y);
        let v = cam.compute_affine().inverse().transform_point3(p.into());
        (v.z < 0.).then_some([v.x, -v.y, -v.z, scale])
    }
    fn movement_spec(&self, index: i32) -> Option<player::actor_physics::ObjParams> {
        (index == 0x3e).then_some(self.facts.flight_spec).flatten()
    }
    // [I] Sora's own lockout and windows are the room's; through a
    // script he can always take a command (the menu shows it).
    fn can_decide_command(&self, object: u32, _command: u16) -> bool {
        object == PLAYER_OBJECT
    }
    fn command_chain(&self, command: u16) -> u16 {
        self.commands.get(&command).map_or(0, |c| c.0)
    }
    fn command_decider(&self, command: u16) -> u8 {
        self.commands.get(&command).map_or(0, |c| c.1)
    }
    fn command_flags(&self, command: u16) -> u32 {
        self.commands.get(&command).map_or(0, |c| c.2)
    }
    fn player_velocity(&self) -> Vec3 {
        self.facts.player_velocity
    }
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        self.room?.floor(x, z, top)
    }
    // Objects the script made (`sysobj_appear`) carry their entry as the
    // model and have no motions of their own here.
    fn clips(&self, model: u32) -> &dyn EnemyClips {
        if model == 0 { &self.clips } else { &NoClips }
    }
    fn player_position(&self) -> Vec3 {
        self.player
    }
    fn player_yaw(&self) -> f32 {
        self.player_yaw
    }
    /// Vertical segments only: the floor where it crosses the segment.
    fn line_hit(&self, from: Vec3, to: Vec3) -> Option<Vec3> {
        if from[0] != to[0] || from[2] != to[2] {
            return None;
        }
        let top = from[1].max(to[1]);
        let floor = self.room?.floor(from[0], from[2], top)?;
        (from[1].min(to[1]) <= floor && floor <= top).then_some([from[0], floor, from[2]])
    }
    fn target_position(&self, _id: u32) -> Option<Vec3> {
        None
    }
    fn target_radius(&self, id: u32) -> Option<f32> {
        (id == ENEMY).then_some(self.facts.enemy_radius)
    }
    // The generated development arena is flat and has no step obstacles.
    fn check_step(&self, _: Vec3, _: Vec3, _: f32, _: f32, _: f32) -> Option<u8> {
        Some(0)
    }
    // The arena keeps enemies in the room.
    fn no_leave(&self) -> bool {
        true
    }
    fn player_invulnerable(&self) -> bool {
        self.facts.player_star
    }
    /// FUN_1403b50f0 for Sora: 0x4000 his origin, 0x4001 the centre of his
    /// first enabled OBJ shape, both with his rotation, in the script's
    /// +Y-down frame. [U] His skeleton's own bones are not handed over.
    fn bone_matrix(&self, object: u32, bone: u32) -> Option<[f32; 16]> {
        let lift = match (object, bone) {
            (PLAYER_OBJECT, 0x4000) => 0.,
            (PLAYER_OBJECT, 0x4001) => self.facts.player_centre,
            _ => return None,
        };
        let (s, c) = self.player_yaw.sin_cos();
        let p = self.player;
        Some([
            c,
            0.,
            -s,
            0.,
            0.,
            1.,
            0.,
            0.,
            s,
            0.,
            c,
            0.,
            p[0],
            -(p[1] + lift),
            p[2],
            1.,
        ])
    }
    fn area_world(&self) -> i32 {
        2
    }
}

/// An attack object a script placed, as the host sees it.
#[derive(Clone, Copy, Debug)]
pub struct ScriptAttack {
    pub slot: usize,
    pub serial: u64,
    pub id: i32,
    pub extra: i32,
    pub position: Vec3,
    pub previous: Option<Vec3>,
    pub radius: f32,
    /// Sora's team (1) is in the attack's team mask.
    pub hits_sora: bool,
}

/// An effect a script started: element `id` of the enemy's own container.
#[derive(Clone, Copy, Debug)]
pub struct ScriptEffect {
    pub id: u16,
    /// Where it was placed (`obj_effect_start_pos`); `None` follows the enemy.
    pub at: Option<Vec3>,
}

/// See [`ScriptBrain::take_reports`].
#[derive(Debug, Default)]
pub struct ScriptReports {
    pub sounds: Vec<(i32, Vec3)>,
    pub voices: Vec<(i32, i32)>,
    pub services: Vec<&'static str>,
    pub unsupported: Vec<&'static str>,
}

/// The attack objects a world's scripts have placed.
pub fn script_attacks(world: &EnemyWorld) -> Vec<ScriptAttack> {
    use enemy::attack::flag;
    world
        .attacks
        .iter()
        .enumerate()
        .filter_map(|(slot, a)| {
            let a = a.as_ref()?;
            if a.flags & flag::HAS_POSITION == 0 && a.flags & flag::LINE == 0 {
                return None;
            }
            let v = |p: [f32; 4]| [p[0], p[1], p[2]];
            Some(ScriptAttack {
                slot,
                serial: a.serial,
                id: a.param_id,
                extra: a.param_extra,
                position: v(a.position),
                previous: Some(v(a.previous)),
                radius: a.current_radius(),
                hits_sora: a.can_hit_team(SORA_TEAM),
            })
        })
        .collect()
}

/// How the engine builds the object around its script (FUN_1403df930):
/// the class from the objentry type, the entry id, and the tables its traps
/// read.
#[derive(Clone, Debug, Default)]
pub struct ScriptSetup {
    pub object_type: u8,
    pub entry_id: u32,
    pub sheets: Option<super::enemy_combat::EnmpSheets>,
    pub patn: Vec<[u8; 32]>,
}

pub struct ScriptBrain {
    setup: ScriptSetup,
    world: EnemyWorld,
    object: Object,
    clips: Arc<Clips>,
    /// Lengths of the captured motions (see [`ClipView`]).
    captured: std::collections::HashMap<usize, u32>,
    payload: Vec<u8>,
    spawn: Vec3,
    player: Vec3,
    /// Sora's heading, set by the host before each tick.
    pub player_yaw: f32,
    /// The game state the host passes in before each tick.
    pub facts: HostFacts,
    /// Faults the interpreter or a missing trap reported, for the host log.
    pub fault: Option<String>,
    /// Effects the script started since the host last took them.
    pub effects: Vec<ScriptEffect>,
    /// Voice lines the script asked for (id, argument), for the host log.
    pub voices: Vec<(i32, i32)>,
    /// Sound effects the script asked for (`obj_play_se`): id and place.
    pub sounds: Vec<(i32, Vec3)>,
    /// Names of the engine services the script called that are not translated.
    pub unsupported: std::collections::BTreeSet<&'static str>,
    /// Names of the requests the script made of subsystems this host does not
    /// have (camera, screen, mission HUD, party, prizes).
    pub services: std::collections::BTreeSet<&'static str>,
    /// What the host has already reported, so each name is reported once.
    reported: std::collections::BTreeSet<&'static str>,
    /// Attack slots the script applied to Sora directly (`attack_strike`),
    /// for the host's hit resolution to take.
    pub strikes: Vec<usize>,
    /// Attack slots the script's objects applied to the enemy's stand-in
    /// directly (Chicken Little's fireworks), with their ATKP ids and serials.
    pub enemy_strikes: Vec<(usize, [i32; 2], u64)>,
    /// Stitch's MP specials not yet carried out (FUN_1403c2040 on Sora).
    pub mp_refills: u32,
    /// Genie's new form number (FUN_1403ddfb0) for the host to keep.
    pub genie_form: Option<u8>,
    /// The summon's limit while it runs (its own script as the program).
    limit: Option<enemy::limit_scene::Scene>,
    /// The command records for the motion-window test.
    commands: CommandWindows,
    /// The commands the script offers this tick (`command_override`'s
    /// records: 0 Attack's row, 1 the reaction, 2 the Limit row) and each
    /// record's target.
    pub offers: [Option<u16>; 8],
    pub offer_targets: [Option<u32>; 8],
    /// Event signals the script's objects posted (FUN_1403aabe0: code and
    /// argument), for the host's mission and area scripts.
    pub signals: Vec<(i32, i32)>,
    /// What the script did to Sora: the motions of its set it captured him
    /// into (slot, blend), and the motions it queued behind them.
    pub sora_captures: Vec<(i32, f32)>,
    pub sora_pushed: Vec<(usize, f32)>,
    /// HP changes (`obj_add_hp`: delta) and MP charges
    /// (`obj_start_mpdrive`: the factor) the script made on Sora.
    pub sora_hp: Vec<i32>,
    pub sora_mpdrive: Vec<f32>,
    /// Presentation requests for the host: the trap's name, the object it
    /// names and its argument words.
    pub presentations: Vec<(&'static str, Option<u32>, [u32; 8])>,
}

pub(super) fn script_payload(source: &ProbeSource, model: &str) -> Result<Vec<u8>, String> {
    let mut library = Library::open(source).map_err(|e| e.to_string())?;
    let bytes = library
        .read(&format!("obj/{model}.mdlx"))
        .map_err(|e| e.to_string())?;
    let table = bar::read_bar(&bytes).map_err(|e| e.to_string())?;
    let entry = table
        .entries
        .iter()
        .find(|e| e.entry_type == 3 && e.offset >= 0 && e.size != 0)
        .ok_or("the model has no object script")?;
    let start = entry.offset as usize;
    bytes
        .get(start..start + entry.size as usize)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| "object script outside the archive".into())
}

impl ScriptBrain {
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        source: &ProbeSource,
        model: &str,
        position: Vec3,
        yaw: f32,
        stats: combat::TargetStats,
        sstm: actor_physics::Sstm,
        clips: Arc<Clips>,
        setup: ScriptSetup,
    ) -> Result<Self, String> {
        let payload = script_payload(source, model)?;
        let mut brain = Self {
            setup,
            world: EnemyWorld::default(),
            object: Object::bare(0),
            clips,
            payload,
            spawn: position,
            player: position,
            player_yaw: 0.,
            facts: HostFacts::default(),
            fault: None,
            effects: Vec::new(),
            voices: Vec::new(),
            sounds: Vec::new(),
            unsupported: Default::default(),
            services: Default::default(),
            reported: Default::default(),
            captured: Default::default(),
            strikes: Vec::new(),
            enemy_strikes: Vec::new(),
            mp_refills: 0,
            genie_form: None,
            limit: None,
            commands: CommandWindows::new(),
            offers: [None; 8],
            offer_targets: [None; 8],
            signals: Vec::new(),
            sora_captures: Vec::new(),
            sora_pushed: Vec::new(),
            sora_hp: Vec::new(),
            sora_mpdrive: Vec::new(),
            presentations: Vec::new(),
        };
        brain.build(position, yaw, stats, sstm, &mut ScriptRandom::default())?;
        Ok(brain)
    }

    fn build(
        &mut self,
        position: Vec3,
        yaw: f32,
        stats: combat::TargetStats,
        sstm: actor_physics::Sstm,
        rng: &mut ScriptRandom,
    ) -> Result<(), String> {
        let mut sys = Sysobj::new(0, position, yaw, stats, sstm);
        sys.turn_rate = actor_physics::ObjParams::defaults(sstm.fall_max).turn;
        sys.appear_way = 2;
        sys.set_class(self.setup.object_type);
        sys.entry_id = self.setup.entry_id;
        self.world = EnemyWorld::default();
        self.world.tolerate_missing = true;
        self.world.patn = self.setup.patn.clone();
        self.world.sheets = self
            .setup
            .sheets
            .clone()
            .map(|s| Box::new(s) as Box<dyn enemy::world::SheetSource>);
        self.world.rng.0 = rng.0;
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        self.object = Object::spawn(&mut self.world, &env, 0, &self.payload, sys)
            .map_err(|e| format!("object script rejected: {e:?}"))?;
        rng.0 = self.world.rng.0;
        Ok(())
    }

    pub fn tick(&mut self, room: &RoomWorld, player: Vec3, rng: &mut ScriptRandom) {
        self.player = player;
        self.world.rng.0 = rng.0;
        self.offers = [None; 8];
        self.offer_targets = [None; 8];
        let env = BrainEnv {
            room: Some(room),
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        if let Some(mut scene) = self.limit.take() {
            // The limit's tick: Sora and the summon take part, the summon's
            // script is the program (its world and object lent to it).
            scene.swap_parts(&mut self.world, &mut self.object);
            let status = scene.tick(&env);
            scene.swap_parts(&mut self.world, &mut self.object);
            if status == enemy::limit_scene::Status::Over {
                enemy::limit_scene::Scene::release(&mut self.world, &mut self.object);
            } else {
                self.limit = Some(scene);
            }
            Self::tick_made(&mut self.world, &mut self.object, &env);
            self.hit_the_map(room);
            self.drain_events();
            // `player_set_exec_rc` (player +0xdd8): the reaction command Sora
            // runs next is the reaction offer.
            if let Some(l) = self.world.limit.as_ref()
                && l.exec_rc != 0
            {
                self.offers[1] = Some(l.exec_rc);
            }
            rng.0 = self.world.rng.0;
            self.record_faults();
            return;
        }
        self.object.tick(&mut self.world, &env);
        Self::tick_made(&mut self.world, &mut self.object, &env);
        self.world.tick_attacks(&env);
        self.hit_the_map(room);
        self.drain_events();
        self.take_player_queue();
        if self.object.sys.gone {
            self.world.attacks.clear();
        }
        rng.0 = self.world.rng.0;
        self.record_faults();
    }

    /// The attack objects the script has placed, for the host's hit tests.
    pub fn attacks(&self) -> Vec<ScriptAttack> {
        script_attacks(&self.world)
    }

    /// An attack of the script struck Sora: it records the strike and the
    /// script's hit callback runs (FUN_1403d1730).
    pub fn mark_attack_hit(&mut self, slot: usize) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        self.object
            .attack_struck(&mut self.world, &env, slot, PLAYER_OBJECT);
        self.record_faults();
    }

    /// The attacks the script's objects applied to the enemy directly.
    pub fn take_enemy_strikes(&mut self) -> Vec<(usize, [i32; 2], u64)> {
        std::mem::take(&mut self.enemy_strikes)
    }

    /// The attacks the script applied to Sora directly, with their parameter
    /// ids (an attack freed since is dropped).
    pub fn take_strikes(&mut self) -> Vec<(usize, i32, i32)> {
        std::mem::take(&mut self.strikes)
            .into_iter()
            .filter_map(|slot| {
                let a = self.world.attacks.get(slot)?.as_ref()?;
                Some((slot, a.param_id, a.param_extra))
            })
            .collect()
    }

    /// Sora's guard turned an attack of the script back: it is marked
    /// reflected and the script's reflect callback runs (FUN_1403d2140).
    pub fn mark_attack_reflected(&mut self, slot: usize) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        self.object
            .attack_reflected(&mut self.world, &env, slot, PLAYER_OBJECT);
        self.record_faults();
    }

    /// The attacks that touch the map this frame get the wall or floor flag
    /// and the point they touched, the way a script reads them
    /// (`attack_is_hit_wall`, `attack_is_hit_floor`, `attack_get_hit_bg_pos`).
    /// A sphere is tested against the room's floor under it and the walls
    /// around it. [U] The executable's own map test for attack objects was
    /// not located; this uses the room's collision directly.
    fn hit_the_map(&mut self, room: &RoomWorld) {
        use enemy::attack::flag;
        for a in self.world.attacks.iter_mut().flatten() {
            a.flags &= !(flag::HIT_WALL | flag::HIT_FLOOR);
            if a.flags & flag::HAS_POSITION == 0 || a.flags & flag::LINE != 0 {
                continue;
            }
            let r = a.current_radius();
            let [x, y, z, _] = a.position;
            if let Some(floor) = room.floor(x, z, y + r)
                && y - r <= floor
            {
                a.flags |= flag::HIT_FLOOR;
                a.hit_position = [x, floor, z, 1.];
            }
            let pushed = room.resolve_walls([x, z], r, y - r, y + r);
            let (dx, dz) = (pushed[0] - x, pushed[1] - z);
            let len = dx.hypot(dz);
            if len > 1.0e-3 {
                // The wall is a radius away from where the sphere was pushed
                // to, back along the push.
                a.flags |= flag::HIT_WALL;
                a.hit_position = [pushed[0] - dx / len * r, y, pushed[1] - dz / len * r, 1.];
            }
        }
    }

    /// The objects the script made (and the enemy's stand-in). The host's
    /// own object stays reachable to them meanwhile (the made objects of a
    /// summon read and drive their maker).
    fn tick_made(world: &mut EnemyWorld, object: &mut Object, env: &BrainEnv<'_>) {
        let id = object.id;
        let me = std::mem::replace(object, enemy::world::Object::bare(id));
        world.held.push(Box::new(me));
        world.tick_objects(env, None);
        if let Some(k) = world.held.iter().rposition(|o| o.id == id) {
            *object = *world.held.remove(k);
        }
        world.deliver_shouts(env, Some(&mut *object));
        world.follow_attachments(env, Some(&mut *object));
        world.tick_captures(Some(object));
    }

    /// FUN_1403d8b40 -> FUN_1403d7fa0 for the summon's limit `command`:
    /// the limit object with Sora (as `player` stands now) and the summon,
    /// and its script's trigger 22. `time` is the gauge (command +0x20).
    pub fn begin_limit(
        &mut self,
        command: u16,
        time: f32,
        player: enemy::sysobj::Sysobj,
    ) -> Result<(), String> {
        let id = self.object.id;
        let world = std::mem::take(&mut self.world);
        let object = std::mem::replace(&mut self.object, Object::bare(id));
        match enemy::limit_scene::Scene::summon(world, object, command, time, player) {
            Ok(mut scene) => {
                scene.swap_parts(&mut self.world, &mut self.object);
                self.limit = Some(scene);
                Ok(())
            }
            Err(failed) => {
                let (e, world, object) = *failed;
                self.world = world;
                self.object = object;
                Err(e)
            }
        }
    }

    /// The lengths of the motions of the object's set its script captures
    /// for Sora.
    pub fn set_captured(&mut self, lengths: std::collections::HashMap<usize, u32>) {
        self.captured = lengths;
    }

    /// The command records the scripts' window tests read.
    pub fn set_commands(&mut self, commands: CommandWindows) {
        self.commands = commands;
    }

    /// Hits that landed this tick count toward the limit's combo (+0xc0).
    pub fn count_limit_hits(&mut self, n: i32) {
        if self.limit.is_some()
            && let Some(l) = self.world.limit.as_mut()
        {
            l.hits += n;
        }
    }

    /// The limit runs.
    pub fn in_limit(&self) -> bool {
        self.limit.is_some()
    }

    /// The limit's state (its gauge time, hits, ending), while it runs.
    pub fn limit_state(&self) -> Option<&enemy::traps::limit::LimitState> {
        self.limit.as_ref().and(self.world.limit.as_ref())
    }

    /// Sora as the limit has him (the scripted stand-in).
    pub fn limit_player(&self) -> Option<&enemy::sysobj::Sysobj> {
        self.limit.as_ref()?;
        self.world.player_proxy.as_deref().map(|p| &p.sys)
    }

    /// A follow-up command Sora took (trigger 23 with the command).
    pub fn limit_follow_up(&mut self, command: u16) {
        let Some(mut scene) = self.limit.take() else {
            return;
        };
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        scene.swap_parts(&mut self.world, &mut self.object);
        if let Some(l) = scene.world.limit.as_mut()
            && l.exec_rc == command
        {
            l.exec_rc = 0;
        }
        scene.follow_up(&env, command);
        scene.swap_parts(&mut self.world, &mut self.object);
        self.limit = Some(scene);
        self.drain_events();
        self.record_faults();
    }

    /// Sort the engine's events into what the host plays and what it only
    /// reports; the list never grows past one frame.
    fn drain_events(&mut self) {
        use enemy::world::Event;
        for event in std::mem::take(&mut self.world.events) {
            match event {
                Event::EffectStart {
                    effect, position, ..
                } => {
                    if let Ok(id) = u16::try_from(effect) {
                        self.effects.push(ScriptEffect { id, at: position });
                    }
                }
                Event::Voice { id, arg, .. } => self.voices.push((id, arg)),
                Event::Unsupported(name) => {
                    self.unsupported.insert(name);
                }
                Event::Presentation { name, .. } => {
                    self.unsupported.insert(name);
                }
                Event::Service {
                    name: "stitch_mp_refill",
                    ..
                } => self.mp_refills += 1,
                Event::Service {
                    name: "genie_form",
                    words,
                    ..
                } => self.genie_form = Some(words[0] as u8),
                // FUN_1403b2850: the script offers a command on a record.
                Event::Service {
                    name: "command_override",
                    words,
                    other,
                    ..
                } => {
                    let r = words[0] as usize;
                    if r < self.offers.len() {
                        self.offers[r] = Some(words[1] as u16);
                        self.offer_targets[r] = other;
                    }
                }
                Event::Service {
                    name: "obj_add_hp",
                    object: Some(PLAYER_OBJECT),
                    words,
                    ..
                } => self.sora_hp.push(words[0] as i32),
                Event::Service {
                    name: "obj_start_mpdrive",
                    object: Some(PLAYER_OBJECT),
                    words,
                    ..
                } => self.sora_mpdrive.push(f32::from_bits(words[0])),
                Event::Service {
                    name: name @ ("camera_watch" | "func_screen_whiteout" | "func_screen_whitein"),
                    object,
                    words,
                    ..
                } => self.presentations.push((name, object, words)),
                Event::Service { name, .. } => {
                    self.services.insert(name);
                }
                Event::Sound { id, position, .. } => self.sounds.push((id, position)),
                // Sora plays what a running limit captures for him (the
                // summon host reads his stand-in's capture); another
                // victim's is not drawn.
                Event::MotionCapture { victim, .. }
                    if victim == PLAYER_OBJECT && self.limit.is_some() => {}
                // A boss captures Sora into a motion of its own set.
                Event::MotionCapture {
                    victim,
                    motion,
                    blend,
                    ..
                } if victim == PLAYER_OBJECT => self.sora_captures.push((motion, blend)),
                Event::MotionCapture { .. } => {
                    self.unsupported.insert("obj_motion_capture");
                }
                Event::AttackStrike {
                    slot,
                    target: Some(PLAYER_OBJECT),
                    ..
                } => self.strikes.push(slot),
                Event::AttackStrike {
                    slot,
                    target: Some(ENEMY),
                    param,
                    serial,
                    ..
                } => self.enemy_strikes.push((slot, param, serial)),
                Event::MagicStart { .. } => {
                    self.unsupported.insert("magic_start_thread");
                }
                Event::Signal { code, argument } => self.signals.push((code, argument)),
                _ => {}
            }
        }
    }

    /// What the script asked of the host since the last call: the sound
    /// effects to play, the voice lines, and the names of the requests that
    /// nothing here plays (each reported once).
    pub fn take_reports(&mut self) -> ScriptReports {
        let new = |set: &std::collections::BTreeSet<&'static str>,
                   reported: &mut std::collections::BTreeSet<&'static str>| {
            set.iter()
                .copied()
                .filter(|name| reported.insert(name))
                .collect::<Vec<_>>()
        };
        ScriptReports {
            sounds: std::mem::take(&mut self.sounds),
            voices: std::mem::take(&mut self.voices),
            services: new(&self.services, &mut self.reported),
            unsupported: new(&self.unsupported, &mut self.reported),
        }
    }

    fn record_faults(&mut self) {
        if self.fault.is_none() {
            if let Some((word, fault)) = self.object.script.faults.first() {
                self.fault = Some(format!("script fault at word {word}: {fault:?}"));
            } else if let Some((entry, (word, fault))) = self
                .world
                .objects
                .iter()
                .flatten()
                .find_map(|o| o.script.faults.first().map(|f| (o.sys.entry_id, f)))
            {
                self.fault = Some(format!(
                    "made object {entry:#x}: script fault at word {word}: {fault:?}"
                ));
            } else if let Some((table, index)) = self.world.missing.first() {
                self.fault = Some(format!("trap {table}:{index} has no translation"));
            }
        }
    }

    pub fn position(&self) -> Vec3 {
        self.object.sys.position
    }
    /// Where the host put an object it places itself (Stitch's camera
    /// space).
    pub fn set_position(&mut self, p: Vec3) {
        self.object.sys.position = p;
    }
    /// Chicken Little's limit deck (mode 5) while his limit mode is on.
    pub fn special_deck(&self) -> Option<[(u16, u16); 3]> {
        let f = self.object.friend.as_deref()?;
        f.limit.then_some(enemy::friend::CHICKEN_LITTLE_DECK)
    }

    /// Chicken Little's aim point (personality +0x40).
    pub fn aim(&self) -> Option<Vec3> {
        Some(self.object.friend.as_deref()?.aim)
    }

    /// Chicken Little's aim point (personality +0x40), as FUN_1401cb830
    /// sets it in his limit mode.
    pub fn set_aim(&mut self, aim: Vec3) {
        if let Some(f) = self.object.friend.as_deref_mut() {
            f.aim = aim;
        }
    }

    /// Stitch's screen position (x right, y down, depth, heading), once
    /// it is set.
    pub fn stitch_screen(&self) -> Option<[f32; 4]> {
        let s = self.object.friend.as_deref()?.stitch.as_deref()?;
        s.valid.then_some(s.screen)
    }
    /// The object's fade (+0xa08): its alpha and whether it is falling.
    pub fn fade(&self) -> (f32, bool) {
        let f = &self.object.sys.fade;
        (f.value, f.rate < 0.)
    }

    /// Record every trap the script calls (a development aid).
    pub fn trace(&mut self) {
        self.world.trace.get_or_insert_with(Vec::new);
    }

    /// The traps called since the last call, in order.
    pub fn take_trace(&mut self) -> Vec<(u16, u16)> {
        self.world
            .trace
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// The summon's friend state for the host log: band, speed, walk, run,
    /// target, flags.
    pub fn friend_state(&self) -> Option<(usize, f32, f32, f32, u32, u32)> {
        let f = self.object.friend.as_deref()?;
        Some((
            f.band,
            self.object.sys.move_speed,
            f.walk,
            f.run,
            self.object.sys.target.id,
            f.flags,
        ))
    }

    /// The traps the script called that have no translation, once each.
    pub fn missing_traps(&self) -> Vec<(u16, u16)> {
        let mut m = self.world.missing.clone();
        m.sort_unstable();
        m.dedup();
        m
    }
    pub fn yaw(&self) -> f32 {
        self.object.sys.yaw
    }
    /// FUN_1403d1730's last step for a hit on Sora: the attack's owner's
    /// OBJ_ATTACK (FUN_140410ce0) with Sora and the attack: a script attack
    /// object (`slot`) or a motion window's attack `param_id`.
    pub fn attack_reached_player(&mut self, slot: Option<usize>, param_id: i32) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        self.object
            .attack_reached(&mut self.world, &env, PLAYER_OBJECT, slot, param_id);
        self.drain_events();
        self.take_player_queue();
        self.record_faults();
    }

    /// The motions the script queued behind Sora's (his stand-in's queue);
    /// a running limit keeps its own.
    fn take_player_queue(&mut self) {
        if self.limit.is_some() || self.world.limit.is_some() {
            return;
        }
        if let Some(p) = self.world.player_proxy.as_mut() {
            self.sora_pushed.append(&mut p.sys.motion_queue);
        }
    }

    /// The lights the script made (FUN_1403f6f80's pool).
    pub fn lights(&self) -> &[Option<enemy::world::Light>] {
        &self.world.lights
    }

    /// The vacuums the script made (FUN_1403f8f20's list).
    pub fn vacuums(&self) -> &[Option<enemy::world::Vacuum>] {
        &self.world.vacuums
    }

    /// FUN_1403ba600 for the object's REACTION shape of `group`.
    pub fn reaction_offered(&self, command: i32, group: i32, default_groups: u32) -> bool {
        let s = &self.object.sys;
        s.reaction_groups
            .offered(s.script_group_bits(), default_groups, command, group)
    }
    /// The engine state a boss run logs: the revenge gauge and its limit,
    /// the active state pattern, the bound sheet and the class bits.
    pub fn boss_state(&self) -> serde_json::Value {
        let s = &self.object.sys;
        serde_json::json!({"yaw":s.yaw,"move":[s.move_direction[0], s.move_direction[2], s.move_speed],
            "velocity":s.velocity,"target":s.target.id,"mode":format!("{:#x}", s.mode_flags()),
            "revenge":s.revenge,"karma_limit":s.karma_limit,
            "patterns":format!("{:#x}", s.active_patterns()),"sheet":s.sheet,
            "max_hp":s.stats.max_hp,"class":format!("{:#x}", s.class_flags),
            "dead":s.state_flags & enemy::sysobj::state_flag::DEAD_MARKED != 0})
    }
    pub fn hp(&self) -> i32 {
        self.object.sys.hp
    }
    pub fn alive(&self) -> bool {
        !self.object.sys.gone && !self.object.acts.locked()
    }
    pub fn targetable(&self) -> bool {
        self.object.targetable()
    }
    pub fn record_mut(&mut self) -> &mut combat::HitRecord {
        &mut self.object.sys.record
    }
    /// The motion's control word (+0x18c).
    pub fn motion_flags(&self) -> u32 {
        self.object.sys.motion.flags.word
    }
    /// The physics mode word (`SysObj::mode_flags`).
    pub fn mode_flags(&self) -> u32 {
        self.object.sys.mode_flags()
    }
    pub fn motion(&self) -> usize {
        self.object.sys.motion.slot
    }
    pub fn elapsed(&self) -> u32 {
        self.object.sys.motion.tick
    }
    pub fn serial(&self) -> u64 {
        self.object.sys.motion.serial
    }
    pub fn scale_y(&self) -> f32 {
        self.object.sys.scale[1]
    }
    pub fn blend(&self) -> f32 {
        self.object.sys.motion.blend
    }
    pub fn gone(&self) -> bool {
        self.object.sys.gone
    }
    pub fn description(&self) -> String {
        self.object
            .acts
            .current
            .map(|i| String::from_utf8_lossy(&self.object.acts.table[i].name).to_string())
            .unwrap_or_default()
    }

    pub fn hit(
        &mut self,
        direction: Vec3,
        high: bool,
        attack: &combat::AttackParams,
        attacker: &combat::AttackerStats,
        difficulty: combat::Difficulty,
        record: super::enemy_battle::HitRecordInfo,
    ) -> Option<i32> {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        let damage = self.object.take_hit(
            &mut self.world,
            &env,
            &enemy::damage::Hit {
                direction,
                high,
                attack,
                attacker,
                difficulty,
                param_id: u32::from(record.param_id),
                revenge: record.revenge,
                pool: record.pool,
                by_leader: record.by_leader,
                by_party: record.by_party,
            },
        );
        self.drain_events();
        self.take_player_queue();
        self.record_faults();
        damage
    }

    pub fn reflect(&mut self, motion: usize, back: f32) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        self.object.reflect(&mut self.world, &env, motion, back);
        self.record_faults();
    }

    /// FUN_1403de610's summon: +0x6c8 |= 0x400000 (`sysobj_is_summon`),
    /// not of the enemy class, on Sora's team [I: the team follows the
    /// object type in the constructor FUN_1401b06e0, not traced].
    pub fn make_summon(&mut self, entry: u32, friend: Option<enemy::friend::Friend>) {
        use enemy::sysobj::class_flag;
        let s = &mut self.object.sys;
        // The objentry id `obj_search_by_entry` matches (+0x6c4's entry);
        // Chicken Little's fireworks find him by it.
        s.entry_id = entry;
        s.class_flags = s.class_flags & !class_flag::ENEMY | class_flag::SUMMON;
        s.team = SORA_TEAM as i32;
        self.object.friend = friend.map(Box::new);
    }

    /// FUN_1401b56e0 mode 1: Sora began attack `attack` at `target` (the
    /// enemy's stand-in, or nothing).
    pub fn note_player_attack(&mut self, attack: u32, target: Option<u32>) {
        enemy::friend::note_player_attack(
            &mut self.world,
            Some(&mut self.object),
            attack,
            target.unwrap_or(0),
        );
    }

    /// FUN_1401b5d60: Sora's attack now (his +0x180), once a frame.
    pub fn age_player_notify(&mut self, current: Option<u32>) {
        enemy::friend::age_player_notify(&mut self.world, current);
    }

    /// FUN_1401b56e0 mode 0: Sora was hit by `attacker`; it joins his
    /// attacker list (once, at most 8, the oldest dropped).
    pub fn note_player_attacker(&mut self, attacker: u32) {
        let list = &mut self.world.player_attackers;
        list.retain(|&o| o != attacker);
        if list.len() == 8 {
            list.remove(0);
        }
        list.push(attacker);
    }

    /// The host builds the objects `sysobj_appear` asks for.
    pub fn set_spawner(&mut self, spawner: Box<dyn enemy::world::Spawner>) {
        self.world.spawner = Some(spawner);
    }

    /// An attack of the script struck `target` (the enemy's stand-in): its
    /// hit callback runs (FUN_1403d1730).
    pub fn mark_attack_hit_on(&mut self, slot: usize, target: u32) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        self.object
            .attack_struck(&mut self.world, &env, slot, target);
        self.record_faults();
    }

    /// FUN_1403b5fa0 (from FUN_1403a8b10, Sora's decided command): trigger
    /// 11 of the object's script with Sora, the command and the target
    /// record [I: no record; 0].
    pub fn command(&mut self, command: u16) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        self.object.fire(
            &mut self.world,
            &env,
            11,
            &[
                enemy::script::Slot::address(enemy::world::PLAYER_HANDLE),
                enemy::script::Slot::int(i32::from(command)),
                enemy::script::Slot::address(0),
            ],
        );
        self.drain_events();
        self.take_player_queue();
        self.record_faults();
    }

    /// FUN_1401cf370 (Genie's first update): his form command from the
    /// form number (FUN_1403de220).
    pub fn set_genie_form(&mut self, command: u16) {
        if let Some(f) = self.object.friend.as_deref_mut() {
            f.genie_form = command;
        }
    }

    /// The owner object of an attack the script placed.
    pub fn attack_owner(&self, slot: usize) -> Option<u32> {
        self.world.attacks.get(slot)?.as_ref()?.owner
    }

    /// The room's enemy as a stand-in in this script's world (an enemy-class
    /// object at [`ENEMY`], team 2), kept at its place; `None` leaves it at
    /// 0 HP (the slot stays the stand-in's).
    pub fn set_enemy(&mut self, at: Option<Vec3>) {
        use enemy::sysobj::class_flag;
        let slots = &mut self.world.objects;
        if slots.len() <= ENEMY as usize {
            slots.resize_with(ENEMY as usize + 1, || None);
        }
        let o = slots[ENEMY as usize].get_or_insert_with(|| {
            let mut o = Object::bare(ENEMY);
            o.sys.class_flags |= class_flag::ENEMY;
            o.sys.team = 2;
            o.sys.model = u32::MAX;
            Box::new(o)
        });
        o.sys.hp = at.is_some() as i32;
        if let Some(p) = at {
            o.sys.position = p;
        }
    }

    /// Start the named act (FUN_1403cac50 / FUN_1403cae60): the running act
    /// and the queue end first.
    pub fn start_act(&mut self, name: &[u8]) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        let actions = self.object.acts.start(name);
        self.object
            .apply_actions(Some((&mut self.world, &env)), actions);
        self.record_faults();
    }

    /// Development aid: set pool 0's HP (the tours use it to reach the end
    /// of a long fight).
    pub fn set_hp(&mut self, hp: i32) {
        self.object.sys.set_pool_hp(0, hp);
    }

    /// Development command: force the death path.
    pub fn defeat(&mut self) {
        if !self.object.targetable() {
            return;
        }
        self.object.sys.hp = 0;
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
        };
        let actions = self.object.acts.start(b"dead");
        self.object
            .apply_actions(Some((&mut self.world, &env)), actions);
    }

    /// Development command: a fresh object at the spawn point.
    pub fn respawn(&mut self) {
        let (stats, sstm, yaw) = (
            self.object.sys.stats,
            self.object.sys.sstm,
            self.object.sys.yaw,
        );
        let spawn = self.spawn;
        let mut rng = ScriptRandom(self.world.rng.0);
        if let Err(e) = self.build(spawn, yaw, stats, sstm, &mut rng) {
            self.fault = Some(e);
        }
    }
}
