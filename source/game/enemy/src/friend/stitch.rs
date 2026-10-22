//! kn::Stitch (personality 0xe, think FUN_1401ca240). Stitch lives in
//! camera space: P+0x650 is a screen position (x right, y down, depth
//! 1024) and a heading; FUN_1401cad30 turns it into the matrix the
//! renderer draws him with (FUN_1403cbfc0), scaled by 0.503644. His object
//! stays in physics mode 3, which never moves it. The host places him from
//! [`Stitch::screen`] and its camera.
//!
//! [I] The motion root's heading (FUN_1403c7a90) is taken as 0 where the
//! executable subtracts it. The limit mode (traps 10:3 / 10:4,
//! FUN_1401c9d60 / FUN_1401c7e00, the commands 0x20d..0x20f and the shots'
//! damage) is not ported.

use super::{Action, Friend, Requests, flag};
use crate::attack::Attack;
use crate::env::Env;
use crate::sysobj::class_flag;
use crate::world::{Event, ObjId, Object, PLAYER_OBJECT, World};
use std::f32::consts::PI;

/// The extra record P+0x28 (id 128): the regeneration period, the HP and
/// MP fractions of the specials, and how often (actions) and how likely a
/// special is.
const REGEN: f32 = 600.;
const HP_PART: f32 = 0.25;
const MP_PART: f32 = 0.75;
const EVERY: f32 = 5.;
const CHANCE: f32 = 0.5;

/// The screen depth (P+0x658) and the matrix scale (DAT_1405b4028).
pub const DEPTH: f32 = 1024.;
pub const SCALE: f32 = 0.503_644;
/// P+0x640 and P+0x644 (FUN_1401c6d00): screen speed and turn per frame.
const MAX_SPEED: f32 = 15.;
const TURN: f32 = PI / 16.;
/// The screen's wrap (DAT_1405b4030..0x1405b4040, DAT_140585f00).
const X_EDGE: f32 = 400.;
const X_WRAP: f32 = 800.;
const Y_EDGE: f32 = 320.;
const Y_WRAP: f32 = 640.;
/// The goal points (x, y) left of the centre (0x140724538) and right of it
/// (0x140724568).
const LEFT: [(f32, f32); 6] = [
    (-277., 68.),
    (-135., 210.),
    (-184., 70.),
    (-130., 180.),
    (-240., 78.),
    (-130., 105.),
];
const RIGHT: [(f32, f32); 6] = [
    (70., 190.),
    (184., 120.),
    (270., 110.),
    (200., 110.),
    (60., 200.),
    (210., 80.),
];
/// The motion blend of his moves (DAT_140623b90).
const BLEND: f32 = 4.;
const MOTION_IDLE: usize = 0;
const MOTION_MOVE: usize = 0xa1;
const MOTION_HP: usize = 0xcb;
const MOTION_MP: usize = 0xcc;
/// FUN_1401caac0's attack parameter.
const SPECIAL_ATKP: i32 = 0x4ba;

/// Effect slot flags (P+0x30 + 0x40·i).
pub mod slot_flag {
    /// Bound to Stitch (bits 0..1 = 1), to Sora (2).
    pub const STITCH: u32 = 1;
    pub const PLAYER: u32 = 2;
    pub const KIND: u32 = 3;
    pub const STAY: u32 = 8;
    /// Left in camera space after its first frame.
    pub const DETACH: u32 = 0x10;
    pub const KILL: u32 = 0x4000_0000;
}

/// One of the 24 effect slots (0x40 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectSlot {
    pub flags: u32,
    /// +0x34: the effect of Stitch's container to start (-1 started).
    pub effect: i32,
    /// +0x4c: the bone it follows.
    pub bone: i32,
    /// +0x48: the motion it lasts for (-1 any).
    pub link: i32,
    /// +0x40 / +0x44: the start flags and argument.
    pub start: [i32; 2],
    /// The running effect (0 none).
    pub handle: u32,
}

