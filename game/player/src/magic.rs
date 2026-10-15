//! Sora's magic, translated from the PC executable: the command colour
//! FUN_140401d60 / FUN_1403c5940, the cast start FUN_1403c5170, the MP
//! rules FUN_1403d3cf0 / FUN_1403c0890 / FUN_1403c0de0, and the six spell
//! programs (the BDX scripts of `magic/*.mag`, trigger 21) written out as
//! state machines. Spells are driven by `magi` rows (03system `pref/magi`)
//! and ATKP ids; hits go through the normal attack pipeline.
//!
//! Positions are the simulation's Y-up space; the executable's Y-down
//! offsets are negated here.

use crate::Vec3;

/// Magic types (magc +0, cmd argument).
pub const FIRE: u8 = 0;
pub const BLIZZARD: u8 = 1;
pub const THUNDER: u8 = 2;
pub const CURE: u8 = 3;
pub const MAGNET: u8 = 4;
pub const REFLECT: u8 = 5;

/// `0x41f00000`-style constants of the MP routines.
/// SSTM +0x7c: MP gained during a charge shortens it by gain × 12.
pub const CHARGE_GAIN_FACTOR: f32 = 12.;
/// 03system `prty` +0x1c: the length of an MP Charge (3000 for Sora).
pub const CHARGE_LENGTH: f32 = 3000.;
/// SSTM +0x48: the air-cast hop height.
pub const AIR_HOP: f32 = 40.;

/// One `magc` record (0x38 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub magic_type: u8,
    pub level: u8,
    /// 0 for any world.
    pub world: u8,
    pub file: String,
    /// The element item whose owned count is the level (0 = level 0).
    pub item: u16,
    pub command: u16,
    pub ground: (i16, i16),
    pub finisher: (i16, i16),
    pub air: (i16, i16),
    pub voice: i8,
    pub voice_finisher: i8,
    pub voice_self: i8,
}

impl Record {
    pub const SIZE: usize = 0x38;

    pub fn read(r: &[u8]) -> Option<Self> {
        let r = r.get(..Self::SIZE)?;
        let h = |at: usize| u16::from_le_bytes([r[at], r[at + 1]]);
        let end = r[4..0x24].iter().position(|&b| b == 0).unwrap_or(32);
        Some(Record {
            magic_type: r[0],
            level: r[1],
            world: r[2],
            file: String::from_utf8_lossy(&r[4..4 + end]).into_owned(),
            item: h(0x24),
            command: h(0x26),
            ground: (h(0x28) as i16, h(0x2a) as i16),
            finisher: (h(0x2c) as i16, h(0x2e) as i16),
            air: (h(0x30) as i16, h(0x32) as i16),
            voice: r[0x34] as i8,
            voice_finisher: r[0x35] as i8,
            voice_self: r[0x36] as i8,
        })
    }

    /// FUN_1403c5dc0: the record in force for `count` owned element items
    /// (the last of the type with level ≤ count, in a world-0 or matching
    /// world), or None when the slot is empty.
    pub fn current(records: &[Record], magic_type: u8, count: u8, world: u8) -> Option<&Record> {
        let mut of_type = records.iter().filter(|r| r.magic_type == magic_type);
        let first = of_type.next()?;
        if first.item != 0 && count == 0 {
            return None;
        }
        let mut best = first;
        for r in records.iter().filter(|r| r.magic_type == magic_type) {
            if r.level <= count && (r.world == 0 || r.world == world) {
                best = r;
            }
        }
        Some(best)
    }
}

/// A `magi` row: 31 dwords (f25 and f28 are integers).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Row(pub [u32; 31]);

impl Row {
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let b = bytes.get(..124)?;
        let mut w = [0u32; 31];
        for (i, v) in w.iter_mut().enumerate() {
            *v = u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
        }
        Some(Row(w))
    }
    pub fn f(&self, i: usize) -> f32 {
        f32::from_bits(self.0[i])
    }
    pub fn i(&self, i: usize) -> i32 {
        self.0[i] as i32
    }
}

/// The row index of a cast (FUN_1403c5170): 2·level − 2, plus 1 for a
/// ground finisher, plus 10 / 20 / 30 for the Wisdom / Master / Final
/// forms of the player character.
pub fn variant_index(level: u8, finisher: bool, form: u8) -> usize {
    let mut idx = 2 * level.max(1) as usize - 2 + finisher as usize;
    match form {
        2 => idx += 10,
        4 => idx += 20,
        5 => idx += 30,
        _ => {}
    }
    idx
}

/// MP and MP Charge (stats +0x180 / +0x184 / +0x1bc / +0x1c0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mp {
    pub cur: i32,
    pub max: i32,
    /// Charge remaining and total (total > 0 while charging).
    pub remaining: f32,
    pub total: f32,
}

impl Mp {
    pub fn new(max: i32) -> Self {
        Mp {
            cur: max,
            max,
            remaining: 0.,
            total: 0.,
        }
    }

    pub fn charging(&self) -> bool {
        self.total > 0.
    }

    /// FUN_1403d3cf0: a change that would take MP below 0 starts a charge
    /// instead (MP 0, total = remaining = `length`).
    pub fn change(&mut self, delta: i32, length: f32) -> i32 {
        if self.cur + delta < 0 {
            self.total = length;
            self.remaining = length;
            self.cur = 0;
            return 0;
        }
        self.add(delta)
    }

