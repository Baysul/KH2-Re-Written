//! Field townspeople (objentry type 15, YS::NPC) of a loaded area.
//!
//! Spawning follows FUN_1403fe590 / FUN_1404165e0 / FUN_1404168c0: the
//! entity's engine yaw is wrap(π − rotY), the position is used as is, and an
//! NPC with a walk path teleports to a random point on it unless the
//! "nearest" flag is set (FUN_14054e130). Behaviour is the rtn_default
//! program's `path_move` / `static_idle` (npc-01 §2.4): the walk motion (1)
//! and the turn-in-place motion (129) switch on the heading error, the
//! heading turns at 10° per tick toward the curve point ~5 units ahead, and
//! the speed is the walk motion's root travel per tick.
//!
//! Not ported yet: the interrupt system (zone actions, NPC meets, the
//! encounter reaction), look-at, NPC push and the other AI programs.
//! Positions here are scene coordinates (Y up); engine yaw Y maps to scene
//! yaw π − Y.
use area_sim::zone::Zone;
use bevy::prelude::*;
use package_reader::{
    ard::{Entity as ArdEntity, WalkPath},
    model_inspection::{self, InspectionModel},
    motion,
    named::Library,
    object_collision,
    objentry::ObjectEntry,
    probe::ProbeSource,
};
use std::collections::HashMap;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// One animation of a model: per-tick vertex positions (root removed), the
/// loop start and the bone matrix of the talk-camera bone.
pub struct Anim {
    pub frames: Vec<Vec<Vec3>>,
    pub loop_start: u32,
    pub bones: Vec<Option<Mat4>>,
    /// Horizontal root travel per tick over one pass of the motion.
    pub travel: f32,
}

pub struct TargetShape {
    pub bone: usize,
    pub offset: Vec3,
    pub pos_h: f32,
    pub radius: f32,
    pub height: f32,
}

/// The motions the path program uses.
pub struct Anims {
    pub idle: Anim,
    pub walk: Option<Anim>,
    pub turn: Option<Anim>,
    /// Other slots: the talk gesture (50) and zone actions (283..=292).
    pub extra: HashMap<u16, Anim>,
    /// look_can_reach: the type-20 collision shape's yaw limit (radius in
    /// degrees), else 15° [npc-01 §5].
    pub look_limit: f32,
}

/// A model with its motions.
pub struct NpcKind {
    pub name: String,
    pub model: InspectionModel,
    pub anims: Anims,
    pub target: Option<TargetShape>,
}

/// Motion slots of an RTN set: 0 R00A idle, 1 R10A walk, 129 R50A turn in
/// place (npc-01 §2.4).
/// Object-collision kind of the talk camera shape.
const CAMERA_SHAPE: u8 = 19;
/// Object-collision kind of the head look-at shape (radius = yaw limit).
const LOOK_SHAPE: u8 = 20;
const SLOT_IDLE: usize = 0;
const SLOT_WALK: usize = 1;
const SLOT_TURN: usize = 129;

fn build_anim(
    model: &InspectionModel,
    selected: &motion::SelectedMotion,
    bone: Option<usize>,
) -> Result<Anim> {
    let (frame_start, frame_end, fps, frame_return) = selected.motion.frame_window()?;
    if !(1. ..=60.).contains(&fps) || !(0. ..frame_end).contains(&frame_start) {
        return Err("NPC motion frame window outside profile".into());
    }
    let scale = 60. / fps;
    let start = (frame_start * scale).round() as u32;
    let end = (frame_end * scale).round() as u32 - start;
    let loop_start = ((frame_return * scale).round() as u32).saturating_sub(start);
    let (mut frames, mut bones) = (Vec::new(), Vec::new());
    for offset in 0..=end {
        let (pose, evaluated) = model
            .skinning
            // One 60 Hz tick advances 30/60 key-time units at 30 fps
            // (R10A: 30 key units over 60 ticks, npc-01 motion table).
            .sample_evaluated(&selected.motion, frame_start + offset as f32 / scale)?;
        let r = Vec3::from_array(evaluated.root_translation);
        frames.push(
            pose.position_bits
                .iter()
                .map(|p| Vec3::from_array(p.map(f32::from_bits)) - r)
                .collect(),
        );
        bones.push(bone.and_then(|b| pose.matrix_bits.get(b)).map(|m| {
            let mut m = Mat4::from_cols_array(&m.map(f32::from_bits));
            m.w_axis -= r.extend(0.);
            m
        }));
    }
    // Root motion per tick: the motion root record's translation (+0x20,
    // the displacement of one pass) over the motion's length in ticks
    // (header word 1) [npc-01 §3.7].
    let root = &selected.motion.root.bytes;
    let word = |at: usize| {
        root.get(at..at + 4)
            .map_or(0., |b| f32::from_le_bytes(b.try_into().unwrap()))
    };
    let ticks = selected
        .motion
        .interpolated_header_words
        .get(1)
        .map_or(0, |&t| t as i32)
        .max(1);
    let travel = word(0x20).hypot(word(0x28)) / ticks as f32;
    Ok(Anim {
        frames,
        loop_start,
        bones,
        travel,
    })
}

