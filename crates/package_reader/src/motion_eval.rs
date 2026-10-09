//! Interpolated-motion evaluation with the motion's own IK skeleton: IK
//! helper joints animated by the inverse curves, the joint processing order,
//! constraints (position, orientation, direction, position/rotation limits),
//! two-joint IK chains, expressions and the root-position curves.
//!
//! Table layouts follow OpenKH docs/kh2/file/anb/motion.md (pin in NOTICE)
//! and kenjiuno's msetDoc. The public sources leave the solver itself
//! undocumented, so the evaluation rules here are this project's reading of
//! the data and are recorded as such in T057; they are not an observed
//! installed-runtime equivalence. The older basic public FK consumer in
//! `motion.rs`/`model_pose.rs` is kept unchanged for its T053 evidence.
use crate::{
    model_skeleton::BoneRecord,
    motion::{Curve, InterpolatedMotion},
};
use std::io;

type Quat = [f32; 4];
type Vec3 = [f32; 3];
const IDENTITY: Quat = [0., 0., 0., 1.];

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

// Hamilton quaternions (x, y, z, w), matching model_pose's product and the
// standard rotation that its row-vector matrices encode.
fn qmul(a: Quat, b: Quat) -> Quat {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}
fn qconj(q: Quat) -> Quat {
    [-q[0], -q[1], -q[2], q[3]]
}
fn qnorm(q: Quat) -> Quat {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n > 0. && n.is_finite() {
        q.map(|c| c / n)
    } else {
        IDENTITY
    }
}
fn rotate(q: Quat, v: Vec3) -> Vec3 {
    let p = qmul(qmul(q, [v[0], v[1], v[2], 0.]), qconj(q));
    [p[0], p[1], p[2]]
}
fn axis_angle(axis: Vec3, angle: f32) -> Quat {
    let (s, c) = (angle * 0.5).sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}
fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn length(a: Vec3) -> f32 {
    dot(a, a).sqrt()
}
fn normalize(a: Vec3) -> Option<Vec3> {
    let n = length(a);
    (n > 1e-6 && n.is_finite()).then(|| a.map(|c| c / n))
}
/// Shortest-arc rotation taking direction `u` onto direction `v`.
fn arc(u: Vec3, v: Vec3) -> Quat {
    let (Some(u), Some(v)) = (normalize(u), normalize(v)) else {
        return IDENTITY;
    };
    let d = dot(u, v);
    if d < -0.999_999 {
        let mut axis = cross([1., 0., 0.], u);
        if length(axis) < 1e-6 {
            axis = cross([0., 1., 0.], u);
        }
        return axis_angle(
            normalize(axis).unwrap_or([0., 0., 1.]),
            std::f32::consts::PI,
        );
    }
    let c = cross(u, v);
    qnorm([c[0], c[1], c[2], 1. + d])
}
/// Local rotation composed as model_pose does: Rz * Ry * Rx.
fn from_euler(r: Vec3) -> Quat {
    let q = qmul(
        axis_angle([0., 0., 1.], r[2]),
        axis_angle([0., 1., 0.], r[1]),
    );
    qmul(q, axis_angle([1., 0., 0.], r[0]))
}
fn to_euler(q: Quat) -> Vec3 {
    let [x, y, z, w] = qnorm(q);
    // Column-vector rotation matrix entries of R = Rz * Ry * Rx.
    let r20 = 2. * (x * z - w * y);
    let r21 = 2. * (y * z + w * x);
    let r22 = 1. - 2. * (x * x + y * y);
    let r10 = 2. * (x * y + w * z);
    let r00 = 1. - 2. * (y * y + z * z);
    [r21.atan2(r22), (-r20).clamp(-1., 1.).asin(), r10.atan2(r00)]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Local {
    pub scale: Vec3,
    pub rotation: Vec3,
    pub translation: Vec3,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct World {
    pub rotation: Quat,
    pub translation: Vec3,
    /// The joint's own scale; matched against the reference, children do
    /// not inherit it.
    pub scale: Vec3,
}
impl World {
    fn compose(parent: &World, local: &Local) -> World {
        World {
            rotation: qnorm(qmul(parent.rotation, from_euler(local.rotation))),
            translation: add(
                parent.translation,
                rotate(parent.rotation, local.translation),
            ),
            scale: local.scale,
        }
    }
    /// Row-vector matrix words in the layout model_pose's skinning expects.
    pub fn matrix(&self) -> [[f32; 4]; 4] {
        let [x, y, z, w] = self.rotation;
        let [tx, ty, tz] = self.translation;
        let [sx, sy, sz] = self.scale;
        let m = [
            [
                1. - 2. * (y * y + z * z),
                2. * (x * y + w * z),
                2. * (x * z - w * y),
                0.,
            ],
            [
                2. * (x * y - w * z),
                1. - 2. * (z * z + x * x),
                2. * (y * z + w * x),
                0.,
            ],
            [
                2. * (x * z + w * y),
                2. * (y * z - w * x),
                1. - 2. * (y * y + x * x),
                0.,
            ],
            [tx, ty, tz, 1.],
        ];
        [
            m[0].map(|v| v * sx),
            m[1].map(|v| v * sy),
            m[2].map(|v| v * sz),
            m[3],
        ]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EvaluationStats {
    pub constraints_applied: usize,
    pub ik_chains_solved: usize,
    pub expressions_applied: usize,
    pub unsupported_constraints: usize,
}

pub struct Evaluated {
    /// World (character space) transforms of every model bone and IK helper.
    pub joints: Vec<World>,
    /// Root position (translation) from the root fcurves, model space.
    pub root_translation: Vec3,
    pub stats: EvaluationStats,
}

/// Standard cubic Hermite sampling with first/last-key holds (cycle type 0),
/// repeat (2) and zero (3); type 1 (additive) offsets by whole cycles.
/// Tangents are slopes per key-time unit, scaled by the key span.
pub fn sample_curve(m: &InterpolatedMotion, curve: &Curve, frame: f32) -> io::Result<Option<f32>> {
    let start = usize::try_from(curve.key_start).map_err(|_| invalid("negative key start"))?;
    let end = start + curve.key_count as usize;
    if curve.key_count == 0 {
        return Ok(None);
    }
    if end > m.keys.len() {
        return Err(invalid("curve key range outside table"));
    }
    let key = |i: usize| m.key_record(i);
    let first = key(start)?;
    let last = key(end - 1)?;
    let span = last[0] - first[0];
    let (mut t, mut offset) = (frame, 0.);
    if frame < first[0] || frame > last[0] {
        let cycle = if frame < first[0] {
            (curve.channel >> 4) & 3
        } else {
            (curve.channel >> 6) & 3
        };
        match cycle {
            3 => return Ok(Some(0.)),
            2 | 1 if span > 0. => {
                let k = ((frame - first[0]) / span).floor();
                t = frame - k * span;
                if cycle == 1 {
                    offset = k * (last[1] - first[1]);
                }
            }
            _ => return Ok(Some(if frame < first[0] { first[1] } else { last[1] })),
        }
    }
    let mut i = start;
    while i + 1 < end && key(i + 1)?[0] <= t {
        i += 1;
    }
    let left = key(i)?;
    if i + 1 >= end {
        return Ok(Some(left[1] + offset));
    }
    let right = key(i + 1)?;
    let dt = right[0] - left[0];
    if dt <= 0. {
        return Ok(Some(right[1] + offset));
    }
    let n = (t - left[0]) / dt;
    let value = match m.keys[i][0] & 3 {
        0 => left[1],
        1 => left[1] + (right[1] - left[1]) * n,
        2 => {
            let (n2, n3) = (n * n, n * n * n);
            (2. * n3 - 3. * n2 + 1.) * left[1]
                + (n3 - 2. * n2 + n) * left[3] * dt
                + (-2. * n3 + 3. * n2) * right[1]
                + (n3 - n2) * right[2] * dt
        }
        _ => 0.,
    };
    if !value.is_finite() {
        return Err(invalid("nonfinite curve sample"));
    }
    Ok(Some(value + offset))
}

fn set_channel(local: &mut Local, channel: u8, value: f32) {
    match channel {
        0..=2 => local.scale[channel as usize] = value,
        3..=5 => local.rotation[(channel - 3) as usize] = value,
        6..=8 => local.translation[(channel - 6) as usize] = value,
        _ => {}
    }
}

struct Constraint {
    kind: u8,
    constrained: usize,
    source: usize,
    limiter: Option<usize>,
    activations: Vec<(f32, bool)>,
    temporary_active: bool,
}
struct Limiter {
    kind: u32,
    global: bool,
    enabled: [bool; 6],
    min: Vec3,
    max: Vec3,
}
impl Limiter {
    /// Clamps each axis against its enabled minimum/maximum.
    fn clamp_axes(&self, values: &mut Vec3) {
        for (axis, v) in values.iter_mut().enumerate() {
            if self.enabled[axis * 2] {
                *v = v.max(self.min[axis]);
            }
            if self.enabled[axis * 2 + 1] {
                *v = v.min(self.max[axis]);
            }
        }
    }
}

fn half(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn float(b: &[u8], at: usize) -> f32 {
    f32::from_bits(word(b, at))
}

struct Node {
    kind: i8,
    global: bool,
    element: i16,
    value: f32,
    car: i16,
    cdr: i16,
}

/// Evaluates every joint of `m` at `frame` (key-time units).
pub fn evaluate(bones: &[BoneRecord], m: &InterpolatedMotion, frame: f32) -> io::Result<Evaluated> {
    let (bone_count, total) = (m.bone_count, m.total_bone_count);
    if bones.len() != bone_count || !frame.is_finite() {
        return Err(invalid("evaluation skeleton/frame mismatch"));
    }
    // Locals and parents for model bones and IK helpers.
    let mut locals = Vec::with_capacity(total);
    let mut parents = Vec::with_capacity(total);
    for bone in bones {
        let t = bone.transform_bits.map(f32::from_bits);
        locals.push(Local {
            scale: [t[0], t[1], t[2]],
            rotation: [t[4], t[5], t[6]],
            translation: [t[8], t[9], t[10]],
        });
        parents.push(bone.header_i32[1]);
    }
    for b in m.helpers.bytes.as_chunks::<64>().0 {
        locals.push(Local {
            scale: [float(b, 16), float(b, 20), float(b, 24)],
            rotation: [float(b, 32), float(b, 36), float(b, 40)],
            translation: [float(b, 48), float(b, 52), float(b, 56)],
        });
        parents.push(i32::from(half(b, 4)));
    }
    if locals.len() != total {
        return Err(invalid("IK helper count mismatch"));
    }
    if parents.iter().any(|&p| p >= 0 && p as usize >= total) {
        return Err(invalid("joint parent outside skeleton"));
    }
    for pose in &m.initial_poses {
        if let Ok(j) = usize::try_from(pose.bone_id)
            && j < total
            && (0..=8).contains(&pose.channel)
        {
            set_channel(
                &mut locals[j],
                pose.channel as u8,
                f32::from_bits(pose.value_bits),
            );
        }
    }
    for curve in &m.forward_curves {
        let j = usize::try_from(curve.joint_id).map_err(|_| invalid("negative curve joint"))?;
        if j >= bone_count {
            return Err(invalid("forward curve joint outside skeleton"));
        }
        if let Some(v) = sample_curve(m, curve, frame)? {
            set_channel(&mut locals[j], curve.channel & 15, v);
        }
    }
    // Root fcurves index the combined forward+inverse curve list.
    let root_indices: Vec<i32> = m
        .root
        .bytes
        .get(48..84)
        .map(|b| (0..9).map(|i| word(b, i * 4) as i32).collect())
        .unwrap_or_default();
    let mut root = Local {
        scale: [1.; 3],
        rotation: [0.; 3],
        translation: m
            .root
            .bytes
            .get(32..44)
            .map(|b| [float(b, 0), float(b, 4), float(b, 8)])
            .unwrap_or([0.; 3]),
    };
    let curve_at = |index: usize| -> Option<&Curve> {
        if index < m.forward_curves.len() {
            m.forward_curves.get(index)
        } else {
            m.inverse_curves.get(index - m.forward_curves.len())
        }
    };
    for (channel, &index) in root_indices.iter().enumerate() {
        if let Ok(index) = usize::try_from(index)
            && let Some(curve) = curve_at(index)
            && let Some(v) = sample_curve(m, curve, frame)?
        {
            set_channel(&mut root, channel as u8, v);
        }
    }
    for curve in &m.inverse_curves {
        let j = usize::try_from(curve.joint_id)
            .ok()
            .map(|j| j + bone_count)
            .filter(|&j| j < total)
            .ok_or_else(|| invalid("inverse curve joint outside IK skeleton"))?;
        if let Some(v) = sample_curve(m, curve, frame)? {
            set_channel(&mut locals[j], curve.channel & 15, v);
        }
    }

    // Constraints, activations and limiters.
    let activation_bytes = &m.activations.bytes;
    let mut constraints = Vec::new();
    for b in m.constraints.bytes.as_chunks::<12>().0 {
        let index = |v: i16| usize::try_from(v).ok().filter(|&j| j < total);
        let (Some(constrained), Some(source)) = (index(half(b, 2)), index(half(b, 4))) else {
            return Err(invalid("constraint joint outside skeleton"));
        };
        let count = usize::try_from(half(b, 8)).unwrap_or(0);
        let first = usize::try_from(half(b, 10)).unwrap_or(0);
        let mut activations = Vec::new();
        for k in first..first + count {
            let at = k * 8;
            let record = activation_bytes
                .get(at..at + 8)
                .ok_or_else(|| invalid("constraint activation outside table"))?;
            activations.push((float(record, 0), word(record, 4) != 0));
        }
        constraints.push(Constraint {
            kind: b[0],
            constrained,
            source,
            limiter: usize::try_from(half(b, 6)).ok(),
            activations,
            temporary_active: b[1] != 0,
        });
    }
    let limiters: Vec<Limiter> = m
        .limiters
        .bytes
        .as_chunks::<48>()
        .0
        .iter()
        .map(|b| {
            let bits = word(b, 0);
            Limiter {
                kind: bits & 7,
                global: bits & 8 != 0,
                enabled: std::array::from_fn(|i| bits & (16 << i) != 0),
                min: [float(b, 16), float(b, 20), float(b, 24)],
                max: [float(b, 32), float(b, 36), float(b, 40)],
            }
        })
        .collect();
    let nodes: Vec<Node> = m
        .expression_nodes
        .bytes
        .as_chunks::<12>()
        .0
        .iter()
        .map(|b| Node {
            kind: b[0] as i8,
            global: b[1] & 1 != 0,
            element: half(b, 2),
            value: float(b, 4),
            car: half(b, 8),
            cdr: half(b, 10),
        })
        .collect();
    let expressions: Vec<(usize, u8, usize)> = m
        .expressions
        .bytes
        .as_chunks::<8>()
        .0
        .iter()
        .filter_map(|b| {
            Some((
                usize::try_from(half(b, 0)).ok().filter(|&j| j < total)?,
                u8::try_from(half(b, 2)).ok()?,
                usize::try_from(half(b, 6))
                    .ok()
                    .filter(|&n| n < nodes.len())?,
            ))
        })
        .collect();
    let joint_flags: Vec<(usize, u8)> = m
        .joints
        .bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| {
            usize::try_from(half(b, 0))
                .ok()
                .filter(|&j| j < total)
                .map(|j| (j, b[2]))
                .ok_or_else(|| invalid("joint order entry outside skeleton"))
        })
        .collect::<io::Result<_>>()?;
    let mut flags = vec![0u8; total];
    for &(j, f) in &joint_flags {
        flags[j] = f;
    }

    // Provisional plain hierarchy pass, so a constraint source that the
    // processing order has not reached yet still has a defined transform.
    let root_world = World {
        rotation: IDENTITY,
        translation: [0.; 3],
        scale: [1.; 3],
    };
    let mut world = vec![root_world; total];
    let mut ready = vec![false; total];
    for _ in 0..total {
        let mut progressed = false;
        for j in 0..total {
            if ready[j] {
                continue;
            }
            let parent = parents[j];
            if parent < 0 {
                world[j] = World::compose(&root_world, &locals[j]);
            } else if ready[parent as usize] {
                world[j] = World::compose(&world[parent as usize], &locals[j]);
            } else {
                continue;
            }
            ready[j] = true;
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    if ready.iter().any(|r| !r) {
        return Err(invalid("joint hierarchy has a cycle"));
    }

    let mut stats = EvaluationStats::default();
    let mut done = vec![false; total];
    let order: Vec<usize> = if joint_flags.len() == total {
        joint_flags.iter().map(|&(j, _)| j).collect()
    } else {
        (0..total).collect()
    };
    for &j in &order {
        // Expressions override this joint's local channels first.
        for &(joint, channel, node) in &expressions {
            if joint == j {
                let ctx = ExprContext {
                    nodes: &nodes,
                    locals: &locals,
                    world: &world,
                    parents: &parents,
                    done: &done,
                    frame,
                };
                if let Some(v) = ctx.eval(node, 0).filter(|v| v.is_finite()) {
                    set_channel(&mut locals[j], channel, v);
                    stats.expressions_applied += 1;
                }
            }
        }
        let parent = parents[j];
        world[j] = if parent < 0 {
            World::compose(&root_world, &locals[j])
        } else {
            World::compose(&world[parent as usize], &locals[j])
        };
        for c in constraints.iter().filter(|c| c.constrained == j) {
            let active = c
                .activations
                .iter()
                .rev()
                .find(|(t, _)| *t <= frame)
                .map_or(c.temporary_active || !c.activations.is_empty(), |a| a.1);
            if !active {
                continue;
            }
            let source = world[c.source];
            match c.kind {
                0 => {
                    let chain_end = flags[j] & 4 != 0;
                    let mid = parents[j];
                    let top = if mid >= 0 { parents[mid as usize] } else { -1 };
                    if chain_end
                        && mid >= 0
                        && top >= 0
                        && flags[mid as usize] & 32 != 0
                        && flags[top as usize] & 3 != 0
                    {
                        let above = parents[top as usize];
                        let hinge_frame = if above >= 0 {
                            world[above as usize].rotation
                        } else {
                            IDENTITY
                        };
                        solve_two_joint(
                            &mut world,
                            &locals,
                            hinge_frame,
                            [top as usize, mid as usize, j],
                            source.translation,
                        );
                        stats.ik_chains_solved += 1;
                        if flags[j] & 8 != 0 {
                            // Reference evaluation: the end joint's rotation is
                            // its own local rotation in character space.
                            world[j].rotation = from_euler(locals[j].rotation);
                        }
                    } else {
                        world[j].translation = source.translation;
                    }
                }
                2 => world[j].rotation = source.rotation,
                3 => {
                    // Aim the joint's local +X (KH2 bone length axis).
                    let x = rotate(world[j].rotation, [1., 0., 0.]);
                    let to = sub(source.translation, world[j].translation);
                    if length(to) > 1e-4 {
                        world[j].rotation = qnorm(qmul(arc(x, to), world[j].rotation));
                    }
                }
                12 => {
                    if let Some(l) = c.limiter.and_then(|i| limiters.get(i)) {
                        world[j].translation = limit_position(l, &source, world[j].translation);
                    }
                }
                13 => {
                    if let Some(l) = c.limiter.and_then(|i| limiters.get(i)) {
                        let parent_rotation = if parent < 0 {
                            IDENTITY
                        } else {
                            world[parent as usize].rotation
                        };
                        let local = qmul(qconj(parent_rotation), world[j].rotation);
                        let mut e = to_euler(local);
                        l.clamp_axes(&mut e);
                        world[j].rotation = qnorm(qmul(parent_rotation, from_euler(e)));
                    }
                }
                _ => {
                    stats.unsupported_constraints += 1;
                    continue;
                }
            }
            stats.constraints_applied += 1;
        }
        done[j] = true;
    }
    if world.iter().any(|w| {
        w.rotation
            .iter()
            .chain(&w.translation)
            .any(|v| !v.is_finite())
    }) {
        return Err(invalid("nonfinite evaluated joint"));
    }
    Ok(Evaluated {
        joints: world,
        root_translation: root.translation,
        stats,
    })
}

fn limit_position(l: &Limiter, source: &World, position: Vec3) -> Vec3 {
    let (origin, rotation) = if l.global {
        ([0.; 3], IDENTITY)
    } else {
        (source.translation, source.rotation)
    };
    let mut local = rotate(qconj(rotation), sub(position, origin));
    if l.kind == 1 {
        // Distance limit: keep the joint within [min.x, max.x] of the source.
        let d = length(local);
        if d > 1e-6 {
            let clamped = d.clamp(l.min[0].min(l.max[0]), l.max[0].max(l.min[0]));
            local = local.map(|c| c * clamped / d);
        }
    } else {
        l.clamp_axes(&mut local);
    }
    add(origin, rotate(rotation, local))
}

/// Rotation whose images of the unit X/Y/Z axes are `x`, `y`, `z`.
fn from_basis(x: Vec3, y: Vec3, z: Vec3) -> Quat {
    // Column-vector matrix R = [x y z]; standard trace-based conversion.
    let (m00, m11, m22) = (x[0], y[1], z[2]);
    let trace = m00 + m11 + m22;
    let q = if trace > 0. {
        let s = (trace + 1.).sqrt() * 2.;
        [
            (y[2] - z[1]) / s,
            (z[0] - x[2]) / s,
            (x[1] - y[0]) / s,
            0.25 * s,
        ]
    } else if m00 > m11 && m00 > m22 {
        let s = (1. + m00 - m11 - m22).sqrt() * 2.;
        [
            0.25 * s,
            (y[0] + x[1]) / s,
            (z[0] + x[2]) / s,
            (y[2] - z[1]) / s,
        ]
    } else if m11 > m22 {
        let s = (1. + m11 - m00 - m22).sqrt() * 2.;
        [
            (y[0] + x[1]) / s,
            0.25 * s,
            (z[1] + y[2]) / s,
            (z[0] - x[2]) / s,
        ]
    } else {
        let s = (1. + m22 - m00 - m11).sqrt() * 2.;
        [
            (z[0] + x[2]) / s,
            (z[1] + y[2]) / s,
            0.25 * s,
            (x[1] - y[0]) / s,
        ]
    };
    qnorm(q)
}

/// Two-joint IK matched against the reference evaluation: the hinge is the
/// chain parent's Y axis projected perpendicular to the target direction;
/// both joints take that hinge as local Z; the middle bends about it by the
/// law-of-cosines angle, with the sign of its bind Z rotation.
fn solve_two_joint(
    world: &mut [World],
    locals: &[Local],
    hinge_frame: Quat,
    [a, b, c]: [usize; 3],
    target: Vec3,
) {
    let l1 = length(locals[b].translation);
    let l2 = length(locals[c].translation);
    let pa = world[a].translation;
    let Some(d) = normalize(sub(target, pa)) else {
        return;
    };
    if l1 < 1e-4 || l2 < 1e-4 {
        return;
    }
    let z = rotate(hinge_frame, [0., 1., 0.]);
    let n = normalize(sub(z, d.map(|c| c * dot(z, d)))).unwrap_or(z);
    let reach = length(sub(target, pa)).clamp((l1 - l2).abs(), l1 + l2);
    let cos_inner = ((l1 * l1 + l2 * l2 - reach * reach) / (2. * l1 * l2)).clamp(-1., 1.);
    let bend = std::f32::consts::PI - cos_inner.acos();
    let sign = if locals[b].rotation[2] < 0. { -1. } else { 1. };
    let lead = (l2 * bend.sin()).atan2(l1 + l2 * bend.cos());
    let x1 = rotate(axis_angle(n, -sign * lead), d);
    let x2 = rotate(axis_angle(n, sign * bend), x1);
    world[a].rotation = from_basis(x1, cross(n, x1), n);
    world[b].rotation = from_basis(x2, cross(n, x2), n);
    world[b].translation = add(pa, rotate(world[a].rotation, locals[b].translation));
    world[c] = World::compose(&world[b], &locals[c]);
}

struct ExprContext<'a> {
    nodes: &'a [Node],
    locals: &'a [Local],
    world: &'a [World],
    parents: &'a [i32],
    done: &'a [bool],
    frame: f32,
}
impl ExprContext<'_> {
    /// Current local channels of `j`: derived from the evaluated world when
    /// the joint has been processed (so constraints are reflected).
    fn local(&self, j: usize) -> Local {
        if !self.done[j] {
            return self.locals[j];
        }
        let parent = self.parents[j];
        let (pr, pt) = if parent < 0 {
            (IDENTITY, [0.; 3])
        } else {
            let p = &self.world[parent as usize];
            (p.rotation, p.translation)
        };
        Local {
            scale: self.world[j].scale,
            rotation: to_euler(qmul(qconj(pr), self.world[j].rotation)),
            translation: rotate(qconj(pr), sub(self.world[j].translation, pt)),
        }
    }
    fn child(&self, index: i16, depth: usize) -> Option<f32> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.eval(i, depth + 1))
    }
    /// List cell `index`: (car, cdr).
    fn cell(&self, index: i16) -> Option<&Node> {
        usize::try_from(index).ok().and_then(|i| self.nodes.get(i))
    }
    fn eval(&self, index: usize, depth: usize) -> Option<f32> {
        if depth > 64 {
            return None;
        }
        let n = self.nodes.get(index)?;
        let a = || self.child(n.car, depth);
        let b = || self.child(n.cdr, depth);
        let channel = |kind: i8| -> Option<f32> {
            let j = usize::try_from(n.element)
                .ok()
                .filter(|&j| j < self.locals.len())?;
            let (rotation, translation) = if n.global {
                (to_euler(self.world[j].rotation), self.world[j].translation)
            } else {
                let l = self.local(j);
                (l.rotation, l.translation)
            };
            Some(match kind {
                32..=34 => translation[(kind - 32) as usize],
                35..=37 => rotation[(kind - 35) as usize],
                _ => 1.,
            })
        };
        Some(match n.kind {
            0 => a()?.sin(),
            1 => a()?.cos(),
            2 => a()?.tan(),
            3 => a()?.clamp(-1., 1.).asin(),
            4 => a()?.clamp(-1., 1.).acos(),
            5 => a()?.atan(),
            6 => a()?.ln(),
            7 => a()?.exp(),
            8 => a()?.abs(),
            9 => a()?.powf(b()?),
            10 => a()?.max(0.).sqrt(),
            11 => a()?.min(b()?),
            12 => a()?.max(b()?),
            13 => (a()? + b()?) * 0.5,
            14 => {
                // (cond . (then . (else . nil)))
                let list = self.cell(n.car)?;
                let cond = self.child(list.car, depth)?;
                let rest = self.cell(list.cdr)?;
                if cond != 0. {
                    self.child(rest.car, depth)?
                } else {
                    let tail = self.cell(rest.cdr)?;
                    self.child(tail.car, depth)?
                }
            }
            17 => a()? % b()?,
            18 => a()? + b()?,
            19 => a()? - b()?,
            20 => a()? * b()?,
            21 => a()? / b()?,
            22 => a()? % b()?,
            23 => f32::from(a()? == b()?),
            24 => f32::from(a()? > b()?),
            25 => f32::from(a()? >= b()?),
            26 => f32::from(a()? < b()?),
            27 => f32::from(a()? <= b()?),
            28 => f32::from(a()? != 0. && b()? != 0.),
            29 => f32::from(a()? != 0. || b()? != 0.),
            30 => self.frame,
            31 => n.value,
            32..=40 => channel(n.kind)?,
            41 => a()?,
            15 | 43 => a()?,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_round_trip_and_composition_order() {
        let e = [0.3, -0.4, 1.1];
        let back = to_euler(from_euler(e));
        for i in 0..3 {
            assert!((back[i] - e[i]).abs() < 1e-5, "{back:?}");
        }
        // Rz*Ry*Rx applies X first: rotate +Y by X 90deg -> +Z, then Z 90deg keeps +Z.
        let v = rotate(
            from_euler([std::f32::consts::FRAC_PI_2, 0., std::f32::consts::FRAC_PI_2]),
            [0., 1., 0.],
        );
        assert!((v[2] - 1.).abs() < 1e-5, "{v:?}");
    }

    #[test]
    fn arc_maps_direction() {
        let q = arc([1., 0., 0.], [0., 0., -2.]);
        let v = rotate(q, [1., 0., 0.]);
        assert!((v[2] + 1.).abs() < 1e-5);
        let q = arc([1., 0., 0.], [-1., 0., 0.]);
        assert!((rotate(q, [1., 0., 0.])[0] + 1.).abs() < 1e-5);
    }

    #[test]
    fn two_joint_ik_reaches_reachable_target() {
        let locals = [
            Local {
                scale: [1.; 3],
                rotation: [0.; 3],
                translation: [0.; 3],
            },
            Local {
                scale: [1.; 3],
                rotation: [0., 0., 0.15],
                translation: [30., 0., 0.],
            },
            Local {
                scale: [1.; 3],
                rotation: [0.; 3],
                translation: [40., 0., 0.],
            },
        ];
        let root = World {
            rotation: IDENTITY,
            translation: [0., 100., 0.],
            scale: [1.; 3],
        };
        let mut world = vec![World::compose(&root, &locals[0])];
        world.push(World::compose(&world[0], &locals[1]));
        world.push(World::compose(&world[1], &locals[2]));
        let target = [10., 50., 20.];
        solve_two_joint(&mut world, &locals, IDENTITY, [0, 1, 2], target);
        let d = length(sub(world[2].translation, target));
        assert!(d < 1e-3, "end misses target by {d}");
        // Segment lengths are preserved.
        assert!((length(sub(world[1].translation, world[0].translation)) - 30.).abs() < 1e-3);
        // Unreachable targets stretch to full extension toward the target.
        solve_two_joint(&mut world, &locals, IDENTITY, [0, 1, 2], [0., -100., 0.]);
        assert!((length(sub(world[2].translation, world[0].translation)) - 70.).abs() < 0.01);
    }

    #[test]
    fn distance_and_box_limits() {
        let source = World {
            rotation: IDENTITY,
            translation: [0.; 3],
            scale: [1.; 3],
        };
        let distance = Limiter {
            kind: 1,
            global: false,
            enabled: [false; 6],
            min: [75.; 3],
            max: [75.; 3],
        };
        let p = limit_position(&distance, &source, [0., 150., 0.]);
        assert!((p[1] - 75.).abs() < 1e-4);
        let floor = Limiter {
            kind: 2,
            global: false,
            enabled: [false, false, true, false, false, false],
            min: [-999., 15., -999.],
            max: [999.; 3],
        };
        assert_eq!(limit_position(&floor, &source, [3., 2., 1.]), [3., 15., 1.]);
    }
}