    /// FUN_1403c0890: gains shorten a running charge by gain × 12 and
    /// refill MP when it ends; otherwise MP clamps to [0, max].
    pub fn add(&mut self, d: i32) -> i32 {
        if self.charging() {
            if d > 0 {
                self.remaining =
                    (self.remaining - d as f32 * CHARGE_GAIN_FACTOR).clamp(0., self.total);
                if self.remaining == 0. {
                    self.remaining = 0.;
                    self.total = 0.;
                    self.cur = self.max;
                }
            }
            return self.cur;
        }
        self.cur = (self.cur + d).clamp(0, self.max);
        self.cur
    }

    /// FUN_1403c0de0, once per tick: the charge runs down at `dt × rate`
    /// (1 + MP Haste terms) and ends with a full MP bar.
    pub fn tick(&mut self, dt: f32, rate: f32) {
        if self.charging() {
            self.remaining = (self.remaining - dt * rate).clamp(0., self.total);
            if self.remaining == 0. {
                self.total = 0.;
                self.cur = self.max;
            }
        }
    }
}

/// FUN_140401d60 case Execute 2: the menu colour of a spell (≥ 4 cannot be
/// selected, 2 = orange "will start MP Charge"). `cost` is cmd +0x1c (0xff
/// for Cure); `dont_charge` is the camp's per-type toggle.
pub fn colour(cost: u8, mp: &Mp, dont_charge: bool, forbidden_area: bool) -> usize {
    let mut c = 0;
    if mp.charging() {
        c = 5;
    } else if mp.cur < cost as i32 {
        // FUN_1403c5940: Cure always, others unless "Don't MP Charge".
        c = if cost == 0xff || !dont_charge { 2 } else { 5 };
    }
    if forbidden_area {
        c = 5;
    }
    c
}

/// What FUN_1403c5170 decides when a spell command is accepted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Start {
    pub motion: i16,
    pub blend: f32,
    pub voice: i8,
    pub row: usize,
    pub airborne: bool,
    pub finisher: bool,
}

/// FUN_1403c5170 (without the script object): the cast motion, blend,
/// voice and `magi` row. `finisher` must already be false for the Lion
/// form and for spells without a finisher motion it falls back to ground.
pub fn start(
    rec: &Record,
    finisher: bool,
    airborne: bool,
    form: u8,
    target_is_self: bool,
) -> Start {
    let mut voice = rec.voice;
    let mut idx = 2 * rec.level.max(1) as usize - 2;
    let (motion, blend, fin, air);
    if !airborne {
        if finisher && rec.finisher.0 >= 0 {
            motion = rec.finisher.0;
            blend = rec.finisher.1;
            voice = rec.voice_finisher;
            idx += 1;
            fin = true;
        } else {
            motion = rec.ground.0;
            blend = rec.ground.1;
            fin = false;
        }
        air = false;
    } else {
        motion = rec.air.0;
        blend = rec.air.1;
        fin = false;
        air = true;
    }
    if target_is_self && rec.voice_self >= 0 {
        voice = rec.voice_self;
    }
    match form {
        2 => idx += 10,
        4 => idx += 20,
        5 => idx += 30,
        _ => {}
    }
    Start {
        motion,
        blend: blend as f32,
        voice,
        row: idx,
        airborne: air,
        finisher: fin,
    }
}

/// An ATTACK_MAGIC hit volume this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    /// Stable per attack object (spell serial and slot).
    pub key: u64,
    pub atkp: u16,
    /// The ATKP sub-table (the variant index).
    pub sub: u8,
    pub centre: Vec3,
    pub radius: f32,
    pub height: f32,
    /// Last tick's centre, for the sweep.
    pub previous: Option<Vec3>,
    /// Movement direction (unit, XZ-and-Y) for the hit direction; None for
    /// a static volume.
    pub moving: Option<Vec3>,
    pub role: Role,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// An ordinary attack (ATKP Type 1).
    Damage,
    /// A guard (ATKP Type 3 barrier): clashes, never tests bodies.
    Guard,
}

/// A direct heal (`attack_strike`): ATKP id, sub-table and whom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Strike {
    pub atkp: u16,
    pub sub: u8,
    /// Party slot (0 = Sora).
    pub party: usize,
}

/// One tick's output of a spell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    pub volumes: Vec<Volume>,
    pub strikes: Vec<Strike>,
    /// Ticks of invulnerability to give the caster (Leaf Bracer).
    pub invulnerable: Option<f32>,
    /// Knock-slide of the caster away from its facing (Blizzard's
    /// recoil, `obj_hitback`): strength, applied once.
    pub recoil: Option<f32>,
}

/// What a spell reads each tick.
pub struct Context<'a> {
    pub dt: f32,
    /// The caster's origin (feet) and facing unit vector in XZ.
    pub caster: Vec3,
    pub facing: [f32; 2],
    /// The caster's TARGET shape centre (blizzard start).
    pub caster_centre: Vec3,
    pub airborne: bool,
    /// The caster is still in the cast motion (spells wait on it).
    pub casting: bool,
    /// The locked target's TARGET centre, if any.
    pub target: Option<Vec3>,
    /// Party members' origins (Sora first).
    pub party: &'a [Vec3],
    /// Ground below a point within `max` (BG ray), if any.
    pub ground: &'a dyn Fn(Vec3, f32) -> Option<Vec3>,
    /// A volume path ended on the background.
    pub bg_hit: &'a dyn Fn(Vec3, f32) -> bool,
}