impl NpcKind {
    pub fn load(source: &ProbeSource, library: &mut Library, object: &ObjectEntry) -> Result<Self> {
        let model_name = String::from_utf8_lossy(&object.model_name).into_owned();
        let set_name = String::from_utf8_lossy(&object.animation_name).into_owned();
        let found = library
            .find(&format!("obj/{model_name}.mdlx"))
            .ok_or_else(|| format!("obj/{model_name}.mdlx is not installed"))?;
        let mut models =
            model_inspection::read_model_at(source, found.index, found.ordinal, found.name_hash)?;
        if models.is_empty() {
            return Err("NPC model has no mesh".into());
        }
        let model = models.remove(0);
        let set = library
            .find(&format!(
                "obj/{}",
                if set_name.ends_with(".mset") {
                    set_name.clone()
                } else {
                    format!("{set_name}.mset")
                }
            ))
            .ok_or_else(|| format!("{set_name} motion set is not installed"))?;
        let slot = |s: usize| -> Option<motion::SelectedMotion> {
            motion::read_slots(source, set.index, set.ordinal, set.name_hash, &[s])
                .ok()
                .and_then(|mut v| (!v.is_empty()).then(|| v.remove(0)))
        };
        let idle = slot(SLOT_IDLE).ok_or("NPC set has no idle motion")?;
        // The talk camera's TARGET shape (FUN_1403be230 fallback) comes
        // from the model archive's object collision (BAR type 23).
        let archive = library.read_found(&found)?;
        let table = package_reader::bar::read_bar(&archive)?;
        let collision = table
            .entries
            .iter()
            .find(|e| e.entry_type == 23 && e.size > 0)
            .map(|e| {
                object_collision::read(
                    &archive[e.offset as usize..e.offset as usize + e.size as usize],
                )
            })
            .transpose()?
            .unwrap_or_default();
        // Camera shape: the type-19 entry, else the type-3 TARGET one
        // (FUN_1403be230 / FUN_1403be0f0, dialogue-01 §6).
        let target = collision
            .iter()
            .find(|e| e.kind == CAMERA_SHAPE && e.bone >= 0)
            .or_else(|| {
                collision
                    .iter()
                    .find(|e| e.kind == object_collision::TARGET && e.bone >= 0)
            })
            .map(|t| TargetShape {
                bone: t.bone as usize,
                offset: Vec3::new(
                    t.position[0] as f32,
                    t.position[1] as f32,
                    t.position[2] as f32,
                ),
                pos_h: t.position_height as f32,
                radius: t.radius as f32,
                height: t.height as f32,
            });
        let bone = target.as_ref().map(|t| t.bone);
        let mut extra = HashMap::new();
        for s in std::iter::once(50u16).chain(283..=292) {
            if let Some(m) = slot(s as usize)
                && let Ok(a) = build_anim(&model, &m, bone)
            {
                extra.insert(s, a);
            }
        }
        let look_limit = collision
            .iter()
            .find(|e| e.kind == LOOK_SHAPE)
            .map_or(0.261_799_4, |e| (e.radius as f32).to_radians());
        Ok(Self {
            name: model_name,
            anims: Anims {
                idle: build_anim(&model, &idle, bone)?,
                walk: slot(SLOT_WALK).and_then(|m| build_anim(&model, &m, bone).ok()),
                turn: slot(SLOT_TURN).and_then(|m| build_anim(&model, &m, bone).ok()),
                extra,
                look_limit,
            },
            model,
            target,
        })
    }
}

/// A walk route: the exe's Hermite path (FUN_1405503e0 / FUN_14054ea80).
pub struct Route {
    /// Scene-space points.
    points: Vec<Vec3>,
    tangents: Vec<Vec3>,
    looped: bool,
    one_way: bool,
    /// Segment index + fraction.
    t: f32,
    /// true = increasing t.
    forward: bool,
}

/// Tangent scale: 0.5 unless the path's flag bit1 is set (FUN_14054e130).
fn tangents(points: &[Vec3], looped: bool, tension: f32) -> Vec<Vec3> {
    let n = points.len();
    if n < 3 {
        let d = points.get(1).copied().unwrap_or_default()
            - points.first().copied().unwrap_or_default();
        return vec![d; n];
    }
    // Parabola through three points: the endpoint tangent keeps the
    // magnitude of the neighbour's central difference (FUN_1405503e0).
    let end = |p0: Vec3, p1: Vec3, p2: Vec3| -> Vec3 {
        let (a, b) = ((p1 - p0).length(), (p2 - p1).length());
        let s = a / (a + b);
        let q = (p1 - p0 * ((1. - s) * (1. - s)) - p2 * (s * s)) / (2. * s * (1. - s));
        let tangent = (q - p0) * 2.;
        let v = p0 * (-2. * (1. - s)) + q * (2. - 4. * s) + p2 * (2. * s);
        tangent * (((p2 - p0) * tension).length() / v.length())
    };
    (0..n)
        .map(|i| {
            if !looped && i == 0 {
                end(points[0], points[1], points[2])
            } else if !looped && i == n - 1 {
                // Mirrored: P(n-1), P(n-2), P(n-3) with the sign of the
                // tangent flipped back.
                -end(points[n - 1], points[n - 2], points[n - 3])
            } else {
                (points[(i + 1) % n] - points[(i + n - 1) % n]) * tension
            }
        })
        .collect()
}

