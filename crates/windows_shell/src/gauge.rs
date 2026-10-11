//! Sora's field gauge, translated from the executable's GAUGE_PLAYER object
//! (vtable 0x1405b2100): creation FUN_14017f270 / FUN_14017fe50 /
//! FUN_14017f9b0, per-frame logic FUN_140181270, ring masks FUN_14017f7d0,
//! bars FUN_14017f670, MP FUN_140181af0 and drive FUN_1401816c0. Layout units
//! are the PS2 512x416 2D space; every gauge sprite and mask adds an X origin
//! of 85.
//!
//! The ring fill uses the original's depth trick: depth-only masks are drawn
//! just before each ring, and depth-tested rings only show where their z is
//! greater than the mask's. `Canvas` reproduces that in software.
use package_reader::sequence::{Modifiers, Player, Quad, Sequence};
use player_sim::camera::wrap;

/// X origin added to every gauge sprite and mask (literal 0x55). Outside
/// 4:3 the HUD anchors right-half elements 85 units further right and
/// left-half ones 85 units further left (0x14017d870, 0x14017d9e0).
pub const X_ORIGIN: f32 = 85.;
/// The 4:3 layout centred in a 16:9 canvas: 512 * (16/9) / (4/3), so layout
/// x = 0 sits CANVAS_PAD units from the canvas's left edge.
pub const CANVAS_WIDTH: f32 = 512. * 4. / 3.;
pub const CANVAS_PAD: f32 = (CANVAS_WIDTH - 512.) / 2.;
pub const CANVAS_HEIGHT: f32 = 416.;
/// Ring angle per HP below 60 (pi/40, 0x14071b004).
const RING_RADIANS_PER_HP: f32 = 0.078_539_8;
/// Bar pixels per HP above 60 (3.0 at 0x1405a9cb8) and bar art width (100,
/// 0x140623be4).
const BAR_PIXELS_PER_HP: f32 = 3.;
const BAR_ART_WIDTH: f32 = 100.;
/// Drive wiper angle per gauge point (0.23 degrees, 0x14071b158).
const DRIVE_RADIANS_PER_POINT: f32 = 0.004_014_26;

/// Per-state tables (normal, low HP, KO): 0x14071afe0, 0x14071afec,
/// 0x14071aff8.
const ROOT_LOOP: [usize; 3] = [137, 132, 137];
const MAX_RING: [usize; 3] = [121, 122, 121];
const MAX_BAR: [usize; 3] = [124, 125, 124];
/// Face group by state (FUN_14017eba0): normal 3, low HP 0, KO 2.
const FACE: [usize; 3] = [3, 0, 2];
/// Drive level digits (0x14071b108).
const DRIVE_DIGITS: [usize; 10] = [28, 29, 30, 31, 32, 33, 34, 35, 36, 37];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Id {
    Root,
    MaxRing,
    ShadowRing,
    HpRing,
    MaxBar,
    ShadowBar,
    HpBar,
    EndCap,
    Face,
    MpCap,
    MpBackground,
    MpFill,
    MpEndCap,
    /// The MP-charge streak (0x568, group 27), shown while charging.
    MpStreak,
    DriveFrame,
    DriveFill,
    DriveDigit,
    Max,
}

struct Element {
    id: Id,
    /// Sheet index: 0 gauge, 1 face.
    sheet: usize,
    player: Player,
    queued: Option<usize>,
    parent: Option<usize>,
    offset: [f32; 2],
    /// Depth tested against masks (GREATER) when non-zero.
    z: i32,
    hidden: bool,
    modifiers: Modifiers,
    /// This frame's child context and base, filled while drawing.
    context: [f32; 2],
    base: [f32; 2],
}

/// Depth-only mask primitives. Quads rotate clockwise about their pivot.
#[derive(Clone, Copy, Debug, PartialEq)]
enum MaskKind {
    /// 100x100 quad centred on the ring.
    Clear,
    /// 64x64 quad with its bottom-right corner on the ring centre, plus a
    /// rectangle chosen by quadrant.
    Ring,
    /// 50x120 quad with its bottom-right corner on the pivot.
    Sector,
}

