//! The field command menu (top level, normal style), translated from the
//! executable's dk::COMMAND_DRAW: update FUN_14015e800, level builder
//! FUN_14015b6d0, cursor FUN_14015ba00, cursor decoration FUN_14015bc30,
//! slot fetch FUN_14015d140, slot draw FUN_14015b4a0 and its text
//! FUN_14016ae30. Input follows FUN_1403b11e0 (wrapping up/down). Layout
//! units are the PS2 512x416 space; the menu's X origin is -85 at 16:9.
use crate::gauge::Draw;
use package_reader::sequence::{Modifiers, Player, Quad, Sequence, color};

/// X origin of the command menu outside exact 4:3 (0xffffffab).
pub const X_ORIGIN: f32 = -85.;
/// Root base (FUN_14016b330(0x1c, 0x142)).
const ROOT_BASE: [f32; 2] = [28., 322.];
/// Row pitch, first-row anchor (row n-1 sits at y 388), cursor shift.
const PITCH: f32 = 22.;
const ROW_ANCHOR: f32 = 410.;
const PANEL_ANCHOR: f32 = 388.;
const CURSOR_SHIFT: f32 = 8.;
/// Text offset, size and font reference height (font 1: 18 px).
const TEXT_OFFSET: [f32; 2] = [10., -8.];
const TEXT_SIZE: f32 = 14.;
const FONT_REFERENCE: f32 = 18.;
/// System font cell and page geometry (international: 18x24, 28 per row).
const CELL: [f32; 2] = [18., 24.];
const CELL_COLUMNS: usize = 28;

/// Groups for kind 0, style 0 (normal) from the tables at 0x140717ef0,
/// 0x140718290/0x1407182a0, 0x1407182c0 and 0x140718388.
const ROOT_LOOP: usize = 6;
const PANEL_INTRO: usize = 17;
const PANEL_LOOP: usize = 18;
const CURSOR: usize = 9;
const ROW: usize = 16;
const LAST_ROW: usize = 11;
const ICON_FRAME: usize = 45;
const POINTER: usize = 20;
/// Row/cursor text colours by colour index (0x140717d78, 0x140717da0).
const ROW_TEXT: [u32; 9] = [
    0x80b4b4b4, 0x80646464, 0x800079b7, 0x80646464, 0x80646464, 0x80646464, 0x80646464, 0x80e6e6e6,
    0x80b4b4b4,
];
const CURSOR_TEXT: [u32; 9] = [
    0x80e6e6e6, 0x80969696, 0x8000a7ff, 0x80969696, 0x80969696, 0x80969696, 0x80969696, 0x80e6e6e6,
    0x80e6e6e6,
];

/// One top-menu row: encoded label, command icon type (1 Attack, 2 Magic,
/// 3 Item, 4 Form...) and colour index (>= 4 cannot be selected).
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub label: Vec<u8>,
    pub icon: u8,
    pub colour: usize,
}

/// System font: HD glyph sheet plus the original width table.
pub struct Font<'a> {
    pub widths: &'a [u8],
}

struct Slot {
    player: Player,
    queued: Option<usize>,
    base: [f32; 2],
    parent: Option<usize>,
    text: Option<Vec<u8>>,
    text_colour: u32,
    disabled: bool,
    context: [f32; 2],
    resolved: [f32; 2],
}

pub struct CommandMenu {
    root: Player,
    slots: Vec<Slot>,
    counter: usize,
    pub items: Vec<Item>,
    pub cursor: usize,
    last_cursor: Option<usize>,
    last_count: usize,
}

/// What the menu did with this frame's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    None,
    /// Execute the command at this row.
    Execute(usize),
    /// The row cannot be selected (error sound in the original).
    Invalid,
}

impl CommandMenu {
    /// Field-reload start: the root skips its intro (DAT_140abc510 = 1) and
    /// shows its loop group with the slots at full alpha.
    pub fn new(seq: &Sequence, items: Vec<Item>) -> Self {
        CommandMenu {
            root: Player::new(seq, ROOT_LOOP),
            slots: Vec::new(),
            counter: 0,
            items,
            cursor: 0,
            last_cursor: None,
            last_count: 0,
        }
    }

    /// Up/down wrap; decide executes or reports an unusable row. After an
    /// execution the cursor returns to the top (FUN_1403b1b80(menu, 0)).
    pub fn input(&mut self, up: bool, down: bool, decide: bool) -> Decision {
        let n = self.items.len();
        if n == 0 {
            return Decision::None;
        }
        if decide {
            return if self.items[self.cursor].colour < 4 {
                let row = self.cursor;
                self.cursor = 0;
                Decision::Execute(row)
            } else {
                Decision::Invalid
            };
        }
        if up {
            self.cursor = if self.cursor > 0 {
                self.cursor - 1
            } else {
                n - 1
            };
        } else if down {
            self.cursor = if self.cursor < n - 1 {
                self.cursor + 1
            } else {
                0
            };
        }
        Decision::None
    }

