//! The camp's Customize screens for Sora (camp-07 spec): the character
//! list with its settings preview (state 0x19: FUN_14035d2a0 /
//! FUN_14035aa80 / FUN_14035e670), Sora's settings list — four shortcut
//! slots, MP Charge per owned spell, Auto-Reload per item in his item slots
//! (0x1a: FUN_14035ccb0 / FUN_14035d6a0 / FUN_14035a4e0) — the choice popups
//! (0x1d–0x1f: FUN_14035c750 / FUN_14035dde0 / FUN_14035a180) and the
//! Auto-Reload warning box (0x1c: FUN_14035c710 / FUN_14035a080). Party
//! members' settings (0x1b: battle style, ability and item frequencies),
//! NEW! marks and the SELECT tutorial notice are not built.
use super::items::{Grid, type_icon};
use super::*;

const PRIO_ROW: u32 = 0x7541;
const PRIO_PANEL: u32 = 0x753d;
const PRIO_BAR: u32 = 0x753e;
const PRIO_TEXT: u32 = 0x753f;
/// P (DAT_140743710) while the camp runs.
const P: u32 = 0x753c;

/// Preview bars per category (0x1405bc2b8): shortcut, MP Charge,
/// Auto-Reload.
const PREVIEW: [usize; 3] = [149, 144, 143];
const LIST_PANEL: [usize; 3] = [227, 228, 229];
/// Row and heading groups per category (0x1405bc218 / 0x1405bc228).
const ROW: [usize; 3] = [264, 263, 262];
const HEADING: [usize; 3] = [282, 281, 280];
const LIST_FRAME: usize = 249;
const BUBBLE: usize = 24;
/// Popup panel {appear, close, idle}, header, rows, frame per category
/// (0x1405bc250 / 270 / 280 / 28a).
const POPUP_PANEL: [[usize; 3]; 3] = [[198, 200, 199], [189, 191, 190], [180, 182, 181]];
const POPUP_HEADER: [usize; 3] = [206, 197, 188];
const POPUP_ROW: [usize; 3] = [202, 193, 184];
const POPUP_FRAME: [usize; 3] = [201, 192, 183];
const WHITE: usize = 123;
const VISIBLE: usize = 9;
const POPUP_VISIBLE: usize = 8;
/// Spells in MP-charge order (0x1405c5d18): Fire, Blizzard, Thunder, Cure,
/// Magnet, Reflect.
const SPELLS: [u16; 6] = [0x15, 0x16, 0x17, 0x18, 0x57, 0x58];
/// Cure's MP Charge cannot be changed (FUN_14035c180).
const CURE: u16 = 0x18;
/// Shortcut button glyphs in the 0x19 preview (0x1405bc298).
const SHORTCUT_GLYPHS: [u8; 4] = [0xdb, 0xdf, 0xde, 0xda];
const MAGIC_TYPE: u8 = 0x12;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomizeTexts {
    /// 0xbb89 (Sora) / 0xbb8a (party members).
    pub help: [Vec<u8>; 2],
    /// Category labels 0x1405bc238: 0x847d, 0x8481, 0x8482.
    pub labels: [Vec<u8>; 3],
    /// Shortcut headings 0x847d..0x8480 (`09` button glyph).
    pub shortcut_headings: [Vec<u8>; 4],
    /// 0x9df7 "Remove Shortcut settings."
    pub remove_help: Vec<u8>,
    /// 0x9df9 / 0x9df8, help 0x9dfb / 0x9dfa.
    pub mp_choices: [Vec<u8>; 2],
    pub mp_help: [Vec<u8>; 2],
    /// 0x9dfd / 0x9dfc, help 0x9dff / 0x9dfe.
    pub reload_choices: [Vec<u8>; 2],
    pub reload_help: [Vec<u8>; 2],
    /// 0x846a: the Auto-Reload OFF warning.
    pub reload_warning: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomizeData {
    /// save+0x36f8: four shortcut command ids (0 = empty).
    pub shortcuts: [u16; 4],
    /// save+0x3744: bit m set = "Don't MP Charge" for spell m.
    pub no_charge: u8,
    pub texts: CustomizeTexts,
}

/// A work-area entry: item id (0 = empty shortcut), on (MP Charge /
/// Auto-Reload), category 0 shortcut / 1 MP Charge / 2 Auto-Reload, and
/// the index inside its category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    id: u16,
    on: bool,
    cat: usize,
    k: usize,
}

#[derive(Default)]
pub(super) struct Ui {
    pub(super) chars: MenuList,
    panel: Option<Id>,
    bars: Vec<Id>,
    texts: Vec<Id>,
    rights: Vec<Id>,
    entries: Vec<Entry>,
    pub(super) list: Grid,
    rows: Vec<Id>,
    labels: Vec<Id>,
    headings: Vec<Id>,
    frame: Option<Id>,
    bubble: Option<Id>,
    // Popup.
    pub(super) popup: Grid,
    cat: usize,
    row: usize,
    candidates: Vec<u16>,
    popup_texts: Vec<Id>,
    /// 0x1c: the state that opened the box; the change waits for the
    /// popup to close (DAT_140bf0070 / 71).
    confirm_from: Option<State>,
    confirmed: bool,
}