struct Mask {
    kind: MaskKind,
    /// Element whose context and base place the mask.
    anchor: usize,
    relative: [f32; 2],
    angle: f32,
    rect: Option<[f32; 4]>,
    z: i32,
    hidden: bool,
}

enum Node {
    Sprite(usize),
    Mask(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GaugeStats {
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    /// Drive gauge mode (0 none, 1 normal), partial 0..=100, full bars and
    /// maximum bars.
    pub drive_mode: u8,
    pub drive: u8,
    pub drive_level: u8,
    pub drive_max: u8,
    /// MP Charge: remaining and total (slot +0x1bc / +0x1c0); None when MP
    /// is not charging.
    pub mp_charge: Option<(f32, f32)>,
}

pub struct Gauge {
    elements: Vec<Element>,
    masks: Vec<Mask>,
    order: Vec<Node>,
    state: usize,
    last_hp: i32,
    shadow_hp: i32,
    last_level: u8,
    max_shown: bool,
    /// MP Charge is showing (the byte at +0x29 of FUN_140181af0).
    charging: bool,
}

/// A drawable this frame: sprite quads (depth tested when z != 0) or
/// depth-only polygons.
pub enum Draw {
    Quads {
        quads: Vec<Quad>,
        z: i32,
        sheet: usize,
    },
    Mask {
        polygons: Vec<[[f32; 2]; 4]>,
        z: i32,
    },
}

impl Gauge {
    /// `sheets[0]` is the gauge sequence; `sheets[1]`, when present, the
    /// character's face sequence.
    pub fn new(sheets: &[&Sequence], stats: &GaugeStats) -> Self {
        let seq = sheets[0];
        let mut g = Gauge {
            elements: Vec::new(),
            masks: Vec::new(),
            order: Vec::new(),
            state: Self::state_of(stats),
            last_hp: stats.hp,
            shadow_hp: stats.hp,
            last_level: stats.drive_level,
            max_shown: false,
            charging: false,
        };
        let root = g.sprite(seq, Id::Root, 136, Some(ROOT_LOOP[g.state]), None, 0);
        g.mask(MaskKind::Clear, root, 0);
        g.mask(MaskKind::Ring, root, 200);
        g.sprite(seq, Id::MaxRing, MAX_RING[g.state], None, Some(root), 100);
        g.mask(MaskKind::Ring, root, 600);
        g.sprite(seq, Id::ShadowRing, 121, None, Some(root), 500);
        g.mask(MaskKind::Ring, root, 2000);
        g.sprite(seq, Id::HpRing, 128, None, Some(root), 1000);
        g.sprite(seq, Id::MaxBar, MAX_BAR[g.state], None, Some(root), 0);
        g.sprite(seq, Id::ShadowBar, 127, None, Some(root), 0);
        g.sprite(seq, Id::HpBar, 129, None, Some(root), 0);
        g.sprite(seq, Id::EndCap, 123, None, Some(root), 0);
        if let Some(face) = sheets.get(1) {
            let i = g.sprite(face, Id::Face, FACE[g.state], None, Some(root), 0);
            g.elements[i].sheet = 1;
        }
        let cap = g.sprite(seq, Id::MpCap, 116, Some(117), Some(root), 0);
        g.sprite(seq, Id::MpBackground, 119, None, Some(cap), 0);
        g.sprite(seq, Id::MpFill, 115, None, Some(cap), 0);
        g.sprite(seq, Id::MpEndCap, 120, None, Some(cap), 0);
        // Created hidden; priority 7005.
        let streak = g.sprite(seq, Id::MpStreak, 27, None, Some(root), 0);
        g.elements[streak].hidden = true;
        let frame = g.sprite(seq, Id::DriveFrame, 21, Some(23), Some(root), 0);
        let sector = g.mask(MaskKind::Sector, frame, 6000);
        g.masks[sector].relative = [50., 100.];
        g.sprite(seq, Id::DriveFill, 25, None, Some(frame), 5000);
        let digit = DRIVE_DIGITS[stats.drive_level.min(9) as usize];
        let digit = g.sprite(seq, Id::DriveDigit, digit, None, Some(frame), 0);
        g.elements[digit].offset = [52., -2.];
        // Priority 7010: drawn after the 7000 elements.
        let max = g.sprite(seq, Id::Max, 20, None, Some(frame), 0);
        g.elements[max].hidden = true;
        g
    }

    fn sprite(
        &mut self,
        seq: &Sequence,
        id: Id,
        group: usize,
        queued: Option<usize>,
        parent: Option<usize>,
        z: i32,
    ) -> usize {
        self.elements.push(Element {
            id,
            sheet: 0,
            player: Player::new(seq, group),
            queued,
            parent,
            offset: [0.; 2],
            z,
            hidden: false,
            modifiers: Modifiers::default(),
            context: [0.; 2],
            base: [0.; 2],
        });
        self.order.push(Node::Sprite(self.elements.len() - 1));
        self.elements.len() - 1
    }

    fn mask(&mut self, kind: MaskKind, anchor: usize, z: i32) -> usize {
        self.masks.push(Mask {
            kind,
            anchor,
            relative: [0.; 2],
            angle: 0.,
            rect: None,
            z,
            hidden: kind == MaskKind::Ring,
        });
        self.order.push(Node::Mask(self.masks.len() - 1));
        self.masks.len() - 1
    }

    fn find(&self, id: Id) -> usize {
        self.elements.iter().position(|e| e.id == id).unwrap()
    }

    fn set_group(&mut self, seq: &Sequence, id: Id, group: usize) {
        let i = self.find(id);
        self.elements[i].player = Player::new(seq, group);
        self.elements[i].queued = None;
        self.elements[i].hidden = false;
    }

    fn set_face(&mut self, sheets: &[&Sequence], group: usize) {
        let Some(face) = sheets.get(1) else { return };
        let i = self.find(Id::Face);
        self.elements[i].player = Player::new(face, group);
        self.elements[i].hidden = false;
    }

    /// Low HP is HP <= MaxHP / 4 with integer division (0x1403c1d30).
    fn state_of(stats: &GaugeStats) -> usize {
        (stats.hp <= stats.max_hp / 4) as usize
    }

    /// Ring mask for value v (FUN_14017f7d0): hidden at 60 and above.
    fn ring_mask(&mut self, mask: usize, v: i32, end_cap: bool) {
        self.masks[mask].hidden = true;
        if v >= 60 {
            return;
        }
        let theta = wrap(v as f32 * RING_RADIANS_PER_HP);
        let m = &mut self.masks[mask];
        m.angle = theta;
        m.rect = Some(if v < 20 {
            [0., -50., 64., 100.]
        } else if v < 40 {
            [0., 0., 64., 50.]
        } else {
            [0., 50., 0., 0.]
        });
        m.hidden = false;
        if end_cap {
            let cap = self.find(Id::EndCap);
            self.elements[cap].modifiers.rotation = -theta;
            self.elements[cap].offset = [0., 0.];
        }
    }

    /// HP-family bar for value v (FUN_14017f670): grows left 3 px per HP
    /// above 60.
    fn bar(&mut self, id: Id, v: i32, end_cap: bool) {
        let i = self.find(id);
        self.elements[i].hidden = true;
        if v < 60 {
            return;
        }
        let len = (v - 60) as f32 * BAR_PIXELS_PER_HP;
        self.elements[i].modifiers.scale_x = len / BAR_ART_WIDTH;
        self.elements[i].hidden = false;
        if end_cap {
            let cap = self.find(Id::EndCap);
            self.elements[cap].modifiers.rotation = wrap(-4.712_389);
            self.elements[cap].offset = [-((len as i32) as f32), 0.];
        }
    }

    /// One 60 Hz frame of the gauge's own logic (FUN_140181270), then each
    /// element's player in creation order (FUN_14016b3c0).
    pub fn update(&mut self, sheets: &[&Sequence], stats: &GaugeStats) {
        let seq = sheets[0];
        let state = Self::state_of(stats);
        if state != self.state {
            self.state = state;
            let root = self.find(Id::Root);
            if self.elements[root].queued.is_some() {
                self.elements[root].queued = Some(ROOT_LOOP[state]);
            } else {
                self.set_group(seq, Id::Root, ROOT_LOOP[state]);
            }
            self.set_group(seq, Id::MaxRing, MAX_RING[state]);
            self.set_group(seq, Id::MaxBar, MAX_BAR[state]);
            self.set_face(sheets, FACE[state]);
        }
        if stats.hp != self.last_hp {
            self.shadow_hp = self.last_hp.max(stats.hp);
            self.last_hp = stats.hp;
            self.set_group(seq, Id::ShadowRing, 126);
            self.set_group(seq, Id::ShadowBar, 127);
        }
        let rings: Vec<usize> = (0..self.masks.len())
            .filter(|&i| self.masks[i].kind == MaskKind::Ring)
            .collect();
        self.ring_mask(rings[0], stats.max_hp, true);
        self.bar(Id::MaxBar, stats.max_hp, true);
        self.ring_mask(rings[1], self.shadow_hp, false);
        self.bar(Id::ShadowBar, self.shadow_hp, false);
        self.ring_mask(rings[2], stats.hp, false);
        self.bar(Id::HpBar, stats.hp, false);
        // MP Charge (FUN_140181af0): entering swaps the MP groups to
        // 110 / 113 / 109 / 114 (queued 111) and the Drive frame loop to 26,
        // and shows the streak; leaving restores 116 / 119 / 115 / 120
        // (queued 117) and 23.
        let charging = stats.mp_charge.is_some();
        if charging != self.charging {
            self.charging = charging;
            let (cap, bg, fill, end, queued, frame) = if charging {
                (110, 113, 109, 114, 111, 26)
            } else {
                (116, 119, 115, 120, 117, 23)
            };
            self.set_group(seq, Id::MpCap, cap);
            let i = self.find(Id::MpCap);
            self.elements[i].queued = Some(queued);
            self.set_group(seq, Id::MpBackground, bg);
            self.set_group(seq, Id::MpFill, fill);
            self.set_group(seq, Id::MpEndCap, end);
            if stats.drive_mode <= 1 {
                self.set_group(seq, Id::DriveFrame, frame);
            }
            let streak = self.find(Id::MpStreak);
            if charging && !matches!(stats.drive_mode, 2..=4) {
                self.set_group(seq, Id::MpStreak, 27);
            } else {
                self.elements[streak].hidden = true;
            }
        }
        let bg = self.find(Id::MpBackground);
        let fill = self.find(Id::MpFill);
        let end = self.find(Id::MpEndCap);
        match stats.mp_charge {
            Some((remaining, total)) => {
                // Charging: background 1.0, end cap at -100, fill by the
                // remaining charge (0x1403c20e0).
                self.elements[bg].modifiers.scale_x = 1.;
                self.elements[end].offset = [-100., 0.];
                self.elements[fill].modifiers.scale_x =
                    if total > 0. { remaining / total } else { 0. };
            }
            None => {
                // Not charging: 1 MP = 1 px (0.1 at 0x1406239f8).
                self.elements[bg].modifiers.scale_x = (stats.max_mp as f32 + 0.1) / 100.;
                self.elements[fill].modifiers.scale_x = (stats.mp as f32 + 0.1) / 100.;
                self.elements[end].offset = [-(((stats.max_mp as f32 + 0.1) as i32) as f32), 0.];
            }
        }
        // Drive, normal mode: a full gauge (100) uses 200 (0x140585efc).
        let d = if stats.drive == 100 {
            200.
        } else {
            stats.drive as f32
        };
        let sector = (0..self.masks.len())
            .find(|&i| self.masks[i].kind == MaskKind::Sector)
            .unwrap();
        self.masks[sector].angle = wrap(d * DRIVE_RADIANS_PER_POINT);
        self.masks[sector].hidden = stats.drive_mode == 0;
        if stats.drive_level != self.last_level {
            self.last_level = stats.drive_level;
            let digit = DRIVE_DIGITS[stats.drive_level.min(9) as usize];
            self.set_group(seq, Id::DriveDigit, digit);
        }
        let max = self.find(Id::Max);
        if stats.drive_level < stats.drive_max {
            self.elements[max].hidden = true;
            self.max_shown = false;
        } else if !self.max_shown {
            self.max_shown = true;
            self.set_group(seq, Id::Max, 20);
        }
        if stats.drive_mode == 0 {
            for id in [Id::DriveFrame, Id::DriveFill, Id::DriveDigit] {
                let i = self.find(id);
                self.elements[i].hidden = true;
            }
        }
        // A finished one-shot group switches to its queued group and
        // advances once, or the sprite hides.
        for e in &mut self.elements {
            let seq = sheets[e.sheet];
            e.player.update(seq, 1.);
            if seq.groups[e.player.group].play_count != 0 && e.player.finished() {
                if let Some(next) = e.queued.take() {
                    e.player = Player::new(seq, next);
                    e.player.update(seq, 1.);
                } else {
                    e.hidden = true;
                }
            }
        }
    }

    /// This frame's drawables in draw order, in canvas layout units (X origin
    /// applied). A child sits at its parent's child context + offset + base.
    pub fn draw(&mut self, sheets: &[&Sequence]) -> Vec<Draw> {
        let mut out = Vec::new();
        for node in &self.order {
            match *node {
                Node::Sprite(i) => {
                    let base = self.elements[i].parent.map_or([0.; 2], |p| {
                        let p = &self.elements[p];
                        [
                            p.context[0] + p.offset[0] + p.base[0],
                            p.context[1] + p.offset[1] + p.base[1],
                        ]
                    });
                    let e = &mut self.elements[i];
                    let seq = sheets[e.sheet];
                    e.base = base;
                    e.context = seq
                        .child_context(&e.player, &e.modifiers)
                        .unwrap_or([0.; 2]);
                    if e.hidden {
                        continue;
                    }
                    let origin = [X_ORIGIN + e.offset[0] + base[0], e.offset[1] + base[1]];
                    out.push(Draw::Quads {
                        quads: seq.quads(&e.player, origin, &e.modifiers),
                        z: e.z,
                        sheet: e.sheet,
                    });
                }
                Node::Mask(m) => {
                    let mask = &self.masks[m];
                    if mask.hidden {
                        continue;
                    }
                    let a = &self.elements[mask.anchor];
                    let pivot = [
                        X_ORIGIN + a.context[0] + a.base[0] + mask.relative[0],
                        a.context[1] + a.base[1] + mask.relative[1],
                    ];
                    let (s, c) = mask.angle.sin_cos();
                    // Clockwise on a Y-down screen.
                    let place =
                        |x: f32, y: f32| [pivot[0] + x * c - y * s, pivot[1] + x * s + y * c];
                    let quad = |[x0, y0, x1, y1]: [f32; 4]| {
                        [place(x0, y0), place(x1, y0), place(x0, y1), place(x1, y1)]
                    };
                    let mut polygons = vec![match mask.kind {
                        MaskKind::Clear => quad([-50., -50., 50., 50.]),
                        MaskKind::Ring => quad([-64., -64., 0., 0.]),
                        MaskKind::Sector => quad([-50., -120., 0., 0.]),
                    }];
                    if let Some([dx, dy, w, h]) = mask.rect
                        && w > 0.
                        && h > 0.
                    {
                        let (x0, y0) = (pivot[0] + dx, pivot[1] + dy);
                        polygons.push([[x0, y0], [x0 + w, y0], [x0, y0 + h], [x0 + w, y0 + h]]);
                    }
                    out.push(Draw::Mask {
                        polygons,
                        z: mask.z,
                    });
                }
            }
        }
        out
    }
}

/// Software rasterizer for the gauge: a straight-alpha RGBA canvas plus a
/// depth buffer emulating the GS depth trick (masks write depth only;
/// depth-tested sprites draw where their z is greater).
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    /// Canvas pixels per layout unit.
    pub scale: f32,
    pub rgba: Vec<u8>,
    color: Vec<[f32; 4]>,
    depth: Vec<i32>,
    dirty: Option<[usize; 4]>,
}

pub struct Texture<'a> {
    pub rgba: &'a [u8],
    pub size: [usize; 2],
    /// Texture texels per sequence texel.
    pub texel_scale: f32,
}

