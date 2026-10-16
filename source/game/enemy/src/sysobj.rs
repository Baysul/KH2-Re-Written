//! The engine object ("sysobj") an enemy script drives, and the per-tick engine
//! work around the script: motion clock, root motion, jump/blow/knock physics
//! and movement through the collision world. The motion and physics rules are
//! those the earlier Shadow/Dusk/Soldier translations established from the
//! executable (FUN_1403c6740 motion advance, FUN_1403b89a0 physics sampling,
//! FUN_1403cbb00 jumps, FUN_1403cb360/cb500 arcs, FUN_1403d4570 knock-slide);
//! offsets in comments are the executable's.

use crate::env::{Clips, Env};
use player::{actor_physics, combat};

pub type Vec3 = [f32; 3];

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [0, 1, 2].map(|k| a[k] - b[k])
}
pub fn hlen(v: Vec3) -> f32 {
    v[0].hypot(v[2])
}

pub fn wrap(a: f32) -> f32 {
    crate::traps::math::wrap_angle(a)
}

/// +0x158: the motion player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Motion {
    /// +0x28/+0x180: the playing motion slot.
    pub slot: usize,
    /// Clock in ticks.
    pub tick: u32,
    /// Increments on every motion start (attack-instance identity).
    pub serial: u64,
    /// Remaining blend, in motion-clock ticks (0 = none).
    pub blend: f32,
    blend_elapsed: u32,
    blend_duration: u32,
}

/// Reaction states the engine holds between a hit and returning to the script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    None,
    Flinch,
    Blow { back: bool },
    BlowLand,
}

/// A request to leave the ground once a motion's take-off event fires
/// (`trap_obj_flyjump`, physics mode 11).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingFly {
    pub direction: Vec3,
    pub height: f32,
    pub speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fly {
    pub arc: actor_physics::Arc,
    pub velocity: [f32; 2],
    pub initial_height: f32,
    pub original_airborne: bool,
    pub fall_started: bool,
}

/// An alpha fade (sysobj+0xa08: value, rate per tick; FUN_1403b7810/77c0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fade {
    pub value: f32,
    pub rate: f32,
}

impl Fade {
    /// FUN_1403b7810: from `from` to `to` over `ticks` (<= 2 means one tick).
    pub fn start(&mut self, from: f32, to: f32, mut ticks: f32) {
        if ticks <= 2.0 {
            ticks = 1.0;
        }
        if ticks > 0.0 {
            self.value = from;
            self.rate = (to - from) / ticks;
        } else {
            self.value = to;
            self.rate = 0.0;
        }
    }

    /// FUN_1403b77c0: true while the fade runs.
    pub fn step(&mut self, frame_time: f32) -> bool {
        if self.rate == 0.0 {
            return false;
        }
        self.value += self.rate * frame_time;
        if self.value > 0.0 {
            if 1.0 <= self.value {
                self.value = 1.0;
                self.rate = 0.0;
            }
            return true;
        }
        self.value = 0.0;
        self.rate = 0.0;
        true
    }
}

/// The record at sysobj+0xbf8: the actor this one is aiming at.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Target {
    /// 0 = nobody.
    pub id: u32,
    /// Search kind it was made with (+0xc), -1 when unset.
    pub kind: i32,
}