impl Camp {
    /// FUN_14035b440 for Sora: four shortcuts, owned spells, the distinct
    /// items of his item slots.
    fn customize_entries(&self) -> Vec<Entry> {
        let table = &self.data.items.table;
        let c = &self.data.customize;
        let mut out = Vec::new();
        for (k, &v) in c.shortcuts.iter().enumerate() {
            let id = if v == 0 {
                0
            } else {
                table
                    .iter()
                    .find(|r| (r.kind == 0 || r.kind == MAGIC_TYPE) && r.command == v)
                    .map_or(0, |r| r.id)
            };
            out.push(Entry {
                id,
                on: false,
                cat: 0,
                k,
            });
        }
        let mut k = 0;
        for (m, &spell) in SPELLS.iter().enumerate() {
            let owned = table.iter().any(|r| r.id == spell && r.kind == MAGIC_TYPE)
                && self.data.items.counts.get(&spell).copied().unwrap_or(0) != 0;
            if owned {
                out.push(Entry {
                    id: spell,
                    on: c.no_charge >> m & 1 == 0,
                    cat: 1,
                    k,
                });
                k += 1;
            }
        }
        let e = self
            .data
            .items
            .equipment
            .first()
            .cloned()
            .unwrap_or_default();
        let mut seen = Vec::new();
        for (s, &cur) in e.items.iter().enumerate() {
            let orig = e.originals.get(s).copied().unwrap_or(0);
            let v = if cur != 0 { cur } else { orig };
            if v == 0 || seen.contains(&v) {
                continue;
            }
            seen.push(v);
            out.push(Entry {
                id: v,
                on: orig == v,
                cat: 2,
                k: seen.len() - 1,
            });
        }
        // A shortcut shows the MP-charge / auto-reload state of its item.
        for i in 0..4 {
            if out[i].id == 0 {
                continue;
            }
            if let Some(later) = out[4..].iter().rev().find(|e| e.id == out[i].id) {
                out[i].on = later.on;
            }
        }
        out
    }

    /// FUN_14035bd90: a row's text (`09 icon` + command or item name).
    fn customize_text(&self, e: &Entry, preview: bool) -> Vec<u8> {
        let r = self.info(e.id);
        let mut out = Vec::new();
        if e.cat == 0 && preview {
            out.extend([0x09, SHORTCUT_GLYPHS[e.k]]);
        } else if e.id != 0 {
            let alt = if r.kind == MAGIC_TYPE { !e.on } else { e.on };
            out.extend([0x09, type_icon(r.kind, alt)]);
        }
        if e.id == 0 {
            out.extend(self.data.items.texts.blank.clone());
        } else if e.cat <= 1 && !r.command_name.is_empty() {
            out.extend(r.command_name);
        } else {
            out.extend(r.name);
        }
        out
    }

    // ---- 0x19 --------------------------------------------------------------

    /// FUN_14035d2a0 + FUN_140359de0: tab 0, the character rows and the
    /// preview panel (18 bar slots with texts and right-aligned glyphs).
    pub(super) fn enter_customize_chars(&mut self, seq: &Sequence) {
        self.state = State::CustomizeChars;
        self.substate = 0;
        self.create_tab(seq, 0);
        let names: Vec<Vec<u8>> = self
            .data
            .panels
            .members
            .iter()
            .map(|m| m.name.clone())
            .collect();
        let cursor = self.memory.customize_char.min(names.len().max(1) - 1);
        let pitch = seq.groups[TOP_ROWS.appear].text_position_x as f32;
        let mut rows = Vec::new();
        for (i, name) in names.into_iter().enumerate() {
            let queued = if i == cursor {
                TOP_ROWS.selected
            } else {
                TOP_ROWS.idle
            };
            let t = self.row_text(name);
            let r = self
                .sprites
                .create(seq, PRIO_ROW, ART, TOP_ROWS.appear, Some(queued), Some(t));
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            self.closers.push((r, TOP_ROWS.close));
            rows.push(r);
        }
        self.custom_ui.chars = MenuList::new(rows, cursor);
        let panel = self.sprites.create(
            seq,
            PRIO_PANEL,
            ART,
            STATUS_PANEL[0],
            Some(STATUS_PANEL[1]),
            None,
        );
        self.closers.push((panel, STATUS_PANEL[2]));
        self.custom_ui.panel = Some(panel);
        let g = &seq.groups[150];
        let pitch = [g.text_position_x as f32, g.ui_padding as f32];
        let right_at = [g.light_position_x as f32, g.text_position_y as f32];
        self.custom_ui.bars.clear();
        self.custom_ui.texts.clear();
        self.custom_ui.rights.clear();
        for k in 0..18 {
            let l = self.sprites.create(seq, PRIO_BAR, ART, 150, None, None);
            self.sprites.set(seq, l, BLANK, None);
            self.child(l, panel);
            self.sprites.get_mut(l).offset =
                [pitch[0] * (k & 1) as f32, pitch[1] * (k >> 1) as f32];
            let v = self.sprites.create(
                seq,
                PRIO_TEXT,
                ART,
                WHITE,
                None,
                Some(self.row_text(Vec::new())),
            );
            self.child(v, l);
            let mut rt = self.row_text(Vec::new());
            rt.align = 1;
            let r = self
                .sprites
                .create(seq, PRIO_TEXT, ART, WHITE, None, Some(rt));
            self.child(r, l);
            self.sprites.get_mut(r).offset = right_at;
            self.custom_ui.bars.push(l);
            self.custom_ui.texts.push(v);
            self.custom_ui.rights.push(r);
        }
        self.customize_preview(seq);
    }