impl Route {
    /// FUN_14054e130. `start_nearest` is flag bit1, `one_way` bit2,
    /// `flatten` bit3. `random` supplies the start parameter and direction.
    pub fn new(
        path: &WalkPath,
        start_nearest: bool,
        one_way: bool,
        flatten: bool,
        random: &mut Lcg,
        position: Vec3,
    ) -> Option<Self> {
        if path.points.len() < 2 {
            return None;
        }
        let flags = path.header[4];
        let looped = flags & 1 != 0;
        let tension = if flags & 2 != 0 { 0. } else { 0.5 };
        let first_y = -path.points[0][1];
        let points: Vec<Vec3> = path
            .points
            .iter()
            .map(|p| Vec3::new(p[0], if flatten { first_y } else { -p[1] }, -p[2]))
            .collect();
        let tangents = tangents(&points, looped, tension);
        let mut route = Self {
            points,
            tangents,
            looped,
            one_way,
            t: 0.,
            forward: true,
        };
        if start_nearest {
            route.t = route.nearest(position, 0., route.segments() as f32);
        } else {
            let span = if looped {
                route.points.len()
            } else {
                route.points.len() - 1
            };
            route.t = random.unit() * span as f32;
            route.forward = random.below(2) != 0;
        }
        Some(route)
    }

    fn segments(&self) -> usize {
        if self.looped {
            self.points.len()
        } else {
            self.points.len() - 1
        }
    }

    /// FUN_14054ea80: the Hermite point at parameter `t`.
    pub fn at(&self, t: f32) -> Vec3 {
        let n = self.points.len();
        let i = (t.floor() as isize).clamp(0, self.segments() as isize - 1) as usize;
        let u = t - i as f32;
        let (j, k) = (i % n, (i + 1) % n);
        let (u2, u3) = (u * u, u * u * u);
        self.tangents[k] * (u3 - u2)
            + self.tangents[j] * (u3 - 2. * u2 + u)
            + self.points[k] * (3. * u2 - 2. * u3)
            + self.points[j] * (2. * u3 - 3. * u2 + 1.)
    }

    pub fn start_point(&self) -> Vec3 {
        self.at(self.t)
    }

    /// Nearest curve parameter to `p` within `[from, to]`: a coarse scan
    /// then a 33-step bisection on the distance slope, as FUN_14054f210 does
    /// per segment.
    fn nearest(&self, p: Vec3, from: f32, to: f32) -> f32 {
        let top = self.segments() as f32 - 1e-4;
        let d2 = |t: f32| self.at(t.min(top)).distance_squared(p);
        let steps = ((to - from) * 16.).ceil().max(1.) as usize;
        let mut best = (f32::MAX, from, 0usize);
        for s in 0..=steps {
            let t = from + (to - from) * s as f32 / steps as f32;
            let d = d2(t);
            if d < best.0 {
                best = (d, t, s);
            }
        }
        let width = (to - from) / steps as f32;
        let (mut lo, mut hi) = ((best.1 - width).max(from), (best.1 + width).min(to));
        for _ in 0..33 {
            let (a, b) = (lo + (hi - lo) / 3., hi - (hi - lo) / 3.);
            if d2(a) < d2(b) {
                hi = b;
            } else {
                lo = a;
            }
        }
        (lo + hi) / 2.
    }

    /// route_reverse (FUN_1405510a0): flips the walking direction unless the
    /// path is one-way.
    pub fn reverse(&mut self) {
        if !self.one_way {
            self.forward = !self.forward;
        }
    }

    /// route_dir_away_from (FUN_140550e90): walk along the path away from
    /// `from`.
    pub fn away_from(&mut self, position: Vec3, from: Vec3) {
        if self.one_way {
            return;
        }
        let d = position - from;
        if d.length() < 0.01 {
            return;
        }
        let span = self.segments() as f32;
        let lo = if self.looped {
            self.t - 1.
        } else {
            (self.t - 1.).max(0.)
        };
        let hi = if self.looped {
            self.t + 1.
        } else {
            (self.t + 1.).min(span - 1e-4)
        };
        let t = self.nearest(position, lo, hi);
        let g = self.at((t + 0.01).min(span - 1e-4)) - self.at(t.min(span - 1e-4));
        self.forward = d.dot(g) >= 0.;
    }

    /// FUN_14054f7c0 → FUN_14054f9d0: the unit direction toward the curve
    /// point more than `ahead` units from `position`, or `None` at the end
    /// of a one-way open path.
    pub fn target(&mut self, position: Vec3, ahead: f32) -> Option<Vec3> {
        let span = self.segments() as f32;
        // Project the position onto the curve near the current parameter
        // (window 10·L = 50 units ≈ a segment either side).
        let window = 1.0;
        let lo = if self.looped {
            self.t - window
        } else {
            (self.t - window).max(0.)
        };
        let hi = if self.looped {
            self.t + window
        } else {
            (self.t + window).min(span - 1e-4)
        };
        let mut projected = self.nearest(position, lo, hi);
        if self.looped {
            projected = projected.rem_euclid(span);
        }
        self.t = projected;
        // March ahead in the walking direction.
        let mut t = self.t;
        for _ in 0..40 {
            // The current segment's chord length (route +0x28).
            let n = self.points.len();
            let i = (t.floor() as isize).clamp(0, span as isize - 1) as usize;
            let chord = (self.points[(i + 1) % n] - self.points[i % n])
                .length()
                .max(1.0);
            let step = (ahead / chord) * 0.5;
            t += if self.forward { step } else { -step };
            if self.looped {
                t = t.rem_euclid(span);
            } else if t >= span {
                if self.one_way {
                    return None;
                }
                self.forward = false;
                t = span - 1e-3;
            } else if t < 0. {
                if self.one_way {
                    return None;
                }
                self.forward = true;
                t = 0.;
            }
            let p = self.at(t);
            if p.distance(position) > ahead {
                let mut d = p - position;
                d.y = 0.;
                return Some(d.normalize_or_zero());
            }
        }
        None
    }
}

