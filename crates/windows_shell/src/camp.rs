//! The camp (pause) menu's top level, translated from the camp process
//! (build FUN_140306040 / FUN_140306790 / FUN_140367b50, state machine
//! FUN_140303260 / FUN_1403037d0, per-frame driver FUN_140303cd0, list
//! input FUN_1402f9440, help FUN_1402fa1d0, digits FUN_1402fb450 /
//! FUN_140324690, LightCur FUN_1402fb3f0). Everything draws from
//! `menu/<lang>/camp.2ld` sequence 1 in layout space (x origin 0).
//! Sub-screens and the 3D party models are not built: deciding an entry
//! plays the decide sound only.
use crate::command::Font;
use crate::sprite2d::{self, Id, LightCur, Sprites, Text};
use package_reader::layout::{Layout, LayoutTask};
use package_reader::sequence::{Modifiers, Player, Quad, Sequence};

/// The camp sprites' SEQD (DAT_140743fd4) and image.
pub const SEQUENCE: usize = 1;
const ART: usize = 0;

/// Layout scenes (L2): open, idle, close.
const SCENE_OPEN: usize = 0;
const SCENE_IDLE: usize = 1;
const SCENE_CLOSE: usize = 2;
/// Layout elements the help text follows (open / idle / close).
const HELP_PARENTS: [usize; 3] = [39, 40, 41];

/// Priorities (draw order: ascending, then creation).
const PRIO_FRAME: u32 = 0x7533;
const PRIO_PANEL: u32 = 0x753b;
const PRIO_ROW: u32 = 0x753c;
const PRIO_BAR: u32 = 0x753d;
const PRIO_LIGHT: u32 = 0x754a;

/// Row groups (entry table 0x1405badb0).
const ROW_APPEAR: usize = 133;
const ROW_CLOSE: usize = 135;
const ROW_IDLE: usize = 134;
const ROW_SELECTED: usize = 132;
const ROW_TEXT_SIZE: f32 = 15.;

const LIGHT: [usize; 3] = [27, 28, 29];
const HELP_IN: usize = 116;
const FRAME: [usize; 3] = [46, 47, 48];
const PROMPT: [usize; 3] = [351, 352, 353];
const MUNNY: [usize; 3] = [107, 108, 109];
const TIME: [usize; 3] = [113, 114, 115];
const MUNNY_DIGIT: usize = 51;
const TIME_DIGIT: usize = 61;
const DIGIT_BLANK: usize = 31;
const COLON: usize = 71;
/// Party status block (FUN_140367b50 mode 0).
const STATUS: [usize; 3] = [101, 102, 103];
const BLOCK: usize = 93;
const SUB_ANCHOR: usize = 90;
/// LV, HP, MP style groups (FUN_140304d70(2/4/6)).
const STYLES: [usize; 3] = [124, 121, 118];
const GAUGE_FRAME: usize = 95;
const GAUGE_BARS: [usize; 2] = [98, 99];
const NAME_SIZE: f32 = 15.;

/// se000 sounds (FUN_1402eb740 via 0x1405ba694): open, move, decide, close.
pub const SOUND_OPEN: u32 = 3;
pub const SOUND_CURSOR: u32 = 1;
pub const SOUND_DECIDE: u32 = 2;
pub const SOUND_CLOSE: u32 = 4;

/// Logical bits (pad mode 7 while a menu runs).
const UP: u64 = 0x10;
const DOWN: u64 = 0x20;
const DECIDE: u64 = 0x4;
const CANCEL: u64 = 0x8;
const START: u64 = 0x100;

/// A top-level entry: id 0..7 (Items, Abilities, Customize, Party, Status,
/// Jiminy's Journal, Roxas's Journal, Config), label and help text.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: u8,
    pub label: Vec<u8>,
    pub help: Vec<u8>,
}

/// One party member's status block values.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub name: Vec<u8>,
    pub level: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
}

/// What the panels show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panels {
    pub munny: u32,
    /// Play time in 60 Hz frames (save+0x2444).
    pub time_frames: u32,
    pub members: Vec<Member>,
    /// "LV", "HP", "MP" (sys.bar 0xb9fc..0xb9fe).
    pub labels: [Vec<u8>; 3],
}

/// What a draw item samples: the camp sheet or the system font.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    Art,
    Font,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Substate {
    /// Rows still appearing; no help yet.
    Opening,
    Idle,
}