/// Feedback from the previous tick's collisions.
#[derive(Clone, Debug, Default)]
pub struct Feedback {
    /// Volume keys that touched a body.
    pub hit: Vec<u64>,
    /// Clash counts of guard volumes by key (obj+0xb0).
    pub reflected: Vec<(u64, u32)>,
}

impl Feedback {
    fn was_hit(&self, key: u64) -> bool {
        self.hit.contains(&key)
    }
    fn reflects(&self, key: u64) -> u32 {
        self.reflected
            .iter()
            .find(|r| r.0 == key)
            .map_or(0, |r| r.1)
    }
}

fn len2(v: [f32; 2]) -> f32 {
    v[0].hypot(v[1])
}

fn norm(v: Vec3) -> Vec3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0. {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        v
    }
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: Vec3, k: f32) -> Vec3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn rot_y(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
}

/// Fire's ring centre above the caster's origin by level (the PAX element
/// 2 / 5 offsets, Y-up): ground 92.88 / 86.33 / 72.0, air 116.20 / 115.89 /
/// 95.74.
fn fire_lift(level: u8, air: bool) -> f32 {
    let t = if air {
        [116.20, 115.89, 95.74]
    } else {
        [92.88, 86.33, 72.0]
    };
    t[(level.clamp(1, 3) - 1) as usize]
}