#[derive(Clone, Debug)]
pub struct Sysobj {
    /// The model the host selects clips with.
    pub model: u32,
    /// +0x670: position.
    pub position: Vec3,
    pub spawn: Vec3,
    /// +0x68c: heading about Y.
    pub yaw: f32,
    /// +0x7a8: the heading `wish_dir` asks the engine to turn toward.
    pub wish_yaw: Option<f32>,
    /// +0x30: scale (y animated by scripts).
    pub scale: Vec3,
    /// Alpha as faded by +0xa08.
    pub alpha: f32,
    pub fade: Fade,
    pub hidden: bool,
    /// +0x9b8 bit 4: leaving; the object is removed once faded.
    pub left: bool,
    pub gone: bool,
    /// +0x120: script-settable flags (`obj_set_flag`).
    pub flags: u32,
    pub hp: i32,
    pub stats: combat::TargetStats,
    pub sstm: actor_physics::Sstm,
    pub motion: Motion,
    pub airborne: bool,
    pub reaction: Reaction,
    /// +0x10/+0x1c: the movement the script requested (direction, speed).
    pub move_direction: Vec3,
    pub move_speed: f32,
    /// +0xa28 / +0xa38: wished and actual displacement of the last tick.
    pub wished: Vec3,
    pub moved: Vec3,
    pub target: Target,
    /// +0x9f4: spawn arguments, and the spawn type byte at +0x1c.
    pub spawn_args: [i32; 8],
    pub appear_way: i32,
    /// Spawned flying (`obj_is_entry_fly`).
    pub entry_fly: bool,
    /// The attacker of the last hit (the damage record's source).
    pub last_attacker: Option<u32>,
    /// The reaction value of the damage record being handled (+0x26).
    pub damage_reaction: u32,
    /// +0xa58/+0xa64: the second movement request (`trap_obj_move_request`).
    pub request_direction: Vec3,
    pub request_speed: f32,
    /// The battle parameter sheet (ENMP) number the script bound.
    pub sheet: i32,
    /// +0x8c8 bit 6: sunk and slithering (Shadow).
    pub shadow_move: bool,
    /// +0x134: turn rate parameter (radians per tick; scripts pass 0 for it).
    pub turn_rate: f32,
    /// `obj_set_movement`'s `+0x118/+0x11c`: the fall and land motions of the
    /// current jump.
    pub jump_motions: Option<(usize, usize)>,
    pub(crate) pending_fly: Option<PendingFly>,
    pub(crate) fly: Option<Fly>,
    /// A plain jump (FUN_1403cbb00): the arc and its horizontal velocity.
    pub(crate) jump: Option<(actor_physics::Arc, [f32; 2])>,
    pub(crate) blow: Option<actor_physics::Blow>,
    pub(crate) knock: Option<actor_physics::Knock>,
    pub(crate) sampler: actor_physics::Sampler,
    pub record: combat::HitRecord,
    /// +0x18c bit 18: the act manager is paused (set by engine states).
    pub act_paused: bool,
}

impl Sysobj {
    /// A zeroed object for tools and tests.
    pub fn bare() -> Self {
        let stats = combat::TargetStats {
            max_hp: 100,
            defense: 0,
            cap: 100,
            min: 0,
            weakness: [100; 7],
        };
        let sstm = actor_physics::Sstm {
            fall_max: 20.,
            blow_brake_xz: 0.,
            blow_min_xz: 0.,
            blow_brake_up: 0.,
            blow_up: 0.,
            blow_speed: 0.,
            blow_to_hit_back: 0.,
            hit_back: 0.,
            hit_back_small: 0.,
            hit_back_to_jump: 0.,
        };
        Self::new(0, [0.; 3], 0., stats, sstm)
    }

    pub fn new(
        model: u32,
        position: Vec3,
        yaw: f32,
        stats: combat::TargetStats,
        sstm: actor_physics::Sstm,
    ) -> Self {
        Self {
            model,
            position,
            spawn: position,
            yaw,
            wish_yaw: None,
            scale: [1.; 3],
            alpha: 1.,
            fade: Fade::default(),
            hidden: false,
            left: false,
            gone: false,
            flags: 0,
            hp: stats.max_hp,
            stats,
            sstm,
            motion: Motion::default(),
            airborne: false,
            reaction: Reaction::None,
            move_direction: [0.; 3],
            move_speed: 0.,
            wished: [0.; 3],
            moved: [0.; 3],
            target: Target::default(),
            spawn_args: [0; 8],
            appear_way: -1,
            entry_fly: false,
            last_attacker: None,
            damage_reaction: 0,
            request_direction: [0.; 3],
            request_speed: 0.,
            sheet: 0,
            shadow_move: false,
            turn_rate: 0.,
            jump_motions: None,
            pending_fly: None,
            fly: None,
            jump: None,
            blow: None,
            knock: None,
            sampler: actor_physics::Sampler::default(),
            record: combat::HitRecord::new(false),
            act_paused: false,
        }
    }

