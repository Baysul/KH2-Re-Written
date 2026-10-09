//! The camp (pause) menu, translated from the camp process: build
//! FUN_140306040 / FUN_140306790 / FUN_140367b50, state machine
//! FUN_140303260 with the top level FUN_1403037d0, the Status screens
//! (0x17 FUN_140351db0 / FUN_140353020 / FUN_1403540b0, 0x18 FUN_140351a00 /
//! FUN_140352be0 / FUN_140353800) and Config (0x24 FUN_140364d70 /
//! FUN_140365580 / FUN_140365a20), the per-frame driver FUN_140303cd0
//! (glove pointer, LightCur), list input FUN_1402f9440, title tabs
//! FUN_140323c60, help FUN_1402fa1d0 and the digit rows FUN_1402fb450 /
//! FUN_140324690. Everything draws from `menu/<lang>/camp.2ld` sequence 1 in
//! layout space (x origin 0). The other sub-screens (Items, Abilities,
//! Customize, Party, Journal) and the 3D party models are not built:
//! deciding those entries plays the decide sound only.
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

/// Top-level rows (entry table 0x1405badb0): appear, close, idle, selected.
const TOP_ROWS: RowGroups = RowGroups {
    appear: 133,
    close: 135,
    idle: 134,
    selected: 132,
};
/// Status character list rows: characters, forms/Summon (0x1405bad68..80).
const FORM_ROWS: RowGroups = RowGroups {
    appear: 129,
    close: 131,
    idle: 130,
    selected: 128,
};
const TEXT_SIZE: f32 = 15.;

const LIGHT: [usize; 3] = [27, 28, 29];
const GLOVE: usize = 25;
const HELP_IN: usize = 116;
const HELP_OUT: usize = 117;
const FRAME: [usize; 3] = [46, 47, 48];
const PROMPT: [usize; 3] = [351, 352, 353];
const MUNNY: [usize; 3] = [107, 108, 109];
const TIME: [usize; 3] = [113, 114, 115];
const MUNNY_DIGIT: usize = 51;
const TIME_DIGIT: usize = 61;
const BLANK: usize = 31;
const COLON: usize = 71;
/// Title tabs (FUN_140323c60): appear, idle, close.
const TAB: [usize; 3] = [75, 76, 77];
/// Party status block (FUN_140367b50 mode 0).
const STATUS: [usize; 3] = [101, 102, 103];
const BLOCK: usize = 93;
const SUB_ANCHOR: usize = 90;
/// LV, HP, MP style groups (FUN_140304d70(2/4/6)); yellow values (124).
const STYLES: [usize; 3] = [124, 121, 118];
const VALUE: usize = 124;
const GAUGE_FRAME: usize = 95;
const GAUGE_BARS: [usize; 2] = [98, 99];
/// Status 0x17 panel, label group; 0x18 panel, banner.
const STATUS_PANEL: [usize; 3] = [140, 141, 142];
const STATUS_LABEL: usize = 137;
const BROWSE_PANEL: [usize; 3] = [320, 321, 322];
const BROWSE_ROW: usize = 323;
const BANNER: [usize; 3] = [237, 238, 239];
/// Stat style s → (0x17 label group, 0x18 row group) (0x1405bbef0).
const STAT_STYLES: [(usize, usize); 5] =
    [(137, 323), (143, 325), (150, 327), (139, 324), (144, 326)];
/// Config panel, label group; choice groups {selected, unselected,
/// cursor frame} by choice count (0x1405bc320..30).
const CONFIG_PANEL: [usize; 3] = [233, 234, 235];
const CONFIG_LABEL: usize = 279;
const CHOICES: [[usize; 3]; 3] = [[261, 260, 259], [255, 254, 253], [258, 257, 256]];

/// Priorities of the sub-screens (camp-04 §2-3).
const PRIO_STATUS_PANEL: u32 = 0x753d;
const PRIO_STATUS_LABEL: u32 = 0x753e;
const PRIO_STATUS_ROW: u32 = 0x753f;
const PRIO_CONFIG_PANEL: u32 = 0x753a;

/// se000 sounds (FUN_1402f6270 → FUN_1402eb740): open, move, decide,
/// close/cancel, invalid.
pub const SOUND_OPEN: u32 = 3;
pub const SOUND_CURSOR: u32 = 1;
pub const SOUND_DECIDE: u32 = 2;
pub const SOUND_CLOSE: u32 = 4;
pub const SOUND_INVALID: u32 = 5;

/// Logical bits (pad mode 7 while a menu runs).
mod bits {
    pub const UP: u64 = 0x10;
    pub const DOWN: u64 = 0x20;
    pub const LEFT: u64 = 0x40;
    pub const RIGHT: u64 = 0x80;
    pub const DECIDE: u64 = 0x4;
    pub const CANCEL: u64 = 0x8;
    pub const START: u64 = 0x100;
    pub const R1: u64 = 0x4000;
    pub const L1: u64 = 0x8000;
    pub const R2: u64 = 0x1_0000;
    pub const L2: u64 = 0x2_0000;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RowGroups {
    appear: usize,
    close: usize,
    idle: usize,
    selected: usize,
}

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

/// What the top-level panels show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panels {
    pub munny: u32,
    /// Play time in 60 Hz frames (save+0x2444).
    pub time_frames: u32,
    pub members: Vec<Member>,
    /// "LV", "HP", "MP" (sys.bar 0xb9fc..0xb9fe).
    pub labels: [Vec<u8>; 3],
}

/// One Status stat (camp-04 §2.4): label, help, style and its encoded
/// value text.
#[derive(Clone, Debug, PartialEq)]
pub struct Stat {
    pub label: Vec<u8>,
    pub help: Vec<u8>,
    pub style: usize,
    pub value: Vec<u8>,
}

/// A Status list entry: its list bit (0 Sora, 1..5 forms, 6 Summon, 7..9
/// party), label, help line and its 20 stats (None = not shown).
#[derive(Clone, Debug, PartialEq)]
pub struct StatusEntry {
    pub bit: u8,
    pub label: Vec<u8>,
    pub help: Vec<u8>,
    pub stats: Vec<Option<Stat>>,
}

/// A Config row: label, choice labels and the help line of each choice.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigRow {
    pub label: Vec<u8>,
    pub choices: Vec<Vec<u8>>,
    pub help: Vec<Vec<u8>>,
}

/// Everything the camp shows besides its art.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Data {
    pub entries: Vec<Entry>,
    pub panels: Panels,
    pub status: Vec<StatusEntry>,
    /// The 0x18 banner ("Switch", 0xb61a).
    pub banner: Vec<u8>,
    /// 8 rows; row 7 (Game Mode) lists all four modes.
    pub config: Vec<ConfigRow>,
    /// save+0x41a4 and save+0x2498.
    pub config_flags: u32,
    pub game_mode: u32,
}

/// Cursor memory kept between camp openings (DAT_140beec30 / 7c).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    /// Top-level entry id.
    pub entry: u8,
    /// Status list bit.
    pub status_bit: u8,
    pub config_row: usize,
}

/// What a draw item samples: the camp sheet or the system font.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    Art,
    Font,
}

/// Camp states (DAT_140bee64c).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Top,
    StatusList,
    StatusBrowse,
    Config,
}