/// CRT-style uniform randoms (FUN_14019bf20 / FUN_14019be90 use rand()).
pub struct Lcg(pub u32);
impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.0 >> 16) & 0x7fff
    }
    pub fn unit(&mut self) -> f32 {
        self.next() as f32 / 32768.
    }
    pub fn below(&mut self, n: u32) -> u32 {
        self.next() % n.max(1)
    }
}

/// Which motion an NPC plays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pose {
    Idle,
    Walk,
    Turn,
    /// Another slot of the set (talk 50, zone actions 283..292).
    Slot(u16),
}

/// A return-parameter record (16 bytes: zone, type, rate, entry, then
/// arg04/arg08/arg0c) of the NPC's spawn point.
#[derive(Clone, Copy, Debug)]
pub struct Action {
    pub zone: u8,
    pub kind: u8,
    pub rate: u8,
    pub entry: u8,
    /// arg04..arg0c as floats (engine coordinates for entry 3).
    pub args: [f32; 3],
}

impl Action {
    pub fn read(raw: &[u8; 16]) -> Self {
        let f = |o: usize| f32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
        Self {
            zone: raw[0],
            kind: raw[1],
            rate: raw[2],
            entry: raw[3],
            args: [f(4), f(8), f(12)],
        }
    }

    /// Motion slot of a record type (FUN_140418720): 0 and 1 idle, 2 is
    /// 0x11f, 4..13 are type + 0x117.
    pub fn motion(self) -> u16 {
        match self.kind {
            0 | 1 => 0,
            2 => 0x11f,
            k => k as u16 + 0x117,
        }
    }
}

/// A behaviour zone (kind 0) of the NPC's spawn point, in engine
/// coordinates.
#[derive(Clone)]
pub struct NpcZone {
    pub id: u16,
    pub zone: Zone,
}

/// What the interrupt record asks for (sys+0xc28 type).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IrqKind {
    None,
    /// Sora entered the reaction sphere (type 4).
    Encounter,
    /// A zone action (type 2).
    Zone,
    /// A "stop" zone action (type 6): the NPC stays put for good.
    Stop,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Face {
    Keep,
    Sora,
    Point(Vec3),
}

#[derive(Clone, Copy, Debug)]
struct Irq {
    kind: IrqKind,
    /// c70: ticks left.
    timer: f32,
    /// c5c: handled by the AI.
    fired: bool,
    /// c5d: fires at once.
    immediate: bool,
    zone: Option<usize>,
    /// c54 / c58 and the walking direction at entry.
    threshold: f32,
    entry: f32,
    reverse_after: bool,
    face: Face,
    motion: u16,
}

impl Irq {
    const NONE: Self = Self {
        kind: IrqKind::None,
        timer: 0.,
        fired: false,
        immediate: false,
        zone: None,
        threshold: 0.,
        entry: 0.,
        reverse_after: false,
        face: Face::Keep,
        motion: 0,
    };
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Wait(u32),
    Face(u32),
    Hold(f32),
    Finish(u32),
}

/// The rtn_default acts that apply to a townsperson.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Act {
    /// static_idle.
    Static,
    /// look: face Sora while he is near.
    Look(f32),
    /// path_move.
    Path,
    /// encount_player.
    Encounter(Phase),
    /// rtn_action.
    Action,
    /// after_talk: stand 60 ticks facing Sora.
    AfterTalk(u32),
    /// fix_idle / stopped: idles on one motion for good.
    Fixed(u16),
}

/// A live field NPC.
pub struct FieldNpc {
    pub kind: usize,
    /// Scene position and yaw (facing (sin yaw, cos yaw)).
    pub position: Vec3,
    pub yaw: f32,
    pub pose: Pose,
    pub frame: u32,
    pub route: Option<Route>,
    pub talk: u16,
    pub reaction: u16,
    pub serial: u16,
    pub entity: usize,
    /// false while a zone-spawned NPC is away (hidden).
    pub alive: bool,
    zones: Vec<NpcZone>,
    actions: Vec<Action>,
    irq: Irq,
    /// prev.type == 4 and the cooldown (cc8).
    prev_encounter: bool,
    cooldown: f32,
    current_zone: Option<usize>,
    act: Act,
}

/// Turn rate per tick on the path (0.087266 × 2.0 = 10°).
pub const PATH_TURN: f32 = 0.174_532_93;
/// Turn rate of look / encounter (0.05236 × 2.0 = 6°).
const LOOK_TURN: f32 = 0.104_72;
/// Walk → turn above this heading error; turn → walk below the small one.
const TURN_ABOVE: f32 = std::f32::consts::FRAC_PI_2;
const WALK_BELOW: f32 = 0.062_832;
/// Look-ahead distance (obj+68).
const LOOKAHEAD: f32 = 5.0;
/// Timing table cd8..cf8 (OBJ_INIT): encounter 360..720, cooldown 180
/// (+60 after talk), zone action 360..720, generic cooldown 180.
const ENCOUNTER_TIME: (f32, f32) = (360., 720.);
const ZONE_TIME: (f32, f32) = (360., 720.);
const COOLDOWN: f32 = 180.;
const TALK_COOLDOWN: f32 = 240.;
/// The towns-person reaction sphere and Sora's OBJ ellipsoid (centre
/// height, radius, height).
const REACTION: [f32; 3] = [200., 200., 200.];
const SORA_OBJ: [f32; 3] = [35., 55., 85.];
/// Distances of static_idle / look (3-D).
const LOOK_START: f32 = 200.;
const LOOK_END: f32 = 300.;
/// Slot of the talk gesture (R30A).
const TALK_SLOT: u16 = 50;

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// obj_turn_dir (FUN_1403b9f70): returns true while still turning.
fn turn_toward(yaw: &mut f32, to: Vec3, rate: f32) -> bool {
    let want = to.x.atan2(to.z);
    let delta = wrap(want - *yaw);
    if delta > rate {
        *yaw = wrap(*yaw + rate);
        true
    } else if delta < -rate {
        *yaw = wrap(*yaw - rate);
        true
    } else {
        *yaw = want;
        false
    }
}

