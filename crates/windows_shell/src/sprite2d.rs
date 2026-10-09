//! dk::Sprite / dk::SpriteMessage as the menus use them: a SEQD player
//! with a queued group (+0x1d8), a parent chain (FUN_140300bd0: base =
//! parent context + offset + base; flags 0x200|0x400 inherit the parent's
//! context alpha), and an optional text drawn by FUN_14016ae30 in font 1
//! (sys). Sprites update and draw in ascending priority, then creation
//! order.
use crate::command::Font;
use crate::dialogue;
use package_reader::sequence::{Modifiers, Player, Quad, Sequence, color};

/// System font reference height and non-JP line spacing (command-spec
/// §5.3).
const FONT_REFERENCE: f32 = 18.;
const LINE_SPACING: f32 = 5.;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(pub usize);

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub bytes: Vec<u8>,
    /// +0x208, PS2 0xAABBGGRR.
    pub colour: u32,
    /// +0x20c (the creation group's TextScale, else 15).
    pub size: f32,
    /// +0x214: 0 left, 1 right, 2 centred.
    pub align: u8,
    /// +0x200 / +0x204.
    pub offset: [f32; 2],
    /// Flag 0x10: add the group's (LightPositionX, TextPositionY).
    pub group_position: bool,
    /// Flag 4: colour from the animation; flag 8: keep +0x208's RGB.
    pub anim_colour: bool,
    pub keep_rgb: bool,
}

impl Text {
    /// FUN_14016aa50's defaults: flags 4|8|0x10, colour 0x80e5e5e5, left.
    pub fn new(bytes: Vec<u8>, size: f32) -> Self {
        Self {
            bytes,
            colour: 0x80e5e5e5,
            size,
            align: 0,
            offset: [0.; 2],
            group_position: true,
            anim_colour: true,
            keep_rgb: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sprite {
    pub prio: u32,
    /// Index of the art sheet the caller binds.
    pub sheet: usize,
    pub player: Player,
    pub queued: Option<usize>,
    pub base: [f32; 2],
    pub offset: [f32; 2],
    pub parent: Option<Id>,
    /// Flags 0x200|0x400: multiplier (1, 1, 1, parent context alpha).
    pub inherit_alpha: bool,
    /// Tz::MenuItem rows die with their parent (FUN_1402f76b0).
    pub die_with_parent: bool,
    pub multiplier: [f32; 4],
    pub text: Option<Text>,
    pub dead: bool,
    /// Set by a close: the sprite dies when its play-once group ends.
    pub die_when_done: bool,
    /// Player scale multipliers (+0x178 / +0x17c; gauge fills).
    pub scale: [f32; 2],
    /// The player's child context and colour as last found (FUN_140192be0
    /// keeps them while no IsActive animation runs).
    pub ctx: [f32; 2],
    pub ctx_color: [f32; 4],
}

impl Sprite {
    fn refresh(&mut self, seq: &Sequence) {
        let m = Modifiers::default();
        if let Some(c) = seq.child_context(&self.player, &m) {
            self.ctx = c;
        }
        if let Some(c) = seq
            .context_color(&self.player, &m)
            .or_else(|| seq.current_color(&self.player))
        {
            self.ctx_color = c;
        }
    }
}

#[derive(Default)]
pub struct Sprites {
    pub list: Vec<Sprite>,
}

/// What one draw item samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    Art(usize),
    Font,
}

/// Splits encoded text into lines at 0x02 and drops other control codes
/// with their parameters (0x01 stays: it is a narrow space).
pub fn lines(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new()];
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x02 {
            out.push(Vec::new());
            i += 1;
        } else if b == 0x01 || b >= 0x20 {
            out.last_mut().unwrap().push(b);
            i += 1;
        } else {
            i += dialogue::code_length(b).max(1);
        }
    }
    out
}