/// FUN_1403adcc0: sprintf digits, '/', '%' and spaces into the font's
/// encoding.
pub fn encode_number(s: &str) -> Vec<u8> {
    s.bytes()
        .map(|b| match b {
            b'0'..=b'9' => 0x21 + (b - b'0'),
            b'/' => 0x4b,
            b'%' => 0x4a,
            _ => 0x01,
        })
        .collect()
}

/// Config values from save+0x41a4 / +0x2498 (FUN_140365580, camp-04 §3.1).
pub fn config_values(flags: u32, mode: u32) -> [usize; 8] {
    let bit = |b: u32| (flags >> b & 1) as usize;
    let summon = match flags >> 9 & 3 {
        2 => 0,
        1 | 3 => 1,
        _ => 2,
    };
    [
        bit(4),
        bit(8),
        bit(7),
        summon,
        1 - bit(3),
        1 - bit(0),
        bit(6),
        mode.min(3) as usize,
    ]
}

/// FUN_140365460: the values back into save+0x41a4.
pub fn config_flags(flags: u32, v: &[usize; 8]) -> u32 {
    let set = |f: u32, b: u32, on: bool| if on { f | 1 << b } else { f & !(1 << b) };
    let mut f = flags;
    f = set(f, 4, v[0] == 1);
    f = set(f, 8, v[1] == 1);
    f = set(f, 7, v[2] == 1);
    f = (f & !(3 << 9)) | ([2u32, 1, 0][v[3].min(2)] << 9);
    f = set(f, 3, v[4] == 0);
    f = set(f, 0, v[5] == 0);
    set(f, 6, v[6] == 1)
}

/// The list object (0x48 B, FUN_1402f7720) and its input FUN_1402f9440.
#[derive(Clone, Debug, Default)]
struct MenuList {
    rows: Vec<Id>,
    /// Number of usable rows (the rest are hidden).
    total: usize,
    cursor: usize,
    /// Enable mask (+0x40); 0x73 by default.
    mask: u32,
    /// Page keys (+0x27) with this page size.
    page: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    None,
    Row(usize),
    Cancel,
    Start,
    Next,
    Previous,
    Left,
    Right,
}

struct Pad {
    trig: u64,
    repeat: u64,
}

impl MenuList {
    fn new(rows: Vec<Id>, cursor: usize) -> Self {
        let total = rows.len();
        Self {
            rows,
            total,
            cursor: cursor.min(total.saturating_sub(1)),
            mask: 0x73,
            page: None,
        }
    }

    fn input(&mut self, p: &Pad) -> (Pick, bool) {
        use bits::*;
        let n = self.total;
        if n == 0 {
            return (Pick::None, false);
        }
        let before = self.cursor;
        if p.repeat & UP != 0 && self.mask & 0x1 != 0 {
            if self.cursor > 0 {
                self.cursor -= 1;
            } else if p.trig & UP != 0 {
                self.cursor = n - 1;
            }
        } else if p.repeat & DOWN != 0 && self.mask & 0x2 != 0 {
            if self.cursor + 1 < n {
                self.cursor += 1;
            } else if p.trig & DOWN != 0 {
                self.cursor = 0;
            }
        } else if let Some(page) = self.page {
            if p.repeat & L1 != 0 && self.mask & 0x2000 != 0 {
                if self.cursor > 0 {
                    self.cursor = self.cursor.saturating_sub(page);
                } else if p.trig & L1 != 0 {
                    self.cursor = n - 1;
                }
            } else if p.repeat & R1 != 0 && self.mask & 0x1000 != 0 {
                if self.cursor + 1 < n {
                    self.cursor = (self.cursor + page).min(n - 1);
                } else if p.trig & R1 != 0 {
                    self.cursor = 0;
                }
            }
        }
        if self.cursor != before {
            return (Pick::None, true);
        }
        let pick = if p.trig & LEFT != 0 && self.mask & 0x4 != 0
            || p.repeat & LEFT != 0 && self.mask & 0x4_0000 != 0
        {
            Pick::Left
        } else if p.trig & RIGHT != 0 && self.mask & 0x8 != 0
            || p.repeat & RIGHT != 0 && self.mask & 0x8_0000 != 0
        {
            Pick::Right
        } else if p.trig & DECIDE != 0 && self.mask & 0x10 != 0 {
            Pick::Row(self.cursor)
        } else if p.trig & CANCEL != 0 && self.mask & 0x20 != 0 {
            Pick::Cancel
        } else if p.trig & START != 0 && self.mask & 0x40 != 0 {
            Pick::Start
        } else if p.trig & R2 != 0 && self.mask & 0x4000 != 0 {
            Pick::Next
        } else if p.trig & L2 != 0 && self.mask & 0x8000 != 0 {
            Pick::Previous
        } else {
            Pick::None
        };
        (pick, false)
    }
}

/// FUN_1402f6270: the sound for a list result; `valid` is the validator.
fn pick_sound(pick: Pick, moved: bool, valid: bool) -> Option<u32> {
    match pick {
        _ if moved => Some(SOUND_CURSOR),
        Pick::Left | Pick::Right => Some(SOUND_CURSOR),
        Pick::Cancel | Pick::Start => Some(SOUND_CLOSE),
        Pick::Row(_) | Pick::Next | Pick::Previous => {
            Some(if valid { SOUND_DECIDE } else { SOUND_INVALID })
        }
        Pick::None => None,
    }
}

/// A title tab with its orb (FUN_140323c60).
struct Tab {
    id: Id,
    light: LightCur,
}

pub struct Camp {
    pub layout: LayoutTask,
    pub sprites: Sprites,
    pub data: Data,
    pub memory: Memory,
    pub state: State,
    /// 0 opening, 1 interactive, 2 leaving for `pending`.
    pub substate: u8,
    pending: Option<State>,
    pub exit: bool,
    pub closed: bool,
    // Top level.
    top: MenuList,
    frame_sprite: Option<Id>,
    prompt: Option<Id>,
    munny: Id,
    time: Id,
    munny_digits: Vec<Player>,
    time_digits: Vec<Player>,
    status_block: Option<Id>,
    // Shared.
    light: LightCur,
    glove: Option<Id>,
    help: Option<Id>,
    help_offset: [f32; 2],
    tabs: [Option<Tab>; 2],
    /// Text for a tab not created yet (FUN_140324480 before FUN_140323c60).
    pending_tab_text: [Option<Vec<u8>>; 2],
    /// Sprites to close on exit, with their close groups.
    closers: Vec<(Id, usize)>,
    // Status.
    status_list: MenuList,
    /// Status entry index per list row.
    status_rows: Vec<usize>,
    panel: Option<Id>,
    labels: Vec<Id>,
    values: Vec<Id>,
    columns: [MenuList; 2],
    column: usize,
    banner: Option<Id>,
    // Config.
    config_values: [usize; 8],
    config_list: MenuList,
    choices: Vec<Vec<Id>>,
    config_frame: Option<Id>,
}

