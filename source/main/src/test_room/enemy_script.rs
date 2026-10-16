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

pub struct ScriptBrain {
    world: EnemyWorld,
    object: Object,
    clips: Arc<Clips>,
    payload: Vec<u8>,
    spawn: Vec3,
    player: Vec3,
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
        };
        self.object.tick(&mut self.world, &env);
        rng.0 = self.world.rng.0;
        self.record_faults();
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