/// FUN_1403adcc0: sprintf digits and '/' into the font's encoding.
pub fn encode_number(s: &str) -> Vec<u8> {
    s.bytes()
        .map(|b| match b {
            b'0'..=b'9' => 0x21 + (b - b'0'),
            b'/' => 0x4b,
            _ => 0x01,
        })
        .collect()
}

pub struct Camp {
    pub layout: LayoutTask,
    pub sprites: Sprites,
    pub entries: Vec<Entry>,
    rows: Vec<Id>,
    pub cursor: usize,
    light: LightCur,
    help: Option<Id>,
    help_offset: [f32; 2],
    frame_sprite: Id,
    prompt: Id,
    munny: Id,
    time: Id,
    munny_digits: Vec<Player>,
    time_digits: Vec<Player>,
    status: Id,
    pub substate: Substate,
    pub exit: bool,
    pub closed: bool,
}

impl Camp {
    /// FUN_140306040 (and the open sound the task plays right after).
    pub fn open(
        layout: &Layout,
        entries: Vec<Entry>,
        cursor: usize,
        panels: &Panels,
    ) -> (Self, Vec<u32>) {
        let seq = &layout.sequences[SEQUENCE];
        let mut task = LayoutTask::new(layout, SCENE_OPEN);
        task.next = Some(SCENE_IDLE);
        let mut sp = Sprites::default();
        let plain = |sp: &mut Sprites, prio: u32, g: [usize; 3]| {
            sp.create(seq, prio, ART, g[0], Some(g[1]), None)
        };
        let frame_sprite = plain(&mut sp, PRIO_FRAME, FRAME);
        let prompt = plain(&mut sp, PRIO_FRAME, PROMPT);
        let munny = plain(&mut sp, PRIO_PANEL, MUNNY);
        let time = plain(&mut sp, PRIO_PANEL, TIME);
        let status = plain(&mut sp, PRIO_PANEL, STATUS);
        let n = panels.members.len();
        let pitch = seq.groups[STATUS[0]].text_position_x as f32;
        let sub_size = seq.groups[SUB_ANCHOR].text_scale as f32;
        let row_pitch = seq.groups[SUB_ANCHOR].ui_padding as f32;
        let value_x = seq.groups[SUB_ANCHOR].light_position_x as f32;
        for (i, m) in panels.members.iter().enumerate() {
            let mut name = Text::new(m.name.clone(), NAME_SIZE);
            name.align = 2;
            name.group_position = false;
            let block = sp.create(seq, PRIO_ROW, ART, BLOCK, None, Some(name));
            sp.attach(block, status);
            sp.get_mut(block).offset = [pitch * (n - i) as f32, 0.];
            let sub = sp.create(seq, PRIO_ROW, ART, SUB_ANCHOR, None, None);
            sp.attach(sub, block);
            let values = [
                format!("{}", m.level),
                format!("{}/{}", m.hp, m.max_hp),
                format!("{}/{}", m.mp, m.max_mp),
            ];
            for k in 0..3 {
                let y = row_pitch * k as f32;
                let text = |bytes: Vec<u8>, align: u8| {
                    let mut t = Text::new(bytes, sub_size);
                    t.align = align;
                    t.group_position = false;
                    t.keep_rgb = false;
                    t
                };
                let label = sp.create(
                    seq,
                    PRIO_ROW,
                    ART,
                    STYLES[k],
                    None,
                    Some(text(panels.labels[k].clone(), 0)),
                );
                sp.attach(label, sub);
                sp.get_mut(label).offset = [0., y];
                let value = sp.create(
                    seq,
                    PRIO_ROW,
                    ART,
                    STYLES[k],
                    None,
                    Some(text(encode_number(&values[k]), 1)),
                );
                sp.attach(value, sub);
                sp.get_mut(value).offset = [value_x, y];
                if k > 0 {
                    let f = sp.create(seq, PRIO_ROW, ART, GAUGE_FRAME, None, None);
                    sp.attach(f, sub);
                    sp.get_mut(f).offset = [0., y];
                    let (now, max) = if k == 1 {
                        (m.hp, m.max_hp)
                    } else {
                        (m.mp, m.max_mp)
                    };
                    let bar = sp.create(seq, PRIO_BAR, ART, GAUGE_BARS[k - 1], None, None);
                    sp.attach(bar, sub);
                    let s = sp.get_mut(bar);
                    s.offset = [0., y];
                    s.scale = [now as f32 / max as f32, 1.];
                }
            }
        }
        let row_pitch = seq.groups[ROW_APPEAR].text_position_x as f32;
        let cursor = cursor.min(entries.len().saturating_sub(1));
        let rows = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let idle = if i == cursor { ROW_SELECTED } else { ROW_IDLE };
                let mut t = Text::new(e.label.clone(), ROW_TEXT_SIZE);
                t.group_position = false;
                t.keep_rgb = false;
                let r = sp.create(seq, PRIO_ROW, ART, ROW_APPEAR, Some(idle), Some(t));
                sp.get_mut(r).offset = [0., row_pitch * i as f32];
                r
            })
            .collect();
        let light = sp.create(seq, PRIO_LIGHT, ART, LIGHT[0], Some(LIGHT[1]), None);
        let mut camp = Camp {
            layout: task,
            sprites: sp,
            entries,
            rows,
            cursor,
            light: LightCur::new(light),

            help: None,
            help_offset: [0.; 2],
            frame_sprite,
            prompt,
            munny,
            time,
            munny_digits: Vec::new(),
            time_digits: Vec::new(),
            status,
            substate: Substate::Opening,
            exit: false,
            closed: false,
        };
        camp.munny_digits = digits(seq, &munny_groups(panels.munny), Vec::new());
        camp.time_digits = digits(seq, &time_groups(panels.time_frames), Vec::new());
        (camp, vec![SOUND_OPEN])
    }

    /// One camp frame (the camp task runs before the sprite tasks): the
    /// state machine with this frame's presses and repeats (logical, mode
    /// 7), then every sprite's update.
    pub fn frame(&mut self, layout: &Layout, trig: u64, repeat: u64, panels: &Panels) -> Vec<u32> {
        let seq = &layout.sequences[SEQUENCE];
        let mut sounds = Vec::new();
        if !self.exit {
            self.top_level(seq, trig, repeat, &mut sounds);
            // FUN_1403249e0: the clock rebuilds every frame.
            let groups = time_groups(panels.time_frames);
            self.time_digits = digits(seq, &groups, std::mem::take(&mut self.time_digits));
        }
        if self.exit {
            self.close(seq);
        }
        self.layout.update(layout, 1.);
        self.sprites.update(seq);
        for d in self
            .munny_digits
            .iter_mut()
            .chain(self.time_digits.iter_mut())
        {
            d.update(seq, 1.);
        }
        self.place_help(layout);
        if let Some(&row) = self.rows.get(self.cursor) {
            let c = self.sprites.context(seq, row);
            let g = &seq.groups[self.sprites.get(row).player.group];
            let target = [
                c[0] + g.light_position_x as f32,
                c[1] + g.text_position_y as f32,
            ];
            self.light.place(&mut self.sprites, target);
        }
        if self.exit && self.layout.dead {
            self.closed = true;
        }
        sounds
    }

    /// FUN_140303260 (state 0 part) and FUN_1403037d0.
    fn top_level(&mut self, seq: &Sequence, trig: u64, repeat: u64, sounds: &mut Vec<u32>) {
        // No input while the layout opens; any press skips it to idle.
        if !self.layout.dead && self.layout.scene != SCENE_IDLE {
            if trig == 0 {
                return;
            }
            self.layout.set_scene(SCENE_IDLE, None);
        }
        // FUN_1402f9440: repeat moves, wrapping only on a fresh press.
        let n = self.entries.len();
        let prev = self.cursor;
        let mut pick = None;
        if n > 0 {
            if repeat & UP != 0 {
                if self.cursor > 0 {
                    self.cursor -= 1;
                } else if trig & UP != 0 {
                    self.cursor = n - 1;
                }
            } else if repeat & DOWN != 0 {
                if self.cursor + 1 < n {
                    self.cursor += 1;
                } else if trig & DOWN != 0 {
                    self.cursor = 0;
                }
            } else if trig & DECIDE != 0 {
                pick = Some(true);
            } else if trig & (CANCEL | START) != 0 {
                pick = Some(false);
            }
        }
        let moved = self.cursor != prev;
        let row0 = self.rows.first().map(|&r| self.sprites.get(r).player.group);
        if (pick.is_some() || moved) && row0 == Some(ROW_APPEAR) {
            // FUN_1402f81a0: the rows skip their intros.
            for (i, &r) in self.rows.clone().iter().enumerate() {
                let idle = if i == prev { ROW_SELECTED } else { ROW_IDLE };
                self.sprites.set(seq, r, idle, None);
            }
        }
        let row0 = self.rows.first().map(|&r| self.sprites.get(r).player.group);
        if self.substate == Substate::Opening && matches!(row0, Some(ROW_IDLE | ROW_SELECTED)) {
            self.show_help(seq);
            self.substate = Substate::Idle;
        }
        if moved {
            sounds.push(SOUND_CURSOR);
            if self.substate == Substate::Idle {
                // FUN_140304570: rows restart idle, the cursor row selected.
                for (i, &r) in self.rows.clone().iter().enumerate() {
                    let g = if i == self.cursor {
                        ROW_SELECTED
                    } else {
                        ROW_IDLE
                    };
                    self.sprites.set(seq, r, g, None);
                }
                self.light.retarget();
                self.show_help(seq);
            }
        }
        match pick {
            Some(true) => sounds.push(SOUND_DECIDE),
            Some(false) => {
                sounds.push(SOUND_CLOSE);
                self.exit = true;
            }
            None => {}
        }
    }

    /// FUN_1402fa1d0: a live help text only swaps its text (no re-fade
    /// while in G116); otherwise it is created on the layout element of the
    /// current scene with that element's (LightPositionX, TextPositionY).
    fn show_help(&mut self, seq: &Sequence) {
        let bytes = self.entries[self.cursor].help.clone();
        if let Some(h) = self.help {
            if let Some(t) = self.sprites.get_mut(h).text.as_mut() {
                t.bytes = bytes;
            }
            if self.sprites.get(h).player.group != HELP_IN {
                self.sprites.set(seq, h, HELP_IN, None);
            }
            return;
        }
        let size = seq.groups[HELP_IN].text_scale as f32;
        let mut t = Text::new(bytes, size);
        t.keep_rgb = false;
        let h = self
            .sprites
            .create(seq, PRIO_ROW, ART, HELP_IN, None, Some(t));
        let g = &seq.groups[HELP_PARENTS[1]];
        self.help_offset = [g.light_position_x as f32, g.text_position_y as f32];
        self.help = Some(h);
    }

    /// The help text's parent is the layout element of the current scene:
    /// its context, with its alpha as the colour multiplier (0x200|0x400).
    fn place_help(&mut self, layout: &Layout) {
        let Some(h) = self.help else { return };
        if self.layout.dead {
            return;
        }
        let m = Modifiers::default();
        for (prop, player, origin) in self.layout.player.visible(layout, [0, 0]) {
            if HELP_PARENTS.contains(&prop.group) {
                let seq = &layout.sequences[prop.sequence];
                let c = seq.child_context(player, &m).unwrap_or([0.; 2]);
                let a = seq.context_color(player, &m).map_or(1., |c| c[3]);
                let s = self.sprites.get_mut(h);
                s.base = [origin[0] + c[0], origin[1] + c[1]];
                s.offset = self.help_offset;
                s.multiplier = [1., 1., 1., a];
                return;
            }
        }
    }

    /// FUN_140303cd0 with the exit flag: everything plays its close group;
    /// the help text goes at once (FUN_1402fa720).
    fn close(&mut self, seq: &Sequence) {
        if !self.layout.dead && self.layout.scene != SCENE_CLOSE {
            self.layout.set_scene(SCENE_CLOSE, None);
        }
        let mut to = |id: Id, group: usize| {
            let s = self.sprites.get(id);
            if !s.dead && s.player.group != group {
                self.sprites.close(seq, id, group);
            }
        };
        for &r in &self.rows {
            to(r, ROW_CLOSE);
        }
        to(self.frame_sprite, FRAME[2]);
        if let Some(l) = self.light.id {
            to(l, LIGHT[2]);
        }
        to(self.munny, MUNNY[2]);
        to(self.time, TIME[2]);
        to(self.prompt, PROMPT[2]);
        to(self.status, STATUS[2]);
        if let Some(h) = self.help.take() {
            self.sprites.kill(h);
        }
    }

    /// The draw list: the layout (0x7532), then every sprite in priority
    /// order with the digit rows after their labels.
    pub fn draw(&self, layout: &Layout, font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
        let seq = &layout.sequences[SEQUENCE];
        let m = Modifiers::default();
        let mut out = Vec::new();
        if !self.layout.dead {
            for (prop, player, origin) in self.layout.player.visible(layout, [0, 0]) {
                out.push((
                    Sheet::Art,
                    layout.sequences[prop.sequence].quads(player, origin, &m),
                ));
            }
        }
        let convert = |items: Vec<(sprite2d::Sheet, Vec<Quad>)>| {
            items.into_iter().map(|(sheet, quads)| match sheet {
                sprite2d::Sheet::Art(_) => (Sheet::Art, quads),
                sprite2d::Sheet::Font => (Sheet::Font, quads),
            })
        };
        // The digit rows (Tz::MultiSeq, 0x753b) follow their labels.
        out.extend(convert(
            self.sprites.draw_where(seq, font, |p| p <= PRIO_PANEL),
        ));
        out.push((Sheet::Art, self.digit_quads(seq)));
        out.extend(convert(
            self.sprites.draw_where(seq, font, |p| p > PRIO_PANEL),
        ));
        out
    }
    /// Tz::MultiSeq: from the label's context + (LightPositionX,
    /// TextPositionY), each digit at the cursor, which then moves left by
    /// its group's TextPositionX; alpha follows the label (0x200|0x400).
    fn digit_quads(&self, seq: &Sequence) -> Vec<Quad> {
        let mut out = Vec::new();
        for (label, players) in [
            (self.munny, &self.munny_digits),
            (self.time, &self.time_digits),
        ] {
            let s = self.sprites.get(label);
            if s.dead {
                continue;
            }
            let g = &seq.groups[s.player.group];
            let c = self.sprites.context(seq, label);
            let mut x = c[0] + g.light_position_x as f32;
            let y = c[1] + g.text_position_y as f32;
            let a = seq
                .context_color(&s.player, &Modifiers::default())
                .map_or(1., |c| c[3]);
            let m = Modifiers {
                color: [1., 1., 1., a],
                ..Modifiers::default()
            };
            for p in players {
                out.extend(seq.quads(p, [x, y], &m));
                x -= seq.groups[p.group].text_position_x as f32;
            }
        }
        out
    }
}