impl EffectSlot {
    const FREE: EffectSlot = EffectSlot {
        flags: 0,
        effect: -1,
        bone: 0,
        link: -1,
        start: [0, 0],
        handle: 0,
    };

    fn free(&self) -> bool {
        self.effect < 0 && self.handle == 0
    }
}

/// The special's attack (P+0x630, its motion P+0x638) and the enemies it
/// has struck.
#[derive(Clone, Debug, PartialEq)]
struct Special {
    attack: usize,
    link: usize,
    struck: Vec<ObjId>,
}

/// kn::Stitch's own state.
#[derive(Clone, Debug, PartialEq)]
pub struct Stitch {
    /// P+0x648 bit 0: the screen position is set.
    pub valid: bool,
    /// P+0x64c: the special's step.
    pub step: i32,
    /// P+0x650: x, y, depth and heading.
    pub screen: [f32; 4],
    /// P+0x660: the goal, its depth and heading.
    pub goal: [f32; 4],
    /// P+0x670 / P+0x67c: the unit move direction and the speed.
    dir: [f32; 3],
    speed: f32,
    /// P+0x684.
    regen: f32,
    /// P+0x790: actions since the last special.
    pub counter: i32,
    pub slots: [EffectSlot; 24],
    special: Option<Special>,
    /// The special asked to strike this frame (FUN_1401caac0's motion).
    strike: Option<usize>,
    /// The MP special asked for Sora's MP (FUN_1403c2040).
    mp_refill: bool,
}

impl Default for Stitch {
    fn default() -> Self {
        Stitch {
            valid: false,
            step: 0,
            screen: [0., 0., DEPTH, 0.],
            goal: [0., 0., DEPTH, 0.],
            dir: [0.; 3],
            speed: 0.,
            regen: REGEN,
            counter: 0,
            slots: [EffectSlot::FREE; 24],
            special: None,
            strike: None,
            mp_refill: false,
        }
    }
}

impl Stitch {
    /// FUN_1401c9620 (trap 10:1): set the screen position (NaN lanes 0);
    /// the goal's heading takes its heading, the depth is 1024.
    pub fn set_screen(&mut self, v: [f32; 4]) {
        self.valid = true;
        self.screen = v;
        self.goal[3] = v[3];
        self.screen[2] = DEPTH;
    }

    /// FUN_1401c9c10 (trap 10:12): take the first free slot (the last when
    /// none is) for `effect` bound to `on` at `bone`, lasting for motion
    /// `link`; returns its index.
    #[allow(clippy::too_many_arguments)]
    pub fn effect(
        &mut self,
        effect: i32,
        on: Option<ObjId>,
        me: ObjId,
        bone: i32,
        link: i32,
        stay: bool,
        start: [i32; 2],
    ) -> usize {
        let i = self.slots.iter().position(EffectSlot::free).unwrap_or(23);
        let mut s = EffectSlot {
            flags: 0,
            effect,
            bone,
            link,
            start,
            handle: 0,
        };
        match on {
            Some(o) if o == me => {
                s.flags |= slot_flag::STITCH;
                if effect == 0 {
                    s.flags |= 0x20;
                }
            }
            Some(PLAYER_OBJECT) => s.flags |= slot_flag::PLAYER,
            _ => {}
        }
        if stay {
            s.flags |= slot_flag::STAY;
        }
        self.slots[i] = s;
        i
    }