impl Texture<'_> {
    /// Bilinear sample in texels, clamped to the edge.
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let [tw, th] = self.size;
        let (u, v) = (u - 0.5, v - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = (u - x0, v - y0);
        let at = |x: f32, y: f32| {
            let x = (x.max(0.) as usize).min(tw - 1);
            let y = (y.max(0.) as usize).min(th - 1);
            let o = (y * tw + x) * 4;
            [0, 1, 2, 3].map(|c| self.rgba[o + c] as f32 / 255.)
        };
        let (a, b, c, d) = (
            at(x0, y0),
            at(x0 + 1., y0),
            at(x0, y0 + 1.),
            at(x0 + 1., y0 + 1.),
        );
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            top + (bottom - top) * fy
        })
    }
}

impl Canvas {
    pub fn new(scale: f32) -> Self {
        let width = (CANVAS_WIDTH * scale).ceil() as usize;
        let height = (CANVAS_HEIGHT * scale).ceil() as usize;
        Canvas {
            width,
            height,
            scale,
            rgba: vec![0; width * height * 4],
            color: vec![[0.; 4]; width * height],
            depth: vec![0; width * height],
            dirty: None,
        }
    }

    fn clear(&mut self) {
        if let Some([x0, y0, x1, y1]) = self.dirty.take() {
            for y in y0..y1 {
                let row = y * self.width;
                self.color[row + x0..row + x1].fill([0.; 4]);
                self.depth[row + x0..row + x1].fill(0);
                self.rgba[(row + x0) * 4..(row + x1) * 4].fill(0);
            }
        }
    }

