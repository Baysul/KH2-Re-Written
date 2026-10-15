//! The form level-up panels (`dk::LEVELUP_INFO` queue B, drivefx-02 §1):
//! FUN_14017ddc0 queues up to three, FUN_14017e1d0 starts them as one
//! batch of 'gaug' sprites stacked by the group's padding, and
//! FUN_14017d6f0 draws the title, level and two text lines over each.
use crate::command::{Font, text_quads_sized, text_width};
use crate::gauge::Draw;
use data::sequence::{Modifiers, Player, Sequence};
use std::collections::VecDeque;

/// DAT_14071ad78: the {one line, two lines} groups by form (0 = Sora).
pub const FORM_GROUPS: [[usize; 2]; 7] = [
    [0x5c, 0x5d],
    [0x5e, 0x5f],
    [0x5a, 0x5b],
    [0x58, 0x59],
    [0x62, 0x63],
    [0x60, 0x61],
    [0x60, 0x61],
];
/// The panels' x origin (left-edge anchoring on the 16:9 frame).
const X_ORIGIN: f32 = -85.;
/// Text size 14 against the font's 18-unit cell.
const TEXT_SIZE: f32 = 14.;

/// One queued panel: the form (0 = Sora's panel, no level), its title,
/// level and one or two encoded lines.
pub struct Entry {
    pub form: u8,
    pub level: u8,
    pub title: Vec<u8>,
    pub line1: Vec<u8>,
    pub line2: Option<Vec<u8>>,
}

struct Panel {
    entry: Entry,
    player: Player,
    /// The stack offset (+0x1c4).
    y: f32,
}

#[derive(Default)]
pub struct FormPanels {
    pending: VecDeque<Entry>,
    batch: Vec<Panel>,
}

impl FormPanels {
    /// FUN_14017ddc0: a fourth pending panel is dropped.
    pub fn push(&mut self, entry: Entry) {
        if self.pending.len() < 3 {
            self.pending.push_back(entry);
        }
    }

    /// FUN_14017e1d0's queue-B step: the batch plays; once its first panel
    /// is gone the pending ones start together, each 44 (one line) or 64
    /// (two lines) units above the previous.
    pub fn update(&mut self, seq: &Sequence) {
        for p in &mut self.batch {
            p.player.update(seq, 1.);
        }
        if self.batch.first().is_some_and(|p| p.player.finished()) {
            self.batch.clear();
        }
        if !self.batch.is_empty() {
            return;
        }
        let mut y = 0.;
        while self.batch.len() < 4
            && let Some(entry) = self.pending.pop_front()
        {
            let group = FORM_GROUPS[(entry.form as usize).min(6)][entry.line2.is_some() as usize];
            let Some(g) = seq.groups.get(group) else {
                continue;
            };
            let padding = g.ui_padding as f32;
            self.batch.push(Panel {
                entry,
                player: Player::new(seq, group),
                y,
            });
            y += padding;
        }
    }

    /// The panels' sprites, then their text (FUN_14017d6f0): line 1 left
    /// at the group's (+0x10, +0x14), the title right-aligned at the text
    /// anchor, the level ("%2d", forms only) at +0x20 and line 2 17 units
    /// under line 1. `sheet` is the gauge sheet, `font_sheet` the system
    /// font's.
    pub fn draw(&self, seq: &Sequence, font: &Font, sheet: usize, font_sheet: usize) -> Vec<Draw> {
        let mut out = Vec::new();
        let m = Modifiers::default();
        for p in &self.batch {
            let origin = [X_ORIGIN, p.y];
            out.push(Draw::Quads {
                quads: seq.quads(&p.player, origin, &m),
                z: 0,
                sheet,
            });
            let g = &seq.groups[p.player.group];
            let ctx = seq.child_context(&p.player, &m).unwrap_or([0.; 2]);
            let alpha = seq
                .context_color(&p.player, &m)
                .map_or(1., |c| c[3].min(1.));
            if alpha <= 0. {
                continue;
            }
            let rgba = [1., 1., 1., alpha];
            let (x, y) = (origin[0] + ctx[0], origin[1] + ctx[1]);
            // The exe's text y is the cell's bottom (24 units at 18) [I].
            let lift = 24. * TEXT_SIZE / 18.;
            let mut text = |s: &[u8], at: [f32; 2]| {
                out.push(Draw::Quads {
                    quads: text_quads_sized(s, [at[0], at[1] - lift], rgba, font, TEXT_SIZE),
                    z: 0,
                    sheet: font_sheet,
                });
            };
            let e = &p.entry;
            let left = x + g.light_position_x as f32;
            let line_y = y + g.text_position_y as f32;
            text(&e.line1, [left, line_y]);
            if let Some(l2) = &e.line2 {
                text(l2, [left, y + 17. + 20.]);
            }
            text(&e.title, [x - text_width(&e.title, font, TEXT_SIZE), y]);
            if e.form != 0 {
                let level = crate::pause_menu::encode_number(&format!("{:2}", e.level));
                text(&level, [x + g.text_position_x as f32, y]);
            }
        }
        out
    }
}
