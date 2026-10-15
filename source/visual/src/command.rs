//! The field command menu (top level, normal style), translated from the
//! executable's dk::COMMAND_DRAW: update FUN_14015e800, level builder
//! FUN_14015b6d0, cursor FUN_14015ba00, cursor decoration FUN_14015bc30,
//! slot fetch FUN_14015d140, slot draw FUN_14015b4a0 and its text
//! FUN_14016ae30. Input follows FUN_1403b11e0 (wrapping up/down). Layout
//! units are the PS2 512x416 space; the menu's X origin is -85 at 16:9.
use crate::gauge::Draw;
use data::sequence::{Modifiers, Player, Quad, Sequence, color};

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
/// Submenu art groups by menu kind, style 0 (T_EF0[kind]: panel, cursor,
/// row, last row; command-01 section 8): Magic 1, Items 2, Drive 3, the
/// party list 4.
fn sub_groups(kind: usize) -> Option<[usize; 4]> {
    match kind {
        1 => Some([60, 57, 59, 58]),
        2 => Some([51, 48, 50, 49]),
        3 => Some([77, 74, 76, 75]),
        4 | 11 => Some([64, 61, 63, 62]),
        // The Limit Form's limits (T_EF0[12]).
        12 => Some([52, 53, 55, 54]),
        _ => None,
    }
}
/// Level step: each submenu level sits 16 units right of the one below.
const LEVEL_STEP: f32 = 16.;

/// Reaction-command slot (0x140718358 normal group 2) at (0x1c, 0x179)
/// less a row pitch per item, like the panel [inferred: above the rows];
/// a failed angle gate shows it at RGB x0.6 (0x1405b1c50 mode 5).
const REACTION_GROUP: usize = 2;
const REACTION_Y: f32 = 377.;
const REACTION_DIM: f32 = 0.6;
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
    /// Pad kind for the 09 da..f9 button prompts (FUN_1403af840): 1 Xbox,
    /// 2 PlayStation, 3 generic; 0 = the keyboard / mouse was used last (key
    /// caps, FUN_1403af130).
    pub pad: u8,
}

struct Slot {
    player: Player,
    queued: Option<usize>,
    base: [f32; 2],
    parent: Option<usize>,
    text: Option<Vec<u8>>,
    text_colour: u32,
    disabled: bool,
    dimmed: bool,
    context: [f32; 2],
    resolved: [f32; 2],
}

/// The reaction command offered above the menu (e.g. Talk).
#[derive(Clone, Debug, PartialEq)]
pub struct Reaction {
    pub label: Vec<u8>,
    /// The candidate failed its angle gate (display mode 5).
    pub dimmed: bool,
}

/// An opened submenu (level 1): its rows and cursor with the kind's art.
pub struct Sub {
    pub kind: usize,
    pub items: Vec<Item>,
    pub cursor: usize,
    slots: Vec<Slot>,
    counter: usize,
    last_cursor: Option<usize>,
    last_count: usize,
}

pub struct CommandMenu {
    root: Player,
    slots: Vec<Slot>,
    counter: usize,
    pub items: Vec<Item>,
    pub reaction: Option<Reaction>,
    pub cursor: usize,
    last_cursor: Option<usize>,
    last_count: usize,
    /// The submenu kind each top row opens on decide (None executes).
    pub opens: Vec<Option<usize>>,
    /// The open submenu, if any.
    pub sub: Option<Sub>,
}

