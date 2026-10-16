//! Script-driven enemy brain: the enemy's own object script, loaded from the
//! installed game and executed by the translated interpreter in the `enemy`
//! crate, replaces the hand-written decision ports. The motion data, collision
//! world and Sora's position come from the surrounding host.
use super::{RoomWorld, enemy_battle::Clips};
use data::{bar, path_lookup::Library, probe_source::ProbeSource};
use enemy::{
    env::{Clips as EnemyClips, Env},
    sysobj::Sysobj,
    world::{Object, World as EnemyWorld},
};
use player::{World, actor_physics, combat, shadow::ScriptRandom};
use std::sync::Arc;

type Vec3 = [f32; 3];

/// The host view the engine reads each tick.
struct BrainEnv<'a> {
    room: Option<&'a RoomWorld>,
    clips: &'a Clips,
    player: Vec3,
    player_yaw: f32,
}

impl Env for BrainEnv<'_> {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        self.room?.floor(x, z, top)
    }
    fn clips(&self, _model: u32) -> &dyn EnemyClips {
        self.clips
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
    // The generated development arena is flat and has no step obstacles.
    fn check_step(&self, _: Vec3, _: Vec3, _: f32, _: f32, _: f32) -> Option<u8> {
        Some(0)
    }
    // The arena keeps enemies in the room.
    fn no_leave(&self) -> bool {
        true
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
}

pub struct ScriptBrain {
    world: EnemyWorld,
    object: Object,
    clips: Arc<Clips>,
    payload: Vec<u8>,
    spawn: Vec3,
    player: Vec3,
    /// Sora's heading, set by the host before each tick.
    pub player_yaw: f32,
    /// Faults the interpreter or a missing trap reported, for the host log.
    pub fault: Option<String>,
}

fn script_payload(source: &ProbeSource, model: &str) -> Result<Vec<u8>, String> {
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
    pub fn load(
        source: &ProbeSource,
        model: &str,
        position: Vec3,
        yaw: f32,
        stats: combat::TargetStats,
        sstm: actor_physics::Sstm,
        clips: Arc<Clips>,
    ) -> Result<Self, String> {
        let payload = script_payload(source, model)?;
        let mut brain = Self {
            world: EnemyWorld::default(),
            object: Object::bare(0),
            clips,
            payload,
            spawn: position,
            player: position,
            player_yaw: 0.,
            fault: None,
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
        self.world = EnemyWorld::default();
        self.world.tolerate_missing = true;
        self.world.rng.0 = rng.0;
        let env = BrainEnv {
            room: None,
            clips: &self.clips,
            player: self.player,
            player_yaw: self.player_yaw,
        };
        self.object = Object::spawn(&mut self.world, &env, 0, &self.payload, sys)
            .map_err(|e| format!("object script rejected: {e:?}"))?;
        rng.0 = self.world.rng.0;
        Ok(())
    }

    pub fn tick(&mut self, room: &RoomWorld, player: Vec3, rng: &mut ScriptRandom) {
        self.player = player;
        self.world.rng.0 = rng.0;
        let env = BrainEnv {
            room: Some(room),
            clips: &self.clips,
            player,
            player_yaw: self.player_yaw,
        };
        self.object.tick(&mut self.world, &env);
        self.world.tick_attacks();
        if self.object.sys.gone {
            self.world.attacks.clear();
        }
        rng.0 = self.world.rng.0;
        self.record_faults();
    }

    /// The attack objects the script has placed, for the host's hit tests.
    pub fn attacks(&self) -> Vec<ScriptAttack> {
        use enemy::attack::flag;
        self.world
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
                })
            })
            .collect()
    }

    /// An attack of the script struck something.
    pub fn mark_attack_hit(&mut self, slot: usize) {
        if let Some(Some(a)) = self.world.attacks.get_mut(slot) {
            a.mark_hit(enemy::attack::flag::STRUCK);
        }
    }

    fn record_faults(&mut self) {
        if self.fault.is_none() {
            if let Some((word, fault)) = self.object.script.faults.first() {
                self.fault = Some(format!("script fault at word {word}: {fault:?}"));
            } else if let Some((table, index)) = self.world.missing.first() {
                self.fault = Some(format!("trap {table}:{index} has no translation"));
            }
        }
    }

    pub fn position(&self) -> Vec3 {
        self.object.sys.position
    }
    pub fn yaw(&self) -> f32 {
        self.object.sys.yaw
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
    ) -> Option<i32> {
        let env = BrainEnv {
            room: None,
            clips: &self.clips,
            player: self.player,
            player_yaw: self.player_yaw,
        };
        let damage = self.object.take_hit(
            &mut self.world,
            &env,
            direction,
            high,
            attack,
            attacker,
            difficulty,
        );
        self.record_faults();
        damage
    }

    pub fn reflect(&mut self, motion: usize, back: f32) {
        let env = BrainEnv {
            room: None,
            clips: &self.clips,
            player: self.player,
            player_yaw: self.player_yaw,
        };
        self.object.reflect(&mut self.world, &env, motion, back);
        self.record_faults();
    }

    /// Development command: force the death path.
    pub fn defeat(&mut self) {
        if !self.object.targetable() {
            return;
        }
        self.object.sys.hp = 0;
        let env = BrainEnv {
            room: None,
            clips: &self.clips,
            player: self.player,
            player_yaw: self.player_yaw,
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
