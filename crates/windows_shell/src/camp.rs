//! The camp (pause) menu's top level, translated from the camp process
//! (init FUN_140304ea0, build FUN_140306040 / FUN_140306790, state machine
//! FUN_140303260 / FUN_1403037d0, per-frame driver FUN_140303cd0, list input
//! FUN_1402f9440, LightCur FUN_1402faf40 / FUN_14034b890). Everything draws
//! from `menu/<lang>/camp.2ld` sequence 1 in raw layout space (x origin 0).
//! Sub-screens are not built: deciding an entry plays the decide sound only.
use crate::command::{self, Font};
use package_reader::layout::{Layout, LayoutTask};
use package_reader::sequence::{Modifiers, Player, Quad, Sequence, color};

/// The camp sprites' SEQD (DAT_140743fd4).
pub const SEQUENCE: usize = 1;

/// Layout scenes (L2): open, idle, close.
const SCENE_OPEN: usize = 0;
const SCENE_IDLE: usize = 1;
const SCENE_CLOSE: usize = 2;
/// Layout elements the help text follows (open / idle / close).
const HELP_PARENTS: [usize; 3] = [39, 40, 41];

/// Row groups (entry table 0x1405badb0): appear, close, idle; the cursor
/// row's idle is the selected group.
const ROW_APPEAR: usize = 133;
const ROW_CLOSE: usize = 135;
const ROW_IDLE: usize = 134;
const ROW_SELECTED: usize = 132;
/// Row label: system font, size 15, colour 0x80e5e5e5.
const ROW_TEXT_SIZE: f32 = 15.;
const ROW_TEXT_COLOUR: u32 = 0x80e5e5e5;

/// LightCur groups and its glide (30 ticks, Hermite with tangent 2.74981).
const LIGHT_APPEAR: usize = 27;
const LIGHT_IDLE: usize = 28;
const LIGHT_CLOSE: usize = 29;
const GLIDE_TICKS: f32 = 30.;
const GLIDE_TANGENT: f32 = 2.74981;

const HELP_IN: usize = 116;
const HELP_OUT: usize = 117;
const HELP_OFFSET_FALLBACK: [f32; 2] = [141., 336.];

const FRAME: [usize; 3] = [46, 47, 48];
const PROMPT: [usize; 3] = [351, 352, 353];
const MUNNY: [usize; 3] = [107, 108, 109];
const TIME: [usize; 3] = [113, 114, 115];
const MUNNY_DIGIT: usize = 51;
const TIME_DIGIT: usize = 61;
const DIGIT_BLANK: usize = 31;
const COLON: usize = 71;
const STATUS: [usize; 3] = [101, 102, 103];

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

/// A dk::Sprite on sequence 1: its player, queued group and position.
#[derive(Clone, Debug)]
pub struct Sprite {
    pub player: Player,
    pub queued: Option<usize>,
    pub base: [f32; 2],
}

impl Sprite {
    pub fn new(seq: &Sequence, group: usize, queued: Option<usize>, base: [f32; 2]) -> Self {
        Self {
            player: Player::new(seq, group),
            queued,
            base,
        }
    }

    pub fn set(&mut self, seq: &Sequence, group: usize, queued: Option<usize>) {
        self.player = Player::new(seq, group);
        self.queued = queued;
    }

    /// One frame; a finished play-once group moves on to the queued one.
    pub fn update(&mut self, seq: &Sequence) {
        self.player.update(seq, 1.);
        if seq.groups[self.player.group].play_count != 0
            && self.player.finished()
            && let Some(next) = self.queued.take()
        {
            self.player = Player::new(seq, next);
            self.player.update(seq, 1.);
        }
    }

    pub fn group(&self) -> usize {
        self.player.group
    }

    /// Settled: a finished play-once group with nothing queued.
    pub fn done(&self) -> bool {
        self.player.finished() && self.queued.is_none()
    }

    pub fn context(&self, seq: &Sequence) -> [f32; 2] {
        let c = seq
            .child_context(&self.player, &Modifiers::default())
            .unwrap_or([0.; 2]);
        [self.base[0] + c[0], self.base[1] + c[1]]
    }

    pub fn quads(&self, seq: &Sequence, m: &Modifiers) -> Vec<Quad> {
        seq.quads(&self.player, self.base, m)
    }

    pub fn colour(&self, seq: &Sequence) -> [f32; 4] {
        seq.current_color(&self.player).unwrap_or([1.; 4])
    }
}