impl Sprites {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &mut self,
        seq: &Sequence,
        prio: u32,
        sheet: usize,
        group: usize,
        queued: Option<usize>,
        text: Option<Text>,
    ) -> Id {
        self.list.push(Sprite {
            prio,
            sheet,
            player: Player::new(seq, group),
            queued,
            base: [0.; 2],
            offset: [0.; 2],
            parent: None,
            inherit_alpha: false,
            die_with_parent: false,
            multiplier: [1.; 4],
            text,
            dead: false,
            die_when_done: false,
            scale: [1.; 2],
            ctx: [0.; 2],
            ctx_color: [1.; 4],
        });
        let id = Id(self.list.len() - 1);
        self.list[id.0].refresh(seq);
        id
    }

    pub fn get(&self, id: Id) -> &Sprite {
        &self.list[id.0]
    }

    pub fn get_mut(&mut self, id: Id) -> &mut Sprite {
        &mut self.list[id.0]
    }

    /// FUN_140300bd0.
    pub fn attach(&mut self, child: Id, parent: Id) {
        let s = self.get_mut(child);
        s.parent = Some(parent);
        s.inherit_alpha = true;
    }

    /// Restart on `group` with `queued` next.
    pub fn set(&mut self, seq: &Sequence, id: Id, group: usize, queued: Option<usize>) {
        let s = self.get_mut(id);
        s.player = Player::new(seq, group);
        s.queued = queued;
        s.die_when_done = false;
        s.refresh(seq);
    }

    /// Plays a close group, then dies (FUN_140300c50(·, group) with no
    /// follow-up).
    pub fn close(&mut self, seq: &Sequence, id: Id, group: usize) {
        self.set(seq, id, group, None);
        self.get_mut(id).die_when_done = true;
    }

    pub fn kill(&mut self, id: Id) {
        self.get_mut(id).dead = true;
    }

    pub fn alive(&self, id: Id) -> bool {
        !self.get(id).dead
    }

    /// Settled in a play-once group with nothing queued.
    pub fn done(&self, id: Id) -> bool {
        let s = self.get(id);
        s.dead || (s.player.finished() && s.queued.is_none())
    }

    fn order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.list.len()).collect();
        order.sort_by_key(|&i| self.list[i].prio);
        order
    }

    /// One frame for every live sprite.
    pub fn update(&mut self, seq: &Sequence) {
        for i in self.order() {
            let parent_dead = self.list[i].parent.is_some_and(|p| self.list[p.0].dead);
            let s = &mut self.list[i];
            if s.dead {
                continue;
            }
            if s.die_with_parent && parent_dead {
                s.dead = true;
                continue;
            }
            s.player.update(seq, 1.);
            if seq.groups[s.player.group].play_count != 0 && s.player.finished() {
                if let Some(next) = s.queued.take() {
                    s.player = Player::new(seq, next);
                    s.player.update(seq, 1.);
                } else if s.die_when_done {
                    s.dead = true;
                }
            }
            s.refresh(seq);
        }
    }

    fn modifiers(&self, i: usize) -> Modifiers {
        let s = &self.list[i];
        let mut m = Modifiers {
            color: s.multiplier,
            scale_x: s.scale[0],
            scale_y: s.scale[1],
            ..Modifiers::default()
        };
        if s.inherit_alpha
            && let Some(p) = s.parent
        {
            let pm = self.modifiers(p.0);
            let a = self.list[p.0].ctx_color[3] * pm.color[3];
            m.color = [s.multiplier[0], s.multiplier[1], s.multiplier[2], a];
        }
        m
    }

    /// Resolved base (FUN_14016abf0): the parent's context + offset + base.
    pub fn resolved_base(&self, id: Id) -> [f32; 2] {
        let s = self.get(id);
        match s.parent {
            Some(p) => {
                let pb = self.resolved_base(p);
                let ps = self.get(p);
                [
                    pb[0] + ps.offset[0] + ps.ctx[0],
                    pb[1] + ps.offset[1] + ps.ctx[1],
                ]
            }
            None => s.base,
        }
    }

    /// Where this sprite's children sit (its context in screen space).
    pub fn context(&self, id: Id) -> [f32; 2] {
        let b = self.resolved_base(id);
        let s = self.get(id);
        [b[0] + s.offset[0] + s.ctx[0], b[1] + s.offset[1] + s.ctx[1]]
    }

    /// The draw list: art, then text, per sprite in priority order.
    pub fn draw(&self, seq: &Sequence, font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
        self.draw_where(seq, font, |_| true)
    }

    /// `draw` for the sprites whose priority passes `keep`.
    pub fn draw_where(
        &self,
        seq: &Sequence,
        font: &Font,
        keep: impl Fn(u32) -> bool,
    ) -> Vec<(Sheet, Vec<Quad>)> {
        let mut out = Vec::new();
        for i in self.order() {
            let s = &self.list[i];
            if !keep(s.prio) || s.dead || s.parent.is_some_and(|p| self.list[p.0].dead) {
                continue;
            }
            let id = Id(i);
            let m = self.modifiers(i);
            let b = self.resolved_base(id);
            let origin = [b[0] + s.offset[0], b[1] + s.offset[1]];
            out.push((Sheet::Art(s.sheet), seq.quads(&s.player, origin, &m)));
            let Some(t) = &s.text else { continue };
            // FUN_14016ae30.
            let mut c = [s.ctx[0] + s.offset[0], s.ctx[1] + s.offset[1]];
            let g = &seq.groups[s.player.group];
            if t.group_position {
                c[0] += g.light_position_x as f32;
                c[1] += g.text_position_y as f32;
            }
            let mut rgba = color(t.colour);
            if t.anim_colour {
                let anim: [f32; 4] = std::array::from_fn(|k| s.ctx_color[k] * m.color[k]);
                rgba = if t.keep_rgb {
                    [rgba[0], rgba[1], rgba[2], anim[3]]
                } else {
                    anim
                };
                rgba = std::array::from_fn(|k| (rgba[k] * m.color[k]).min(255. / 128.));
            }
            let x = (t.offset[0].trunc() + b[0] + c[0]).trunc();
            let y = (t.offset[1].trunc() + b[1] + c[1]).trunc();
            out.push((
                Sheet::Font,
                text_quads(
                    &t.bytes,
                    [x, y],
                    t.size,
                    t.align,
                    g.ui_padding as f32,
                    rgba,
                    true,
                    font,
                ),
            ));
        }
        out
    }
}