    /// FUN_1401c80f0 (trap 10:16).
    pub fn kill(&mut self, slot: usize) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.flags |= slot_flag::KILL;
        }
    }

    /// FUN_1401c75e0: a new goal. Kinds 0..7 are fixed points; any other
    /// is random: half the time one of the six points on Stitch's side,
    /// else one on a side jittered or shifted a screen's width or height.
    pub fn pick_goal(&mut self, kind: i32, f: &mut FriendRng<'_>) {
        self.goal[2] = DEPTH;
        let fixed = match kind {
            0 | 1 => Some((-260., -180.)),
            2 => Some((260., -180.)),
            3 => Some((260., 180.)),
            4 => Some((-260., 180.)),
            5 => Some((0., 0.)),
            6 => Some((-230., 90.)),
            7 => Some((138., 106.)),
            _ => None,
        };
        if let Some((x, y)) = fixed {
            self.goal[0] = x;
            self.goal[1] = y;
            return;
        }
        self.random_goal(f);
    }

    /// The random part of FUN_1401c75e0 (and FUN_1401c8950's first call).
    fn random_goal(&mut self, f: &mut FriendRng<'_>) {
        let right_side = self.screen[0] >= 0.;
        if f.rand01() >= 0.5 {
            let jitter = (f.rand01() * 0.5 + 0.5) * 60.;
            // FUN_1401c75a0 draws from the left points, FUN_1401c78f0 from
            // the right ones.
            match f.rand(4) {
                0 => {
                    if right_side {
                        self.set_xy(f.pick(&LEFT));
                        self.goal[0] += jitter;
                    } else {
                        self.set_xy(f.pick(&RIGHT));
                        self.goal[0] -= jitter;
                    }
                }
                1 => {
                    if right_side {
                        self.set_xy(f.pick(&LEFT));
                        self.goal[0] += X_WRAP;
                    } else {
                        self.set_xy(f.pick(&RIGHT));
                        self.goal[0] -= X_WRAP;
                    }
                }
                2 => {
                    self.set_xy(f.pick(if right_side { &RIGHT } else { &LEFT }));
                    self.goal[1] += Y_WRAP - jitter;
                }
                3 => {
                    self.set_xy(f.pick(if right_side { &RIGHT } else { &LEFT }));
                    self.goal[1] -= Y_WRAP;
                }
                _ => {}
            }
        } else {
            self.set_xy(f.pick(if right_side { &RIGHT } else { &LEFT }));
        }
    }

    fn set_xy(&mut self, (x, y): (f32, f32)) {
        self.goal[0] = x;
        self.goal[1] = y;
    }

    /// FUN_1401c8950's step: toward the goal at most 15 a frame, the goal's
    /// heading from the move (atan2(dx, -dy)).
    fn steer(&mut self) -> f32 {
        let d = [
            self.goal[0] - self.screen[0],
            self.goal[1] - self.screen[1],
            self.goal[2] - self.screen[2],
        ];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        self.dir = if len > 0. {
            d.map(|c| c / len)
        } else {
            [0.; 3]
        };
        self.speed = len.min(MAX_SPEED);
        if self.dir[0] != 0. || self.dir[1] != 0. {
            self.goal[3] = self.dir[0].atan2(-self.dir[1]);
        }
        len
    }

    /// FUN_1401c7a30: at the goal (within a frame's move and turn).
    fn arrived(&self) -> bool {
        let d = [
            self.screen[0] - self.goal[0],
            self.screen[1] - self.goal[1],
            self.screen[2] - self.goal[2],
        ];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < MAX_SPEED
            && wrap(self.screen[3] - self.goal[3]).abs() < TURN
    }
}

/// The friend's generators, lent to Stitch's goal picking.
pub struct FriendRng<'a>(pub &'a mut Friend);

impl FriendRng<'_> {
    fn rand(&mut self, n: u32) -> u32 {
        self.0.rand(n)
    }
    fn rand01(&mut self) -> f32 {
        self.0.rand01()
    }
    fn pick(&mut self, t: &[(f32, f32); 6]) -> (f32, f32) {
        t[self.0.rand(6) as usize]
    }
}

fn wrap(a: f32) -> f32 {
    let mut a = a % (2. * PI);
    if a > PI {
        a -= 2. * PI;
    } else if a < -PI {
        a += 2. * PI;
    }
    a
}

/// FUN_1403b74c0: turn `from` toward `to` by at most `limit`.
fn turn(from: f32, to: f32, limit: f32) -> f32 {
    wrap(from + wrap(to - from).clamp(-limit, limit))
}