    #[allow(clippy::too_many_arguments)]
    fn slot(
        &mut self,
        seq: &Sequence,
        group: usize,
        force: bool,
        base: [f32; 2],
        text: Option<Vec<u8>>,
        colour: u32,
    ) -> usize {
        let i = self.counter;
        if i == self.slots.len() {
            self.slots.push(Slot {
                player: Player::new(seq, group),
                queued: None,
                base,
                parent: None,
                text: None,
                text_colour: colour,
                disabled: false,
                context: [0.; 2],
                resolved: [0.; 2],
            });
        } else if force {
            self.slots[i].player = Player::new(seq, group);
            self.slots[i].queued = None;
        }
        let s = &mut self.slots[i];
        if text.is_some() {
            s.text = text;
        }
        s.base = base;
        s.parent = None;
        s.disabled = false;
        s.text_colour = colour;
        self.counter += 1;
        i
    }

    /// One frame: root update, the level-0 build (FUN_14015b6d0), then the
    /// slots' players in creation order.
    pub fn update(&mut self, seq: &Sequence) {
        self.root.update(seq, 1.);
        self.counter = 0;
        let n = self.items.len();
        let cur = self.cursor.min(n.saturating_sub(1));
        if self.last_count != n {
            self.slots.clear();
        }
        let x = ROOT_BASE[0];
        let first = self.slots.is_empty();
        let panel = self.slot(
            seq,
            PANEL_INTRO,
            false,
            [x, PANEL_ANCHOR - PITCH * n as f32],
            None,
            0x80b4b4b4,
        );
        if first
            || self.slots[panel].queued.is_some()
            || self.slots[panel].player.group != PANEL_LOOP
        {
            self.slots[panel].queued = Some(PANEL_LOOP);
        }
        let mut y = ROW_ANCHOR - PITCH * n as f32;
        let mut cursor_y = y;
        for i in 0..n {
            if i != cur {
                let group = if i == n - 1 { LAST_ROW } else { ROW };
                let item = &self.items[i];
                let (label, colour) = (item.label.clone(), ROW_TEXT[item.colour.min(8)]);
                self.slot(seq, group, true, [x, y], Some(label), colour);
            } else {
                cursor_y = y;
            }
            y += PITCH;
        }
        if n > 0 {
            let force = self.last_cursor != Some(cur) || self.last_count != n;
            let item = &self.items[cur];
            let (label, colour, icon) = (
                item.label.clone(),
                CURSOR_TEXT[item.colour.min(8)],
                item.icon,
            );
            let c = self.slot(
                seq,
                CURSOR,
                force,
                [x + CURSOR_SHIFT, cursor_y],
                Some(label),
                colour,
            );
            // Decoration (FUN_14015bc30): icon frame, category icon and the
            // bouncing pointer, all parented to the cursor row.
            let icon_group = match icon {
                1 => Some(37),
                2 => Some(41),
                3 => Some(39),
                4 => Some(44),
                5 => Some(43),
                6 => Some(38),
                7 => Some(40),
                _ => None,
            };
            let frame = self.slot(seq, ICON_FRAME, force, [0., 0.], None, colour);
            match icon_group {
                Some(g) => {
                    self.slots[frame].parent = Some(c);
                    let ic = self.slot(seq, g, force, [0., 0.], None, colour);
                    self.slots[ic].parent = Some(c);
                }
                None => {
                    self.slots[frame].disabled = true;
                    let placeholder = self.slot(seq, 2, force, [0., 0.], None, 0x80e6e6e6);
                    self.slots[placeholder].disabled = true;
                }
            }
            let pointer = self.slot(seq, POINTER, force, [0., 0.], None, colour);
            self.slots[pointer].parent = Some(c);
        }
        self.last_cursor = Some(cur);
        self.last_count = n;
        self.slots.truncate(self.counter);
        for s in &mut self.slots {
            s.player.update(seq, 1.);
            if seq.groups[s.player.group].play_count != 0
                && s.player.finished()
                && let Some(next) = s.queued.take()
            {
                s.player = Player::new(seq, next);
                s.player.update(seq, 1.);
            }
        }
    }