    /// FUN_1403c86a0 / FUN_1403c88c0: begin a motion. A blend of at most two
    /// ticks takes the immediate path.
    pub fn motion_start(&mut self, slot: usize, blend: f32, clips: &dyn Clips) -> bool {
        if clips.length(slot) == 0 {
            return false;
        }
        let m = &mut self.motion;
        m.slot = slot;
        m.tick = 0;
        m.serial = m.serial.wrapping_add(1);
        m.blend = if blend <= 2. { 0. } else { blend };
        m.blend_duration = m.blend as u32;
        m.blend_elapsed = 0;
        true
    }

    /// FUN_1403c6dc0: like start, but nothing happens if `slot` already plays.
    pub fn motion_change(&mut self, slot: usize, blend: f32, clips: &dyn Clips) -> bool {
        if self.motion.slot == slot {
            return true;
        }
        self.motion_start(slot, blend, clips)
    }

    /// +0x18c bit 1 (the motion ended on this tick's step).
    pub fn motion_ended(&self, clips: &dyn Clips) -> bool {
        clips.ended(self.motion.slot, self.motion.tick)
    }

    /// Hit-stun suspension (FUN_1403ca7c0): a damage (10..=39) or reflect
    /// (140..=150) motion still plays.
    pub fn hit_stunned(&self) -> bool {
        let s = self.motion.slot;
        (10..=39).contains(&s) || (140..=150).contains(&s)
    }

    /// FUN_1403b9f70 (`obj_turn_dir`): turn toward the direction's heading by
    /// at most `2 * rate` radians. Returns true while the heading differs.
    pub fn turn_dir(&mut self, direction: Vec3, rate: f32) -> bool {
        let rate = if rate == 0. { self.turn_rate } else { rate };
        let step = rate * actor_physics::PHYSICS_STEP;
        let delta = wrap(direction[0].atan2(direction[2]) - self.yaw);
        if delta > step {
            self.yaw = wrap(step + self.yaw);
            true
        } else if -step <= delta {
            self.yaw = wrap(self.yaw);
            false
        } else {
            self.yaw = wrap(self.yaw - step);
            true
        }
    }

    pub fn forward(&self) -> Vec3 {
        [self.yaw.sin(), 0., self.yaw.cos()]
    }

    /// Applies a requested displacement through the collision world.
    fn apply_movement(&mut self, wished: Vec3, env: &dyn Env) {
        let from = self.position;
        self.position = env.movement(from, wished);
        self.wished = wished;
        self.moved = sub(self.position, from);
    }

    /// One tick of engine work after the script ran: heading, root motion,
    /// physics and the motion clock.
    pub fn tick(&mut self, env: &dyn Env, no_control: bool) {
        if self.gone {
            return;
        }
        let clips = env.clips(self.model);
        let old_serial = self.motion.serial;
        let reacting = self.reaction != Reaction::None && self.hp > 0;
        if reacting {
            self.update_reaction(clips, env);
        }
        if !reacting
            && !no_control
            && let Some(desired) = self.wish_yaw
        {
            let delta = wrap(desired - self.yaw);
            let step = self.turn_rate.max(0.) * 2.;
            self.yaw = wrap(self.yaw + delta.clamp(-step, step));
        }
        if self.pending_fly.is_some()
            && clips.takeoff(self.motion.slot, self.motion.tick) == Some(true)
        {
            let launch = self.pending_fly.take().expect("pending launch");
            let floor = env.floor(self.position[0], self.position[2], self.position[1]);
            self.fly = Some(Fly {
                arc: actor_physics::Arc::launch(self.position[1], launch.height),
                velocity: [
                    launch.direction[0] * launch.speed,
                    launch.direction[2] * launch.speed,
                ],
                initial_height: floor.map_or(0., |f| (self.position[1] - f).max(0.)),
                original_airborne: self.airborne,
                fall_started: false,
            });
            self.airborne = true;
            self.sampler.reset();
        }
        if self.fly.is_some() {
            self.update_fly(clips, env, old_serial);
        } else if let Some((arc, velocity)) = self.jump {
            self.update_jump(arc, velocity, env);
        } else if !reacting {
            let root = if old_serial == self.motion.serial {
                clips.root_delta(self.motion.slot, self.motion.tick)
            } else {
                [0.; 3]
            };
            let (sin, cos) = self.yaw.sin_cos();
            let wished = if self.move_speed != 0. {
                self.move_direction.map(|v| v * self.move_speed)
            } else {
                [
                    root[0] * cos + root[2] * sin,
                    root[1],
                    -root[0] * sin + root[2] * cos,
                ]
            };
            self.apply_movement(wished, env);
        }
        self.advance_motion(clips, old_serial);
    }