/// The entry index of each of Stitch's actions in his table
/// (0x140724340).
pub const IDLE: usize = 0;
pub const MOVE: usize = 1;
pub const HP: usize = 2;
pub const MP: usize = 3;

/// A running limit (FUN_1403d8ad0) [I: the running limit's command], the
/// object's +0x124 bit 11, and for the specials the move-only window.
fn blocked(me: &Object, env: &dyn Env, specials: bool) -> bool {
    env.limit_command() != 0
        || me.sys.state_flags & 1 << 11 != 0
        || (specials && me.sys.motion.flags.word & 1 << 30 != 0)
}

fn continues(f: &Friend) -> bool {
    f.flags & flag::CONTINUES != 0
}

/// FUN_1401b3970's walk: a live enemy-class object that is not down.
fn enemy_up(world: &World) -> bool {
    world.objects.iter().flatten().any(|o| {
        o.sys.class_flags & class_flag::ENEMY != 0 && o.sys.hp > 0 && !o.sys.gone && o.targetable()
    })
}

/// The test half of Stitch's action routines.
pub fn test(action: Action, world: &World, me: &Object, f: &mut Friend, env: &dyn Env) -> bool {
    let Some(s) = f.stitch.as_deref() else {
        return false;
    };
    match action {
        // FUN_1401c7f40.
        Action::StitchIdle => {
            !(blocked(me, env, false)
                || attack_ready(world, me, f, env)
                || (continues(f) && f.wait <= 0.))
        }
        // FUN_1401c8950.
        Action::StitchMove => {
            let arrived = s.arrived();
            if blocked(me, env, false) || attack_ready(world, me, f, env) {
                return false;
            }
            if continues(f) {
                !arrived
            } else {
                f.current != Some(MOVE)
            }
        }
        // FUN_1401ca6e0: Sora's HP under a quarter while an enemy is up.
        Action::StitchHp => {
            if blocked(me, env, true) {
                return false;
            }
            if continues(f) {
                return s.step != 4;
            }
            env.player_hp()
                .is_some_and(|(hp, max)| hp < (max as f32 * HP_PART) as i32)
                && enemy_up(world)
        }
        // FUN_1401c8100: Sora's MP under three quarters.
        Action::StitchMp => {
            if blocked(me, env, true) {
                return false;
            }
            if continues(f) {
                return s.step != 3;
            }
            env.player_mp()
                .is_some_and(|(mp, max)| mp < (max as f32 * MP_PART) as i32)
        }
        // FUN_1401c8610 / FUN_1401c8700 / FUN_1401c8440: only a running
        // limit command continues.
        Action::StitchCommand(motion) => continues(f) && me.sys.motion.slot == motion as usize,
        _ => false,
    }
}

/// FUN_1401b2b20: another of the table's attack entries (flag bit 1) would
/// pass its test now (tested as a first call).
fn attack_ready(world: &World, me: &Object, f: &mut Friend, env: &dyn Env) -> bool {
    let saved = f.flags & flag::CONTINUES;
    f.flags &= !flag::CONTINUES;
    let table = super::table(f.personality);
    let ready = table
        .iter()
        .filter(|e| {
            e.flags & 2 != 0 && !matches!(e.action, Action::StitchIdle | Action::StitchMove)
        })
        .any(|e| test(e.action, world, me, f, env));
    f.flags |= saved;
    ready
}