    /// Draws the root, then each slot: sprite, then its label in the system
    /// font. `sheet` and `font_sheet` index the canvas textures.
    pub fn draw(
        &mut self,
        seq: &Sequence,
        font: &Font,
        sheet: usize,
        font_sheet: usize,
    ) -> Vec<Draw> {
        let m = Modifiers::default();
        let mut out = vec![Draw::Quads {
            quads: seq.quads(&self.root, [X_ORIGIN + ROOT_BASE[0], ROOT_BASE[1]], &m),
            z: 0,
            sheet,
        }];
        for i in 0..self.slots.len() {
            let base = match self.slots[i].parent {
                Some(p) => {
                    let p = &self.slots[p];
                    [p.context[0] + p.resolved[0], p.context[1] + p.resolved[1]]
                }
                None => self.slots[i].base,
            };
            let s = &mut self.slots[i];
            s.resolved = base;
            s.context = seq.child_context(&s.player, &m).unwrap_or([0.; 2]);
            if s.disabled {
                continue;
            }
            out.push(Draw::Quads {
                quads: seq.quads(&s.player, [X_ORIGIN + base[0], base[1]], &m),
                z: 0,
                sheet,
            });
            if let Some(text) = &s.text {
                // Text RGB from the colour table; alpha from the row's
                // current animation colour (flags 4 and 8 set).
                let alpha = seq.current_color(&s.player).map_or(1., |c| c[3]);
                let mut rgba = color(s.text_colour);
                rgba[3] = alpha;
                let origin = [
                    X_ORIGIN + base[0] + s.context[0] + TEXT_OFFSET[0],
                    base[1] + s.context[1] + TEXT_OFFSET[1],
                ];
                out.push(Draw::Quads {
                    quads: text_quads(text, origin, rgba, font),
                    z: 0,
                    sheet: font_sheet,
                });
            }
        }
        out
    }
}

/// Glyph quads for an international system-font string at size 14: glyph
/// index = byte - 0x20, cell 18x24 scaled by 14/18, advance =
/// (int)(width * scale + 0.5); 0x01 advances a third of a cell
/// (FUN_1403e2be0).
pub fn text_quads(text: &[u8], origin: [f32; 2], rgba: [f32; 4], font: &Font) -> Vec<Quad> {
    let scale = TEXT_SIZE / FONT_REFERENCE;
    let mut x = origin[0];
    let mut out = Vec::new();
    for &byte in text {
        if byte == 0x01 {
            x += ((CELL[0] / 3.) as f64 * scale as f64 + 0.5).floor() as f32;
            continue;
        }
        if byte < 0x20 {
            continue;
        }
        let index = (byte - 0x20) as usize;
        let Some(&width) = font.widths.get(index) else {
            continue;
        };
        let (u, v) = (
            (index % CELL_COLUMNS) as f32 * CELL[0],
            (index / CELL_COLUMNS) as f32 * CELL[1],
        );
        let (w, h) = (CELL[0] * scale, CELL[1] * scale);
        let y = origin[1];
        out.push(Quad {
            corners: [[x, y], [x + w, y], [x, y + h], [x + w, y + h]],
            uv: [
                u as i32,
                v as i32,
                (u + CELL[0]) as i32,
                (v + CELL[1]) as i32,
            ],
            colors: [rgba; 4],
            blend: 0,
        });
        x += (width as f64 * scale as f64 + 0.5).floor() as f32;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_advances_by_scaled_width() {
        let widths = [11u8; 100];
        let font = Font { widths: &widths };
        let q = text_quads(&[0x2e, 0x01, 0x2e], [10., 20.], [1.; 4], &font);
        assert_eq!(q.len(), 2);
        // 'A' is glyph 14: cell (14 * 18, 0).
        assert_eq!(q[0].uv, [252, 0, 270, 24]);
        // 11 * 14/18 + 0.5 -> 9; space 6 * 14/18 + 0.5 -> 5.
        assert_eq!(q[1].corners[0][0], 10. + 9. + 5.);
    }

    #[test]
    fn cursor_wraps_and_execution_returns_to_top() {
        let items = (0..4)
            .map(|i| Item {
                label: vec![0x2e],
                icon: 1,
                colour: if i == 1 { 5 } else { 0 },
            })
            .collect();
        let mut menu = CommandMenu {
            root: Player {
                group: 0,
                time: 0.,
                state: 1,
                plays_left: 0,
            },
            slots: Vec::new(),
            counter: 0,
            items,
            cursor: 0,
            last_cursor: None,
            last_count: 0,
        };
        menu.input(true, false, false);
        assert_eq!(menu.cursor, 3);
        assert_eq!(menu.input(false, false, true), Decision::Execute(3));
        assert_eq!(menu.cursor, 0);
        menu.input(false, true, false);
        assert_eq!(menu.input(false, false, true), Decision::Invalid);
    }
}