fn range(rng: &mut Lcg, (a, b): (f32, f32)) -> f32 {
    a + rng.unit() * (b - a)
}

impl FieldNpc {
    /// FUN_1404168c0 for one entity of a type-6 spawn point.
    pub fn spawn(
        kind: usize,
        entity: &ArdEntity,
        entity_index: usize,
        path: Option<&WalkPath>,
        random: &mut Lcg,
    ) -> Self {
        let position = Vec3::new(entity.position[0], -entity.position[1], -entity.position[2]);
        let engine_yaw = wrap(std::f32::consts::PI - entity.rotation[1]);
        let yaw = std::f32::consts::PI - engine_yaw;
        let flags = entity.arg2 & 0x7f;
        let fixed = flags & 1 != 0;
        let route = if fixed {
            None
        } else {
            path.and_then(|p| {
                Route::new(
                    p,
                    flags & 2 != 0,
                    flags & 4 != 0,
                    flags & 8 != 0,
                    random,
                    position,
                )
            })
        };
        let mut npc = Self {
            kind,
            position,
            yaw,
            pose: Pose::Idle,
            frame: 0,
            route,
            talk: entity.talk,
            reaction: entity.reaction,
            serial: entity.serial,
            entity: entity_index,
            alive: true,
            zones: Vec::new(),
            actions: Vec::new(),
            irq: Irq::NONE,
            prev_encounter: false,
            cooldown: 0.,
            current_zone: None,
            act: if fixed { Act::Fixed(0) } else { Act::Static },
        };
        // The route start teleports the NPC onto the curve (nearest flag
        // clear): FUN_14054e130 / FUN_1403b64d0.
        if let Some(r) = &npc.route {
            if flags & 2 == 0 {
                npc.position = r.start_point();
            }
            npc.pose = Pose::Turn;
            npc.act = Act::Path;
        }
        npc
    }

    /// The spawn point's behaviour zones and action records. A fixed NPC's
    /// idle motion is the record with zone 0 and entry 4 (FUN_1404168c0).
    pub fn set_behaviour(&mut self, zones: Vec<NpcZone>, actions: Vec<Action>) {
        if let Act::Fixed(_) = self.act {
            let slot = actions
                .iter()
                .find(|a| a.zone == 0 && a.entry == 4)
                .map_or(0, |a| a.motion());
            self.act = Act::Fixed(slot);
        }
        self.zones = zones;
        self.actions = actions;
    }

    /// sora is in the reaction sphere (FUN_140417910).
    fn in_range(&self, sora: Vec3) -> bool {
        let d = (self.position + Vec3::Y * REACTION[2]) - (sora + Vec3::Y * SORA_OBJ[2]);
        let (r, h) = (REACTION[0] + SORA_OBJ[0], REACTION[1] + SORA_OBJ[1]);
        (d.x * d.x + d.z * d.z) / (r * r) + d.y * d.y / (h * h) < 1.
    }

    /// FUN_140421100: how far across the zone (along its x axis) a point is.
    fn progress(zone: &Zone, engine: [f32; 3]) -> f32 {
        let hx = zone.half[0];
        (hx + zone.to_local(engine)[0]) / (2. * hx)
    }

    fn engine(&self) -> [f32; 3] {
        [self.position.x, -self.position.y, -self.position.z]
    }

    /// FUN_1404183f0: the weighted zone action roll on entering a zone.
    fn roll_zone_action(&mut self, zone_index: usize, rng: &mut Lcg) {
        let id = self.zones[zone_index].id as u8;
        let roll = rng.below(100) as i32;
        let mut acc = 0;
        let mut chosen = None;
        for a in self.actions.iter().filter(|a| a.zone == id && a.kind != 3) {
            acc += a.rate as i32;
            if roll < acc {
                chosen = Some(*a);
                break;
            }
        }
        let Some(a) = chosen else { return };
        let reverse_after = self
            .actions
            .iter()
            .find(|r| r.zone == id && r.kind == 3)
            .is_some_and(|r| (rng.below(100) as i32) < r.rate as i32);
        let face = match a.entry {
            2 => Face::Sora,
            3 => Face::Point(Vec3::new(a.args[0], -a.args[1], -a.args[2])),
            _ => Face::Keep,
        };
        self.irq = Irq {
            kind: if a.kind == 1 {
                IrqKind::Stop
            } else {
                IrqKind::Zone
            },
            timer: range(rng, ZONE_TIME),
            fired: false,
            immediate: false,
            zone: Some(zone_index),
            threshold: rng.unit(),
            entry: Self::progress(&self.zones[zone_index].zone, self.engine()),
            reverse_after,
            face,
            motion: a.motion(),
        };
    }