/// A running spell.
#[derive(Clone, Debug, PartialEq)]
pub struct Spell {
    serial: u64,
    magic_type: u8,
    level: u8,
    idx: usize,
    row: Row,
    form: u8,
    age: f32,
    airborne: bool,
    state: State,
    finished: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Fire {
        left: f32,
        rings: u8,
    },
    Blizzard {
        shards: Vec<Shard>,
        released: bool,
    },
    Thunder {
        bolts: Vec<Bolt>,
        queue: Vec<(f32, Vec3, f32)>,
    },
    Cure {
        done: bool,
        leaf: f32,
        leaf_centre: Vec3,
    },
    Magnet {
        centre: Vec3,
        life: f32,
        placed: bool,
    },
    Reflect {
        barrier: f32,
        centre_offset: Vec3,
        count: u32,
        seen: u32,
        counter: Option<Counter>,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct Shard {
    key: u64,
    pos: Vec3,
    dir: Vec3,
    life: f32,
    fade: f32,
    pierce: bool,
    rate: f32,
    radius: f32,
    speed: f32,
    previous: Option<Vec3>,
    dead: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Bolt {
    key: u64,
    pos: Vec3,
    column: Vec3,
    atkp: u16,
    wait: f32,
    live: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct Counter {
    stage: u8,
    left: f32,
    tier: u8,
    centre: Vec3,
}

impl Spell {
    /// Releases a spell: trigger 21 of the program with the `magi` row.
    /// `ctx` is the state at the release tick.
    pub fn new(
        serial: u64,
        magic_type: u8,
        level: u8,
        idx: usize,
        row: Row,
        form: u8,
        ctx: &Context,
    ) -> Spell {
        let (airborne, wisdom, master, fin) =
            (ctx.airborne, idx / 10 == 1, idx / 10 == 2, idx / 10 == 3);
        let odd = idx & 1 == 1;
        let state = match magic_type {
            FIRE => {
                // Duration f2; the HD patch turns 44 into 42 at 60 fps.
                let mut left = row.f(2);
                if left == 44. && ctx.dt < 2. {
                    left = 42.;
                }
                let _ = (wisdom, master, odd, fin);
                State::Fire {
                    left,
                    rings: if fin { 2 } else { 1 },
                }
            }
            BLIZZARD => State::Blizzard {
                shards: Vec::new(),
                released: false,
            },
            THUNDER => State::Thunder {
                bolts: Vec::new(),
                queue: Vec::new(),
            },
            CURE => State::Cure {
                done: false,
                leaf: 0.,
                leaf_centre: ctx.caster_centre,
            },
            MAGNET => State::Magnet {
                centre: ctx.caster,
                life: 16. + row.f(16) + row.f(17),
                placed: false,
            },
            _ => {
                // Barrier centre offsets (Y-up), rotated by the facing at
                // release: ground (−22, 85, −18.2), air (0, 108.9, 0),
                // Wisdom/Final (0, 90, −12.4).
                let off = if airborne {
                    [0., 108.9, 0.]
                } else if wisdom || fin {
                    [0., 90., -12.4]
                } else {
                    [-22., 85., -18.2]
                };
                State::Reflect {
                    barrier: BARRIER_LIFE,
                    centre_offset: off,
                    count: 0,
                    seen: 0,
                    counter: None,
                }
            }
        };
        Spell {
            serial,
            magic_type,
            level,
            idx,
            row,
            form,
            age: 0.,
            airborne,
            state,
            finished: false,
        }
    }

    /// Leaf Bracer (TR20 of Cure on oneself, ability 114): 60 ticks of
    /// invulnerability and an ATKP 1247 super guard (r 100) at the caster's
    /// TARGET centre, from the cast's start.
    pub fn leaf_bracer(serial: u64, ctx: &Context) -> Spell {
        Spell {
            serial,
            magic_type: CURE,
            level: 1,
            idx: 0,
            row: Row::default(),
            form: 0,
            age: 0.,
            airborne: false,
            state: State::Cure {
                done: true,
                leaf: 60.,
                leaf_centre: ctx.caster_centre,
            },
            finished: false,
        }
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    pub fn magic_type(&self) -> u8 {
        self.magic_type
    }

    fn key(&self, slot: u8) -> u64 {
        self.serial << 8 | slot as u64
    }

    /// One tick of the spell (the script's `halt` interval).
    pub fn tick(&mut self, ctx: &Context, fb: &Feedback) -> Output {
        let mut out = Output::default();
        let dt = ctx.dt;
        let key0 = self.key(0);
        let idx = self.idx;
        let row = self.row;
        let sub = idx as u8;
        let facing = [ctx.facing[0], 0., ctx.facing[1]];
        let first = self.age == 0.;
        match &mut self.state {
            State::Fire { .. } if first => {
                // TR21: the release flash and ring effect, then `halt`.
            }
            State::Fire { left, rings } => {
                let lift = fire_lift(self.level, self.airborne);
                let centre = [ctx.caster[0], ctx.caster[1] + lift, ctx.caster[2]];
                let (r, h) = (row.f(0), row.f(1));
                if *left > 0. {
                    out.volumes.push(Volume {
                        key: key0,
                        atkp: 502,
                        sub,
                        centre,
                        radius: if idx % 10 == 1 && idx / 10 == 3 {
                            450.
                        } else {
                            r
                        },
                        height: h,
                        previous: None,
                        moving: None,
                        role: Role::Damage,
                    });
                    if *rings > 1 {
                        out.volumes.push(Volume {
                            key: key0 + 1,
                            atkp: 502,
                            sub,
                            centre,
                            radius: r - 30.,
                            height: h - 30.,
                            previous: None,
                            moving: None,
                            role: Role::Damage,
                        });
                    }
                    *left -= dt;
                } else {
                    self.finished = true;
                }
            }
            State::Blizzard { shards, released } => {
                // TR21 releases the cast effect, `halt`s once, then recoils
                // the caster and starts the shards (section 4.2).
                if self.age >= 1. && !*released {
                    *released = true;
                    out.recoil = Some(row.f(7));
                    let d = norm(facing);
                    let wis = idx / 10 == 1;
                    let master = idx / 10 == 2;
                    let fin = idx / 10 == 3;
                    let odd = idx & 1 == 1;
                    // (direction, pierces, homing factor)
                    let mut dirs: Vec<(Vec3, bool, f32)> = Vec::new();
                    if wis && odd {
                        dirs.push((d, false, 0.5));
                        for (a, k) in [(0.610865, 1.0), (1.22173, 2.0)] {
                            dirs.push((rot_y(d, a), false, k));
                            dirs.push((rot_y(d, -a), false, k));
                        }
                    } else if master {
                        dirs.push((d, false, 1.0));
                        for y in [-0.96, 0.96] {
                            dirs.push(([d[0], y, d[2]], false, 1.0));
                        }
                        for y in [0.4, -0.4] {
                            let v = [d[0], y, d[2]];
                            dirs.push((rot_y(v, std::f32::consts::FRAC_PI_4), false, 1.0));
                            dirs.push((rot_y(v, -std::f32::consts::FRAC_PI_4), false, 1.0));
                        }
                    } else {
                        dirs.push((d, true, 1.0));
                        // Final: even rows twin; odd rows by the cast
                        // counter S[44], which is not kept here.
                        if fin && !odd {
                            dirs.push((d, true, 1.0));
                        }
                    }
                    // The shards start at the caster's TARGET shape centre
                    // (PAX element 1, bone 0x4003 [I: the type 3 entry]).
                    for (i, (dir, pierce, k)) in dirs.into_iter().enumerate() {
                        shards.push(Shard {
                            key: key0 + 1 + i as u64,
                            pos: ctx.caster_centre,
                            dir: norm(dir),
                            life: row.f(4) - 12.,
                            fade: 12.,
                            pierce,
                            rate: row.f(3) * k,
                            radius: row.f(6),
                            speed: row.f(5),
                            previous: None,
                            dead: false,
                        });
                    }
                }
                for s in shards.iter_mut() {
                    if s.dead {
                        continue;
                    }
                    // Hit / reflect / background end the shard before it
                    // moves again (loop head).
                    if fb.was_hit(s.key) && !s.pierce || fb.reflects(s.key) > 0 {
                        s.dead = true;
                        continue;
                    }
                    if (ctx.bg_hit)(s.pos, s.radius) {
                        s.dead = true;
                        continue;
                    }
                    out.volumes.push(Volume {
                        key: s.key,
                        atkp: 248,
                        sub,
                        centre: s.pos,
                        radius: s.radius,
                        height: s.radius,
                        previous: s.previous,
                        moving: if s.previous.is_some() {
                            Some(s.dir)
                        } else {
                            None
                        },
                        role: Role::Damage,
                    });
                    s.previous = Some(s.pos);
                    // vector_homing toward the target, then advance.
                    if let Some(t) = ctx.target
                        && s.rate > 0.
                    {
                        let want = norm([t[0] - s.pos[0], t[1] - s.pos[1], t[2] - s.pos[2]]);
                        let dot = (s.dir[0] * want[0] + s.dir[1] * want[1] + s.dir[2] * want[2])
                            .clamp(-1., 1.);
                        let angle = dot.acos();
                        let step = (s.rate * dt).min(angle);
                        if angle > 1e-6 {
                            let k = step / angle;
                            let v = [
                                s.dir[0] + (want[0] - s.dir[0]) * k,
                                s.dir[1] + (want[1] - s.dir[1]) * k,
                                s.dir[2] + (want[2] - s.dir[2]) * k,
                            ];
                            s.dir = norm(v);
                        }
                    }
                    s.pos = add(s.pos, scale(s.dir, s.speed * dt));
                    if s.life > 0. {
                        s.life -= dt;
                    } else {
                        s.fade -= dt;
                        if s.fade <= 0. {
                            s.dead = true;
                        }
                    }
                }
                shards.retain(|s| !s.dead);
                if shards.is_empty() && *released {
                    self.finished = true;
                }
            }
            State::Thunder { bolts, queue } => {
                if first {
                    let wis = idx / 10 == 1;
                    let master = idx / 10 == 2;
                    let fin = idx / 10 == 3;
                    let odd = idx & 1 == 1;
                    let origin = ctx.caster;
                    let dir = norm(facing);
                    let (p, raise) = match ctx.target {
                        Some(t) => (t, 100.),
                        None => {
                            let d = if master {
                                scale(rot_y(dir, std::f32::consts::FRAC_PI_4), 800.)
                            } else {
                                scale(dir, 300.)
                            };
                            let mut p = add(origin, d);
                            p[1] += 80.;
                            (p, 330.)
                        }
                    };
                    let mut at = 0.;
                    if master {
                        for t in [0., 0.47, 0.78] {
                            let q = [
                                origin[0] + (p[0] - origin[0]) * t,
                                origin[1] + (p[1] - origin[1]) * t,
                                origin[2] + (p[2] - origin[2]) * t,
                            ];
                            queue.push((at, q, raise));
                            at += 16.;
                        }
                    }
                    queue.push((at, p, raise));
                    if fin {
                        let r = 2.5 * row.f(9);
                        let n = if odd { 8 } else { 3 };
                        for k in 1..n {
                            // bg_get_random_pos_air [U]: a fixed ring.
                            let a = k as f32 * std::f32::consts::TAU / n as f32;
                            let q = [p[0] + a.cos() * r * 0.7, p[1], p[2] + a.sin() * r * 0.7];
                            queue.push((at + 8. * k as f32, q, 330.));
                        }
                    }
                    if wis && odd {
                        let mut v =
                            rot_y(scale(dir, row.f(9) * 1.5), 2. * std::f32::consts::FRAC_PI_3);
                        for k in 1..=3 {
                            queue.push((at + 8. * k as f32, add(p, v), 330.));
                            v = rot_y(v, 2. * std::f32::consts::FRAC_PI_3);
                        }
                    }
                }
                // Start bolts whose time has come.
                let age = self.age;
                let mut started = Vec::new();
                queue.retain(|&(t, pos, raise)| {
                    if t <= age {
                        started.push((pos, raise));
                        false
                    } else {
                        true
                    }
                });
                for (pos, raise) in started {
                    // The strike point: ground within f8 below, else air.
                    let from = [pos[0], pos[1] + raise, pos[2]];
                    let mut at = pos;
                    let mut atkp = 251;
                    if let Some(g) = (ctx.ground)(from, 10000.)
                        && pos[1] - g[1] < row.f(8)
                    {
                        at = g;
                        atkp = 250;
                    }
                    let n = bolts.len() as u64;
                    bolts.push(Bolt {
                        key: self.serial << 8 | (1 + n * 2),
                        pos: at,
                        column: [at[0], at[1] + row.f(10), at[2]],
                        atkp,
                        wait: row.f(12),
                        live: row.f(13),
                    });
                }
                let serial = self.serial;
                for b in bolts.iter_mut() {
                    if b.wait > 0. {
                        b.wait -= dt;
                        continue;
                    }
                    if b.live > 0. {
                        out.volumes.push(Volume {
                            key: b.key,
                            atkp: b.atkp,
                            sub,
                            centre: b.pos,
                            radius: row.f(9),
                            height: row.f(9),
                            previous: None,
                            moving: None,
                            role: Role::Damage,
                        });
                        out.volumes.push(Volume {
                            key: serial << 8 | ((b.key & 0xff) + 1),
                            atkp: 249,
                            sub,
                            centre: b.column,
                            radius: row.f(11),
                            height: row.f(10),
                            previous: None,
                            moving: None,
                            role: Role::Damage,
                        });
                        b.live -= dt;
                    }
                }
                bolts.retain(|b| b.wait > 0. || b.live > 0.);
                if bolts.is_empty() && queue.is_empty() {
                    self.finished = true;
                }
            }
            State::Cure {
                done,
                leaf,
                leaf_centre,
            } => {
                if !*done {
                    *done = true;
                    // The heal centre is the target (Sora himself here).
                    let centre = ctx.party.first().copied().unwrap_or(ctx.caster);
                    for (i, p) in ctx.party.iter().enumerate() {
                        let d = [p[0] - centre[0], p[2] - centre[2]];
                        if len2(d) >= row.f(14) {
                            continue;
                        }
                        out.strikes.push(Strike {
                            atkp: if i == 0 { 708 } else { 709 },
                            sub,
                            party: i,
                        });
                    }
                }
                if *leaf > 0. {
                    if first {
                        out.invulnerable = Some(60.);
                    }
                    // ATKP 1247 super guard, r 100 at the caster's TARGET
                    // centre for 60 ticks.
                    *leaf_centre = ctx.caster_centre;
                    out.volumes.push(Volume {
                        key: key0,
                        atkp: 1247,
                        sub: 0,
                        centre: *leaf_centre,
                        radius: 100.,
                        height: 100.,
                        previous: None,
                        moving: None,
                        role: Role::Guard,
                    });
                    *leaf -= dt;
                }
                if *leaf <= 0. && *done {
                    self.finished = true;
                }
            }
            State::Magnet {
                centre,
                life,
                placed,
            } => {
                if !*placed {
                    *placed = true;
                    // P: the target, else the caster; lowered to
                    // f15 above the ground below it.
                    let mut p = ctx.target.unwrap_or(ctx.caster);
                    let f15 = row.f(15);
                    let q = [p[0], p[1] + f15, p[2]];
                    if let Some(g) = (ctx.ground)(q, 2. * f15) {
                        p = [g[0], g[1] + f15, g[2]];
                    }
                    *centre = p;
                }
                if *life > 0. {
                    out.volumes.push(Volume {
                        key: key0,
                        atkp: 658,
                        sub,
                        centre: *centre,
                        radius: row.f(19),
                        height: row.f(19),
                        previous: None,
                        moving: None,
                        role: Role::Damage,
                    });
                    if self.form != 0 && (idx / 10 >= 1) {
                        out.volumes.push(Volume {
                            key: key0 + 1,
                            atkp: 976,
                            sub,
                            centre: *centre,
                            radius: row.f(15) * 0.5,
                            height: row.f(15) * 0.5,
                            previous: None,
                            moving: None,
                            role: Role::Damage,
                        });
                    }
                    *life -= dt;
                } else {
                    self.finished = true;
                }
            }
            State::Reflect {
                barrier,
                centre_offset,
                count,
                seen,
                counter,
            } => {
                if let Some(c) = counter {
                    // Counter phases A then B (§4.6).
                    let (a, b) = match c.tier {
                        1 => (
                            (574u16, row.f(23), row.f(24)),
                            (667u16, row.f(23) + 20., row.f(24) + 20.),
                        ),
                        2 => (
                            (576, row.f(26), row.f(27)),
                            (668, row.f(26) + 20., row.f(27) + 20.),
                        ),
                        _ => (
                            (578, row.f(29), row.f(30)),
                            (669, row.f(29) + 20., row.f(30) + 20.),
                        ),
                    };
                    let (atkp, r, h) = if c.stage == 0 { a } else { b };
                    out.volumes.push(Volume {
                        key: key0 + 10 + c.stage as u64,
                        atkp,
                        sub,
                        centre: c.centre,
                        radius: r,
                        height: h,
                        previous: None,
                        moving: None,
                        role: Role::Damage,
                    });
                    c.left -= dt;
                    if c.left <= 0. {
                        if c.stage == 0 {
                            c.stage = 1;
                            c.left = row.f(22);
                        } else {
                            self.finished = true;
                        }
                    }
                } else {
                    let centre = {
                        let o = *centre_offset;
                        let f = ctx.facing;
                        // The offset rotates with the caster's facing.
                        [
                            ctx.caster[0] + o[0] * f[1] + o[2] * f[0],
                            ctx.caster[1] + o[1],
                            ctx.caster[2] - o[0] * f[0] + o[2] * f[1],
                        ]
                    };
                    let key = key0;
                    let hits = fb.reflects(key);
                    if hits > *seen {
                        *seen = hits;
                    }
                    *count = *seen;
                    if *barrier > 0. {
                        out.volumes.push(Volume {
                            key,
                            atkp: 573,
                            sub,
                            centre,
                            radius: row.f(20),
                            height: row.f(20),
                            previous: None,
                            moving: None,
                            role: Role::Guard,
                        });
                        *barrier -= dt;
                    } else if *count == 0 {
                        self.finished = true;
                    } else {
                        let n = *count as i32;
                        let tier = if n >= row.i(28) {
                            3
                        } else if n >= row.i(25) {
                            2
                        } else {
                            1
                        };
                        *counter = Some(Counter {
                            stage: 0,
                            left: row.f(21),
                            tier,
                            centre,
                        });
                    }
                }
            }
        }
        self.age += dt;
        out
    }
}

/// [I] The barrier's PAX lifetime was not decoded: element 0's PDATA ends
/// by VM frame 27 (≈ 54 ticks at the 30 Hz VM).
pub const BARRIER_LIFE: f32 = 54.;

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(t: u8, level: u8) -> Record {
        Record {
            magic_type: t,
            level,
            world: 0,
            file: String::new(),
            item: 0x15 + t as u16,
            command: 0x31 + level as u16,
            ground: (56, 8),
            finisher: (57, 8),
            air: (58, 8),
            voice: 7,
            voice_finisher: 11,
            voice_self: -1,
        }
    }

    fn row(vals: &[(usize, f32)]) -> Row {
        let mut w = [0u32; 31];
        for &(i, v) in vals {
            w[i] = v.to_bits();
        }
        Row(w)
    }

    fn flat_ctx<'a>(
        party: &'a [Vec3],
        ground: &'a dyn Fn(Vec3, f32) -> Option<Vec3>,
        bg: &'a dyn Fn(Vec3, f32) -> bool,
    ) -> Context<'a> {
        Context {
            dt: 1.,
            caster: [0., 0., 0.],
            facing: [0., 1.],
            caster_centre: [0., 70., 0.],
            airborne: false,
            casting: true,
            target: None,
            party,
            ground,
            bg_hit: bg,
        }
    }

    fn floor(from: Vec3, max: f32) -> Option<Vec3> {
        (from[1] >= 0. && from[1] <= max + 1.).then_some([from[0], 0., from[2]])
    }

    #[test]
    fn variant_index_and_start_follow_the_cast_rules() {
        let r = rec(FIRE, 2);
        assert_eq!(variant_index(2, false, 0), 2);
        assert_eq!(variant_index(2, true, 0), 3);
        assert_eq!(variant_index(1, true, 2), 11);
        assert_eq!(variant_index(3, false, 5), 34);
        let s = start(&r, true, false, 0, false);
        assert_eq!((s.motion, s.voice, s.row, s.finisher), (57, 11, 3, true));
        // Air casts never use the finisher motion or the odd row.
        let a = start(&r, true, true, 0, false);
        assert_eq!(
            (a.motion, a.row, a.finisher, a.airborne),
            (58, 2, false, true)
        );
        // A spell without a finisher motion falls back to the ground one.
        let mut cure = rec(CURE, 1);
        cure.finisher = (-1, 0);
        let c = start(&cure, true, false, 0, false);
        assert_eq!((c.motion, c.row), (56, 0));
    }

    #[test]
    fn current_record_follows_the_owned_count() {
        let records: Vec<Record> = (1..=3).map(|l| rec(FIRE, l)).collect();
        assert_eq!(Record::current(&records, FIRE, 0, 0), None);
        assert_eq!(Record::current(&records, FIRE, 2, 0).unwrap().level, 2);
        assert_eq!(Record::current(&records, FIRE, 9, 0).unwrap().level, 3);
        assert!(Record::current(&records, THUNDER, 1, 0).is_none());
    }

    #[test]
    fn mp_charge_starts_only_below_zero_and_ends_with_a_full_bar() {
        let mut mp = Mp::new(100);
        assert_eq!(mp.change(-12, CHARGE_LENGTH), 88);
        assert!(!mp.charging());
        mp.cur = 10;
        assert_eq!(mp.change(-12, CHARGE_LENGTH), 0);
        assert!(mp.charging());
        assert_eq!((mp.remaining, mp.total), (3000., 3000.));
        // Gains shorten the charge by 12 per point.
        mp.add(10);
        assert_eq!(mp.remaining, 3000. - 120.);
        // Haste doubles the run-down.
        mp.tick(1., 2.);
        assert_eq!(mp.remaining, 3000. - 120. - 2.);
        mp.tick(5000., 1.);
        assert!(!mp.charging());
        assert_eq!(mp.cur, 100);
    }

    #[test]
    fn colour_marks_charging_and_orange_casts() {
        let mut mp = Mp::new(100);
        assert_eq!(colour(12, &mp, false, false), 0);
        mp.cur = 5;
        assert_eq!(colour(12, &mp, false, false), 2);
        assert_eq!(colour(12, &mp, true, false), 5);
        // Cure always starts a charge, whatever the toggle says.
        assert_eq!(colour(0xff, &mp, true, false), 2);
        mp.change(-12, CHARGE_LENGTH);
        assert_eq!(colour(12, &mp, false, false), 5);
        assert_eq!(colour(12, &Mp::new(100), false, true), 5);
    }

    #[test]
    fn fire_ring_follows_the_caster_for_the_patched_duration() {
        let party = [[0., 0., 0.]];
        let r = row(&[(0, 200.), (1, 80.), (2, 44.)]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let mut ctx = flat_ctx(&party, &ground, &bg);
        let mut s = Spell::new(1, FIRE, 1, 0, r, 0, &ctx);
        let mut ticks = 0;
        let mut volumes = 0;
        while !s.finished() {
            ctx.caster = [ticks as f32, 0., 0.];
            let o = s.tick(&ctx, &Feedback::default());
            if let Some(v) = o.volumes.first() {
                volumes += 1;
                assert_eq!(v.atkp, 502);
                assert_eq!(v.centre, [ticks as f32, 92.88, 0.]);
                assert_eq!((v.radius, v.height), (200., 80.));
            }
            ticks += 1;
            assert!(ticks < 100);
        }
        // One `halt`, then 42 ticks of volume (the 60 fps patch).
        assert_eq!(volumes, 42);
        assert_eq!(ticks, 44);
    }

    #[test]
    fn blizzard_shard_flies_straight_pierces_and_fades() {
        let party = [[0., 0., 0.]];
        let r = row(&[(4, 30.), (5, 40.), (6, 80.), (7, 50.)]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let ctx = flat_ctx(&party, &ground, &bg);
        let mut s = Spell::new(2, BLIZZARD, 1, 0, r, 0, &ctx);
        let mut fb = Feedback::default();
        let mut last = None;
        let mut n = 0;
        while !s.finished() {
            let o = s.tick(&ctx, &fb);
            if let Some(v) = o.volumes.first() {
                last = Some(*v);
                fb.hit = vec![v.key];
            }
            n += 1;
            assert!(n < 100);
        }
        let v = last.unwrap();
        // Pierces: still alive after hits; moved 40 per tick along +Z.
        assert_eq!(v.atkp, 248);
        assert!(v.centre[2] > 40. * 10.);
        assert_eq!(v.centre[1], 70.);
        // Life 30 − 12 then 12 of fade: 30 ticks in all.
        assert!((29..=31).contains(&n));
    }

    #[test]
    fn thunder_strikes_the_ground_under_the_target_after_the_delay() {
        let party = [[0., 0., 0.]];
        let r = row(&[
            (8, 150.),
            (9, 170.),
            (10, 340.),
            (11, 30.),
            (12, 6.),
            (13, 10.),
        ]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let mut ctx = flat_ctx(&party, &ground, &bg);
        ctx.target = Some([100., 50., 200.]);
        let mut s = Spell::new(3, THUNDER, 1, 0, r, 0, &ctx);
        let fb = Feedback::default();
        let mut first_hit = None;
        let mut count = 0;
        for t in 0..40 {
            let o = s.tick(&ctx, &fb);
            if !o.volumes.is_empty() {
                first_hit.get_or_insert(t);
                count += 1;
                let strike = o.volumes.iter().find(|v| v.atkp == 250).unwrap();
                assert_eq!(strike.centre, [100., 0., 200.]);
                let col = o.volumes.iter().find(|v| v.atkp == 249).unwrap();
                assert_eq!((col.radius, col.height, col.centre[1]), (30., 340., 340.));
            }
            if s.finished() {
                break;
            }
        }
        assert_eq!(first_hit, Some(6));
        assert_eq!(count, 10);
        // Without a target: 300 ahead, raised 80, so the ground is within
        // 150 below and the strike is the ground one at y 0.
        let mut ctx2 = flat_ctx(&party, &ground, &bg);
        ctx2.target = None;
        let mut s = Spell::new(4, THUNDER, 1, 0, r, 0, &ctx2);
        let mut seen = None;
        for _ in 0..20 {
            let o = s.tick(&ctx2, &fb);
            if let Some(v) = o.volumes.first() {
                seen = Some(*v);
                break;
            }
        }
        let v = seen.unwrap();
        assert_eq!((v.atkp, v.centre), (250, [0., 0., 300.]));
    }

    #[test]
    fn cure_heals_the_party_in_range_and_leaf_bracer_guards() {
        let party = [[0., 0., 0.], [100., 0., 0.], [900., 0., 0.]];
        let r = row(&[(14, 280.)]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let ctx = flat_ctx(&party, &ground, &bg);
        let mut s = Spell::new(5, CURE, 1, 0, r, 0, &ctx);
        let o = s.tick(&ctx, &Feedback::default());
        assert_eq!(
            o.strikes,
            vec![
                Strike {
                    atkp: 708,
                    sub: 0,
                    party: 0
                },
                Strike {
                    atkp: 709,
                    sub: 0,
                    party: 1
                },
            ]
        );
        assert!(o.volumes.is_empty() && s.finished());
        // Leaf Bracer is its own thread from the cast's start.
        let mut leaf = Spell::leaf_bracer(6, &ctx);
        let o = leaf.tick(&ctx, &Feedback::default());
        assert_eq!(o.invulnerable, Some(60.));
        assert_eq!((o.volumes[0].atkp, o.volumes[0].role), (1247, Role::Guard));
        let mut n = 1;
        while !leaf.finished() {
            leaf.tick(&ctx, &Feedback::default());
            n += 1;
        }
        assert_eq!(n, 60);
    }

    #[test]
    fn magnet_sits_above_the_ground_and_lives_167_ticks() {
        let party = [[0., 0., 0.]];
        let r = row(&[(15, 180.), (16, 16.), (17, 135.), (19, 250.)]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let mut ctx = flat_ctx(&party, &ground, &bg);
        ctx.target = Some([40., 30., 60.]);
        let mut s = Spell::new(6, MAGNET, 1, 0, r, 0, &ctx);
        let first = s.tick(&ctx, &Feedback::default());
        assert_eq!(first.volumes[0].centre, [40., 180., 60.]);
        assert_eq!(first.volumes[0].radius, 250.);
        let mut n = 1;
        while !s.finished() {
            s.tick(&ctx, &Feedback::default());
            n += 1;
        }
        assert_eq!(n, 168);
    }

    #[test]
    fn reflect_barrier_counts_clashes_and_counters_by_tier() {
        let party = [[0., 0., 0.]];
        let mut w = [0u32; 31];
        let set = |w: &mut [u32; 31], i: usize, v: f32| w[i] = v.to_bits();
        set(&mut w, 20, 110.);
        set(&mut w, 21, 28.);
        set(&mut w, 22, 6.);
        set(&mut w, 23, 240.);
        set(&mut w, 24, 150.);
        w[25] = 5;
        set(&mut w, 26, 300.);
        set(&mut w, 27, 180.);
        w[28] = 10;
        set(&mut w, 29, 380.);
        set(&mut w, 30, 220.);
        let r = Row(w);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let ctx = flat_ctx(&party, &ground, &bg);
        let mut s = Spell::new(7, REFLECT, 1, 0, r, 0, &ctx);
        let key = 7u64 << 8;
        let mut fb = Feedback::default();
        let mut guard_seen = false;
        let mut counters = Vec::new();
        for t in 0..200 {
            if t == 10 {
                fb.reflected = vec![(key, 6)];
            }
            let o = s.tick(&ctx, &fb);
            for v in &o.volumes {
                if v.atkp == 573 {
                    guard_seen = true;
                    assert_eq!(v.role, Role::Guard);
                    assert_eq!(v.centre, [-22. * 1. + 0., 85., -18.2]);
                } else {
                    counters.push((v.atkp, v.radius));
                }
            }
            if s.finished() {
                break;
            }
        }
        assert!(guard_seen);
        // Six reflects: tier 2 (≥ 5): ATKP 576 for 28 ticks, then 668.
        assert_eq!(counters.iter().filter(|c| c.0 == 576).count(), 28);
        assert_eq!(counters.iter().filter(|c| c.0 == 668).count(), 6);
        assert_eq!(counters[0].1, 300.);
        assert!(s.finished());
    }
}