impl Camp {
    /// FUN_140306040 (and the open sound the task plays right after).
    pub fn open(layout: &Layout, data: Data, memory: Memory) -> (Self, Vec<u32>) {
        let seq = &layout.sequences[SEQUENCE];
        let mut task = LayoutTask::new(layout, SCENE_OPEN);
        task.next = Some(SCENE_IDLE);
        let mut sp = Sprites::default();
        let munny = sp.create(seq, PRIO_PANEL, ART, MUNNY[0], Some(MUNNY[1]), None);
        let time = sp.create(seq, PRIO_PANEL, ART, TIME[0], Some(TIME[1]), None);
        let mut camp = Camp {
            layout: task,
            sprites: sp,
            config_values: config_values(data.config_flags, data.game_mode),
            data,
            memory,
            state: State::Top,
            substate: 0,
            pending: None,
            exit: false,
            closed: false,
            top: MenuList::default(),
            frame_sprite: None,
            prompt: None,
            munny,
            time,
            munny_digits: Vec::new(),
            time_digits: Vec::new(),
            status_block: None,
            light: LightCur::default(),
            glove: None,
            help: None,
            help_offset: [0.; 2],
            tabs: [None, None],
            pending_tab_text: [None, None],
            closers: vec![(munny, MUNNY[2]), (time, TIME[2])],
            status_list: MenuList::default(),
            status_rows: Vec::new(),
            panel: None,
            labels: Vec::new(),
            values: Vec::new(),
            columns: [MenuList::default(), MenuList::default()],
            column: 0,
            banner: None,
            config_list: MenuList::default(),
            choices: Vec::new(),
            config_frame: None,
        };
        camp.build_top(seq);
        camp.munny_digits = digits(seq, &munny_groups(camp.data.panels.munny), Vec::new());
        let t = time_groups(camp.data.panels.time_frames);
        camp.time_digits = digits(seq, &t, Vec::new());
        (camp, vec![SOUND_OPEN])
    }

    fn row_text(&self, bytes: Vec<u8>) -> Text {
        let mut t = Text::new(bytes, TEXT_SIZE);
        t.group_position = false;
        t.keep_rgb = false;
        t
    }

    /// FUN_140306790 + FUN_140306400 + FUN_140367b50: rows, party frame,
    /// status block and settings prompt (also the rebuild on return).
    fn build_top(&mut self, seq: &Sequence) {
        let cursor = self
            .data
            .entries
            .iter()
            .position(|e| e.id == self.memory.entry)
            .unwrap_or(0);
        let pitch = seq.groups[TOP_ROWS.appear].text_position_x as f32;
        let mut rows = Vec::new();
        for (i, e) in self.data.entries.clone().iter().enumerate() {
            let idle = if i == cursor {
                TOP_ROWS.selected
            } else {
                TOP_ROWS.idle
            };
            let t = self.row_text(e.label.clone());
            let r = self
                .sprites
                .create(seq, PRIO_ROW, ART, TOP_ROWS.appear, Some(idle), Some(t));
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            self.closers.push((r, TOP_ROWS.close));
            rows.push(r);
        }
        self.top = MenuList::new(rows, cursor);
        let frame = self
            .sprites
            .create(seq, PRIO_FRAME, ART, FRAME[0], Some(FRAME[1]), None);
        self.closers.push((frame, FRAME[2]));
        self.frame_sprite = Some(frame);
        if self.prompt.is_none_or(|p| !self.sprites.alive(p)) {
            let p = self
                .sprites
                .create(seq, PRIO_FRAME, ART, PROMPT[0], Some(PROMPT[1]), None);
            self.closers.push((p, PROMPT[2]));
            self.prompt = Some(p);
        }
        self.status_block = Some(self.build_status_block(seq));
        self.state = State::Top;
        self.substate = 0;
    }

