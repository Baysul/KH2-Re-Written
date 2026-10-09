//! The Shadow Heartless (M_EX020): its AI program `m_ex020` translated from
//! the decoded bdscript (acts appear_shadow, mode_battle, btl_attack,
//! btl_short, btl_long, mode_revenge, rvg_short, rvg_long, battle_idle,
//! atk_craw_l/r/f, move_target, move_shadow_round, leave, dead) on top of
//! the engine's act manager (FUN_1403cae60 act_start, FUN_1403cab00
//! act_push, FUN_1403ca550 per-tick update, FUN_1403ca7c0 hit-stun) and the
//! engine reactions (flinch/blow, FUN_1403d2fe0 and FUN_1403d43b0).
//!
//! Units: positions in the simulation's Y-up space; one tick is one 60 Hz
//! frame (`trap_frametime` = 1.0). Test-room choices are named constants.

use crate::combat::{self, AttackParams, AttackerStats, TargetStats};

pub type Vec3 = [f32; 3];

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn hlen(v: Vec3) -> f32 {
    v[0].hypot(v[2])
}
fn wrap(a: f32) -> f32 {
    crate::camera::wrap(a)
}

/// The scripts' shared random generator (LCG x69069, initial 1;
/// `random_getf` = state * 2^-32 * x).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptRandom(pub u32);
impl Default for ScriptRandom {
    fn default() -> Self {
        ScriptRandom(1)
    }
}
impl ScriptRandom {
    pub fn getf(&mut self, x: f32) -> f32 {
        self.0 = self.0.wrapping_mul(69069);
        (self.0 as f64 / 4_294_967_296.0) as f32 * x
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        self.0 = self.0.wrapping_mul(69069);
        a + (b - a) * (self.0 as f64 / 4_294_967_296.0) as f32
    }
    pub fn get(&mut self, n: u32) -> u32 {
        self.0 = self.0.wrapping_mul(69069);
        let step = 4_294_967_296u64.div_ceil(n as u64);
        (self.0 as u64 / step) as u32
    }
}

/// Motion slots (direct slot = motion id in M_EX020.mset).
pub mod motion {
    pub const IDLE: usize = 0;
    pub const WALK: usize = 2;
    pub const JUMP: usize = 3;
    pub const FALL: usize = 4;
    pub const LAND: usize = 5;
    pub const FLINCH: [usize; 4] = [10, 11, 12, 13];
    pub const BLOW: [usize; 2] = [16, 18];
    pub const BLOW_LAND: [usize; 2] = [17, 19];
    pub const LEAVE: usize = 45;
    pub const FOOTWORK: usize = 47;
    pub const SINK: usize = 161;
    pub const RISE: usize = 162;
    pub const CLAW_L: usize = 201;
    pub const CLAW_R: usize = 202;
    pub const CLAW_F: usize = 203;
    /// ATKP ReflectMotion of the claws: 140 (ATKP 9/10), 141 (ATKP 11).
    pub const REFLECT: [usize; 2] = [140, 141];
    pub const ALL: [usize; 22] = [
        IDLE, WALK, JUMP, FALL, LAND, 10, 11, 12, 13, 16, 17, 18, 19, LEAVE, FOOTWORK, SINK, RISE,
        CLAW_L, CLAW_R, CLAW_F, 140, 141,
    ];
}

/// Clip data the AI needs: length and loop start in ticks, and the root
/// translation (model space, Y up, Z forward) at a tick.
pub trait Clips {
    fn length(&self, slot: usize) -> u32;
    fn loop_start(&self, slot: usize) -> u32;
    fn root(&self, slot: usize, tick: u32) -> Vec3;
}

/// Engine queries the AI uses.
pub trait Env {
    /// Floor height under a point (highest walkable surface below `top`).
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32>;
}