/// The values the panels show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panels {
    pub munny: u32,
    /// Play time in 60 Hz frames (save+0x2444).
    pub time_frames: u32,
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

struct Light {
    sprite: Sprite,
    /// Glide start (the last drawn position) and ticks into the glide.
    from: Option<[f32; 2]>,
    tick: f32,
    drawn: Option<[f32; 2]>,
}

pub struct Camp {
    pub layout: LayoutTask,
    pub entries: Vec<Entry>,
    rows: Vec<Sprite>,
    pub cursor: usize,
    light: Light,
    help: Sprite,
    help_entry: Option<usize>,
    frame: Sprite,
    prompt: Sprite,
    munny: Sprite,
    time: Sprite,
    munny_digits: Vec<Sprite>,
    time_digits: Vec<Sprite>,
    status: Sprite,
    pub substate: Substate,
    pub exit: bool,
    pub closed: bool,
}

/// FUN_14034b890: f(s) = t(s^3 - 2s^2 + s) + (3s^2 - 2s^3).
fn glide(s: f32) -> f32 {
    GLIDE_TANGENT * (s * s * s - 2. * s * s + s) + (3. * s * s - 2. * s * s * s)
}

impl Camp {
    /// FUN_140306040 (and the open sound the task plays right after).
    pub fn open(layout: &Layout, entries: Vec<Entry>, cursor: usize) -> (Self, Vec<u32>) {
        let seq = &layout.sequences[SEQUENCE];
        let mut task = LayoutTask::new(layout, SCENE_OPEN);
        task.next = Some(SCENE_IDLE);
        let pitch = seq.groups[ROW_APPEAR].text_position_x as f32;
        let cursor = cursor.min(entries.len().saturating_sub(1));
        let rows = (0..entries.len())
            .map(|i| {
                let idle = if i == cursor { ROW_SELECTED } else { ROW_IDLE };
                Sprite::new(seq, ROW_APPEAR, Some(idle), [0., pitch * i as f32])
            })
            .collect();
        let sprite = |groups: [usize; 3]| Sprite::new(seq, groups[0], Some(groups[1]), [0.; 2]);
        let camp = Camp {
            layout: task,
            entries,
            rows,
            cursor,
            light: Light {
                sprite: Sprite::new(seq, LIGHT_APPEAR, Some(LIGHT_IDLE), [0.; 2]),
                from: None,
                tick: 0.,
                drawn: None,
            },
            help: Sprite::new(seq, HELP_OUT, None, [0.; 2]),
            help_entry: None,
            frame: sprite(FRAME),
            prompt: sprite(PROMPT),
            munny: sprite(MUNNY),
            time: sprite(TIME),
            munny_digits: (0..6)
                .map(|_| Sprite::new(seq, DIGIT_BLANK, None, [0.; 2]))
                .collect(),
            time_digits: (0..9)
                .map(|_| Sprite::new(seq, DIGIT_BLANK, None, [0.; 2]))
                .collect(),
            status: sprite(STATUS),
            substate: Substate::Opening,
            exit: false,
            closed: false,
        };
        (camp, vec![SOUND_OPEN])
    }

    /// One camp frame with the pad's new presses and repeats (logical,
    /// mode 7): the state machine, then every sprite's own update.
    pub fn frame(&mut self, layout: &Layout, trig: u64, repeat: u64, panels: &Panels) -> Vec<u32> {
        let seq = &layout.sequences[SEQUENCE];
        let mut sounds = Vec::new();
        if !self.exit {
            self.top_level(layout, trig, repeat, &mut sounds);
        }
        if self.exit {
            self.close(seq);
        }
        self.layout.update(layout, 1.);
        for s in self
            .rows
            .iter_mut()
            .chain([
                &mut self.light.sprite,
                &mut self.help,
                &mut self.frame,
                &mut self.prompt,
                &mut self.munny,
                &mut self.time,
                &mut self.status,
            ])
            .chain(self.munny_digits.iter_mut())
            .chain(self.time_digits.iter_mut())
        {
            s.update(seq);
        }
        self.set_digits(seq, panels);
        if self.exit && self.layout.dead {
            self.closed = true;
        }
        sounds
    }