    fn build_status_block(&mut self, seq: &Sequence) -> Id {
        let sp = &mut self.sprites;
        let status = sp.create(seq, PRIO_PANEL, ART, STATUS[0], Some(STATUS[1]), None);
        self.closers.push((status, STATUS[2]));
        let panels = self.data.panels.clone();
        let n = panels.members.len();
        let pitch = seq.groups[STATUS[0]].text_position_x as f32;
        let sub_size = seq.groups[SUB_ANCHOR].text_scale as f32;
        let row_pitch = seq.groups[SUB_ANCHOR].ui_padding as f32;
        let value_x = seq.groups[SUB_ANCHOR].light_position_x as f32;
        let child = |sp: &mut Sprites, id: Id, parent: Id| {
            sp.attach(id, parent);
            sp.get_mut(id).die_with_parent = true;
        };
        for (i, m) in panels.members.iter().enumerate() {
            let mut name = Text::new(m.name.clone(), TEXT_SIZE);
            name.align = 2;
            name.group_position = false;
            let block = sp.create(seq, PRIO_ROW, ART, BLOCK, None, Some(name));
            child(sp, block, status);
            sp.get_mut(block).offset = [pitch * (n - i) as f32, 0.];
            let sub = sp.create(seq, PRIO_ROW, ART, SUB_ANCHOR, None, None);
            child(sp, sub, block);
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
                child(sp, label, sub);
                sp.get_mut(label).offset = [0., y];
                let value = sp.create(
                    seq,
                    PRIO_ROW,
                    ART,
                    STYLES[k],
                    None,
                    Some(text(encode_number(&values[k]), 1)),
                );
                child(sp, value, sub);
                sp.get_mut(value).offset = [value_x, y];
                if k > 0 {
                    let f = sp.create(seq, PRIO_ROW, ART, GAUGE_FRAME, None, None);
                    child(sp, f, sub);
                    sp.get_mut(f).offset = [0., y];
                    let (now, max) = if k == 1 {
                        (m.hp, m.max_hp)
                    } else {
                        (m.mp, m.max_mp)
                    };
                    let bar = sp.create(seq, PRIO_BAR, ART, GAUGE_BARS[k - 1], None, None);
                    child(sp, bar, sub);
                    let s = sp.get_mut(bar);
                    s.offset = [0., y];
                    s.scale = [now as f32 / max.max(1) as f32, 1.];
                }
            }
        }
        status
    }

    /// One camp frame (the camp task runs before the sprite tasks): the
    /// state machine with this frame's presses and repeats (logical, mode
    /// 7), the driver, then every sprite's update.
    pub fn frame(&mut self, layout: &Layout, trig: u64, repeat: u64, time_frames: u32) -> Vec<u32> {
        let seq = &layout.sequences[SEQUENCE];
        let pad = Pad { trig, repeat };
        let mut sounds = Vec::new();
        if !self.exit {
            // FUN_1403249e0: the clock rebuilds every frame.
            let groups = time_groups(time_frames);
            self.time_digits = digits(seq, &groups, std::mem::take(&mut self.time_digits));
            // No input while the layout opens; any press skips it to idle.
            let gated = !self.layout.dead && self.layout.scene != SCENE_IDLE;
            if gated && trig != 0 {
                self.layout.set_scene(SCENE_IDLE, None);
            }
            if !gated || trig != 0 {
                let moved = match self.state {
                    State::Top => self.top_level(seq, &pad, &mut sounds),
                    State::StatusList => self.status_list(seq, &pad, &mut sounds),
                    State::StatusBrowse => self.status_browse(seq, &pad, &mut sounds),
                    State::Config => self.config(seq, &pad, &mut sounds),
                };
                self.driver(seq, moved);
            }
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
        self.place_lights(seq);
        if self.exit && self.layout.dead {
            self.closed = true;
        }
        sounds
    }

    fn rows_of(&self, state: State) -> (&MenuList, RowGroupsFor) {
        match state {
            State::Top => (&self.top, RowGroupsFor::Top),
            State::StatusList => (&self.status_list, RowGroupsFor::Status),
            State::StatusBrowse => (&self.columns[self.column], RowGroupsFor::None),
            State::Config => (&self.config_list, RowGroupsFor::None),
        }
    }

    fn groups_for(&self, kind: RowGroupsFor, row: usize) -> Option<RowGroups> {
        match kind {
            RowGroupsFor::Top => Some(TOP_ROWS),
            RowGroupsFor::Status => {
                let bit = self.data.status[self.status_rows[row]].bit;
                Some(if bit == 0 || bit > 6 {
                    TOP_ROWS
                } else {
                    FORM_ROWS
                })
            }
            RowGroupsFor::None => None,
        }
    }

    /// FUN_140303cd0: the glove and LightCur follow the active list; on a
    /// cursor move the rows restart idle and the cursor row its selected
    /// group.
    fn driver(&mut self, seq: &Sequence, moved: bool) {
        let state = self.state;
        let (list, kind) = self.rows_of(state);
        let rows = list.rows.clone();
        let cursor = list.cursor;
        let live = rows.iter().any(|&r| self.sprites.alive(r));
        if self.substate == 1 && live {
            if moved {
                for (i, &r) in rows.iter().enumerate() {
                    if let Some(g) = self.groups_for(kind, i) {
                        let group = if i == cursor { g.selected } else { g.idle };
                        self.sprites.set(seq, r, group, None);
                    }
                }
                self.light.retarget();
            }
            let row = rows[cursor];
            match self.glove {
                Some(g) if self.sprites.alive(g) => self.sprites.get_mut(g).parent = Some(row),
                _ => {
                    let g = self.sprites.create(seq, PRIO_LIGHT, ART, GLOVE, None, None);
                    self.sprites.get_mut(g).parent = Some(row);
                    self.glove = Some(g);
                }
            }
            if self.light.id.is_none_or(|l| !self.sprites.alive(l)) {
                let l = self
                    .sprites
                    .create(seq, PRIO_LIGHT, ART, LIGHT[0], Some(LIGHT[1]), None);
                self.light = LightCur::new(l);
            }
        } else if self.substate == 2 {
            if let Some(g) = self.glove.take() {
                self.sprites.kill(g);
            }
            if let Some(l) = self.light.id
                && self.sprites.alive(l)
                && self.sprites.get(l).player.group != LIGHT[2]
            {
                self.sprites.close(seq, l, LIGHT[2]);
            }
        }
    }

    /// The LightCur on the active row (0x24: on its selected choice) and
    /// each tab's orb on its tab, after the sprite update.
    fn place_lights(&mut self, seq: &Sequence) {
        let target_of = |sp: &Sprites, id: Id| {
            let c = sp.context(id);
            let g = &seq.groups[sp.get(id).player.group];
            [
                c[0] + g.light_position_x as f32,
                c[1] + g.text_position_y as f32,
            ]
        };
        if self.light.id.is_some_and(|l| self.sprites.alive(l)) {
            let (list, _) = self.rows_of(self.state);
            if let Some(&row) = list.rows.get(list.cursor) {
                let anchor = if self.state == State::Config {
                    self.sprites.get(row).parent.unwrap_or(row)
                } else {
                    row
                };
                if self.sprites.alive(anchor) {
                    let t = target_of(&self.sprites, anchor);
                    self.light.place(&mut self.sprites, t);
                }
            }
        }
        for tab in self.tabs.iter_mut().flatten() {
            if self.sprites.alive(tab.id) {
                let t = target_of(&self.sprites, tab.id);
                tab.light.place(&mut self.sprites, t);
            }
        }
    }

    // ---- top level -------------------------------------------------------

    /// FUN_1403037d0.
    fn top_level(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let prev = self.top.cursor;
        let (pick, moved) = self.top.input(pad);
        if self.substate == 2 {
            self.wait_top(seq, pick, moved);
            return false;
        }
        let row0 = self
            .top
            .rows
            .first()
            .map(|&r| self.sprites.get(r).player.group);
        if (pick != Pick::None || moved) && row0 == Some(TOP_ROWS.appear) {
            // FUN_1402f81a0: the rows skip their intros.
            for (i, &r) in self.top.rows.clone().iter().enumerate() {
                let g = if i == prev {
                    TOP_ROWS.selected
                } else {
                    TOP_ROWS.idle
                };
                self.sprites.set(seq, r, g, None);
            }
        }
        let row0 = self
            .top
            .rows
            .first()
            .map(|&r| self.sprites.get(r).player.group);
        if self.substate == 0
            && matches!(row0, Some(g) if g == TOP_ROWS.idle || g == TOP_ROWS.selected)
        {
            let help = self.data.entries[self.top.cursor].help.clone();
            self.show_help(seq, help);
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved && self.substate == 1 {
            let help = self.data.entries[self.top.cursor].help.clone();
            self.show_help(seq, help);
        }
        match pick {
            Pick::Cancel | Pick::Start => {
                self.memory.entry = self.data.entries[self.top.cursor].id;
                self.exit = true;
            }
            Pick::Row(i) => {
                let entry = self.data.entries[i].clone();
                self.memory.entry = entry.id;
                let pending = match entry.id {
                    4 => Some(State::StatusList),
                    7 => Some(State::Config),
                    _ => None,
                };
                if let Some(p) = pending {
                    self.leave_top(seq, p, entry.label);
                }
            }
            _ => {}
        }
        moved
    }

    /// The same-frame hand-over (camp-04 §1.1).
    fn leave_top(&mut self, seq: &Sequence, pending: State, label: Vec<u8>) {
        self.pending = Some(pending);
        self.tab_text(0, label);
        for &r in &self.top.rows.clone() {
            self.sprites.close(seq, r, TOP_ROWS.close);
        }
        self.hide_help(seq);
        if let Some(f) = self.frame_sprite {
            self.sprites.close(seq, f, FRAME[2]);
        }
        if let Some(s) = self.status_block {
            self.sprites.close(seq, s, STATUS[2]);
        }
        if let Some(p) = self.prompt {
            self.sprites.close(seq, p, PROMPT[2]);
        }
        self.substate = 2;
    }

    /// Substate 2: commit once rows and the status block are gone; any
    /// press kills them at once.
    fn wait_top(&mut self, seq: &Sequence, pick: Pick, moved: bool) {
        let rows_dead = self.top.rows.iter().all(|&r| !self.sprites.alive(r));
        let block_dead = self.status_block.is_none_or(|s| !self.sprites.alive(s));
        if rows_dead && block_dead {
            match self.pending.take() {
                Some(State::StatusList) => self.enter_status(seq),
                Some(State::Config) => self.enter_config(seq),
                _ => {}
            }
        } else if pick != Pick::None || moved {
            for &r in &self.top.rows {
                self.sprites.kill(r);
            }
            if let Some(s) = self.status_block {
                self.sprites.kill(s);
            }
        }
    }

    // ---- shared pieces ---------------------------------------------------

    /// FUN_140324480: a tab's text (created by the screen's entry).
    fn tab_text(&mut self, i: usize, bytes: Vec<u8>) {
        if let Some(tab) = &self.tabs[i]
            && let Some(t) = self.sprites.get_mut(tab.id).text.as_mut()
        {
            t.bytes = bytes;
            return;
        }
        self.pending_tab_text[i] = Some(bytes);
    }

    /// FUN_140323c60: tab `i` (G75 → 76) at (120·i, 0) with its orb.
    fn create_tab(&mut self, seq: &Sequence, i: usize) {
        if self.tabs[i]
            .as_ref()
            .is_some_and(|t| self.sprites.alive(t.id))
        {
            return;
        }
        let bytes = self.pending_tab_text[i].take().unwrap_or_default();
        let mut t = Text::new(bytes, TEXT_SIZE);
        t.group_position = false;
        let id = self
            .sprites
            .create(seq, PRIO_ROW, ART, TAB[0], Some(TAB[1]), Some(t));
        self.sprites.get_mut(id).offset =
            [seq.groups[TAB[0]].text_position_x as f32 * i as f32, 0.];
        self.closers.push((id, TAB[2]));
        let l = self
            .sprites
            .create(seq, PRIO_BAR, ART, LIGHT[0], Some(LIGHT[1]), None);
        self.tabs[i] = Some(Tab {
            id,
            light: LightCur::new(l),
        });
    }

    fn close_tab(&mut self, seq: &Sequence, i: usize) {
        if let Some(tab) = &self.tabs[i]
            && self.sprites.alive(tab.id)
        {
            let id = tab.id;
            self.sprites.close(seq, id, TAB[2]);
        }
    }

    fn kill_tab(&mut self, i: usize) {
        if let Some(tab) = self.tabs[i].take() {
            self.sprites.kill(tab.id);
            if let Some(l) = tab.light.id {
                self.sprites.kill(l);
            }
        }
    }

    fn tab_alive(&self, i: usize) -> bool {
        self.tabs[i]
            .as_ref()
            .is_some_and(|t| self.sprites.alive(t.id))
    }

    /// FUN_1402fa1d0: a live help text only swaps its text (no re-fade
    /// while in G116; a fading one restarts G116); otherwise it is created
    /// on the layout element with that element's (LightPositionX,
    /// TextPositionY).
    fn show_help(&mut self, seq: &Sequence, bytes: Vec<u8>) {
        if let Some(h) = self.help
            && self.sprites.alive(h)
        {
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

    /// FUN_1402fa510: the help fades out (G117) and goes.
    fn hide_help(&mut self, seq: &Sequence) {
        if let Some(h) = self.help
            && self.sprites.alive(h)
        {
            self.sprites.close(seq, h, HELP_OUT);
        }
    }

    /// The help's parent is the layout element of the current scene: its
    /// context, with its alpha as the colour multiplier (0x200|0x400).
    fn place_help(&mut self, layout: &Layout) {
        let Some(h) = self.help else { return };
        if self.layout.dead || !self.sprites.alive(h) {
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

    fn child(&mut self, id: Id, parent: Id) {
        self.sprites.attach(id, parent);
        self.sprites.get_mut(id).die_with_parent = true;
    }

    /// Back to the top level (camp-04 §1.3): the rebuild plays no sound.
    fn return_top(&mut self, seq: &Sequence) {
        self.kill_tab(0);
        self.kill_tab(1);
        self.build_top(seq);
    }

    // ---- Status 0x17 -----------------------------------------------------

    /// FUN_140353020: tab 0, the character list and the stat panel.
    fn enter_status(&mut self, seq: &Sequence) {
        self.state = State::StatusList;
        self.substate = 0;
        self.create_tab(seq, 0);
        let pitch = seq.groups[TOP_ROWS.appear].text_position_x as f32;
        let cursor = self
            .data
            .status
            .iter()
            .position(|e| e.bit == self.memory.status_bit)
            .unwrap_or(0);
        let mut rows = Vec::new();
        for (i, e) in self.data.status.clone().iter().enumerate() {
            let g = if e.bit == 0 || e.bit > 6 {
                TOP_ROWS
            } else {
                FORM_ROWS
            };
            let queued = if i == cursor { g.selected } else { g.idle };
            let t = self.row_text(e.label.clone());
            let r = self
                .sprites
                .create(seq, PRIO_STATUS_ROW, ART, g.appear, Some(queued), Some(t));
            self.sprites.get_mut(r).offset = [0., pitch * i as f32];
            self.closers.push((r, g.close));
            rows.push(r);
        }
        self.status_rows = (0..self.data.status.len()).collect();
        self.status_list = MenuList::new(rows, cursor);
        let panel = self.sprites.create(
            seq,
            PRIO_STATUS_PANEL,
            ART,
            STATUS_PANEL[0],
            Some(STATUS_PANEL[1]),
            None,
        );
        self.closers.push((panel, STATUS_PANEL[2]));
        self.panel = Some(panel);
        self.labels.clear();
        self.values.clear();
        let g = &seq.groups[STATUS_LABEL];
        let value_offset = [g.light_position_x as f32, g.text_position_y as f32];
        for _ in 0..20 {
            let mut t = self.row_text(Vec::new());
            t.size = TEXT_SIZE;
            let l = self
                .sprites
                .create(seq, PRIO_STATUS_LABEL, ART, BLANK, None, Some(t));
            self.child(l, panel);
            let mut vt = Text::new(Vec::new(), TEXT_SIZE);
            vt.align = 1;
            vt.group_position = false;
            vt.keep_rgb = false;
            let v = self
                .sprites
                .create(seq, PRIO_STATUS_ROW, ART, VALUE, None, Some(vt));
            self.child(v, l);
            self.sprites.get_mut(v).offset = value_offset;
            self.labels.push(l);
            self.values.push(v);
        }
        self.layout_status_panel(seq);
        let help = self.data.status[cursor].help.clone();
        self.show_help(seq, help);
    }

    /// FUN_1403540b0: labels and values of the cursor character, compacted
    /// per column at (170·col, 24·rank).
    fn layout_status_panel(&mut self, seq: &Sequence) {
        let entry = self.data.status[self.status_rows[self.status_list.cursor]].clone();
        let mut rank = [0usize; 2];
        for k in 0..20 {
            let (l, v) = (self.labels[k], self.values[k]);
            match entry.stats.get(k).cloned().flatten() {
                Some(stat) => {
                    let col = k / 10;
                    let group = STAT_STYLES[stat.style.min(4)].0;
                    let gg = &seq.groups[group];
                    let offset = [
                        gg.text_position_x as f32 * col as f32,
                        gg.ui_padding as f32 * rank[col] as f32,
                    ];
                    rank[col] += 1;
                    self.sprites.set(seq, l, group, None);
                    let s = self.sprites.get_mut(l);
                    s.offset = offset;
                    if let Some(t) = s.text.as_mut() {
                        t.bytes = stat.label;
                    }
                    if let Some(t) = self.sprites.get_mut(v).text.as_mut() {
                        t.bytes = stat.value;
                    }
                }
                None => {
                    self.sprites.set(seq, l, BLANK, None);
                    if let Some(t) = self.sprites.get_mut(l).text.as_mut() {
                        t.bytes.clear();
                    }
                    if let Some(t) = self.sprites.get_mut(v).text.as_mut() {
                        t.bytes.clear();
                    }
                }
            }
        }
    }

    /// FUN_140351db0.
    fn status_list(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.status_list.input(pad);
        if self.substate == 2 {
            let panel_dead = self.panel.is_none_or(|p| !self.sprites.alive(p));
            let rows_dead = self
                .status_list
                .rows
                .iter()
                .all(|&r| !self.sprites.alive(r));
            let tab_ok = self.pending != Some(State::Top) || !self.tab_alive(0);
            if panel_dead && rows_dead && tab_ok {
                match self.pending.take() {
                    Some(State::StatusBrowse) => self.enter_browse(seq),
                    _ => self.return_top(seq),
                }
            } else if pick != Pick::None || moved {
                if let Some(p) = self.panel {
                    self.sprites.kill(p);
                }
                for &r in &self.status_list.rows {
                    self.sprites.kill(r);
                }
                if self.pending == Some(State::Top) {
                    self.kill_tab(0);
                }
            }
            return false;
        }
        let row0 = self
            .status_list
            .rows
            .first()
            .map(|&r| self.sprites.get(r).player.group);
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == STATUS_PANEL[1]);
        let row0_idle = row0.is_some_and(|g| matches!(g, 134 | 132 | 130 | 128));
        if (pick != Pick::None || moved) && !(row0_idle && panel_idle) {
            self.skip_status_intros(seq);
        }
        let row0 = self
            .status_list
            .rows
            .first()
            .map(|&r| self.sprites.get(r).player.group);
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == STATUS_PANEL[1]);
        if self.substate == 0
            && row0.is_some_and(|g| matches!(g, 134 | 132 | 130 | 128))
            && panel_idle
        {
            self.substate = 1;
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        let cursor_entry = self.status_rows[self.status_list.cursor];
        if moved && self.substate == 1 {
            self.layout_status_panel(seq);
            let help = self.data.status[cursor_entry].help.clone();
            self.show_help(seq, help);
        }
        match pick {
            Pick::Start => {
                self.memory.status_bit = self.data.status[cursor_entry].bit;
                self.exit = true;
            }
            Pick::Cancel => {
                self.hide_help(seq);
                self.close_tab(seq, 0);
                self.close_status_list(seq);
                self.memory.status_bit = self.data.status[cursor_entry].bit;
                self.pending = Some(State::Top);
                self.substate = 2;
            }
            Pick::Row(_) => {
                self.hide_help(seq);
                self.close_status_list(seq);
                let label = self.data.status[cursor_entry].label.clone();
                self.pending_tab_text[1] = Some(label);
                self.memory.status_bit = self.data.status[cursor_entry].bit;
                self.pending = Some(State::StatusBrowse);
                self.substate = 2;
            }
            _ => {}
        }
        moved
    }

    fn skip_status_intros(&mut self, seq: &Sequence) {
        let cursor = self.status_list.cursor;
        for (i, &r) in self.status_list.rows.clone().iter().enumerate() {
            if let Some(g) = self.groups_for(RowGroupsFor::Status, i) {
                let group = if i == cursor { g.selected } else { g.idle };
                if self.sprites.get(r).player.group == g.appear {
                    self.sprites.set(seq, r, group, None);
                }
            }
        }
        if let Some(p) = self.panel {
            self.sprites.set(seq, p, STATUS_PANEL[1], None);
        }
        for i in 0..2 {
            if let Some(tab) = &self.tabs[i]
                && self.sprites.alive(tab.id)
            {
                let id = tab.id;
                self.sprites.set(seq, id, TAB[1], None);
            }
        }
    }

    fn close_status_list(&mut self, seq: &Sequence) {
        if let Some(p) = self.panel {
            self.sprites.close(seq, p, STATUS_PANEL[2]);
        }
        for (i, &r) in self.status_list.rows.clone().iter().enumerate() {
            if let Some(g) = self.groups_for(RowGroupsFor::Status, i) {
                self.sprites.close(seq, r, g.close);
            }
        }
    }

    // ---- Status 0x18 -----------------------------------------------------

    /// FUN_140352be0 + FUN_140353800: tab 1, the stat browser panel, two
    /// columns of rows with their values, and the "Switch" banner.
    fn enter_browse(&mut self, seq: &Sequence) {
        self.state = State::StatusBrowse;
        self.substate = 0;
        self.create_tab(seq, 1);
        let entry = self.data.status[self.status_rows[self.status_list.cursor]].clone();
        let panel = self.sprites.create(
            seq,
            PRIO_PANEL,
            ART,
            BROWSE_PANEL[0],
            Some(BROWSE_PANEL[1]),
            None,
        );
        self.closers.push((panel, BROWSE_PANEL[2]));
        self.panel = Some(panel);
        let g = &seq.groups[BROWSE_ROW];
        let value_offset = [g.light_position_x as f32, g.text_position_y as f32];
        self.values.clear();
        for c in 0..2 {
            let mut rows = Vec::new();
            for _ in 0..10 {
                let t = self.row_text(Vec::new());
                let r = self
                    .sprites
                    .create(seq, PRIO_ROW, ART, BLANK, None, Some(t));
                self.child(r, panel);
                rows.push(r);
            }
            self.columns[c] = MenuList::new(rows, 0);
            self.columns[c].total = 0;
            self.columns[c].mask = (0x73 & !0x10) | 0xc_c000 | 0x3000;
            self.columns[c].page = Some(10);
        }
        for col in 0..2 {
            let mut j = 0;
            for k in col * 10..col * 10 + 10 {
                let Some(stat) = entry.stats.get(k).cloned().flatten() else {
                    continue;
                };
                let row = self.columns[col].rows[j];
                let group = STAT_STYLES[stat.style.min(4)].1;
                let gg = &seq.groups[group];
                let offset = [
                    gg.text_position_x as f32 * col as f32,
                    gg.ui_padding as f32 * j as f32,
                ];
                self.sprites.set(seq, row, group, None);
                let s = self.sprites.get_mut(row);
                s.offset = offset;
                if let Some(t) = s.text.as_mut() {
                    t.bytes = stat.label.clone();
                }
                let mut vt = Text::new(stat.value.clone(), TEXT_SIZE);
                vt.align = 1;
                vt.group_position = false;
                vt.keep_rgb = false;
                let v = self
                    .sprites
                    .create(seq, PRIO_BAR, ART, VALUE, None, Some(vt));
                self.child(v, row);
                self.sprites.get_mut(v).offset = value_offset;
                self.values.push(v);
                j += 1;
            }
            self.columns[col].total = j;
        }
        self.column = 0;
        if self.banner.is_none_or(|b| !self.sprites.alive(b)) {
            let mut t = Text::new(self.data.banner.clone(), TEXT_SIZE);
            t.align = 1;
            t.group_position = false;
            let b = self
                .sprites
                .create(seq, PRIO_ROW, ART, BANNER[0], Some(BANNER[1]), Some(t));
            self.closers.push((b, BANNER[2]));
            self.banner = Some(b);
        }
        self.browse_help(seq);
    }

    /// FUN_140353c40 for 0x18: the help line of the stat under the cursor.
    fn browse_help(&mut self, seq: &Sequence) {
        let entry = &self.data.status[self.status_rows[self.status_list.cursor]];
        let col = self.column;
        let cursor = self.columns[col].cursor;
        let help = entry.stats[col * 10..col * 10 + 10]
            .iter()
            .flatten()
            .nth(cursor)
            .map(|s| s.help.clone());
        if let Some(h) = help {
            self.show_help(seq, h);
        }
    }

    /// FUN_140351a00.
    fn status_browse(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let col = self.column;
        let (pick, mut moved) = self.columns[col].input(pad);
        if self.substate == 2 {
            let panel_dead = self.panel.is_none_or(|p| !self.sprites.alive(p));
            let tab_dead = !self.tab_alive(1);
            let banner_ok = self.pending != Some(State::StatusList)
                || self.banner.is_none_or(|b| !self.sprites.alive(b));
            if panel_dead && tab_dead && banner_ok {
                self.kill_tab(1);
                match self.pending.take() {
                    Some(State::StatusBrowse) => self.enter_browse(seq),
                    _ => self.enter_status_again(seq),
                }
            } else if pick != Pick::None || moved {
                if let Some(p) = self.panel {
                    self.sprites.kill(p);
                }
                self.kill_tab(1);
                if self.pending == Some(State::StatusList)
                    && let Some(b) = self.banner
                {
                    self.sprites.kill(b);
                }
            }
            return false;
        }
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == BROWSE_PANEL[1]);
        if (pick != Pick::None || moved) && !panel_idle {
            if let Some(p) = self.panel {
                self.sprites.set(seq, p, BROWSE_PANEL[1], None);
            }
            if let Some(b) = self.banner {
                self.sprites.set(seq, b, BANNER[1], None);
            }
            for i in 0..2 {
                if let Some(tab) = &self.tabs[i] {
                    let id = tab.id;
                    if self.sprites.alive(id) {
                        self.sprites.set(seq, id, TAB[1], None);
                    }
                }
            }
        }
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == BROWSE_PANEL[1]);
        if self.substate == 0 && panel_idle {
            self.substate = 1;
        }
        // FUN_140352b10: L2/R2 need more than one character.
        let valid = self.data.status.len() > 1;
        if let Some(s) = pick_sound(pick, moved, valid) {
            sounds.push(s);
        }
        match pick {
            Pick::Left | Pick::Right => {
                let other = 1 - col;
                if self.columns[other].total > 0 {
                    let index = self.columns[col].cursor;
                    self.column = other;
                    self.columns[other].cursor = index.min(self.columns[other].total - 1);
                    moved = true;
                }
            }
            Pick::Next | Pick::Previous if valid => {
                let n = self.data.status.len();
                let c = self.status_list.cursor;
                let next = if pick == Pick::Next {
                    (c + 1) % n
                } else {
                    (c + n - 1) % n
                };
                self.status_list.cursor = next;
                let e = &self.data.status[self.status_rows[next]];
                self.memory.status_bit = e.bit;
                let label = e.label.clone();
                self.pending_tab_text[1] = Some(label);
                self.hide_help(seq);
                self.close_tab(seq, 1);
                self.close_browse(seq);
                self.pending = Some(State::StatusBrowse);
                self.substate = 2;
            }
            Pick::Start => self.exit = true,
            Pick::Cancel => {
                self.hide_help(seq);
                self.close_tab(seq, 1);
                self.close_browse(seq);
                if let Some(b) = self.banner {
                    self.sprites.close(seq, b, BANNER[2]);
                }
                self.pending = Some(State::StatusList);
                self.substate = 2;
            }
            _ => {}
        }
        if moved && self.substate == 1 {
            self.browse_help(seq);
        }
        moved
    }

    fn close_browse(&mut self, seq: &Sequence) {
        if let Some(p) = self.panel {
            self.sprites.close(seq, p, BROWSE_PANEL[2]);
        }
    }

    /// FUN_140353020(0): back to the character list, objects kept.
    fn enter_status_again(&mut self, seq: &Sequence) {
        self.enter_status(seq);
    }

    // ---- Config 0x24 -----------------------------------------------------

    /// FUN_140365580 + FUN_140364a10: tab 0, the panel, 8 labels, their
    /// choices and invisible anchor rows.
    fn enter_config(&mut self, seq: &Sequence) {
        self.state = State::Config;
        self.substate = 0;
        self.create_tab(seq, 0);
        self.config_values = config_values(self.data.config_flags, self.data.game_mode);
        let panel = self.sprites.create(
            seq,
            PRIO_CONFIG_PANEL,
            ART,
            CONFIG_PANEL[0],
            Some(CONFIG_PANEL[1]),
            None,
        );
        self.closers.push((panel, CONFIG_PANEL[2]));
        self.panel = Some(panel);
        let pitch = seq.groups[CONFIG_LABEL].ui_padding as f32;
        let label_size = seq.groups[CONFIG_LABEL].text_scale as f32;
        self.choices.clear();
        let mut rows = Vec::new();
        for i in 0..self.data.config.len().min(8) {
            let row = self.data.config[i].clone();
            let mut t = Text::new(row.label.clone(), label_size);
            t.align = 2;
            t.group_position = false;
            let l = self
                .sprites
                .create(seq, PRIO_PANEL, ART, CONFIG_LABEL, None, Some(t));
            self.child(l, panel);
            self.sprites.get_mut(l).offset = [0., pitch * i as f32];
            let mut cs = Vec::new();
            for _ in 0..4 {
                let mut t = Text::new(Vec::new(), TEXT_SIZE);
                t.align = 2;
                t.group_position = false;
                let c = self
                    .sprites
                    .create(seq, PRIO_PANEL, ART, BLANK, None, Some(t));
                self.child(c, panel);
                cs.push(c);
            }
            self.choices.push(cs);
            let anchor = self.sprites.create(seq, PRIO_ROW, ART, BLANK, None, None);
            self.sprites.get_mut(anchor).die_with_parent = true;
            rows.push(anchor);
        }
        self.config_list = MenuList::new(rows, self.memory.config_row);
        self.config_list.page = Some(8);
        self.config_layout(seq);
        self.config_mask();
    }

    fn config_mask(&mut self) {
        let base = (0x73 & !0x10) | 0x3000;
        self.config_list.mask = if self.config_list.cursor == 7 {
            base
        } else {
            base | 0xc_0000
        };
    }

    /// FUN_140365a20: each row's choices (selected / unselected looks), the
    /// anchor rows on the selected choice, and the cursor frame.
    fn config_layout(&mut self, seq: &Sequence) {
        let mode = self.data.game_mode.min(3) as usize;
        for i in 0..self.choices.len() {
            let row = &self.data.config[i];
            let n = if i == 7 {
                1
            } else {
                row.choices.len().clamp(1, 3)
            };
            let pair = CHOICES[n - 1];
            for c in 0..4 {
                let id = self.choices[i][c];
                if c >= n {
                    self.sprites.set(seq, id, BLANK, None);
                    if let Some(t) = self.sprites.get_mut(id).text.as_mut() {
                        t.bytes.clear();
                    }
                    continue;
                }
                let selected = i == 7 || self.config_values[i] == c;
                let group = if selected { pair[0] } else { pair[1] };
                if self.sprites.get(id).player.group != group {
                    self.sprites.set(seq, id, group, None);
                }
                let g = &seq.groups[group];
                let label = if i == 7 {
                    row.choices.get(mode).cloned().unwrap_or_default()
                } else {
                    row.choices[c].clone()
                };
                let s = self.sprites.get_mut(id);
                s.offset = [g.ui_padding as f32 * c as f32, 26. * i as f32];
                if let Some(t) = s.text.as_mut() {
                    t.bytes = label;
                    t.offset = [g.text_position_x as f32, 0.];
                }
            }
            let chosen = if i == 7 { 0 } else { self.config_values[i] };
            let anchor = self.config_list.rows[i];
            self.sprites.get_mut(anchor).parent = Some(self.choices[i][chosen]);
        }
        self.update_config_frame(seq);
    }

    /// FUN_1403657e0: the cursor frame on the selected choice (substate 1).
    fn update_config_frame(&mut self, seq: &Sequence) {
        if self.substate != 1 {
            if let Some(f) = self.config_frame.take() {
                self.sprites.kill(f);
            }
            return;
        }
        let row = self.config_list.cursor;
        let n = if row == 7 {
            1
        } else {
            self.data.config[row].choices.len().clamp(1, 3)
        };
        let chosen = if row == 7 { 0 } else { self.config_values[row] };
        let parent = self.choices[row][chosen];
        let group = CHOICES[n - 1][2];
        match self.config_frame {
            Some(f) if self.sprites.alive(f) => {
                if self.sprites.get(f).parent != Some(parent)
                    || self.sprites.get(f).player.group != group
                {
                    self.sprites.set(seq, f, group, None);
                    self.sprites.get_mut(f).parent = Some(parent);
                }
            }
            _ => {
                let f = self.sprites.create(seq, PRIO_BAR, ART, group, None, None);
                self.sprites.get_mut(f).parent = Some(parent);
                self.config_frame = Some(f);
            }
        }
    }

    fn config_help(&mut self, seq: &Sequence) {
        let row = self.config_list.cursor;
        let v = if row == 7 {
            self.data.game_mode.min(3) as usize
        } else {
            self.config_values[row]
        };
        if let Some(h) = self
            .data
            .config
            .get(row)
            .and_then(|r| r.help.get(v))
            .cloned()
        {
            self.show_help(seq, h);
        }
    }

    /// FUN_140364d70.
    fn config(&mut self, seq: &Sequence, pad: &Pad, sounds: &mut Vec<u32>) -> bool {
        let (pick, moved) = self.config_list.input(pad);
        if self.substate == 2 {
            let panel_dead = self.panel.is_none_or(|p| !self.sprites.alive(p));
            if panel_dead && !self.tab_alive(0) {
                self.return_top(seq);
            } else if pick != Pick::None || moved {
                if let Some(p) = self.panel {
                    self.sprites.kill(p);
                }
                self.kill_tab(0);
            }
            return false;
        }
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == CONFIG_PANEL[1]);
        if (pick != Pick::None || moved) && !panel_idle {
            if let Some(p) = self.panel {
                self.sprites.set(seq, p, CONFIG_PANEL[1], None);
            }
            if let Some(tab) = &self.tabs[0] {
                let id = tab.id;
                if self.sprites.alive(id) {
                    self.sprites.set(seq, id, TAB[1], None);
                }
            }
        }
        let panel_idle = self
            .panel
            .is_some_and(|p| self.sprites.get(p).player.group == CONFIG_PANEL[1]);
        if self.substate == 0 && panel_idle {
            self.substate = 1;
            self.config_help(seq);
            self.update_config_frame(seq);
        }
        if let Some(s) = pick_sound(pick, moved, true) {
            sounds.push(s);
        }
        if moved {
            self.config_mask();
            self.config_layout(seq);
            self.config_help(seq);
        }
        match pick {
            Pick::Left | Pick::Right => {
                let row = self.config_list.cursor;
                let n = self.data.config[row].choices.len().clamp(1, 3);
                let v = &mut self.config_values[row];
                *v = if pick == Pick::Left {
                    (*v + n - 1) % n
                } else {
                    (*v + 1) % n
                };
                self.config_layout(seq);
                self.config_help(seq);
                self.light.retarget();
            }
            Pick::Start => {
                self.save_config();
                self.exit = true;
            }
            Pick::Cancel => {
                self.save_config();
                self.hide_help(seq);
                self.close_tab(seq, 0);
                if let Some(p) = self.panel {
                    self.sprites.close(seq, p, CONFIG_PANEL[2]);
                }
                if let Some(f) = self.config_frame.take() {
                    self.sprites.kill(f);
                }
                self.substate = 2;
                self.pending = Some(State::Top);
            }
            _ => {}
        }
        moved
    }

    /// FUN_140365460: the values back into the save flags; the cursor row
    /// is remembered.
    fn save_config(&mut self) {
        self.data.config_flags = config_flags(self.data.config_flags, &self.config_values);
        self.memory.config_row = self.config_list.cursor;
    }

    // ---- exit and drawing ------------------------------------------------

    /// FUN_140303cd0 with the exit flag (and the sub-screens' exit hooks):
    /// everything plays its close group; glove and help go at once.
    fn close(&mut self, seq: &Sequence) {
        if !self.layout.dead && self.layout.scene != SCENE_CLOSE {
            self.layout.set_scene(SCENE_CLOSE, None);
        }
        for (id, group) in self.closers.clone() {
            let s = self.sprites.get(id);
            if !s.dead && s.player.group != group && !s.die_when_done {
                self.sprites.close(seq, id, group);
            }
        }
        if let Some(l) = self.light.id
            && self.sprites.alive(l)
            && self.sprites.get(l).player.group != LIGHT[2]
        {
            self.sprites.close(seq, l, LIGHT[2]);
        }
        if let Some(g) = self.glove.take() {
            self.sprites.kill(g);
        }
        if let Some(h) = self.help.take() {
            self.sprites.kill(h);
        }
        if let Some(f) = self.config_frame.take() {
            self.sprites.kill(f);
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
            let c = self.sprites.context(label);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowGroupsFor {
    Top,
    Status,
    None,
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
                BLANK
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
    g.resize(9, BLANK);
    g
}

/// Players for digit groups, keeping a slot's player when its group is
/// unchanged (a group is only set when it changes).
fn digits(seq: &Sequence, groups: &[usize], mut old: Vec<Player>) -> Vec<Player> {
    old.resize_with(groups.len(), || Player::new(seq, BLANK));
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
        assert_eq!(
            encode_number("12/30%"),
            vec![0x22, 0x23, 0x4b, 0x24, 0x21, 0x4a]
        );
    }

    #[test]
    fn config_values_round_trip_the_save_bits() {
        // Default new-game config 0x40b: vibration on (bit 0), map off?
        let v = config_values(0x40b, 1);
        assert_eq!(v, [0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(config_flags(0x40b, &v), 0x40b);
        let mut w = v;
        w[3] = 1;
        w[5] = 1;
        w[6] = 1;
        let f = config_flags(0x40b, &w);
        assert_eq!(f & 0x600, 0x200);
        assert_eq!(f & 1, 0);
        assert_eq!(f & 0x40, 0x40);
        assert_eq!(config_values(f, 1), w);
    }

    #[test]
    fn lists_page_and_switch_with_the_exe_codes() {
        let rows: Vec<Id> = (0..8).map(Id).collect();
        let mut l = MenuList::new(rows, 0);
        l.mask = (0x73 & !0x10) | 0x3000 | 0xc_0000;
        l.page = Some(8);
        let p = |trig: u64, repeat: u64| Pad { trig, repeat };
        assert_eq!(l.input(&p(bits::R1, bits::R1)), (Pick::None, true));
        assert_eq!(l.cursor, 7);
        assert_eq!(l.input(&p(bits::R1, bits::R1)), (Pick::None, true));
        assert_eq!(l.cursor, 0);
        assert_eq!(l.input(&p(0, bits::LEFT)), (Pick::Left, false));
        assert_eq!(l.input(&p(bits::DECIDE, bits::DECIDE)), (Pick::None, false));
        assert_eq!(l.input(&p(bits::CANCEL, 0)), (Pick::Cancel, false));
        assert_eq!(pick_sound(Pick::Next, false, false), Some(SOUND_INVALID));
    }
}