    /// Pixel bounds of a triangle clipped to the canvas, marked dirty.
    fn bounds(&mut self, p: &[[f32; 2]; 3]) -> Option<[usize; 4]> {
        let min = |k: usize| p.iter().map(|v| v[k]).fold(f32::MAX, f32::min);
        let max = |k: usize| p.iter().map(|v| v[k]).fold(f32::MIN, f32::max);
        let x0 = min(0).floor().max(0.) as usize;
        let y0 = min(1).floor().max(0.) as usize;
        let x1 = (max(0).ceil().max(0.) as usize).min(self.width);
        let y1 = (max(1).ceil().max(0.) as usize).min(self.height);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let b = [x0, y0, x1, y1];
        self.dirty = Some(match self.dirty {
            None => b,
            Some(d) => [d[0].min(x0), d[1].min(y0), d[2].max(x1), d[3].max(y1)],
        });
        Some(b)
    }

    /// Calls `f(canvas, pixel index, barycentric weights)` for each pixel
    /// centre inside the triangle.
    fn raster(&mut self, p: [[f32; 2]; 3], mut f: impl FnMut(&mut Self, usize, [f32; 3])) {
        let area =
            (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
        if area.abs() < 1e-6 {
            return;
        }
        let Some(b) = self.bounds(&p) else { return };
        for y in b[1]..b[3] {
            for x in b[0]..b[2] {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let edge = |a: [f32; 2], c: [f32; 2]| {
                    (c[0] - a[0]) * (py - a[1]) - (px - a[0]) * (c[1] - a[1])
                };
                let w = [
                    edge(p[1], p[2]) / area,
                    edge(p[2], p[0]) / area,
                    edge(p[0], p[1]) / area,
                ];
                if w.iter().all(|&v| v >= 0.) {
                    f(self, y * self.width + x, w);
                }
            }
        }
    }

    /// Clears last frame's area and draws this frame; `rgba` then holds
    /// straight-alpha pixels.
    pub fn render(&mut self, draws: &[Draw], textures: &[Texture]) {
        self.clear();
        let k = self.scale;
        for draw in draws {
            match draw {
                Draw::Mask { polygons, z } => {
                    for q in polygons {
                        let q = q.map(|[x, y]| [(x + CANVAS_PAD) * k, y * k]);
                        for tri in [[q[0], q[1], q[2]], [q[1], q[3], q[2]]] {
                            self.raster(tri, |c, i, _| c.depth[i] = *z);
                        }
                    }
                }
                Draw::Quads { quads, z, sheet } => {
                    for quad in quads {
                        self.quad(quad, *z, &textures[*sheet]);
                    }
                }
            }
        }
        if let Some([x0, y0, x1, y1]) = self.dirty {
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * self.width + x;
                    let c = self.color[i];
                    let o = i * 4;
                    if c[3] > 0. {
                        for k in 0..3 {
                            self.rgba[o + k] = ((c[k] / c[3]).min(1.) * 255. + 0.5) as u8;
                        }
                        self.rgba[o + 3] = (c[3].min(1.) * 255. + 0.5) as u8;
                    } else {
                        self.rgba[o..o + 4].fill(0);
                    }
                }
            }
        }
    }

    fn quad(&mut self, quad: &Quad, z: i32, texture: &Texture) {
        let k = self.scale;
        let q = quad.corners.map(|[x, y]| [(x + CANVAS_PAD) * k, y * k]);
        let [u0, v0, u1, v1] = quad.uv.map(|v| v as f32 * texture.texel_scale);
        let uv = [[u0, v0], [u1, v0], [u0, v1], [u1, v1]];
        for t in [[0usize, 1, 2], [1, 3, 2]] {
            self.raster(t.map(|i| q[i]), |c, i, w| {
                if z != 0 && z <= c.depth[i] {
                    return;
                }
                let mix =
                    |f: &dyn Fn(usize) -> f32| f(t[0]) * w[0] + f(t[1]) * w[1] + f(t[2]) * w[2];
                let tex = texture.sample(mix(&|j| uv[j][0]), mix(&|j| uv[j][1]));
                let vc: [f32; 4] = std::array::from_fn(|ch| mix(&|j| quad.colors[j][ch]));
                let a = (tex[3] * vc[3]).clamp(0., 1.);
                if a <= 0. {
                    return;
                }
                let src: [f32; 3] = std::array::from_fn(|ch| (tex[ch] * vc[ch]).min(1.));
                let dst = &mut c.color[i];
                match quad.blend {
                    1 => {
                        for ch in 0..3 {
                            dst[ch] = (dst[ch] + src[ch] * a).min(1.);
                        }
                        dst[3] = dst[3].max(a);
                    }
                    2 => {
                        for ch in 0..3 {
                            dst[ch] = (dst[ch] - src[ch] * a).max(0.);
                        }
                    }
                    _ => {
                        for ch in 0..3 {
                            dst[ch] = src[ch] * a + dst[ch] * (1. - a);
                        }
                        dst[3] = a + dst[3] * (1. - a);
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_hide_depth_tested_pixels_only() {
        let mut canvas = Canvas::new(1.);
        let white = [255u8; 16];
        let texture = Texture {
            rgba: &white,
            size: [2, 2],
            texel_scale: 1.,
        };
        let square = |x: f32| Quad {
            corners: [[x, 0.], [x + 4., 0.], [x, 4.], [x + 4., 4.]],
            uv: [0, 0, 2, 2],
            colors: [[1.; 4]; 4],
            blend: 0,
        };
        let draws = [
            Draw::Mask {
                polygons: vec![[[0., 0.], [4., 0.], [0., 4.], [4., 4.]]],
                z: 200,
            },
            Draw::Quads {
                quads: vec![square(0.), square(8.)],
                z: 100,
                sheet: 0,
            },
            Draw::Quads {
                quads: vec![square(0.)],
                z: 300,
                sheet: 0,
            },
        ];
        canvas.render(&draws, std::slice::from_ref(&texture));
        let pad = CANVAS_PAD.ceil() as usize;
        let alpha = |c: &Canvas, x: usize, y: usize| c.rgba[(y * c.width + x + pad) * 4 + 3];
        // z 100 is hidden by the z 200 mask; z 300 shows; unmasked shows.
        assert_eq!(alpha(&canvas, 1, 1), 255);
        assert_eq!(alpha(&canvas, 9, 1), 255);
        canvas.render(&draws[..2], std::slice::from_ref(&texture));
        assert_eq!(alpha(&canvas, 1, 1), 0);
        assert_eq!(alpha(&canvas, 9, 1), 255);
    }

    #[test]
    fn low_hp_follows_integer_quarter() {
        let stats = |hp| GaugeStats {
            hp,
            max_hp: 20,
            mp: 100,
            max_mp: 100,
            drive_mode: 1,
            drive: 0,
            drive_level: 0,
            drive_max: 3,
            mp_charge: None,
        };
        assert_eq!(Gauge::state_of(&stats(5)), 1);
        assert_eq!(Gauge::state_of(&stats(6)), 0);
    }
}