    /// FUN_140303260 (state 0 part) and FUN_1403037d0.
    fn top_level(&mut self, layout: &Layout, trig: u64, repeat: u64, sounds: &mut Vec<u32>) {
        let seq = &layout.sequences[SEQUENCE];
        // No input while the layout opens; any press skips to idle.
        if !self.layout.dead && self.layout.scene != SCENE_IDLE {
            if trig == 0 {
                return;
            }
            self.layout.set_scene(SCENE_IDLE, None);
        }
        // FUN_1402f9440: repeat moves, wrapping only on a fresh press.
        let n = self.entries.len();
        let prev = self.cursor;
        let mut r = None;
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
                r = Some(Ok(self.cursor));
            } else if trig & CANCEL != 0 {
                r = Some(Err(-2));
            } else if trig & START != 0 {
                r = Some(Err(-4));
            }
        }
        let moved = self.cursor != prev;
        if (r.is_some() || moved)
            && self
                .rows
                .first()
                .is_some_and(|row| row.group() == ROW_APPEAR)
        {
            // FUN_1402f81a0: skip the row intros.
            for (i, row) in self.rows.iter_mut().enumerate() {
                let idle = if i == prev { ROW_SELECTED } else { ROW_IDLE };
                row.set(seq, row.queued.unwrap_or(idle), None);
            }
        }
        if self.substate == Substate::Opening
            && self
                .rows
                .first()
                .is_some_and(|row| row.group() != ROW_APPEAR)
        {
            self.substate = Substate::Idle;
            self.show_help(seq);
        }
        if moved {
            sounds.push(SOUND_CURSOR);
            if self.substate == Substate::Idle {
                // FUN_140304570: rows restart idle, the cursor row selected.
                for (i, row) in self.rows.iter_mut().enumerate() {
                    row.set(
                        seq,
                        if i == self.cursor {
                            ROW_SELECTED
                        } else {
                            ROW_IDLE
                        },
                        None,
                    );
                }
                self.light.from = self.light.drawn;
                self.light.tick = 0.;
                self.show_help(seq);
            }
        }
        match r {
            Some(Err(_)) => {
                sounds.push(SOUND_CLOSE);
                self.exit = true;
            }
            Some(Ok(_)) => sounds.push(SOUND_DECIDE),
            None => {}
        }
    }

    /// FUN_140307030: the help line of the cursor entry fades in.
    fn show_help(&mut self, seq: &Sequence) {
        if self.help_entry != Some(self.cursor) {
            self.help_entry = Some(self.cursor);
            self.help.set(seq, HELP_IN, None);
        }
    }

    /// FUN_140303cd0 with the exit flag: everything plays its close group.
    fn close(&mut self, seq: &Sequence) {
        if !self.layout.dead && self.layout.scene != SCENE_CLOSE {
            self.layout.set_scene(SCENE_CLOSE, None);
        }
        let to = |s: &mut Sprite, group: usize| {
            if s.group() != group && s.queued != Some(group) {
                s.set(seq, group, None);
            }
        };
        for row in &mut self.rows {
            to(row, ROW_CLOSE);
        }
        to(&mut self.frame, FRAME[2]);
        to(&mut self.light.sprite, LIGHT_CLOSE);
        to(&mut self.munny, MUNNY[2]);
        to(&mut self.time, TIME[2]);
        to(&mut self.prompt, PROMPT[2]);
        to(&mut self.status, STATUS[2]);
        if self.help_entry.is_some() {
            to(&mut self.help, HELP_OUT);
        }
    }

    /// Digit sprites (FUN_1402fb450 munny, FUN_140324690 time): right to
    /// left; a changed digit restarts its group.
    fn set_digits(&mut self, seq: &Sequence, p: &Panels) {
        let set = |s: &mut Sprite, group: usize| {
            if s.group() != group {
                s.set(seq, group, None);
                s.player.update(seq, 1.);
            }
        };
        let mut v = p.munny.min(999_999);
        for (k, s) in self.munny_digits.iter_mut().enumerate() {
            let group = if k == 0 || v > 0 {
                MUNNY_DIGIT + (v % 10) as usize
            } else {
                DIGIT_BLANK
            };
            set(s, group);
            v /= 10;
        }
        let hours = p.time_frames / 216_000;
        let minutes = p.time_frames % 216_000 / 3600;
        let mut groups = vec![
            TIME_DIGIT + (minutes % 10) as usize,
            TIME_DIGIT + (minutes / 10) as usize,
            COLON,
        ];
        let mut h = hours;
        loop {
            groups.push(TIME_DIGIT + (h % 10) as usize);
            h /= 10;
            if h == 0 {
                break;
            }
        }
        for (k, s) in self.time_digits.iter_mut().enumerate() {
            set(s, groups.get(k).copied().unwrap_or(DIGIT_BLANK));
        }
    }

    /// The draw list in priority order (layout 0x7532, frame and prompt
    /// 0x7533, panels 0x753b, rows 0x753c, LightCur 0x754a; help text).
    pub fn draw(&mut self, layout: &Layout, font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
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
        for s in [&self.frame, &self.prompt, &self.status] {
            out.push((Sheet::Art, s.quads(seq, &m)));
        }
        // Munny / time labels and their digits (colour x the label's).
        for (label, digits) in [
            (&self.munny, &self.munny_digits),
            (&self.time, &self.time_digits),
        ] {
            out.push((Sheet::Art, label.quads(seq, &m)));
            let g = &seq.groups[label.group()];
            let c = label.context(seq);
            let mut x = c[0] + g.light_position_x as f32;
            let y = c[1] + g.text_position_y as f32;
            let tint = Modifiers {
                color: label.colour(seq),
                ..m
            };
            for d in digits.iter() {
                if d.group() == DIGIT_BLANK {
                    continue;
                }
                out.push((Sheet::Art, seq.quads(&d.player, [x, y], &tint)));
                x -= seq.groups[d.group()].text_position_x as f32;
            }
        }
        for (i, row) in self.rows.iter().enumerate() {
            out.push((Sheet::Art, row.quads(seq, &m)));
            let c = row.context(seq);
            let mut rgba = color(ROW_TEXT_COLOUR);
            rgba[3] = row.colour(seq)[3];
            out.push((
                Sheet::Font,
                command::text_quads_sized(&self.entries[i].label, c, rgba, font, ROW_TEXT_SIZE),
            ));
        }
        // LightCur: the selected row's context + (LightPositionX,
        // TextPositionY) of its group, gliding from where it was drawn.
        if let Some(row) = self.rows.get(self.cursor) {
            let g = &seq.groups[row.group()];
            let c = row.context(seq);
            let target = [
                c[0] + g.light_position_x as f32,
                c[1] + g.text_position_y as f32,
            ];
            let at = match self.light.from {
                Some(from) if self.light.tick < GLIDE_TICKS => {
                    let f = glide(self.light.tick / GLIDE_TICKS);
                    [
                        from[0] + (target[0] - from[0]) * f,
                        from[1] + (target[1] - from[1]) * f,
                    ]
                }
                _ => target,
            };
            self.light.tick += 1.;
            self.light.drawn = Some(at);
            self.light.sprite.base = at;
            out.push((Sheet::Art, self.light.sprite.quads(seq, &m)));
        }
        if let Some(e) = self.help_entry.and_then(|i| self.entries.get(i)) {
            let origin = self.help_origin(layout);
            let size = seq.groups[self.help.group()].text_scale.max(1) as f32;
            let rgba = self.help.colour(seq);
            for (k, line) in e.help.split(|&b| b == 0x02).enumerate() {
                let at = [origin[0], origin[1] + k as f32 * size * 24. / 18.];
                out.push((
                    Sheet::Font,
                    command::text_quads_sized(line, at, rgba, font, size),
                ));
            }
        }
        out
    }

    /// The help text sits at its layout element's context plus that
    /// group's (LightPositionX, TextPositionY) (FUN_1402fa5b0).
    fn help_origin(&self, layout: &Layout) -> [f32; 2] {
        let m = Modifiers::default();
        if !self.layout.dead {
            for (prop, player, origin) in self.layout.player.visible(layout, [0, 0]) {
                if HELP_PARENTS.contains(&prop.group) {
                    let seq = &layout.sequences[prop.sequence];
                    let g = &seq.groups[prop.group];
                    let c = seq.child_context(player, &m).unwrap_or([0.; 2]);
                    return [
                        origin[0] + c[0] + g.light_position_x as f32,
                        origin[1] + c[1] + g.text_position_y as f32,
                    ];
                }
            }
        }
        HELP_OFFSET_FALLBACK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glide_starts_and_ends_on_its_points() {
        assert_eq!(glide(0.), 0.);
        assert!((glide(1.) - 1.).abs() < 1e-6);
        // The tangent overshoots early: past halfway well before s = 0.5.
        assert!(glide(0.3) > 0.5);
    }
}