/// Test-room choices for values the trace did not resolve.
/// Default turn rate (sysobj+0x134, untraced); `turn_dir` turns by up to
/// 2.0 x rate per call.
pub const TURN_RATE: f32 = 0.13;
/// Jump gravity constant read from the jump setup (FUN_1403cbb00).
pub const GRAVITY: f32 = 0.408_163;
/// Flinch slide brake and blow brake (SSTM BlowBrakeXZ 0.8, BlowMinXZ 0.1).
pub const SLIDE_BRAKE: f32 = 0.8;
pub const BLOW_BRAKE_XZ: f32 = 0.8;
pub const BLOW_MIN_XZ: f32 = 0.1;
/// Respawn delay after leaving/dying (test room only).
pub const RESPAWN_TICKS: u32 = 120;
/// Knock-slide decay per tick (DAT_1405aaa40) and the ReflectHitBack used
/// when the ATKP field is −1 (DAT_1405b532c).
pub const KNOCK_DECAY: f32 = 0.9;
pub const DEFAULT_HIT_BACK: f32 = 50.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActKind {
    AppearShadow,
    ModeBattle,
    ModeRevenge,
    BtlAttack,
    BtlShort,
    BtlLong,
    RvgShort,
    RvgLong,
    BattleIdle,
    ClawL,
    ClawR,
    ClawF,
    MoveTarget,
    MoveShadowRound,
    Leave,
    Dead,
}

impl ActKind {
    fn priority(self) -> u16 {
        match self {
            ActKind::ClawL | ActKind::ClawR | ActKind::ClawF => 200,
            ActKind::Leave | ActKind::Dead => 300,
            _ => 100,
        }
    }
    fn locked(self) -> bool {
        matches!(self, ActKind::Leave | ActKind::Dead)
    }
}