/// Munny digit groups, ones first: stops at the first leading zero except
/// the ones digit; 6 slots.
fn munny_groups(value: u32) -> Vec<usize> {
    let mut v = value % 1_000_000;
    (0..6)
        .map(|k| {
            let g = if k == 0 || v > 0 {
                MUNNY_DIGIT + (v % 10) as usize
            } else {
                DIGIT_BLANK
            };
            v /= 10;
            g
        })
        .collect()
}

/// Clock groups (FUN_140157390: h = t/216000, m = t%216000/3600): minute
/// ones and tens, the colon, then up to 3 hour digits; 9 slots.
fn time_groups(frames: u32) -> Vec<usize> {
    let hours = frames / 216_000;
    let minutes = frames % 216_000 / 3600;
    let mut g = vec![
        TIME_DIGIT + (minutes % 10) as usize,
        TIME_DIGIT + (minutes / 10) as usize,
        COLON,
    ];
    let mut h = hours;
    for k in 0..3 {
        if k > 0 && h == 0 {
            break;
        }
        g.push(TIME_DIGIT + (h % 10) as usize);
        h /= 10;
    }
    g.resize(9, DIGIT_BLANK);
    g
}

/// Players for digit groups, keeping a slot's player when its group is
/// unchanged (a group is only set when it changes).
fn digits(seq: &Sequence, groups: &[usize], mut old: Vec<Player>) -> Vec<Player> {
    old.resize_with(groups.len(), || Player::new(seq, DIGIT_BLANK));
    old.into_iter()
        .zip(groups)
        .map(|(p, &g)| if p.group == g { p } else { Player::new(seq, g) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_follow_the_exe_rules() {
        assert_eq!(munny_groups(0), vec![51, 31, 31, 31, 31, 31]);
        assert_eq!(munny_groups(1205), vec![56, 51, 53, 52, 31, 31]);
        // 2 h 05 min: 5, 0, colon, 2, blanks.
        let t = 2 * 216_000 + 5 * 3600 + 59;
        assert_eq!(time_groups(t), vec![66, 61, 71, 63, 31, 31, 31, 31, 31]);
        assert_eq!(encode_number("12/30"), vec![0x22, 0x23, 0x4b, 0x24, 0x21]);
    }
}