    /// FUN_1404179e0 (the parts that apply without NPC-NPC meetings).
    fn detect(&mut self, sora: Vec3, rng: &mut Lcg) {
        let in_range = self.in_range(sora);
        let encounter = self.irq.kind == IrqKind::Encounter;
        if encounter || (self.prev_encounter && self.cooldown > 0.) || !in_range {
            if self.cooldown <= 0. {
                self.cooldown = 0.;
                let probe = [self.position.x, -self.position.y - 10., -self.position.z];
                let z = self.zones.iter().position(|z| z.zone.contains(probe));
                if z != self.current_zone {
                    if !self.irq.fired {
                        self.irq = Irq::NONE;
                        if let Some(z) = z {
                            self.roll_zone_action(z, rng);
                        }
                    }
                    self.current_zone = z;
                }
            }
        } else {
            self.irq = Irq {
                kind: IrqKind::Encounter,
                timer: range(rng, ENCOUNTER_TIME),
                immediate: true,
                face: Face::Sora,
                ..Irq::NONE
            };
        }
    }

    /// irq_pending (FUN_1404181d0).
    fn pending(&mut self) -> bool {
        if self.irq.kind == IrqKind::None {
            return false;
        }
        if self.irq.fired {
            return false;
        }
        if self.irq.immediate {
            self.irq.fired = true;
            return true;
        }
        if let Some(z) = self.irq.zone {
            let f = Self::progress(&self.zones[z].zone, self.engine());
            let fire = if self.irq.entry >= 0.5 {
                f <= self.irq.threshold
            } else {
                f >= self.irq.threshold
            };
            if fire {
                self.irq.fired = true;
                return true;
            }
        }
        false
    }

    /// irq_is_over (FUN_140418310).
    fn over(&self, sora: Vec3) -> bool {
        if self.irq.kind == IrqKind::Encounter && !self.in_range(sora) {
            return true;
        }
        !(self.irq.fired && self.irq.timer > 0.)
    }

    /// irq_end (FUN_140417e60): the 180-tick cooldown applies whichever
    /// type ended (the exe reads the type after clearing it).
    fn end_irq(&mut self) {
        self.prev_encounter = self.irq.kind == IrqKind::Encounter;
        self.irq = Irq::NONE;
        self.cooldown = COOLDOWN;
    }

    /// handle_irq (rtn_default).
    fn handle(&mut self) {
        match self.irq.kind {
            IrqKind::Encounter => self.act = Act::Encounter(Phase::Wait(20)),
            IrqKind::Zone => {
                self.pose = Pose::Slot(self.irq.motion);
                self.act = Act::Action;
            }
            IrqKind::Stop => {
                self.route = None;
                self.act = Act::Fixed(self.irq.motion);
            }
            IrqKind::None => {}
        }
    }

    /// Talk end (FUN_140416d00 + after_talk): face Sora, wait 60 ticks.
    pub fn after_talk(&mut self, sora: Vec3) {
        let to = sora - self.position;
        self.yaw = to.x.atan2(to.z);
        self.irq = Irq::NONE;
        self.prev_encounter = true;
        self.cooldown = TALK_COOLDOWN;
        self.act = if let Act::Fixed(s) = self.act {
            Act::Fixed(s)
        } else {
            Act::AfterTalk(60)
        };
        self.pose = Pose::Idle;
    }

    /// The running act, for logs.
    pub fn act_name(&self) -> String {
        format!("{:?}", self.act)
    }

    fn resume(&mut self) {
        self.act = if self.route.is_some() {
            Act::Path
        } else {
            Act::Static
        };
    }

    /// Sora within `limit` of the NPC's facing (look_can_reach).
    fn can_reach(&self, sora: Vec3, limit: f32) -> bool {
        let to = sora - self.position;
        wrap(to.x.atan2(to.z) - self.yaw).abs() <= limit
    }

    /// One tick of the AI. `held` = a talk is running with this NPC (the
    /// caller owns its facing; the talk gesture plays).
    pub fn tick(&mut self, anims: &Anims, sora: Vec3, held: bool, rng: &mut Lcg) {
        self.frame += 1;
        if held {
            self.pose = if anims.extra.contains_key(&TALK_SLOT) {
                Pose::Slot(TALK_SLOT)
            } else {
                Pose::Idle
            };
            self.advance(anims);
            return;
        }
        self.cooldown = (self.cooldown - 1.).max(0.);
        self.irq.timer = (self.irq.timer - 1.).max(0.);
        if !matches!(self.act, Act::Fixed(_)) {
            self.detect(sora, rng);
        }
        let before = self.pose;
        match self.act {
            Act::Fixed(slot) => {
                self.pose = if slot == 0 {
                    Pose::Idle
                } else {
                    Pose::Slot(slot)
                };
            }
            Act::AfterTalk(n) => {
                self.pose = Pose::Idle;
                if n <= 1 {
                    if let Some(r) = self.route.as_mut() {
                        r.away_from(self.position, sora);
                    }
                    self.resume();
                } else {
                    self.act = Act::AfterTalk(n - 1);
                }
            }
            Act::Static => {
                self.pose = Pose::Idle;
                if self.position.distance(sora) <= LOOK_START {
                    self.act = Act::Look(10.);
                }
            }
            Act::Look(t) => {
                let to = sora - self.position;
                if self.position.distance(sora) >= LOOK_END {
                    self.pose = Pose::Idle;
                    self.act = Act::Static;
                } else if !self.can_reach(sora, anims.look_limit) {
                    turn_toward(&mut self.yaw, to, LOOK_TURN);
                    self.pose = Pose::Turn;
                    self.act = Act::Look(10.);
                } else if t <= 0. {
                    self.pose = Pose::Idle;
                    self.act = Act::Look(0.);
                } else {
                    self.act = Act::Look(t - 1.);
                }
            }
            Act::Path => self.path_move(anims, sora),
            Act::Action => {
                if self.pending() {
                    self.handle();
                }
                if self.act == Act::Action {
                    if self.over(sora) {
                        if self.irq.reverse_after
                            && let Some(r) = self.route.as_mut()
                        {
                            r.reverse();
                        }
                        self.end_irq();
                        self.resume();
                    } else {
                        let to = match self.irq.face {
                            Face::Keep => None,
                            Face::Sora => Some(sora - self.position),
                            Face::Point(p) => Some(p - self.position),
                        };
                        if let Some(mut d) = to {
                            d.y = 0.;
                            turn_toward(&mut self.yaw, d.normalize_or_zero(), PATH_TURN);
                        }
                    }
                }
            }
            Act::Encounter(phase) => self.encounter(phase, anims, sora),
        }
        if self.pose != before {
            self.frame = 0;
        }
        self.advance(anims);
    }