/// Per-act running state.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Start,
    /// A counted wait or ramp.
    Timer {
        t: f32,
        total: f32,
    },
    /// A slither leg toward `dest` (move_shadow_round legs 1 and 2).
    Leg {
        leg: u8,
        t: f32,
        dest: Vec3,
    },
    Rise {
        t: f32,
        total: f32,
    },
    /// Waiting for a played motion to end.
    Motion,
    Air {
        stage: u8,
    },
    Fade {
        t: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Act {
    kind: ActKind,
    step: Step,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reaction {
    None,
    Flinch,
    Blow { back: bool },
    BlowLand,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    pub position: Vec3,
    pub spawn: Vec3,
    pub yaw: f32,
    pub hp: i32,
    pub stats: TargetStats,
    pub motion: usize,
    pub motion_tick: u32,
    /// Object scale Y (sysobj+0x34): 0.1 while sunk.
    pub scale_y: f32,
    pub alpha: f32,
    pub gone: bool,
    pub reaction: Reaction,
    /// Shadow-move flag (sysobj+0x8c8 0x40): sunk and slithering.
    pub sunk: bool,
    /// Increments on every motion start (attack-instance identity).
    pub motion_serial: u32,
    velocity: Vec3,
    slide: Vec3,
    /// Knock-slide (FUN_1403d4960): unit direction and speed.
    knock: Option<(Vec3, f32)>,
    airborne: bool,
    /// Weave phase in degrees (W4).
    weave: f32,
    act: Option<Act>,
    queue: Vec<ActKind>,
    hit_by: Vec<u32>,
    gone_ticks: u32,
    /// Last tick's moved distance, for the slither's stuck test.
    moved: f32,
    wished: f32,
}

impl Shadow {
    pub fn new(position: Vec3, yaw: f32, stats: TargetStats) -> Self {
        let mut s = Shadow {
            position,
            spawn: position,
            yaw,
            hp: stats.max_hp,
            stats,
            motion: motion::IDLE,
            motion_tick: 0,
            scale_y: 1.,
            alpha: 0.,
            gone: false,
            reaction: Reaction::None,
            sunk: false,
            motion_serial: 0,
            velocity: [0.; 3],
            slide: [0.; 3],
            knock: None,
            airborne: false,
            weave: 0.,
            act: None,
            queue: Vec::new(),
            hit_by: Vec::new(),
            gone_ticks: 0,
            moved: 0.,
            wished: 0.,
        };
        // OBJ_MAIN: act_start("appear_shadow"); act_push("mode_battle").
        s.act_start(ActKind::AppearShadow);
        s.act_push(ActKind::ModeBattle);
        s
    }

    pub fn alive(&self) -> bool {
        !self.gone
            && !matches!(
                self.act.map(|a| a.kind),
                Some(ActKind::Leave | ActKind::Dead)
            )
    }

    /// Hits are ignored while sunk (shadow-move) or leaving [inferred].
    pub fn targetable(&self) -> bool {
        self.alive() && !self.sunk
    }

    pub fn current_act(&self) -> Option<ActKind> {
        self.act.map(|a| a.kind)
    }

    fn act_start(&mut self, kind: ActKind) {
        if self.act.is_some_and(|a| a.kind.locked()) {
            return;
        }
        self.queue.clear();
        self.act = Some(Act {
            kind,
            step: Step::Start,
        });
    }

    fn act_push(&mut self, kind: ActKind) {
        if self.act.is_some_and(|a| a.kind.locked()) || self.queue.len() >= 16 {
            return;
        }
        self.queue.push(kind);
    }

    fn act_clear(&mut self) {
        if self.act.is_some_and(|a| a.kind.locked()) {
            return;
        }
        self.act = None;
        self.queue.clear();
    }

    fn motion_change(&mut self, slot: usize) {
        if self.motion != slot {
            self.motion_start(slot);
        }
    }

    fn motion_start(&mut self, slot: usize) {
        self.motion = slot;
        self.motion_tick = 0;
        self.motion_serial = self.motion_serial.wrapping_add(1);
    }

    fn turn_toward(&mut self, v: Vec3, rate: f32) {
        if hlen(v) < 1e-4 {
            return;
        }
        let target = v[0].atan2(v[2]);
        let d = wrap(target - self.yaw);
        let step = 2.0 * rate;
        self.yaw = wrap(self.yaw + d.clamp(-step, step));
    }

    fn forward(&self) -> Vec3 {
        [self.yaw.sin(), 0., self.yaw.cos()]
    }

    /// Engine hit handling for one attack instance; returns the damage, or
    /// None if this instance already hit or the Shadow cannot be hit.
    pub fn hit(
        &mut self,
        instance: u32,
        direction: Vec3,
        high: bool,
        attack: &AttackParams,
        attacker: &AttackerStats,
    ) -> Option<i32> {
        if !self.targetable() || self.hit_by.contains(&instance) {
            return None;
        }
        self.hit_by.push(instance);
        let dmg = combat::damage(attacker, attack, &self.stats);
        self.hp = (self.hp - dmg).max(0);
        let back = self.forward()[0] * direction[0] + self.forward()[2] * direction[2] >= 0.;
        let class = combat::reaction_class(attack.reaction);
        if class == 4 {
            let [up, speed] = attack.knockback.map(|v| v as f32);
            self.velocity = [direction[0] * speed, up, direction[2] * speed];
            self.airborne = true;
            self.reaction = Reaction::Blow { back };
            self.motion_start(motion::BLOW[back as usize]);
        } else {
            let speed = attack.knockback[0] as f32 * 0.1;
            self.slide = direction.map(|c| c * speed);
            self.reaction = Reaction::Flinch;
            self.motion_start(motion::FLINCH[(back as usize) * 2 + (!high) as usize]);
        }
        if self.hp <= 0 {
            // Engine: act_start("dead") at HP 0.
            self.act_start(ActKind::Dead);
        } else if class > 1 {
            // OBJ_DAMAGE: a reacting hit by the player starts mode_revenge.
            self.act_start(ActKind::ModeRevenge);
        }
        Some(dmg)
    }

    /// The claw was guarded (FUN_140411060 → FUN_1403d4650, ground): play
    /// the ATKP's ReflectMotion, knock-slide back from the facing at
    /// hit_back × 0.1, clear the acts and start ReflectAction
    /// (`mode_revenge`). The air variant is not decoded.
    pub fn reflect(&mut self, reflect_motion: usize, hit_back: f32) {
        if !self.alive() {
            return;
        }
        self.motion_start(reflect_motion);
        self.reaction = Reaction::Flinch;
        let hb = if hit_back < 0. {
            DEFAULT_HIT_BACK
        } else {
            hit_back
        };
        if hb > 0. {
            let f = self.forward();
            self.knock = Some(([-f[0], 0., -f[2]], hb * 0.1));
        }
        self.act_clear();
        self.act_start(ActKind::ModeRevenge);
    }

    /// One tick: engine reactions and physics, then the act manager.
    pub fn tick(
        &mut self,
        rng: &mut ScriptRandom,
        clips: &dyn Clips,
        env: &dyn Env,
        target: Vec3,
        player: Vec3,
    ) {
        if self.gone {
            self.gone_ticks += 1;
            if self.gone_ticks >= RESPAWN_TICKS {
                let stats = self.stats;
                let (spawn, yaw) = (self.spawn, self.yaw);
                *self = Shadow::new(spawn, yaw, stats);
            }
            return;
        }
        let before = self.position;
        self.motion_tick += 1;
        let length = clips.length(self.motion).max(1);
        let finished = self.motion_tick >= length;
        // Root motion of the playing clip, along the facing.
        let looping = matches!(
            self.motion,
            motion::IDLE | motion::WALK | motion::FOOTWORK | motion::SINK
        );
        if finished && looping {
            self.motion_tick = clips.loop_start(self.motion).min(length - 1);
        }
        let t = self.motion_tick.min(length - 1);
        if t > 0 {
            let d = sub(clips.root(self.motion, t), clips.root(self.motion, t - 1));
            let (s, c) = self.yaw.sin_cos();
            self.position[0] += d[0] * c + d[2] * s;
            self.position[2] += -d[0] * s + d[2] * c;
        }
        // Engine reactions.
        match self.reaction {
            Reaction::Flinch if finished => self.reaction = Reaction::None,
            Reaction::BlowLand if finished => self.reaction = Reaction::None,
            _ => {}
        }
        if let Some((dir, v)) = self.knock {
            if v < 1. {
                self.knock = None;
            } else {
                self.position = add(self.position, dir.map(|c| c * v));
                self.knock = Some((dir, v * KNOCK_DECAY));
            }
        }
        if self.slide.iter().any(|v| v.abs() > 1e-3) {
            self.position = add(self.position, self.slide);
            self.slide = self.slide.map(|v| v * SLIDE_BRAKE);
        }
        if self.airborne {
            self.position = add(self.position, self.velocity);
            if matches!(self.reaction, Reaction::Blow { .. }) {
                self.velocity[0] *= BLOW_BRAKE_XZ;
                self.velocity[2] *= BLOW_BRAKE_XZ;
                if self.velocity[0].hypot(self.velocity[2]) < BLOW_MIN_XZ {
                    self.velocity[0] = 0.;
                    self.velocity[2] = 0.;
                }
            }
            self.velocity[1] -= GRAVITY;
            if let Some(floor) =
                env.floor(self.position[0], self.position[2], self.position[1] + 30.)
                && self.position[1] <= floor
                && self.velocity[1] <= 0.
            {
                self.position[1] = floor;
                self.velocity = [0.; 3];
                self.airborne = false;
                if let Reaction::Blow { back } = self.reaction {
                    self.reaction = Reaction::BlowLand;
                    self.motion_start(motion::BLOW_LAND[back as usize]);
                }
            }
        } else if let Some(floor) =
            env.floor(self.position[0], self.position[2], self.position[1] + 30.)
        {
            self.position[1] = floor;
        }
        // OBJ_MAIN: keep an act running.
        if self.act.is_none() && self.queue.is_empty() {
            self.act_start(ActKind::ModeBattle);
        }
        // Hit-stun: acts below priority 300 wait while a damage motion plays.
        // `plays` = the damage or reflect motion has not ended yet.
        let stunned = ((10..=39).contains(&self.motion) || (140..=150).contains(&self.motion))
            && self.motion_tick < length;
        let mut budget = 8;
        loop {
            if self.act.is_none() {
                if self.queue.is_empty() {
                    break;
                }
                let next = self.queue.remove(0);
                self.act = Some(Act {
                    kind: next,
                    step: Step::Start,
                });
            }
            let act = self.act.unwrap();
            if stunned && act.kind.priority() < 300 {
                break;
            }
            let before_act = self.act;
            let ended = self.run(act, rng, clips, target, player);
            budget -= 1;
            if budget == 0 {
                break;
            }
            if ended {
                if self.act == before_act {
                    self.act = None;
                }
                continue;
            }
            if self.act != before_act && self.act.is_some_and(|a| a.step == Step::Start) {
                // A newly started act also runs in the same tick.
                continue;
            }
            break;
        }
        let moved = sub(self.position, before);
        self.moved = hlen(moved);
    }

    fn set(&mut self, kind: ActKind, step: Step) {
        if self.act.is_some_and(|a| a.kind == kind) {
            self.act = Some(Act { kind, step });
        }
    }

    /// Runs one tick of an act. Returns true when the act ended.
    fn run(
        &mut self,
        act: Act,
        rng: &mut ScriptRandom,
        clips: &dyn Clips,
        target: Vec3,
        player: Vec3,
    ) -> bool {
        let to_target = {
            let mut v = sub(target, self.position);
            v[1] = 0.;
            v
        };
        let d = hlen(to_target);
        let leave_check = |s: &Shadow| {
            let v = sub(player, s.position);
            (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() > 2000.
        };
        match act.kind {
            ActKind::ModeBattle => {
                if leave_check(self) {
                    self.act_start(ActKind::Leave);
                } else if d < 300. {
                    self.act_start(ActKind::BtlAttack);
                } else if d < 600. {
                    self.act_start(ActKind::BtlShort);
                } else {
                    self.act_start(ActKind::BtlLong);
                }
                false
            }
            ActKind::BtlAttack => {
                // battle_idle started then cleared: a net no-op.
                self.act_clear();
                if rng.getf(1.) < 0.5 {
                    if d < 150. {
                        self.act_start(ActKind::ClawL);
                        if rng.getf(1.) < 0.2 {
                            self.act_push(ActKind::MoveShadowRound);
                        }
                    } else if d < 350. {
                        let r = rng.getf(1.);
                        if r < 0.3 {
                            self.act_start(ActKind::MoveShadowRound);
                        } else if r < 0.9 {
                            self.act_start(ActKind::ClawR);
                        } else {
                            self.act_start(ActKind::ClawF);
                        }
                    } else if rng.getf(1.) < 0.2 {
                        self.act_start(ActKind::ClawF);
                    } else {
                        self.act_start(ActKind::ClawR);
                    }
                } else {
                    self.act_start(ActKind::MoveShadowRound);
                }
                self.act_push(ActKind::BattleIdle);
                false
            }
            ActKind::BtlShort => {
                if rng.getf(1.) < 0.5 {
                    self.act_start(ActKind::BattleIdle);
                    self.act_push(ActKind::MoveTarget);
                } else {
                    self.act_start(ActKind::MoveTarget);
                    self.act_push(ActKind::BattleIdle);
                    self.act_push(ActKind::MoveShadowRound);
                    self.act_push(ActKind::BattleIdle);
                }
                false
            }
            ActKind::BtlLong => {
                self.act_start(ActKind::MoveShadowRound);
                if rng.getf(1.) < 0.2 {
                    if leave_check(self) {
                        self.act_start(ActKind::Leave);
                    } else {
                        self.act_push(ActKind::MoveTarget);
                    }
                }
                false
            }
            ActKind::ModeRevenge => {
                if d < 600. {
                    self.act_start(ActKind::RvgShort);
                } else {
                    self.act_start(ActKind::RvgLong);
                }
                false
            }
            ActKind::RvgShort => {
                if d < 400. {
                    if rng.getf(1.) < 0.3 {
                        self.act_start(ActKind::ClawF);
                    } else {
                        self.act_start(ActKind::ClawR);
                    }
                } else {
                    self.act_start(ActKind::MoveShadowRound);
                }
                false
            }
            ActKind::RvgLong => {
                if leave_check(self) {
                    self.act_start(ActKind::Leave);
                } else {
                    self.act_start(ActKind::MoveShadowRound);
                }
                false
            }
            ActKind::BattleIdle => match act.step {
                Step::Start => {
                    let total = rng.range(60., 90.);
                    self.motion_change(motion::FOOTWORK);
                    self.set(act.kind, Step::Timer { t: total, total });
                    false
                }
                Step::Timer { t, total } => {
                    self.turn_toward(to_target, TURN_RATE);
                    let t = t - 1.;
                    if t <= 0. || self.motion != motion::FOOTWORK {
                        self.motion_change(motion::IDLE);
                        return true;
                    }
                    self.set(act.kind, Step::Timer { t, total });
                    false
                }
                _ => true,
            },
            ActKind::ClawL | ActKind::ClawR => match act.step {
                Step::Start => {
                    self.turn_toward(to_target, TURN_RATE);
                    let m = if act.kind == ActKind::ClawL {
                        motion::CLAW_L
                    } else {
                        motion::CLAW_R
                    };
                    self.motion_start(m);
                    self.set(act.kind, Step::Motion);
                    false
                }
                _ => {
                    let m = if act.kind == ActKind::ClawL {
                        motion::CLAW_L
                    } else {
                        motion::CLAW_R
                    };
                    self.motion != m || self.motion_tick + 1 >= clips.length(m)
                }
            },
            ActKind::ClawF => match act.step {
                Step::Start => {
                    // set_dir: instant facing; jump height 100, speed 5.
                    if hlen(to_target) > 1e-4 {
                        self.yaw = to_target[0].atan2(to_target[2]);
                    }
                    let f = self.forward();
                    self.velocity = [f[0] * 5., (2. * GRAVITY * 100.).sqrt(), f[2] * 5.];
                    self.airborne = true;
                    self.motion_start(motion::JUMP);
                    self.set(act.kind, Step::Air { stage: 0 });
                    false
                }
                Step::Air { stage } => {
                    if !self.airborne {
                        self.motion_start(motion::LAND);
                        return true;
                    }
                    match stage {
                        0 => {
                            self.motion_start(motion::CLAW_F);
                            self.set(act.kind, Step::Air { stage: 1 });
                        }
                        1 if self.motion_tick + 1 >= clips.length(motion::CLAW_F) => {
                            self.motion_start(motion::FALL);
                            self.set(act.kind, Step::Air { stage: 2 });
                        }
                        _ => {}
                    }
                    false
                }
                _ => true,
            },
            ActKind::MoveTarget => match act.step {
                Step::Start => {
                    let total = rng.range(60., 90.);
                    let _side = rng.get(2);
                    self.set(act.kind, Step::Timer { t: total, total });
                    self.run(
                        Act {
                            kind: act.kind,
                            step: Step::Timer { t: total, total },
                        },
                        rng,
                        clips,
                        target,
                        player,
                    )
                }
                Step::Timer { t, total } => {
                    if d < 150. || t <= 0. {
                        self.motion_change(motion::IDLE);
                        return true;
                    }
                    // walk_step: turn toward pos + v*500, walk (root motion).
                    self.turn_toward(to_target, TURN_RATE);
                    self.motion_change(motion::WALK);
                    self.set(act.kind, Step::Timer { t: t - 1., total });
                    false
                }
                _ => true,
            },
            ActKind::MoveShadowRound => match act.step {
                Step::Start => {
                    self.motion_start(motion::SINK);
                    let total = clips.loop_start(motion::SINK) as f32;
                    self.set(act.kind, Step::Timer { t: 0., total });
                    false
                }
                Step::Timer { t, total } => {
                    // Sink: scale Y from 1 to 0 over the loop top.
                    self.turn_toward(to_target, TURN_RATE);
                    self.scale_y = (1. - t / total.max(1.)).max(0.);
                    let t = t + 1.;
                    if t >= total {
                        self.scale_y = 1.;
                        self.sunk = true;
                        let angle = rng.getf(std::f32::consts::TAU);
                        let r = rng.range(200., 300.);
                        // rot_y(angle) applied to (r, 0, 0).
                        let dest = [
                            self.position[0] + r * angle.cos(),
                            self.position[1],
                            self.position[2] - r * angle.sin(),
                        ];
                        let t = rng.range(75., 120.);
                        self.set(act.kind, Step::Leg { leg: 1, t, dest });
                    } else {
                        self.set(act.kind, Step::Timer { t, total });
                    }
                    false
                }
                Step::Leg { leg, t, dest } => {
                    self.scale_y = 0.1;
                    let v = sub(dest, self.position);
                    let weave = (60. * self.weave.to_radians().sin()).to_radians();
                    let (s, c) = weave.sin_cos();
                    let v2 = [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c];
                    self.turn_toward(v2, TURN_RATE);
                    self.weave += 3. + rng.getf(2.);
                    let len = (v2[0] * v2[0] + v2[1] * v2[1] + v2[2] * v2[2]).sqrt();
                    let speed = len.min(14.);
                    if len > 1e-4 {
                        // Horizontal part of the 3-D direction [inferred].
                        self.position[0] += v2[0] / len * speed;
                        self.position[2] += v2[2] / len * speed;
                    }
                    self.wished = speed;
                    let t = t - 1.;
                    let h = {
                        let mut h = sub(dest, self.position);
                        h[1] = 0.;
                        hlen(h)
                    };
                    if t <= 0. || h < 100. {
                        if leg == 1 {
                            let t = rng.range(75., 120.);
                            self.set(
                                act.kind,
                                Step::Leg {
                                    leg: 2,
                                    t,
                                    dest: target,
                                },
                            );
                        } else {
                            // Rise.
                            self.sunk = false;
                            self.motion_start(motion::RISE);
                            let total = clips.length(motion::RISE) as f32;
                            self.set(act.kind, Step::Rise { t: 1., total });
                        }
                    } else {
                        self.set(act.kind, Step::Leg { leg, t, dest });
                    }
                    false
                }
                Step::Rise { t, total } => {
                    self.turn_toward(to_target, TURN_RATE);
                    self.scale_y = (t / total.max(1.)).min(1.);
                    let t = t + 1.;
                    if t >= total {
                        self.scale_y = 1.;
                        self.motion_change(motion::FOOTWORK);
                        return true;
                    }
                    self.set(act.kind, Step::Rise { t, total });
                    false
                }
                _ => true,
            },
            ActKind::AppearShadow => match act.step {
                Step::Start => {
                    // set_dir toward Sora; fade in over 30 ticks; rise.
                    let v = sub(player, self.position);
                    if hlen(v) > 1e-4 {
                        self.yaw = v[0].atan2(v[2]);
                    }
                    self.motion_start(motion::RISE);
                    let total = clips.length(motion::RISE) as f32;
                    self.scale_y = 0.;
                    self.set(act.kind, Step::Rise { t: 1., total });
                    false
                }
                Step::Rise { t, total } => {
                    self.alpha = (t / 30.).min(1.);
                    self.scale_y = (t / total.max(1.)).min(1.);
                    let t = t + 1.;
                    if t >= total {
                        self.scale_y = 1.;
                        self.alpha = 1.;
                        self.motion_change(motion::FOOTWORK);
                        let wait = rng.getf(30.);
                        self.set(
                            act.kind,
                            Step::Timer {
                                t: wait,
                                total: wait,
                            },
                        );
                    } else {
                        self.set(act.kind, Step::Rise { t, total });
                    }
                    false
                }
                Step::Timer { t, total } => {
                    if t <= 0. {
                        return true;
                    }
                    self.set(act.kind, Step::Timer { t: t - 1., total });
                    false
                }
                _ => true,
            },
            ActKind::Leave => match act.step {
                Step::Start => {
                    self.motion_start(motion::LEAVE);
                    self.set(act.kind, Step::Fade { t: 0. });
                    false
                }
                Step::Fade { t } => {
                    self.alpha = (1. - t / 30.).max(0.);
                    if t >= 60. {
                        self.gone = true;
                        self.gone_ticks = 0;
                        return true;
                    }
                    self.set(act.kind, Step::Fade { t: t + 1. });
                    false
                }
                _ => true,
            },
            ActKind::Dead => match act.step {
                // wait 15; fadeout 15; wait 15; leave.
                Step::Start => {
                    self.set(act.kind, Step::Fade { t: 0. });
                    false
                }
                Step::Fade { t } => {
                    if t >= 15. {
                        self.alpha = (1. - (t - 15.) / 15.).max(0.);
                    }
                    if t >= 45. {
                        self.gone = true;
                        self.gone_ticks = 0;
                        return true;
                    }
                    self.set(act.kind, Step::Fade { t: t + 1. });
                    false
                }
                _ => true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct TestClips(HashMap<usize, u32>);
    impl Clips for TestClips {
        fn length(&self, slot: usize) -> u32 {
            *self.0.get(&slot).unwrap_or(&40)
        }
        fn loop_start(&self, _slot: usize) -> u32 {
            24
        }
        fn root(&self, _slot: usize, _tick: u32) -> Vec3 {
            [0.; 3]
        }
    }
    struct Flat;
    impl Env for Flat {
        fn floor(&self, _x: f32, _z: f32, _top: f32) -> Option<f32> {
            Some(0.)
        }
    }

    fn stats() -> TargetStats {
        TargetStats::enemy(50, 10000, 1000, [100; 7], 20, 2)
    }

    #[test]
    fn script_random_matches_the_lcg() {
        let mut r = ScriptRandom::default();
        assert_eq!(r.getf(1.), 69069. / 4_294_967_296.);
        assert_eq!(r.0, 69069);
    }

    #[test]
    fn appears_then_fights_near_the_target() {
        let clips = TestClips(HashMap::new());
        let mut rng = ScriptRandom::default();
        let mut s = Shadow::new([0.; 3], 0., stats());
        assert_eq!(s.current_act(), Some(ActKind::AppearShadow));
        let mut acts = Vec::new();
        for _ in 0..600 {
            s.tick(&mut rng, &clips, &Flat, [0., 0., 120.], [0., 0., 120.]);
            if let Some(a) = s.current_act()
                && acts.last() != Some(&a)
            {
                acts.push(a);
            }
        }
        assert!(acts.contains(&ActKind::BattleIdle) || acts.contains(&ActKind::ClawL));
        assert!(
            acts.iter()
                .any(|a| matches!(a, ActKind::ClawL | ActKind::MoveShadowRound))
        );
    }

    #[test]
    fn killing_hit_starts_dead_and_leaves() {
        let clips = TestClips(HashMap::new());
        let mut rng = ScriptRandom::default();
        let mut s = Shadow::new([0.; 3], 0., stats());
        for _ in 0..200 {
            s.tick(&mut rng, &clips, &Flat, [0., 0., 500.], [0., 0., 500.]);
        }
        s.sunk = false;
        let finisher = AttackParams {
            power: 1000,
            stat: 0,
            element: 0,
            reaction: 7,
            knockback: [3, 45],
        };
        let sora = AttackerStats {
            strength: 5,
            magic: 7,
            max: 99999,
            min: 1,
        };
        assert!(s.hit(1, [0., 0., 1.], true, &finisher, &sora).is_some());
        assert_eq!(s.current_act(), Some(ActKind::Dead));
        assert!(s.hit(1, [0., 0., 1.], true, &finisher, &sora).is_none());
        for _ in 0..60 {
            s.tick(&mut rng, &clips, &Flat, [0., 0., 500.], [0., 0., 500.]);
        }
        assert!(s.gone);
    }
}