    fn advance_motion(&mut self, clips: &dyn Clips, old_serial: u64) {
        let m = &mut self.motion;
        if m.serial != old_serial {
            return;
        }
        if m.blend > 0. {
            m.blend_elapsed = if m.blend_elapsed == 0 {
                2
            } else {
                m.blend_elapsed.saturating_add(1)
            };
            if m.blend_elapsed >= m.blend_duration {
                m.tick = m.blend_elapsed - m.blend_duration;
                m.blend = 0.;
                m.blend_elapsed = 0;
            } else {
                m.blend = (m.blend_duration - m.blend_elapsed) as f32;
            }
        } else {
            m.tick = clips.advance(m.slot, m.tick);
        }
    }

    fn update_jump(&mut self, mut arc: actor_physics::Arc, mut velocity: [f32; 2], env: &dyn Env) {
        if self.sampler.due(0.) {
            let dt = actor_physics::PHYSICS_STEP;
            // Without a move request the take-off velocity decays (FUN_1403b7ad0).
            velocity = actor_physics::steer(
                velocity,
                [0.; 2],
                0.,
                actor_physics::AIR_INERTIA,
                actor_physics::AIR_DRAG,
            );
            let dy = arc.step(self.position[1], self.sstm.fall_max, false, dt);
            self.sampler.store([velocity[0] * dt, dy, velocity[1] * dt]);
        }
        self.jump = Some((arc, velocity));
        let d = self.sampler.displacement();
        self.apply_movement(d, env);
        if let Some(floor) = env.floor(self.position[0], self.position[2], self.position[1] + 30.)
            && self.position[1] <= floor
            && d[1] <= 0.
        {
            self.position[1] = floor;
            self.jump = None;
            self.airborne = false;
            self.sampler = actor_physics::Sampler::default();
        }
    }

    fn update_fly(&mut self, clips: &dyn Clips, env: &dyn Env, old_serial: u64) {
        let mut fly = self.fly.expect("active fly jump");
        let slot = self.motion.slot;
        if !fly.fall_started && fly.arc.falling_motion() && self.reaction == Reaction::None {
            if !self.motion_start(155 + 1, actor_physics::MOTION_BLEND, clips) {
                return;
            }
            fly.fall_started = true;
        }
        let height = env
            .floor(self.position[0], self.position[2], self.position[1])
            .map_or(0., |f| (self.position[1] - f).max(0.));
        if fly.arc.t >= fly.arc.rise && height <= fly.initial_height {
            self.airborne = fly.original_airborne;
            self.fly = None;
            self.sampler.reset();
            self.motion_start(155 + 2, 0., clips);
            return;
        }
        let frozen = clips
            .gravity_frozen(slot, self.motion.tick)
            .unwrap_or(false);
        if self.sampler.due(self.move_speed) {
            let dt = actor_physics::PHYSICS_STEP;
            let dy = fly
                .arc
                .step(self.position[1], self.sstm.fall_max, frozen, dt);
            fly.velocity = actor_physics::steer(
                fly.velocity,
                [self.move_direction[0], self.move_direction[2]],
                self.move_speed,
                actor_physics::AIR_INERTIA,
                actor_physics::AIR_DRAG,
            );
            self.sampler
                .store([fly.velocity[0] * dt, dy, fly.velocity[1] * dt]);
        }
        let root = if old_serial == self.motion.serial {
            clips.root_delta(self.motion.slot, self.motion.tick)
        } else {
            [0.; 3]
        };
        let (sin, cos) = self.yaw.sin_cos();
        let root = [
            root[0] * cos + root[2] * sin,
            root[1],
            -root[0] * sin + root[2] * cos,
        ];
        let base = self.sampler.displacement();
        let wished = [0, 1, 2].map(|axis| base[axis] + root[axis]);
        self.apply_movement(wished, env);
        fly.arc.base -= self.moved[1] - wished[1];
        self.fly = Some(fly);
    }