/// What the menu did with this frame's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    None,
    /// Execute the command at this row.
    Execute(usize),
    /// The row cannot be selected (error sound in the original).
    Invalid,
    /// The row opens its submenu (the caller supplies it with `open_sub`).
    Open(usize),
    /// Execute row `index` of the open submenu (opened by top row `row`);
    /// the menu closes and the top cursor returns to 0.
    ExecuteSub {
        row: usize,
        index: usize,
    },
    /// The submenu was closed with back.
    Closed,
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
            reaction: None,
            cursor: 0,
            last_cursor: None,
            last_count: 0,
            opens: Vec::new(),
            sub: None,
        }
    }

    /// Opens the submenu of `kind` on top of the top level.
    pub fn open_sub(&mut self, kind: usize, items: Vec<Item>) {
        self.sub = Some(Sub {
            kind,
            items,
            cursor: 0,
            slots: Vec::new(),
            counter: 0,
            last_cursor: None,
            last_count: 0,
        });
    }

    pub fn close_sub(&mut self) {
        self.sub = None;
    }

    /// Up/down wrap; decide executes or reports an unusable row. After an
    /// execution the cursor returns to the top (FUN_1403b1b80(menu, 0)).
    pub fn input(&mut self, up: bool, down: bool, decide: bool) -> Decision {
        self.input_with_back(up, down, decide, false)
    }

    /// `input` with the back button: it closes an open submenu.
    pub fn input_with_back(&mut self, up: bool, down: bool, decide: bool, back: bool) -> Decision {
        if let Some(sub) = self.sub.as_mut() {
            let n = sub.items.len();
            if back || n == 0 {
                self.sub = None;
                return Decision::Closed;
            }
            if decide {
                return if sub.items[sub.cursor].colour < 4 {
                    let index = sub.cursor;
                    let row = self.cursor;
                    self.sub = None;
                    self.cursor = 0;
                    Decision::ExecuteSub { row, index }
                } else {
                    Decision::Invalid
                };
            }
            if up {
                sub.cursor = if sub.cursor > 0 {
                    sub.cursor - 1
                } else {
                    n - 1
                };
            } else if down {
                sub.cursor = if sub.cursor < n - 1 {
                    sub.cursor + 1
                } else {
                    0
                };
            }
            return Decision::None;
        }
        let n = self.items.len();
        if n == 0 {
            return Decision::None;
        }
        if decide {
            return if self.items[self.cursor].colour < 4 {
                let row = self.cursor;
                if self.opens.get(row).copied().flatten().is_some() {
                    return Decision::Open(row);
                }
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
                dimmed: false,
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
        s.dimmed = false;
        s.text_colour = colour;
        self.counter += 1;
        i
    }

    /// The submenu level's build (FUN_14015b6d0 with L = 1): the kind's
    /// panel, its rows bottom-anchored like the top level, and the cursor
    /// row moved 8 right.
    fn update_sub(sub: &mut Sub, seq: &Sequence) {
        let Some([panel, cursor_group, row_group, last_group]) = sub_groups(sub.kind) else {
            return;
        };
        let n = sub.items.len();
        let cur = sub.cursor.min(n.saturating_sub(1));
        if sub.last_count != n {
            sub.slots.clear();
        }
        sub.counter = 0;
        let x = ROOT_BASE[0] + LEVEL_STEP;
        let place = |sub: &mut Sub,
                     group: usize,
                     force: bool,
                     base: [f32; 2],
                     text: Option<Vec<u8>>,
                     colour: u32| {
            let i = sub.counter;
            if i == sub.slots.len() {
                sub.slots.push(Slot {
                    player: Player::new(seq, group),
                    queued: None,
                    base,
                    parent: None,
                    text: None,
                    text_colour: colour,
                    disabled: false,
                    dimmed: false,
                    context: [0.; 2],
                    resolved: [0.; 2],
                });
            } else if force {
                sub.slots[i].player = Player::new(seq, group);
                sub.slots[i].queued = None;
            }
            let s = &mut sub.slots[i];
            if text.is_some() {
                s.text = text;
            }
            s.base = base;
            s.text_colour = colour;
            sub.counter += 1;
        };
        place(
            sub,
            panel,
            false,
            [x, PANEL_ANCHOR - PITCH * n as f32],
            None,
            0x80b4b4b4,
        );
        let mut y = ROW_ANCHOR - PITCH * n as f32;
        let mut cursor_y = y;
        for i in 0..n {
            if i != cur {
                let group = if i == n - 1 { last_group } else { row_group };
                let item = &sub.items[i];
                let (label, colour) = (item.label.clone(), ROW_TEXT[item.colour.min(8)]);
                place(sub, group, true, [x, y], Some(label), colour);
            } else {
                cursor_y = y;
            }
            y += PITCH;
        }
        if n > 0 {
            let force = sub.last_cursor != Some(cur) || sub.last_count != n;
            let item = &sub.items[cur];
            let (label, colour) = (item.label.clone(), CURSOR_TEXT[item.colour.min(8)]);
            place(
                sub,
                cursor_group,
                force,
                [x + CURSOR_SHIFT, cursor_y],
                Some(label),
                colour,
            );
        }
        sub.last_cursor = Some(cur);
        sub.last_count = n;
        sub.slots.truncate(sub.counter);
        for s in &mut sub.slots {
            s.player.update(seq, 1.);
        }
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
        if let Some(r) = self.reaction.clone() {
            let i = self.slot(
                seq,
                REACTION_GROUP,
                false,
                [x, REACTION_Y - PITCH * n as f32],
                Some(r.label),
                ROW_TEXT[0],
            );
            self.slots[i].dimmed = r.dimmed;
        }
        self.last_cursor = Some(cur);
        self.last_count = n;
        if let Some(sub) = self.sub.as_mut() {
            Self::update_sub(sub, seq);
        }
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
        draw_slots(&mut self.slots, seq, font, sheet, font_sheet, &mut out);
        if let Some(sub) = self.sub.as_mut() {
            draw_slots(&mut sub.slots, seq, font, sheet, font_sheet, &mut out);
        }
        out
    }
}

/// Draws one level's slots in creation order: sprite, then label.
fn draw_slots(
    slots: &mut [Slot],
    seq: &Sequence,
    font: &Font,
    sheet: usize,
    font_sheet: usize,
    out: &mut Vec<Draw>,
) {
    let m = Modifiers::default();
    for i in 0..slots.len() {
        let base = match slots[i].parent {
            Some(p) => {
                let p = &slots[p];
                [p.context[0] + p.resolved[0], p.context[1] + p.resolved[1]]
            }
            None => slots[i].base,
        };
        let s = &mut slots[i];
        s.resolved = base;
        s.context = seq.child_context(&s.player, &m).unwrap_or([0.; 2]);
        if s.disabled {
            continue;
        }
        let mut m = m;
        if s.dimmed {
            m.color = [REACTION_DIM, REACTION_DIM, REACTION_DIM, 1.];
        }
        out.push(Draw::Quads {
            quads: seq.quads(&s.player, [X_ORIGIN + base[0], base[1]], &m),
            z: 0,
            sheet,
        });
        if let Some(text) = &s.text {
            // Text RGB from the colour table; alpha from the row's current
            // animation colour (flags 4 and 8 set).
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
}

/// Glyph quads for an international system-font string at size 14: glyph
/// index = byte - 0x20, cell 18x24 scaled by 14/18, advance =
/// (int)(width * scale + 0.5); 0x01 advances a third of a cell
/// (FUN_1403e2be0).
pub fn text_quads(text: &[u8], origin: [f32; 2], rgba: [f32; 4], font: &Font) -> Vec<Quad> {
    text_quads_sized(text, origin, rgba, font, TEXT_SIZE)
}

/// Width of a system-font string at `size` (the sum of its advances).
pub fn text_width(text: &[u8], font: &Font, size: f32) -> f32 {
    let scale = size / FONT_REFERENCE;
    text.iter()
        .map(|&byte| {
            if byte == 0x01 {
                ((CELL[0] / 3.) as f64 * scale as f64 + 0.5).floor() as f32
            } else if byte < 0x20 {
                0.
            } else {
                font.widths
                    .get((byte - 0x20) as usize)
                    .map_or(0., |&w| (w as f64 * scale as f64 + 0.5).floor() as f32)
            }
        })
        .sum()
}

/// `text_quads` at any size (cell 18x24 scaled by size / 18).
pub fn text_quads_sized(
    text: &[u8],
    origin: [f32; 2],
    rgba: [f32; 4],
    font: &Font,
    size: f32,
) -> Vec<Quad> {
    let scale = size / FONT_REFERENCE;
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
        let font = Font {
            widths: &widths,
            pad: 1,
        };
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
            reaction: None,
            cursor: 0,
            last_cursor: None,
            last_count: 0,
            opens: Vec::new(),
            sub: None,
        };
        menu.input(true, false, false);
        assert_eq!(menu.cursor, 3);
        assert_eq!(menu.input(false, false, true), Decision::Execute(3));
        assert_eq!(menu.cursor, 0);
        menu.input(false, true, false);
        assert_eq!(menu.input(false, false, true), Decision::Invalid);
    }

    #[test]
    fn a_submenu_opens_executes_and_backs_out() {
        let items = (0..3)
            .map(|_| Item {
                label: vec![0x2e],
                icon: 2,
                colour: 0,
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
            reaction: None,
            cursor: 0,
            last_cursor: None,
            last_count: 0,
            opens: vec![None, Some(1), None],
            sub: None,
        };
        // Row 1 opens the submenu and the cursor stays on it.
        menu.input(false, true, false);
        assert_eq!(menu.input(false, false, true), Decision::Open(1));
        assert_eq!(menu.cursor, 1);
        let sub_items = vec![
            Item {
                label: vec![0x2e],
                icon: 2,
                colour: 0,
            },
            Item {
                label: vec![0x2e],
                icon: 2,
                colour: 5,
            },
        ];
        menu.open_sub(1, sub_items);
        // Back closes it without leaving the top row.
        assert_eq!(
            menu.input_with_back(false, false, false, true),
            Decision::Closed
        );
        assert_eq!(menu.cursor, 1);
        // Reopen, move to the unusable row (error), back to the usable one,
        // decide: the sub closes and the top cursor returns to 0.
        menu.open_sub(
            1,
            vec![
                Item {
                    label: vec![0x2e],
                    icon: 2,
                    colour: 0,
                },
                Item {
                    label: vec![0x2e],
                    icon: 2,
                    colour: 5,
                },
            ],
        );
        menu.input(false, true, false);
        assert_eq!(menu.input(false, false, true), Decision::Invalid);
        menu.input(true, false, false);
        assert_eq!(
            menu.input(false, false, true),
            Decision::ExecuteSub { row: 1, index: 0 }
        );
        assert!(menu.sub.is_none());
        assert_eq!(menu.cursor, 0);
    }
}
