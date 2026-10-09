//! The "ppp" particle VM of the PC exe, translated: an effect instance runs
//! one PDATA program block. Each entry is a part type whose programs
//! (keyframed adds, integrators, matrix builders, emitters and draws) act
//! on a per-part work block at the byte offsets the data names.
//!
//! Stepping follows FUN_1402c28e0 / FUN_1404b0fd0 / FUN_1404b09e0: an
//! update pass every game frame, programs gated to even part frames, and a
//! draw pass that advances part frames and instance time by 0x1000 every
//! second game frame (one VM tick = 2.0 frame units, 30 Hz at 60 fps).
pub mod math;
mod mega_birth;
mod programs;

use math::{IDENTITY, Mat, Vec4};
use package_reader::effect::{Dpd, Program, i32_at, u32_at};
use std::sync::Arc;

/// Frame value parts saturate at (FUN_1404b09e0).
const FRAME_CAP: u32 = 0x6fff_f000;
const NO_LOOP: i32 = 0x7000_0000;

/// The CRT `rand()` the exe draws from (MSVC LCG). The game's own seed
/// and call history are not reproducible, so sequences differ in detail.
#[derive(Debug, Clone)]
pub struct Rng(pub u32);

impl Default for Rng {
    fn default() -> Self {
        Rng(1)
    }
}

impl Rng {
    pub fn rand(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.0 >> 16) & 0x7fff
    }

    /// FUN_1404dc710: three rand() low bytes as a 24-bit fraction.
    pub fn unit(&mut self) -> f32 {
        let a = self.rand() & 0xff;
        let b = self.rand() & 0xff;
        let c = self.rand() & 0xff;
        ((a << 16) | (b << 8) | c) as f32 * (1. / 16_777_216.)
    }
}

/// One drawn sprite quad. Corners are in the draw space of the effect
/// (view space: x right, y down, z forward, after the identity "ede0").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub corners: [[f32; 3]; 4],
    /// Texel coordinates (u = S/16, v = T/8 in the effect atlas).
    pub uv: [[f32; 2]; 4],
    /// Sprite colour × part colour, 1.0 = PS2 0x80.
    pub colour: [f32; 4],
    /// GS ALPHA register byte: 0x48 additive, 0x44 alpha blend.
    pub blend: u8,
    /// Texture base pointer from the frame's TEX0 (selects the atlas).
    pub tbp: u16,
}

pub(crate) struct Part {
    pub frame: u32,
    pub cur: Mat,
    pub draw: Mat,
    pub just_created: bool,
    /// Work variables from header +0xa0.
    pub vars: Vec<u8>,
    /// Current keyframe position per program (absolute in the DPD).
    pub cursors: Vec<usize>,
    /// pppRyjMegaBirth sub-particle pools by work offset u0.
    pub mega: std::collections::HashMap<usize, mega_birth::Pool>,
}

impl Part {
    pub fn f32(&self, o: usize) -> f32 {
        self.vars
            .get(o..o + 4)
            .map_or(0., |w| f32::from_le_bytes(w.try_into().unwrap()))
    }
    pub fn set_f32(&mut self, o: usize, v: f32) {
        if let Some(w) = self.vars.get_mut(o..o + 4) {
            w.copy_from_slice(&v.to_le_bytes());
        }
    }
    pub fn i32(&self, o: usize) -> i32 {
        self.f32(o).to_bits() as i32
    }
    pub fn set_i32(&mut self, o: usize, v: i32) {
        self.set_f32(o, f32::from_bits(v as u32));
    }
    pub fn i16(&self, o: usize) -> i16 {
        self.vars
            .get(o..o + 2)
            .map_or(0, |w| i16::from_le_bytes([w[0], w[1]]))
    }
    pub fn set_i16(&mut self, o: usize, v: i16) {
        if let Some(w) = self.vars.get_mut(o..o + 2) {
            w.copy_from_slice(&v.to_le_bytes());
        }
    }
    pub fn u8(&self, o: usize) -> u8 {
        self.vars.get(o).copied().unwrap_or(0)
    }
    pub fn set_u8(&mut self, o: usize, v: u8) {
        if let Some(b) = self.vars.get_mut(o) {
            *b = v;
        }
    }
    pub fn vec4(&self, o: usize) -> Vec4 {
        std::array::from_fn(|i| self.f32(o + 4 * i))
    }
    pub fn set_vec4(&mut self, o: usize, v: Vec4) {
        for (i, x) in v.into_iter().enumerate() {
            self.set_f32(o + 4 * i, x);
        }
    }
    pub fn zero(&mut self, o: usize, n: usize) {
        if let Some(w) = self.vars.get_mut(o..o + n) {
            w.fill(0);
        }
    }
}