    fn update_reaction(&mut self, clips: &dyn Clips, env: &dyn Env) {
        if let Reaction::Blow { back } = self.reaction {
            if let Some(blow) = &mut self.blow {
                if self.sampler.due(0.) {
                    self.sampler.store(blow.step(
                        &self.sstm,
                        self.sstm.fall_max,
                        actor_physics::PHYSICS_STEP,
                    ));
                }
                let d = self.sampler.displacement();
                let from = self.position;
                self.position = env.movement(from, d);
                if d[1] <= 0.
                    && env
                        .floor(self.position[0], self.position[2], from[1])
                        .is_some_and(|y| self.position[1] <= y)
                {
                    self.blow = None;
                    self.airborne = false;
                    self.reaction = Reaction::BlowLand;
                    self.motion_start(if back { 19 } else { 17 }, 0., clips);
                }
            }
        } else {
            if let Some(knock) = &mut self.knock {
                if let Some(d) = knock.step() {
                    self.position = env.movement(self.position, d);
                } else {
                    self.knock = None;
                }
            }
            if clips.ended(self.motion.slot, self.motion.tick) {
                self.reaction = Reaction::None;
                self.motion_start(0, 0., clips);
            }
        }
    }

    /// FUN_1403cbb00 (`method_obj_jump`): leave the ground now. `height` is
    /// the Y-up apex height, `speed` the horizontal speed along `direction`.
    pub fn launch_jump(&mut self, direction: Vec3, height: f32, speed: f32) {
        let len = hlen(direction);
        let dir = if len > 0. {
            [direction[0] / len, direction[2] / len]
        } else {
            [0.; 2]
        };
        self.jump = Some((
            actor_physics::Arc::launch(self.position[1], height),
            [dir[0] * speed, dir[1] * speed],
        ));
        self.sampler.reset();
        self.airborne = true;
    }

    /// FUN_1403ba9a0: true while the object is airborne or in a special state;
    /// an act first run waits for it to clear.
    pub fn air_blocked(&self) -> bool {
        self.is_jump() || self.flags >> 23 & 1 != 0
    }

    /// +0x18c bit 9: the playing clip holds the actor out of control.
    pub fn no_control(&self, env: &dyn Env) -> bool {
        env.clips(self.model)
            .no_control(self.motion.slot, self.motion.tick)
            .unwrap_or(false)
    }

    /// Physics mode entry for a fall (FUN_1403b8150): the parabola continues
    /// from the given Y-up vertical speed with no horizontal velocity.
    pub fn start_fall(&mut self, vertical_up: f32) {
        self.jump = Some((
            actor_physics::Arc::fall(self.position[1], vertical_up),
            [0.; 2],
        ));
        self.sampler.reset();
        self.airborne = true;
    }

    /// Read a word of the engine object that scripts access directly through
    /// its handle (`.d` addressing). Only the offsets found in installed
    /// scripts are mapped: +0x30..+0x38 is the scale.
    pub fn peek_word(&self, offset: u32) -> Option<u32> {
        match offset {
            0x30 => Some(self.scale[0].to_bits()),
            0x34 => Some(self.scale[1].to_bits()),
            0x38 => Some(self.scale[2].to_bits()),
            _ => None,
        }
    }

    /// Write a word of the engine object (see [`Sysobj::peek_word`]).
    pub fn poke_word(&mut self, offset: u32, value: u32) -> bool {
        match offset {
            0x30 => self.scale[0] = f32::from_bits(value),
            0x34 => self.scale[1] = f32::from_bits(value),
            0x38 => self.scale[2] = f32::from_bits(value),
            _ => return false,
        }
        true
    }

    /// `trap_obj_is_jump`: the physics state byte at +0xc bit 0.
    pub fn is_jump(&self) -> bool {
        self.jump.is_some() || self.fly.is_some()
    }
}