/// The run half of Stitch's action routines.
pub fn run(action: Action, me: &mut Object, f: &mut Friend, env: &dyn Env) {
    let first = !continues(f);
    match action {
        Action::StitchIdle => {
            if first {
                // FUN_1401b7b70 with the blob's wait (+0x70 / +0x74), longer
                // in battle, shorter when he faces away from the camera's
                // axis.
                let (mut lo, mut hi) = f.blob.wait;
                if env.battle() {
                    lo = lo + lo + 60.;
                    hi = hi + hi + 60.;
                }
                let heading = f.stitch.as_deref().map_or(0., |s| s.screen[3]);
                if wrap(heading).abs() > PI / 4. {
                    lo *= 0.3;
                    hi *= 0.3;
                }
                f.wait = f.rand_range(lo, hi);
            }
            me.sys
                .motion_change(MOTION_IDLE, BLEND, env.clips(me.sys.model));
            if let Some(s) = f.stitch.as_deref_mut() {
                s.dir = [0.; 3];
                s.speed = 0.;
            }
        }
        Action::StitchMove => {
            if first {
                let mut s = f.stitch.take().unwrap_or_default();
                s.goal[2] = DEPTH;
                s.random_goal(&mut FriendRng(f));
                f.stitch = Some(s);
                me.sys
                    .motion_change(MOTION_MOVE, BLEND, env.clips(me.sys.model));
            }
            if let Some(s) = f.stitch.as_deref_mut() {
                s.steer();
            }
        }
        Action::StitchHp => special(me, f, env, true, first),
        Action::StitchMp => special(me, f, env, false, first),
        _ => {}
    }
}

/// FUN_1401ca6e0 / FUN_1401c8100: go to the special's point (fast while
/// far), turn to face the camera, then play the special: HP (motion 0xcb)
/// strikes every enemy (FUN_1401caac0); MP (motion 0xcc) refills Sora's MP.
fn special(me: &mut Object, f: &mut Friend, env: &dyn Env, hp: bool, first: bool) {
    let id = me.id;
    let Some(s) = f.stitch.as_deref_mut() else {
        return;
    };
    if first {
        s.step = 0;
        s.goal = if hp {
            [-230., 90., DEPTH, s.goal[3]]
        } else {
            [138., 106., DEPTH, s.goal[3]]
        };
    }
    let clips = env.clips(me.sys.model);
    let motion = if hp { MOTION_HP } else { MOTION_MP };
    match s.step {
        0 => {
            if s.steer() >= 100. {
                return;
            }
            s.goal[3] = 0.;
        }
        1 => {
            // The heading is not steered here: the goal faces the camera.
            let h = s.goal[3];
            s.steer();
            s.goal[3] = h;
            if !s.arrived() {
                return;
            }
        }
        2 => {
            s.dir = [0.; 3];
            s.speed = 0.;
            s.screen = s.goal;
            me.sys.motion_start(motion, 0., clips);
            if hp {
                s.effect(0xc, None, id, -1, motion as i32, false, [1, 0]);
                let k = s.effect(0xd, Some(id), id, -1, motion as i32, false, [1, 0]);
                s.slots[k].flags |= slot_flag::DETACH;
                s.strike = Some(motion);
            } else {
                // FUN_1403c2040: Sora's MP to its maximum; FUN_1403b6670:
                // effect 10 of Stitch's container on Sora.
                s.effect(0xb, Some(id), id, -1, motion as i32, false, [1, 0]);
                s.mp_refill = true;
            }
        }
        3 if hp => {
            if me.sys.motion.slot != motion || me.sys.motion_ended() {
                s.step += 1;
            }
            s.strike = Some(motion);
            return;
        }
        _ => return,
    }
    s.step += 1;
}

/// One frame of Stitch: the think (FUN_1401ca240), the special's strikes
/// (FUN_1401caac0), then the personality's global pre-update for the next
/// frame (FUN_1401c9080, his effect slots), which the executable runs
/// before any object's motion advances.
pub fn tick(world: &mut World, me: &mut Object, f: &mut Friend, env: &dyn Env, out: &mut Requests) {
    think(world, me, f, env, out);
    strikes(world, me, f);
    if let Some(s) = f.stitch.as_deref_mut()
        && std::mem::take(&mut s.mp_refill)
    {
        world.events.push(Event::Service {
            name: "stitch_mp_refill",
            object: Some(PLAYER_OBJECT),
            other: Some(me.id),
            words: [0; 8],
            vector: None,
        });
        let handle = world.next_effect_handle();
        world.events.push(Event::EffectStart {
            object: me.id,
            effect: 10,
            arguments: [1, 0],
            handle,
            position: None,
            bound: true,
            other: Some(PLAYER_OBJECT),
        });
    }
    effects(world, me, f, env);
}