struct Group {
    entry: usize,
    start: i32,
    parts: Vec<usize>,
}

/// Matrices the exe keeps in globals while an instance runs.
pub(crate) struct Ctx<'a> {
    pub dpd: &'a Dpd,
    /// ec20 instance world, ec60 view, ed60 = view·world, ede0 draw space.
    pub world: Mat,
    pub view: Mat,
    pub ed60: Mat,
    pub ede0: Mat,
    /// f010..f018: lengths of the instance world axes.
    pub scales: [f32; 3],
    pub colour: Vec4,
    /// Instance skip flag (+0xc4): odd half of a VM tick.
    pub skip: bool,
    pub rng: &'a mut Rng,
    pub quads: &'a mut Vec<Quad>,
    pub births: Vec<programs::Birth>,
    pub unsupported: &'a mut Vec<u32>,
}

/// A running effect.
pub struct Instance {
    dpd: Arc<Dpd>,
    pdata: usize,
    groups: Vec<Group>,
    parts: Vec<Option<Part>>,
    time: i32,
    end: i32,
    looping: bool,
    wait: f32,
    acc: f32,
    pub world: Mat,
    pub colour: Vec4,
    finished: bool,
    /// Program types met that this translation does not run.
    pub unsupported: Vec<u32>,
}

impl Instance {
    /// Starts PDATA `pdata` of `dpd` with world matrix `world` (FUN_1404b1600
    /// and FUN_1404b1280 composed by the caller) after `wait` frames.
    pub fn new(dpd: Arc<Dpd>, pdata: usize, world: Mat, wait: f32) -> Option<Self> {
        let data = dpd.pdata.get(pdata)?;
        let groups = data
            .entries
            .iter()
            .enumerate()
            .map(|(entry, e)| Group {
                entry,
                start: e.start_frame,
                parts: Vec::new(),
            })
            .collect();
        let (end, looping) = (data.end_time, data.looping);
        Some(Instance {
            dpd,
            pdata,
            groups,
            parts: Vec::new(),
            time: 0,
            end,
            looping,
            wait,
            acc: 0.,
            world,
            colour: [1.; 4],
            finished: false,
            unsupported: Vec::new(),
        })
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    pub fn live_parts(&self) -> usize {
        self.parts.iter().flatten().count()
    }

    /// The PDATA header's own transform (offset, angle, scale), which
    /// FUN_1404b1280 applies after the element/instance transform.
    pub fn local_matrix(&self) -> Mat {
        let d = &self.dpd.pdata[self.pdata];
        let mut m = math::rot_xyz([d.angle[0], d.angle[1], d.angle[2]]);
        for (c, s) in d.scale.iter().take(3).enumerate() {
            m[c] = math::scale(m[c], *s);
        }
        m[3] = [d.offset[0], d.offset[1], d.offset[2], 1.];
        m
    }

    /// One game frame: the update pass, then the draw pass (which advances
    /// time every second frame). Returns the quads drawn this frame.
    pub fn frame(&mut self, view: &Mat, rng: &mut Rng) -> Vec<Quad> {
        let mut quads = Vec::new();
        if self.finished {
            return quads;
        }
        if self.wait > 0. {
            self.wait -= 1.;
            if self.wait > 0. {
                return quads;
            }
        }
        self.acc += 1.;
        let skip = self.acc < 2.;
        if !skip {
            self.acc -= 2.;
        }
        let world = mul_world(&self.world, &self.local_matrix());
        let dpd = self.dpd.clone();
        let scales = std::array::from_fn(|c| {
            let a = world[c];
            (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
        });
        let mut unsupported = std::mem::take(&mut self.unsupported);
        let mut ctx = Ctx {
            dpd: &dpd,
            world,
            view: *view,
            ed60: math::mul(view, &world),
            ede0: IDENTITY,
            scales,
            colour: self.colour,
            skip,
            rng,
            quads: &mut quads,
            births: Vec::new(),
            unsupported: &mut unsupported,
        };
        self.update_pass(&mut ctx);
        self.draw_pass(&mut ctx, true);
        self.unsupported = unsupported;
        if !skip {
            self.time += 0x1000;
            if !self.looping && self.end <= self.time {
                self.finished = true;
                self.parts.clear();
                for g in &mut self.groups {
                    g.parts.clear();
                }
            }
        }
        quads
    }

    fn entry_programs(&self, group: usize) -> &[Program] {
        &self.dpd.pdata[self.pdata].entries[self.groups[group].entry].programs
    }

    /// FUN_1404b02f0 / FUN_1404aff20: a new part of `group` at frame 0,
    /// cursors on the first keyframes, create hooks run. A spawned part is
    /// inserted after the group's first part.
    fn create(&mut self, group: usize, ctx: &mut Ctx) -> usize {
        let entry = &ctx.dpd.pdata[self.pdata].entries[self.groups[group].entry];
        let mut part = Part {
            frame: 0,
            cur: IDENTITY,
            draw: IDENTITY,
            just_created: true,
            vars: vec![0; entry.work_size.saturating_sub(0xa0)],
            cursors: entry.programs.iter().map(|p| p.params).collect(),
            mega: Default::default(),
        };
        for p in &entry.programs {
            programs::create(p, &mut part, ctx);
        }
        let id = match self.parts.iter().position(Option::is_none) {
            Some(free) => {
                self.parts[free] = Some(part);
                free
            }
            None => {
                self.parts.push(Some(part));
                self.parts.len() - 1
            }
        };
        let list = &mut self.groups[group].parts;
        let at = list.len().min(1);
        list.insert(at, id);
        id
    }

    fn spawn_groups(&mut self, ctx: &mut Ctx) {
        for g in 0..self.groups.len() {
            if self.groups[g].start <= self.time {
                self.groups[g].start = i32::MAX;
                let id = self.create(g, ctx);
                if let Some(p) = self.parts[id].as_mut() {
                    p.just_created = false;
                }
            }
        }
    }

    fn births(&mut self, ctx: &mut Ctx) {
        for b in std::mem::take(&mut ctx.births) {
            if b.entry >= self.groups.len() {
                continue;
            }
            let id = self.create(b.entry, ctx);
            if let Some(p) = self.parts[id].as_mut() {
                p.set_vec4(b.position_var, b.position);
                p.set_vec4(b.velocity_var, b.velocity);
            }
        }
    }

    /// Cursor advance: when the next keyframe's time word equals the part
    /// frame, it becomes current (the word may lie past the keyframes, as
    /// in the exe).
    fn advance(dpd: &Dpd, part: &mut Part, j: usize, p: &Program) {
        let next = part.cursors[j] + p.param_size;
        if u32_at(&dpd.bytes, next).is_ok_and(|t| t == part.frame) {
            part.cursors[j] = next;
        }
    }

    /// FUN_1404b0fd0: spawn due groups, then run every update-only program
    /// over each group's parts.
    fn update_pass(&mut self, ctx: &mut Ctx) {
        self.spawn_groups(ctx);
        for g in 0..self.groups.len() {
            let programs: Vec<Program> = self.entry_programs(g).to_vec();
            for (j, p) in programs.iter().enumerate() {
                if !programs::has_update(p.kind) || programs::has_draw(p.kind) {
                    continue;
                }
                for id in self.groups[g].parts.clone() {
                    let Some(part) = self.parts[id].as_mut() else {
                        continue;
                    };
                    if part.just_created {
                        continue;
                    }
                    Self::advance(ctx.dpd, part, j, p);
                    let cursor = part.cursors[j];
                    programs::update(p, cursor, part, ctx);
                    self.births(ctx);
                }
            }
        }
    }

    /// FUN_1404b09e0: programs with a draw function (updating first when
    /// they have one), then the frame advance and lifetime check.
    fn draw_pass(&mut self, ctx: &mut Ctx, draw: bool) {
        for g in 0..self.groups.len() {
            if self.groups[g].parts.is_empty() {
                continue;
            }
            let programs: Vec<Program> = self.entry_programs(g).to_vec();
            for (j, p) in programs.iter().enumerate() {
                if !programs::has_draw(p.kind) {
                    continue;
                }
                for id in self.groups[g].parts.clone() {
                    let Some(part) = self.parts[id].as_mut() else {
                        continue;
                    };
                    if part.just_created {
                        continue;
                    }
                    Self::advance(ctx.dpd, part, j, p);
                    let cursor = part.cursors[j];
                    if programs::has_update(p.kind) {
                        programs::update(p, cursor, part, ctx);
                    }
                    if draw {
                        programs::draw(p, cursor, part, ctx);
                    }
                }
            }
            for id in self.groups[g].parts.clone() {
                let Some(part) = self.parts[id].as_mut() else {
                    continue;
                };
                if !ctx.skip {
                    if part.just_created {
                        part.just_created = false;
                    } else {
                        part.frame = (part.frame + 0x1000) & 0xffff_f000;
                        part.frame = part.frame.min(FRAME_CAP);
                    }
                } else if !part.just_created {
                    part.frame |= 1;
                }
            }
        }
        if ctx.skip {
            return;
        }
        // Lifetime: parts at or past the entry's total frame are destroyed
        // (loop ranges are not used by the effects translated here).
        for g in 0..self.groups.len() {
            let entry = &ctx.dpd.pdata[self.pdata].entries[self.groups[g].entry];
            if entry.loop_start < NO_LOOP && entry.loop_end < NO_LOOP {
                ctx.unsupported.push(0x1000_0000);
                continue;
            }
            let total = entry.total_frame as u32;
            let programs = entry.programs.clone();
            let mut keep = Vec::new();
            for id in self.groups[g].parts.clone() {
                let done = self.parts[id]
                    .as_ref()
                    .is_some_and(|p| total <= p.frame && !p.just_created);
                if done {
                    if let Some(mut part) = self.parts[id].take() {
                        for p in &programs {
                            programs::destroy(p, &mut part, ctx);
                        }
                    }
                } else {
                    keep.push(id);
                }
            }
            self.groups[g].parts = keep;
        }
    }
}

/// ec20 composed with the PDATA local transform (FUN_1404b1280 order).
fn mul_world(world: &Mat, local: &Mat) -> Mat {
    math::mul(world, local)
}

/// The instance world matrix for translation `t`, radians `r` and scale
/// `s` (FUN_1404b1280: identity → RotX → RotY → RotZ, axes scaled).
pub fn instance_matrix(t: [f32; 3], r: [f32; 3], s: [f32; 3]) -> Mat {
    let mut m = math::rot_radians(&IDENTITY, r);
    for c in 0..3 {
        m[c] = math::scale(m[c], s[c]);
    }
    m[3] = [t[0], t[1], t[2], 1.];
    m
}

pub(crate) fn key_i32(dpd: &Dpd, at: usize) -> i32 {
    i32_at(&dpd.bytes, at).unwrap_or(0)
}

#[cfg(test)]
mod tests;