    fn encounter(&mut self, phase: Phase, anims: &Anims, sora: Vec3) {
        let to = sora - self.position;
        match phase {
            Phase::Wait(n) => {
                self.pose = Pose::Idle;
                self.act = if n <= 1 {
                    Act::Encounter(Phase::Face(20))
                } else {
                    Act::Encounter(Phase::Wait(n - 1))
                };
            }
            Phase::Face(n) => {
                // The script's angle trap gets the same vector twice, so
                // its motion choice is always idle [npc-01 §2.4].
                turn_toward(&mut self.yaw, to, LOOK_TURN);
                self.pose = Pose::Idle;
                self.act = if n <= 1 {
                    Act::Encounter(Phase::Hold(10.))
                } else {
                    Act::Encounter(Phase::Face(n - 1))
                };
            }
            Phase::Hold(t) => {
                if self.over(sora) {
                    self.pose = Pose::Idle;
                    self.act = Act::Encounter(Phase::Finish(10));
                } else if !self.can_reach(sora, anims.look_limit) {
                    turn_toward(&mut self.yaw, to, LOOK_TURN);
                    self.pose = Pose::Turn;
                    self.act = Act::Encounter(Phase::Hold(10.));
                } else if t <= 0. {
                    self.pose = Pose::Idle;
                    self.act = Act::Encounter(Phase::Hold(0.));
                } else {
                    self.act = Act::Encounter(Phase::Hold(t - 1.));
                }
            }
            Phase::Finish(n) => {
                self.pose = Pose::Idle;
                if n <= 1 {
                    if let Some(r) = self.route.as_mut() {
                        r.away_from(self.position, sora);
                    }
                    self.end_irq();
                    self.resume();
                } else {
                    self.act = Act::Encounter(Phase::Finish(n - 1));
                }
            }
        }
    }

    /// The path_move act: steer along the route, walk or turn in place.
    fn path_move(&mut self, anims: &Anims, _sora: Vec3) {
        let Some(route) = self.route.as_mut() else {
            self.resume();
            return;
        };
        let facing = Vec3::new(self.yaw.sin(), 0., self.yaw.cos());
        let toward = route.target(self.position, LOOKAHEAD).unwrap_or(facing);
        // vector_angle_y(facing, target): wrap(atan2(t) − atan2(f)).
        let error = wrap(toward.x.atan2(toward.z) - facing.x.atan2(facing.z));
        match self.pose {
            Pose::Walk => {
                if (error > TURN_ABOVE || error < -TURN_ABOVE) && anims.turn.is_some() {
                    self.pose = Pose::Turn;
                }
            }
            _ => {
                if error > -WALK_BELOW && error < WALK_BELOW && anims.walk.is_some() {
                    self.pose = Pose::Walk;
                }
            }
        }
        turn_toward(&mut self.yaw, toward, PATH_TURN);
        if self.pose == Pose::Walk
            && let Some(walk) = &anims.walk
        {
            self.position += Vec3::new(self.yaw.sin(), 0., self.yaw.cos()) * walk.travel;
        }
        if self.pending() {
            self.handle();
        }
    }