/// FUN_1401ca240.
fn think(world: &mut World, me: &mut Object, f: &mut Friend, env: &dyn Env, out: &mut Requests) {
    // Mode 3 every frame: the object does not move by itself.
    me.sys.to_camera_mode();
    me.sys.move_speed = 0.;
    let word = me.sys.motion.flags.word;
    let controlled = word & 0x1c != 0;
    if controlled || word & 1 << 30 != 0 {
        if controlled {
            f.flags |= flag::CONTROLLED;
            if let Some(s) = f.stitch.as_deref_mut()
                && !s.valid
            {
                s.set_screen([0.; 4]);
            }
        }
        if let Some(s) = f.stitch.as_deref_mut()
            && s.valid
        {
            if s.screen[0] < -X_EDGE && s.goal[0] < -X_EDGE {
                s.screen[0] += X_WRAP;
                s.goal[0] += X_WRAP;
            }
            if s.screen[0] > X_EDGE && s.goal[0] > X_EDGE {
                s.screen[0] -= X_WRAP;
                s.goal[0] -= X_WRAP;
            }
            if s.screen[1] < -Y_EDGE && s.goal[1] < -Y_EDGE {
                s.screen[1] += Y_WRAP;
                s.goal[1] += Y_WRAP;
            }
            if s.screen[1] > Y_EDGE && s.goal[1] > Y_EDGE {
                s.screen[1] -= Y_WRAP;
                s.goal[1] -= Y_WRAP;
            }
        }
        if let Some(s) = f.stitch.as_deref_mut() {
            s.dir = [0.; 3];
            s.speed = 0.;
        }
        let counter = f.stitch.as_deref().map_or(0, |s| s.counter);
        let due = env.battle() && env.limit_command() == 0 && counter as f32 > EVERY;
        let mut default = true;
        if due {
            if let Some(s) = f.stitch.as_deref_mut() {
                s.counter = 0;
            }
            if f.rand01() < CHANCE {
                // The special by which part is lower: HP/MaxHP ≤ MP/MaxMP
                // → the HP special.
                let (hp, max_hp) = env.player_hp().unwrap_or((1, 1));
                let (mp, max_mp) = env.player_mp().unwrap_or((1, 1));
                let k = if max_mp * hp <= mp * max_hp { HP } else { MP };
                let action = super::table(f.personality)[k].action;
                f.current = None;
                f.flags &= !flag::CONTINUES;
                run(action, me, f, env);
                f.current = Some(k);
                default = false;
            }
        }
        if default {
            let before = f.current;
            super::choose(world, me, f, env, out);
            if (before != f.current || !continues(f))
                && let Some(s) = f.stitch.as_deref_mut()
            {
                s.counter += 1;
            }
        }
        if let Some(s) = f.stitch.as_deref_mut() {
            s.screen[3] = turn(s.screen[3], s.goal[3], TURN);
            for k in 0..3 {
                s.screen[k] += s.dir[k] * s.speed;
            }
        }
    }
    // Sora regenerates while hurt: every 600 frames his class's +0xe8 runs
    // with (1, 0) [U: its effect is not traced].
    if let Some((hp, max)) = env.player_hp()
        && hp < max
        && let Some(s) = f.stitch.as_deref_mut()
    {
        s.regen -= 1.;
        if s.regen <= 0. {
            world.events.push(Event::Service {
                name: "stitch_regen",
                object: Some(PLAYER_OBJECT),
                other: Some(me.id),
                words: [1, 0, 0, 0, 0, 0, 0, 0],
                vector: None,
            });
            s.regen = REGEN;
        }
    }
}