/// System font text as FUN_1403aed80 / FUN_1403afd90 / FUN_1403b0280 /
/// FUN_1404070f0 lay it out (fonts 0..2 share the sys glyphs, cell 18x24,
/// reference 18, spacing 5): `origin` is the top of the first line; each
/// glyph's cell bottom sits at line top + lineH + 2. Alignment 1 / 2 moves
/// left by the widest line's width / half of it. Font 1 (`shadow`) draws a
/// black glyph at (+2, +1) with half alpha under each glyph.
#[allow(clippy::too_many_arguments)]
pub fn text_quads(
    bytes: &[u8],
    origin: [f32; 2],
    size: f32,
    align: u8,
    extra: f32,
    rgba: [f32; 4],
    shadow: bool,
    font: &Font,
) -> Vec<Quad> {
    let k = size / FONT_REFERENCE;
    let line_h = ((FONT_REFERENCE * k + 0.5) as i32).max((FONT_REFERENCE * k) as i32) as f32;
    let (qw, qh) = (
        ((18. * 16. * k) as i32) as f32 / 16.,
        ((24. * 16. * k) as i32) as f32 / 16.,
    );
    let narrow = (k * 6. + 0.5) as i32 as f32;
    let advance = |w: u8| (w as f64 * k as f64 + 0.5) as i32 as f32;
    let text: Vec<u8> = bytes.iter().copied().take_while(|&b| b != 0x10).collect();
    let lines = lines(&text);
    let width = |line: &[u8]| -> f32 {
        (0..line.len())
            .map(|i| match line[i] {
                0x01 | 0x20 => narrow,
                _ => dialogue::glyph_index(line, i)
                    .and_then(|g| font.widths.get(g))
                    .map_or(0., |&w| advance(w)),
            })
            .sum()
    };
    let w = lines.iter().map(|l| width(l)).fold(0., f32::max);
    let x0 = match align {
        1 => origin[0] - w,
        2 => origin[0] - (w as i32 >> 1) as f32,
        _ => origin[0],
    };
    let shadow_rgba = [0., 0., 0., ((rgba[3] * 128.) as i32 >> 1) as f32 / 128.];
    let mut out = Vec::new();
    for (j, line) in lines.iter().enumerate() {
        let top = origin[1] + j as f32 * (line_h + LINE_SPACING + extra) + line_h + 2. - qh;
        let mut x = x0;
        for i in 0..line.len() {
            if matches!(line[i], 0x01 | 0x20) {
                x += narrow;
                continue;
            }
            let Some(g) = dialogue::glyph_index(line, i) else {
                continue;
            };
            let Some(&gw) = font.widths.get(g) else {
                continue;
            };
            let (u, v) = (((g % 280) % 28) as i32 * 18, ((g % 280) / 28) as i32 * 24);
            let quad = |dx: f32, dy: f32, colour: [f32; 4]| Quad {
                corners: [
                    [x + dx, top + dy],
                    [x + dx + qw, top + dy],
                    [x + dx, top + dy + qh],
                    [x + dx + qw, top + dy + qh],
                ],
                uv: [u, v, u + 18, v + 24],
                colors: [colour; 4],
                blend: 0,
            };
            if shadow {
                out.push(quad(2., 1., shadow_rgba));
            }
            out.push(quad(0., 0., rgba));
            x += advance(gw);
        }
    }
    out
}
/// Tz::LightCur's motion (FUN_1402faf40 / FUN_1402fb3f0 / FUN_14034b890):
/// the first attach snaps; a re-target glides from the last drawn position
/// over 30 ticks with f(s) = 2.74981(s^3 - 2s^2 + s) + (3s^2 - 2s^3).
#[derive(Clone, Debug, Default)]
pub struct LightCur {
    pub id: Option<Id>,
    from: Option<[f32; 2]>,
    tick: f32,
    drawn: Option<[f32; 2]>,
}