    /// FUN_14035e670: each category's rows, a new grid line per category.
    fn customize_preview(&mut self, seq: &Sequence) {
        self.custom_ui.entries = self.customize_entries();
        for k in 0..self.custom_ui.bars.len() {
            let (l, v, r) = (
                self.custom_ui.bars[k],
                self.custom_ui.texts[k],
                self.custom_ui.rights[k],
            );
            self.sprites.set(seq, l, BLANK, None);
            self.sprites.set(seq, v, WHITE, None);
            self.set_text(v, Vec::new());
            self.set_text(r, Vec::new());
        }
        let mut slot = 0usize;
        for (c, &bar) in PREVIEW.iter().enumerate() {
            let entries: Vec<Entry> = self
                .custom_ui
                .entries
                .iter()
                .copied()
                .filter(|e| e.cat == c)
                .collect();
            for e in &entries {
                if slot >= self.custom_ui.bars.len() {
                    break;
                }
                let text = self.customize_text(e, true);
                self.sprites.set(seq, self.custom_ui.bars[slot], bar, None);
                self.set_text(self.custom_ui.texts[slot], text);
                slot += 1;
            }
            if entries.len() % 2 == 1 {
                slot += 1;
            }
        }
    }

    fn close_customize_chars(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.custom_ui.panel, STATUS_PANEL[2]);
        for &r in &self.custom_ui.chars.rows.clone() {
            self.close_opt(seq, Some(r), TOP_ROWS.close);
        }
    }

    /// FUN_14035aa80.
    pub(super) fn customize_chars(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let prev = self.custom_ui.chars.cursor;
        let (pick, moved) = self.custom_ui.chars.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let busy = self.alive_opt(self.custom_ui.panel)
                || self.rows_alive(&self.custom_ui.chars.clone())
                || (p == Some(State::Top) && self.tab_alive(0));
            if !busy {
                match self.pending.take() {
                    Some(State::Top) => self.return_top(seq),
                    _ => self.enter_customize_list(seq),
                }
            } else if pad.trig != 0 {
                self.kill_opt(self.custom_ui.panel);
                for &r in &self.custom_ui.chars.rows {
                    self.sprites.kill(r);
                }
            }
            return false;
        }
        let row0 = self.group_of(self.custom_ui.chars.rows.first().copied());
        if (pick != Pick::None || moved) && row0 == Some(TOP_ROWS.appear) {
            for (i, &r) in self.custom_ui.chars.rows.clone().iter().enumerate() {
                let g = if i == prev {
                    TOP_ROWS.selected
                } else {
                    TOP_ROWS.idle
                };
                self.sprites.set(seq, r, g, None);
            }
        }
        let row0 = self.group_of(self.custom_ui.chars.rows.first().copied());
        if self.substate == 0
            && matches!(row0, Some(g) if g == TOP_ROWS.idle || g == TOP_ROWS.selected)
        {
            let h = self.data.customize.texts.help[usize::from(self.custom_ui.chars.cursor != 0)]
                .clone();
            self.show_help(seq, h);
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            self.customize_preview(seq);
        }
        let cursor = self.custom_ui.chars.cursor;
        match pick {
            Pick::Start => self.exit = true,
            Pick::Cancel => {
                self.hide_help(seq);
                self.close_tab(seq, 0);
                self.close_customize_chars(seq);
                self.pending = Some(State::Top);
                self.substate = 2;
            }
            Pick::Row(r) => {
                let name = self.data.panels.members[r].name.clone();
                self.pending_tab_text[1] = Some(name);
                self.hide_help(seq);
                self.close_customize_chars(seq);
                self.pending = Some(State::CustomizeList);
                self.substate = 2;
            }
            _ => {}
        }
        if matches!(pick, Pick::Start | Pick::Cancel | Pick::Row(_)) {
            if cursor != self.memory.customize_char {
                self.memory.customize_list = (0, 0);
            }
            self.memory.customize_char = cursor;
        }
        moved
    }

    // ---- 0x1a --------------------------------------------------------------

    /// FUN_14035ccb0: tab 1, the panel, 9 rows with labels and headings,
    /// the scrollbar and the Switch banner.
    fn enter_customize_list(&mut self, seq: &Sequence) {
        self.state = State::CustomizeList;
        self.substate = 0;
        self.create_tab(seq, 0);
        self.create_tab(seq, 1);
        self.custom_ui.entries = self.customize_entries();
        let n = self.custom_ui.entries.len();
        let panel = self
            .sprites
            .create(seq, P, ART, LIST_PANEL[0], Some(LIST_PANEL[1]), None);
        self.closers.push((panel, LIST_PANEL[2]));
        self.custom_ui.panel = Some(panel);
        let pitch = seq.groups[ROW[0]].ui_padding as f32;
        let heading_pitch = seq.groups[HEADING[0]].ui_padding as f32;
        let heading_size = seq.groups[HEADING[0]].text_scale as f32;
        self.custom_ui.rows.clear();
        self.custom_ui.labels.clear();
        self.custom_ui.headings.clear();
        for i in 0..VISIBLE {
            let r = self.sprites.create(seq, P, ART, BLANK, None, None);
            self.child(r, panel);
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            let t = self.sprites.create(
                seq,
                P + 2,
                ART,
                WHITE,
                None,
                Some(self.row_text(Vec::new())),
            );
            self.child(t, r);
            self.custom_ui.rows.push(r);
            self.custom_ui.labels.push(t);
        }
        for k in 0..VISIBLE {
            let mut t = Text::new(Vec::new(), heading_size);
            t.align = 2;
            t.keep_rgb = false;
            let h = self.sprites.create(seq, P + 1, ART, BLANK, None, Some(t));
            self.child(h, panel);
            self.sprites.get_mut(h).offset = [0., heading_pitch * k as f32];
            self.custom_ui.headings.push(h);
        }
        self.build_scrollbar(seq, panel, P + 1, n, n.min(VISIBLE));
        // §9: the remembered cursor, fixed up against the list size.
        let (mut cur, mut s) = self.memory.customize_list;
        let abs = cur + s;
        if n <= abs || n < VISIBLE + s {
            while s > 0 && n < VISIBLE + s {
                s -= 1;
            }
            cur = abs.saturating_sub(s).min(VISIBLE - 1);
            if n <= s + cur {
                cur = 0;
                s = 0;
            }
        }
        let mut list = MenuList::new(self.custom_ui.rows.clone(), cur);
        list.total = n;
        list.page = Some(VISIBLE);
        self.custom_ui.list = Grid {
            list,
            scroll: s,
            cols: 1,
            lines: VISIBLE,
        };
        self.customize_mask();
        let t = self.data.abilities.texts.switch.clone();
        self.show_banner(seq, t);
        self.customize_skin(seq);
    }

    fn customize_cursor_entry(&self) -> Option<Entry> {
        self.custom_ui
            .entries
            .get(self.custom_ui.list.list.cursor + self.custom_ui.list.scroll)
            .copied()
    }

    /// The notice category of the row under the cursor, when one has its
    /// flag.
    fn notice_for_entry(&self) -> Option<u8> {
        let c = match self.customize_cursor_entry()?.cat {
            1 => 1,
            2 => 0,
            _ => return None,
        };
        self.notice_available(c).then_some(c)
    }

    /// §4.1 / §4.5: page keys, L2/R2, △ on a non-shortcut row.
    fn customize_mask(&mut self) {
        let mut mask = 0x73 | 0x3000 | 0xc000;
        if self.customize_cursor_entry().is_some_and(|e| e.cat != 0) {
            mask |= 0x200;
        }
        // SELECT: MP Charge rows show the MP Charge notice (1), Auto-Reload
        // rows the Auto-Reload one (0) [I].
        if self.notice_for_entry().is_some() {
            mask |= 0x400;
        }
        self.custom_ui.list.list.mask = mask;
    }

    /// FUN_14035d6a0: headings at each category's first row (each shortcut
    /// row has its own), row groups, labels.
    fn customize_skin(&mut self, seq: &Sequence) {
        for k in 0..VISIBLE {
            for id in [self.custom_ui.rows[k], self.custom_ui.headings[k]] {
                self.sprites.set(seq, id, BLANK, None);
            }
            self.set_text(self.custom_ui.headings[k], Vec::new());
            let t = self.custom_ui.labels[k];
            self.sprites.set(seq, t, WHITE, None);
            self.set_text(t, Vec::new());
        }
        let n = self.custom_ui.entries.len();
        let s = self.custom_ui.list.scroll;
        self.scroll_to(seq, s, n, n.min(VISIBLE));
        let entries = self.custom_ui.entries.clone();
        for v in 0..VISIBLE {
            let Some(e) = entries.get(s + v) else { break };
            if e.k == 0 || e.cat == 0 {
                let h = self.custom_ui.headings[v];
                self.sprites.set(seq, h, HEADING[e.cat], None);
                let text = if e.cat == 0 {
                    self.data.customize.texts.shortcut_headings[e.k].clone()
                } else {
                    self.data.customize.texts.labels[e.cat].clone()
                };
                self.set_text(h, text);
            }
            self.sprites
                .set(seq, self.custom_ui.rows[v], ROW[e.cat], None);
            let text = self.customize_text(e, false);
            self.set_text(self.custom_ui.labels[v], text);
        }
    }

    /// FUN_14035dac0 in 0x1a: the row item's description.
    fn customize_list_help(&mut self, seq: &Sequence) {
        let h = self
            .customize_cursor_entry()
            .filter(|e| e.id != 0)
            .map(|e| self.info(e.id).description)
            .unwrap_or_default();
        self.show_help(seq, h);
    }

    /// FUN_14035e930: the cursor frame on the hand's row and, on a row △
    /// can change, the bubble on the hand.
    fn customize_frame(&mut self, seq: &Sequence, group: usize, bubble: bool) {
        let glove = self.glove.filter(|&g| self.sprites.alive(g));
        let Some(glove) = glove else {
            let f = self.custom_ui.frame.take();
            self.kill_opt(f);
            let b = self.custom_ui.bubble.take();
            self.kill_opt(b);
            return;
        };
        let offset = self
            .sprites
            .get(glove)
            .parent
            .map_or([0.; 2], |r| self.sprites.get(r).offset);
        let f = match self.custom_ui.frame {
            Some(f) if self.sprites.alive(f) => f,
            _ => {
                let f = self.sprites.create(seq, P + 2, ART, group, None, None);
                self.child(f, self.custom_ui.panel.unwrap());
                self.custom_ui.frame = Some(f);
                f
            }
        };
        self.sprites.get_mut(f).offset = offset;
        if bubble {
            if !self.alive_opt(self.custom_ui.bubble) {
                let mut t = Text::new(self.data.abilities.texts.triangle.clone(), TEXT_SIZE);
                t.align = 2;
                let b = self
                    .sprites
                    .create(seq, PRIO_LIGHT, ART, BUBBLE, None, Some(t));
                self.custom_ui.bubble = Some(b);
            }
            let b = self.custom_ui.bubble.unwrap();
            self.sprites.get_mut(b).parent = Some(glove);
        } else {
            let b = self.custom_ui.bubble.take();
            self.kill_opt(b);
        }
    }

    fn close_customize_list(&mut self, seq: &Sequence) {
        self.close_opt(seq, self.custom_ui.panel, LIST_PANEL[2]);
        self.close_scrollbar(seq);
        let f = self.custom_ui.frame.take();
        self.kill_opt(f);
        let b = self.custom_ui.bubble.take();
        self.kill_opt(b);
    }

    /// FUN_1403597e0: MP Charge on / off for a spell entry.
    fn toggle_charge(&mut self, id: u16) {
        if let Some(m) = SPELLS.iter().position(|&s| s == id) {
            self.data.customize.no_charge ^= 1 << m;
        }
    }

    /// FUN_14035ebe0: turning `id`'s Auto-Reload off would empty a used-up
    /// slot (orig == id, cur ≠ id).
    fn reload_off_empties(&self, id: u16) -> bool {
        let e = self.data.items.equipment.first();
        e.is_some_and(|e| {
            e.items
                .iter()
                .zip(&e.originals)
                .any(|(&cur, &orig)| orig == id && cur != id)
        })
    }

    /// FUN_1403598e0 for Sora: Auto-Reload of `id` on / off in every slot
    /// holding it.
    fn toggle_reload(&mut self, id: u16, on: bool) {
        if let Some(e) = self.data.items.equipment.first_mut() {
            for s in 0..e.items.len() {
                if e.items[s] == id || e.originals.get(s) == Some(&id) {
                    e.originals[s] = if on { id } else { 0 };
                }
            }
        }
    }

    /// FUN_14035a4e0 (Sora).
    pub(super) fn customize_list(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let scroll = self.custom_ui.list.scroll;
        let (pick, moved) = self.custom_ui.list.input(pad);
        if self.substate == 2 {
            let p = self.pending;
            let busy = self.alive_opt(self.custom_ui.panel)
                || self.item_ui.scroll.iter().any(|&s| self.sprites.alive(s))
                || (p == Some(State::CustomizeChars) && self.tab_alive(1))
                || (p != Some(State::CustomizeChars) && self.alive_opt(self.banner));
            if !busy {
                match self.pending.take() {
                    Some(State::CustomizeChars) => {
                        self.kill_tab(1);
                        self.kill_opt(self.banner);
                        self.enter_customize_chars(seq);
                    }
                    _ => self.enter_customize_popup(seq),
                }
            } else if pad.trig != 0 {
                self.kill_opt(self.custom_ui.panel);
                for &s in &self.item_ui.scroll {
                    self.sprites.kill(s);
                }
            }
            return false;
        }
        if (pick != Pick::None || moved)
            && self.group_of(self.custom_ui.panel) != Some(LIST_PANEL[1])
        {
            for (id, g) in [
                (self.custom_ui.panel, LIST_PANEL[1]),
                (self.banner, BANNER[1]),
            ] {
                if let Some(i) = id
                    && self.sprites.alive(i)
                {
                    self.sprites.set(seq, i, g, None);
                }
            }
        }
        if self.substate == 0 && self.group_of(self.custom_ui.panel) == Some(LIST_PANEL[1]) {
            self.customize_list_help(seq);
            self.substate = 1;
        }
        // FUN_14035c180: L2/R2 need another member; Cure's MP Charge is
        // fixed.
        let entry = self.customize_cursor_entry();
        let cure = entry.is_some_and(|e| e.cat == 1 && e.id == CURE);
        let valid = match pick {
            Pick::Next | Pick::Previous => self.data.panels.members.len() > 1,
            Pick::Row(_) | Pick::Triangle => !cure,
            _ => true,
        };
        if let Some(s) = pick_sound(pick, moved, valid) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            if self.custom_ui.list.scroll != scroll {
                self.customize_skin(seq);
            }
            self.customize_mask();
            self.customize_list_help(seq);
        }
        let cursor = (self.custom_ui.list.list.cursor, self.custom_ui.list.scroll);
        if valid {
            match pick {
                Pick::Triangle => {
                    if let Some(e) = entry {
                        match e.cat {
                            1 => self.toggle_charge(e.id),
                            2 if e.on && self.reload_off_empties(e.id) => {
                                self.open_reload_warning(seq, State::CustomizeList, e.id);
                            }
                            2 => self.toggle_reload(e.id, !e.on),
                            _ => {}
                        }
                        self.custom_ui.entries = self.customize_entries();
                        self.customize_skin(seq);
                        self.customize_list_help(seq);
                    }
                }
                Pick::Select => {
                    self.memory.customize_list = cursor;
                    if let Some(c) = self.notice_for_entry() {
                        self.start_notices(seq, Some(c), State::CustomizeList);
                    }
                }
                Pick::Start => {
                    self.memory.customize_list = cursor;
                    self.exit = true;
                }
                Pick::Cancel => {
                    self.memory.customize_list = cursor;
                    self.hide_help(seq);
                    self.close_banner(seq);
                    self.close_tab(seq, 1);
                    self.close_customize_list(seq);
                    self.pending = Some(State::CustomizeChars);
                    self.substate = 2;
                }
                Pick::Row(r) => {
                    self.memory.customize_list = cursor;
                    let e = self.custom_ui.entries[r];
                    let label = if e.cat == 0 {
                        self.data.customize.texts.shortcut_headings[e.k].clone()
                    } else {
                        self.data.customize.texts.labels[e.cat].clone()
                    };
                    self.pending_tab_text[2] = Some(label);
                    self.custom_ui.row = r;
                    self.close_banner(seq);
                    self.hide_help(seq);
                    self.close_customize_list(seq);
                    self.pending = Some(State::CustomizePopup(e.cat as u8));
                    self.substate = 2;
                }
                _ => {}
            }
        }
        if self.state == State::CustomizeList {
            let bubble = self.customize_cursor_entry().is_some_and(|e| e.cat != 0);
            self.customize_frame(seq, LIST_FRAME, bubble);
        }
        moved
    }

    // ---- 0x1d–0x1f -----------------------------------------------------------

    /// FUN_14034c770(filter 10) + FUN_14034caa0: spells, then consumables,
    /// owned, in item-table order.
    fn shortcut_candidates(&self) -> Vec<u16> {
        let owned = |id: u16| self.data.items.counts.get(&id).copied().unwrap_or(0) != 0;
        let table = &self.data.items.table;
        table
            .iter()
            .filter(|r| r.kind == MAGIC_TYPE && owned(r.id))
            .chain(table.iter().filter(|r| r.kind == 0 && owned(r.id)))
            .map(|r| r.id)
            .collect()
    }

    /// FUN_14035c750: tab 2, the popup panel, its header with the row's
    /// text, the choice rows, the scrollbar; the cursor starts on the
    /// current value.
    fn enter_customize_popup(&mut self, seq: &Sequence) {
        let e = self.custom_ui.entries[self.custom_ui.row];
        let c = e.cat;
        self.state = State::CustomizePopup(c as u8);
        self.substate = 0;
        self.custom_ui.cat = c;
        self.create_tab(seq, 2);
        self.custom_ui.candidates = if c == 0 {
            self.shortcut_candidates()
        } else {
            Vec::new()
        };
        let n = match c {
            0 => self.custom_ui.candidates.len() + 1,
            _ => 2,
        };
        let panel = self.sprites.create(
            seq,
            P,
            ART,
            POPUP_PANEL[c][0],
            Some(POPUP_PANEL[c][2]),
            None,
        );
        self.closers.push((panel, POPUP_PANEL[c][1]));
        self.custom_ui.panel = Some(panel);
        let header = self
            .sprites
            .create(seq, P + 1, ART, POPUP_HEADER[c], None, None);
        self.child(header, panel);
        let text = self.customize_text(&e, false);
        let h = self
            .sprites
            .create(seq, P + 2, ART, WHITE, None, Some(self.row_text(text)));
        self.child(h, header);
        let k = n.min(POPUP_VISIBLE);
        let pitch = seq.groups[POPUP_ROW[c]].ui_padding as f32;
        let mut rows = Vec::new();
        self.custom_ui.popup_texts.clear();
        for i in 0..k {
            let r = self.sprites.create(seq, P, ART, POPUP_ROW[c], None, None);
            self.child(r, panel);
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            let t = self.sprites.create(
                seq,
                P + 2,
                ART,
                WHITE,
                None,
                Some(self.row_text(Vec::new())),
            );
            self.child(t, r);
            rows.push(r);
            self.custom_ui.popup_texts.push(t);
        }
        self.build_scrollbar(seq, panel, P + 1, n, k);
        // FUN_14035c2a0.
        let value = match c {
            0 => self
                .custom_ui
                .candidates
                .iter()
                .position(|&id| id == e.id && e.id != 0)
                .map_or(0, |p| p + 1),
            _ => usize::from(e.on),
        };
        let (v, s) = popup_start(value, n);
        let mut list = MenuList::new(rows, v);
        list.total = n;
        list.page = Some(k);
        self.custom_ui.popup = Grid {
            list,
            scroll: s,
            cols: 1,
            lines: k,
        };
        self.customize_popup_texts(seq);
    }

    /// FUN_14035dde0: the visible choices.
    fn customize_popup_texts(&mut self, seq: &Sequence) {
        let c = self.custom_ui.cat;
        let s = self.custom_ui.popup.scroll;
        let n = self.custom_ui.popup.list.total;
        let k = self.custom_ui.popup_texts.len();
        self.scroll_to(seq, s, n, k);
        let t = self.data.customize.texts.clone();
        let choice = |i: usize| -> Vec<u8> {
            match c {
                1 => t.mp_choices[i.min(1)].clone(),
                _ => t.reload_choices[i.min(1)].clone(),
            }
        };
        for i in 0..k {
            let a = s + i;
            let text = if c == 0 {
                if a == 0 {
                    self.data.items.texts.blank.clone()
                } else {
                    let id = self.custom_ui.candidates[a - 1];
                    self.customize_text(
                        &Entry {
                            id,
                            on: false,
                            cat: 0,
                            k: 0,
                        },
                        false,
                    )
                }
            } else {
                choice(a)
            };
            self.set_text(self.custom_ui.popup_texts[i], text);
        }
    }

    fn customize_popup_help(&mut self, seq: &Sequence) {
        let a = self.custom_ui.popup.list.cursor + self.custom_ui.popup.scroll;
        let t = &self.data.customize.texts;
        let h = match self.custom_ui.cat {
            0 if a == 0 => t.remove_help.clone(),
            0 => self.info(self.custom_ui.candidates[a - 1]).description,
            1 => t.mp_help[a.min(1)].clone(),
            _ => t.reload_help[a.min(1)].clone(),
        };
        self.show_help(seq, h);
    }

    /// FUN_14035c710 + FUN_1402f6520(0x1c): the warning box.
    fn open_reload_warning(&mut self, seq: &Sequence, from: State, id: u16) {
        self.custom_ui.confirm_from = Some(from);
        self.custom_ui.confirmed = false;
        self.custom_ui.row = self
            .custom_ui
            .entries
            .iter()
            .position(|e| e.cat == 2 && e.id == id)
            .unwrap_or(self.custom_ui.row);
        let text = self.data.customize.texts.reload_warning.clone();
        let (no, yes) = (
            self.data.items.texts.no.clone(),
            self.data.items.texts.yes.clone(),
        );
        self.dim_glove(seq);
        self.open_dialog(seq, text, vec![no, yes]);
        self.state = State::CustomizeConfirm;
        self.substate = 0;
    }

    /// FUN_14035a080.
    pub(super) fn customize_confirm(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let (pick, moved) = self.item_ui.choices.input(pad);
        if self.substate == 2 {
            if !self.dialog_alive() {
                self.undim_last();
                self.state = self
                    .custom_ui
                    .confirm_from
                    .take()
                    .unwrap_or(State::CustomizeList);
                self.substate = 1;
                self.pending = None;
            }
            return false;
        }
        if self.substate == 0 && self.group_of(self.item_ui.dialog_box()) == Some(7) {
            self.substate = 1;
        }
        if self.substate != 1 {
            return false;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if matches!(pick, Pick::Row(_) | Pick::Cancel) {
            if pick == Pick::Row(1) {
                if self.custom_ui.confirm_from == Some(State::CustomizeList) {
                    let id = self.custom_ui.entries[self.custom_ui.row].id;
                    self.toggle_reload(id, false);
                    self.custom_ui.entries = self.customize_entries();
                    self.customize_skin(seq);
                }
                self.custom_ui.confirmed = true;
            }
            self.close_dialog(seq);
            self.substate = 2;
        }
        moved
    }

    /// FUN_14035a180.
    pub(super) fn customize_popup(
        &mut self,
        seq: &Sequence,
        pad: &Pad,
        sounds: &mut Vec<u32>,
    ) -> bool {
        let c = self.custom_ui.cat;
        let scroll = self.custom_ui.popup.scroll;
        let (pick, moved) = self.custom_ui.popup.input(pad);
        if self.substate == 2 {
            let busy = self.alive_opt(self.custom_ui.panel)
                || self.item_ui.scroll.iter().any(|&s| self.sprites.alive(s))
                || self.tab_alive(2);
            if !busy {
                self.pending = None;
                self.kill_tab(2);
                self.enter_customize_list(seq);
            } else if pad.trig != 0 {
                self.kill_opt(self.custom_ui.panel);
                for &s in &self.item_ui.scroll {
                    self.sprites.kill(s);
                }
            }
            return false;
        }
        if (pick != Pick::None || moved)
            && self.group_of(self.custom_ui.panel) != Some(POPUP_PANEL[c][2])
            && let Some(p) = self.custom_ui.panel
        {
            self.sprites.set(seq, p, POPUP_PANEL[c][2], None);
        }
        if self.substate == 0 && self.group_of(self.custom_ui.panel) == Some(POPUP_PANEL[c][2]) {
            self.customize_popup_help(seq);
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            if self.custom_ui.popup.scroll != scroll {
                self.customize_popup_texts(seq);
            }
            self.customize_popup_help(seq);
        }
        let mut close = self.custom_ui.confirmed;
        if !close {
            match pick {
                Pick::Start => self.exit = true,
                Pick::Cancel => close = true,
                Pick::Row(r) => {
                    let e = self.custom_ui.entries[self.custom_ui.row];
                    close = true;
                    match c {
                        0 => {
                            self.data.customize.shortcuts[e.k] = if r == 0 {
                                0
                            } else {
                                self.info(self.custom_ui.candidates[r - 1]).command
                            };
                        }
                        1 if e.on != (r != 0) => self.toggle_charge(e.id),
                        2 if e.on != (r != 0) => {
                            if r == 0 && self.reload_off_empties(e.id) {
                                self.open_reload_warning(seq, State::CustomizePopup(2), e.id);
                                close = false;
                            } else {
                                self.toggle_reload(e.id, r != 0);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        } else {
            // Back from the warning with "Yes": apply on the popup's exit.
            let id = self.custom_ui.entries[self.custom_ui.row].id;
            self.toggle_reload(id, false);
            self.custom_ui.confirmed = false;
        }
        if close {
            self.hide_help(seq);
            self.close_tab(seq, 2);
            self.close_opt(seq, self.custom_ui.panel, POPUP_PANEL[c][1]);
            self.close_scrollbar(seq);
            let f = self.custom_ui.frame.take();
            self.kill_opt(f);
            self.pending = Some(State::CustomizeList);
            self.substate = 2;
        }
        if matches!(self.state, State::CustomizePopup(_)) && self.substate != 2 {
            self.customize_frame(seq, POPUP_FRAME[c], false);
        }
        moved
    }
}

/// FUN_14035c2a0: (cursor, scroll) for value index `value` of `n` choices
/// with 8 visible rows.
fn popup_start(value: usize, n: usize) -> (usize, usize) {
    let (mut v, mut s) = (value, 0);
    if v > 7 {
        s = v - 7;
        v = 7;
    }
    if s + v >= n {
        (v, s) = (n - 1, (v + 1).saturating_sub(n));
    }
    (v, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popups_open_on_the_current_value() {
        assert_eq!(popup_start(0, 4), (0, 0));
        assert_eq!(popup_start(3, 4), (3, 0));
        // Value 10 of 12: scrolled so it sits on the last visible row.
        assert_eq!(popup_start(10, 12), (7, 3));
    }
}