/// FUN_1401c9080 and FUN_1401c7da0: start the slots' effects, kill the
/// marked ones and those whose motion has passed, and free the special's
/// attack when its motion is over.
fn effects(world: &mut World, me: &mut Object, f: &mut Friend, env: &dyn Env) {
    let Some(s) = f.stitch.as_deref_mut() else {
        return;
    };
    let alive = !me.sys.gone;
    let motion = me.sys.motion.slot as i32;
    for slot in s.slots.iter_mut() {
        if slot.handle == 0 {
            if slot.effect >= 0 {
                let handle = world.next_effect_handle();
                let kind = slot.flags & slot_flag::KIND;
                world.events.push(Event::EffectStart {
                    object: me.id,
                    effect: slot.effect,
                    arguments: [slot.start[0] & !1, slot.start[1]],
                    handle,
                    position: None,
                    bound: kind == slot_flag::STITCH,
                    other: (kind == slot_flag::PLAYER).then_some(PLAYER_OBJECT),
                });
                slot.handle = handle;
                slot.effect = -1;
            }
            continue;
        }
        if !alive || slot.flags & slot_flag::KILL != 0 {
            world.events.push(Event::EffectCommand {
                trap: 51,
                handle: slot.handle,
                words: [slot.handle, 0],
                vector: None,
            });
            *slot = EffectSlot::FREE;
            continue;
        }
        if !env.effect_active(slot.handle) {
            if slot.start[0] & 1 != 0 {
                slot.flags |= slot_flag::KILL;
            }
        } else if slot.link >= 0 && slot.flags & slot_flag::KIND < 2 && slot.link != motion {
            slot.flags |= slot_flag::KILL;
        }
    }
    if let Some(sp) = &s.special
        && (!alive || sp.link as i32 != motion || me.sys.motion_ended())
    {
        if let Some(a) = world.attacks.get_mut(sp.attack) {
            *a = None;
        }
        s.special = None;
    }
}

/// FUN_1401caac0: the special's attack (ATKP 0x4ba, made once) strikes
/// every enemy it can, once each.
fn strikes(world: &mut World, me: &Object, f: &mut Friend) {
    let Some(s) = f.stitch.as_deref_mut() else {
        return;
    };
    let Some(link) = s.strike.take() else {
        return;
    };
    if s.special.is_none() {
        // [I] FUN_1403cf830's operands as `attack_new`'s: no group, extra 0.
        let mut a = Attack::new(Some(me.id), SPECIAL_ATKP, -1, 0, me.sys.team);
        world.attack_serial += 1;
        a.serial = world.attack_serial;
        let i = match world.attacks.iter().position(Option::is_none) {
            Some(i) => i,
            None => {
                world.attacks.push(None);
                world.attacks.len() - 1
            }
        };
        world.attacks[i] = Some(a);
        s.special = Some(Special {
            attack: i,
            link,
            struck: Vec::new(),
        });
    }
    let Some(sp) = s.special.as_mut() else {
        return;
    };
    let targets: Vec<ObjId> = world
        .objects
        .iter()
        .flatten()
        .filter(|o| {
            o.sys.class_flags & class_flag::ENEMY != 0
                && o.sys.hp > 0
                && !o.sys.gone
                && o.targetable()
                && !sp.struck.contains(&o.id)
        })
        .map(|o| o.id)
        .collect();
    let Some(Some(a)) = world.attacks.get(sp.attack) else {
        return;
    };
    let (param, serial, owner) = ([a.param_id, a.param_extra], a.serial, a.owner);
    for t in targets {
        sp.struck.push(t);
        world.events.push(Event::AttackStrike {
            attack: crate::world::ATTACK_HANDLES + 0x100 * sp.attack as u32,
            slot: sp.attack,
            target: Some(t),
            arguments: [0, 0],
            param,
            serial,
            owner,
        });
    }
}