pub fn glide(s: f32) -> f32 {
    2.74981 * (s * s * s - 2. * s * s + s) + (3. * s * s - 2. * s * s * s)
}

impl LightCur {
    pub fn new(id: Id) -> Self {
        Self {
            id: Some(id),
            ..Self::default()
        }
    }

    pub fn retarget(&mut self) {
        self.from = self.drawn;
        self.tick = 0.;
    }

    /// Places the orb for this frame toward `target`: the glide clock
    /// advances (clamped at 30) before the draw, and each axis moves by
    /// (int)(f(t/30) x distance).
    pub fn place(&mut self, sprites: &mut Sprites, target: [f32; 2]) {
        let Some(id) = self.id else { return };
        self.tick = (self.tick + 1.).min(30.);
        let at = match self.from {
            Some(from) => {
                let f = glide(self.tick / 30.);
                [
                    from[0] + ((target[0] - from[0]) * f).trunc(),
                    from[1] + ((target[1] - from[1]) * f).trunc(),
                ]
            }
            None => target,
        };
        self.drawn = Some(at);
        sprites.get_mut(id).base = at;
    }

    /// Instant removal (vtable +0x20).
    pub fn kill(&mut self, sprites: &mut Sprites) {
        if let Some(id) = self.id.take() {
            sprites.kill(id);
        }
        self.from = None;
        self.drawn = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_split_and_drop_codes() {
        // "AB" newline "C" then a colour code with its parameters.
        let l = lines(&[0x2e, 0x2f, 0x02, 0x30, 0x07, 0xf0, 0xf0, 0xf0, 0xf0]);
        assert_eq!(l, vec![vec![0x2e, 0x2f], vec![0x30]]);
    }
}