    fn anim<'a>(&self, anims: &'a Anims) -> &'a Anim {
        match self.pose {
            Pose::Walk => anims.walk.as_ref().unwrap_or(&anims.idle),
            Pose::Turn => anims.turn.as_ref().unwrap_or(&anims.idle),
            Pose::Slot(s) => anims.extra.get(&s).unwrap_or(&anims.idle),
            Pose::Idle => &anims.idle,
        }
    }

    fn advance(&mut self, anims: &Anims) {
        let a = self.anim(anims);
        let len = a.frames.len() as u32;
        if self.frame >= len {
            self.frame = a.loop_start.min(len.saturating_sub(1));
        }
    }

    /// Current vertex positions.
    pub fn frame_positions<'a>(&self, anims: &'a Anims) -> &'a [Vec3] {
        let a = self.anim(anims);
        &a.frames[(self.frame as usize).min(a.frames.len() - 1)]
    }

    /// The talk-camera bone matrix this tick.
    pub fn bone(&self, anims: &Anims) -> Option<Mat4> {
        let a = self.anim(anims);
        a.bones
            .get((self.frame as usize).min(a.bones.len().saturating_sub(1)))
            .copied()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(looped: bool) -> Route {
        let pts: Vec<[f32; 3]> = vec![
            [0., 0., 0.],
            [100., 0., 0.],
            [100., 0., 100.],
            [0., 0., 100.],
        ];
        let path = WalkPath {
            header: [0, 0, 4, 0, looped as u8, 0, 0, 0],
            points: pts,
        };
        let mut rng = Lcg(1);
        Route::new(&path, true, false, false, &mut rng, Vec3::ZERO).unwrap()
    }

    #[test]
    fn hermite_passes_through_its_points() {
        let r = square(true);
        for (i, p) in r.points.iter().enumerate() {
            assert!(r.at(i as f32).distance(*p) < 1e-3);
        }
        // Closed loop: parameter n returns to point 0.
        assert!(r.at(4. - 1e-4).distance(r.points[0]) < 0.5);
    }

    #[test]
    fn open_path_end_tangents_follow_the_parabola() {
        let r = square(false);
        assert!(r.tangents[0].is_finite() && r.tangents[3].is_finite());
        // The start tangent heads along the first chord's side.
        assert!(r.tangents[0].dot(r.points[1] - r.points[0]) > 0.);
    }

    #[test]
    fn target_points_ahead_along_the_walking_direction() {
        let mut r = square(true);
        r.forward = true;
        r.t = 0.;
        let d = r.target(Vec3::new(0., 0., 0.), 5.).unwrap();
        assert!((d.length() - 1.).abs() < 1e-3, "{d:?}");
        assert!(d.x.abs() > 0.5 || d.z.abs() > 0.5, "{d:?}");
    }

    #[test]
    fn walk_switches_to_turn_on_a_large_heading_error() {
        let anim = |frames: usize, travel: f32| Anim {
            frames: vec![vec![Vec3::ZERO]; frames],
            loop_start: 0,
            bones: vec![None; frames],
            travel,
        };
        let kind = Anims {
            idle: anim(1, 0.),
            walk: Some(anim(4, 1.0)),
            turn: Some(anim(2, 0.)),
            extra: HashMap::new(),
            look_limit: 0.26,
        };
        let mut npc = FieldNpc {
            kind: 0,
            position: Vec3::ZERO,
            yaw: std::f32::consts::PI,
            pose: Pose::Walk,
            frame: 0,
            route: Some(square(true)),
            talk: 0,
            reaction: 0,
            serial: 0,
            entity: 0,
            alive: true,
            zones: Vec::new(),
            actions: Vec::new(),
            irq: Irq::NONE,
            prev_encounter: false,
            cooldown: 0.,
            current_zone: None,
            act: Act::Path,
        };
        npc.tick(&kind, Vec3::new(1e6, 0., 1e6), false, &mut Lcg(3));
        assert_eq!(npc.pose, Pose::Turn);
    }
}

#[cfg(test)]
mod walk_tests {
    use super::*;

    #[test]
    fn an_npc_follows_an_open_path() {
        let pts: Vec<[f32; 3]> = vec![
            [507., 0., -515.],
            [937., 0., -433.],
            [1344., 0., -215.],
            [1668., 0., 216.],
            [1848., 0., 808.],
            [1886., 0., 1329.],
        ];
        let path = WalkPath {
            header: [0, 0, 6, 0, 0, 0, 0, 0],
            points: pts,
        };
        let mut rng = Lcg(7);
        let mut entity = ArdEntity {
            object: 1,
            position: [507., 0., -515.],
            rotation: [0.; 3],
            spawn_type: 0,
            spawn_arg: 0,
            serial: 0,
            arg1: 0,
            arg2: 2, // start at the nearest point
            talk: 0,
            respawn_delay: 0,
            reaction: 0,
            radius: 0,
            required_count: 0,
            raw: [0; 0x40],
        };
        entity.arg2 = 2;
        let mut npc = FieldNpc::spawn(0, &entity, 0, Some(&path), &mut rng);
        let anim = |frames: usize, travel: f32| Anim {
            frames: vec![vec![Vec3::ZERO]; frames],
            loop_start: 0,
            bones: vec![None; frames],
            travel,
        };
        let anims = Anims {
            idle: anim(2, 0.),
            walk: Some(anim(60, 1.87)),
            turn: Some(anim(60, 0.)),
            extra: HashMap::new(),
            look_limit: 0.26,
        };
        let start = npc.position;
        for _ in 0..600 {
            npc.tick(&anims, Vec3::new(1e6, 0., 1e6), false, &mut Lcg(3));
        }
        let moved = npc.position.distance(start);
        assert!(moved > 500., "moved only {moved}: {:?}", npc.position);
    }
}

#[cfg(test)]
mod probe_tests {
    use super::*;

    #[test]
    fn target_is_stable_near_the_end_of_the_path() {
        let pts: Vec<[f32; 3]> = vec![
            [507., 0., -515.],
            [937., 0., -433.],
            [1344., 0., -215.],
            [1668., 0., 216.],
            [1848., 0., 808.],
            [1886., 0., 1329.],
        ];
        let path = WalkPath {
            header: [0, 0, 6, 0, 0, 0, 0, 0],
            points: pts,
        };
        let mut rng = Lcg(7);
        let mut r = Route::new(&path, true, false, false, &mut rng, Vec3::ZERO).unwrap();
        let p = Vec3::new(1852.37, 0., -833.45);
        for forward in [true, false] {
            r.forward = forward;
            r.t = 4.0;
            let a = r.target(p, 5.).unwrap();
            let b = r.target(p + a * 1.0, 5.).unwrap();
            assert!(a.dot(b) > 0.9, "forward {forward}: {a:?} {b:?} t {}", r.t);
            eprintln!("forward {forward}: {a:?} {b:?} t {}", r.t);
        }
    }
}
