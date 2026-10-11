//! The test room's debug menu (F3, or both stick buttons on a controller):
//! give items, restore and level Sora, change Drive form, control the enemy
//! and warp between areas. A development aid, not part of the original
//! game: a plain box and list drawn on the HUD canvas. The field is paused
//! while it is open.
use super::*;
use player_sim::forms;

/// Effects the menu leaves switched on.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cheats {
    /// Sora takes no damage.
    pub invincible: bool,
    /// The enemy stands still.
    pub enemy_frozen: bool,
}

/// One white texel, for the box and the highlight bar.
pub const WHITE: [u8; 4] = [255; 4];

/// Rows shown at once, and the layout in HUD units (512 × 416).
const ROWS: usize = 15;
const ROW_HEIGHT: f32 = 19.;
const TEXT_SIZE: f32 = 14.;
const BOX: [f32; 4] = [96., 36., 416., 400.];
const MARGIN: f32 = 14.;
/// Held arrows repeat after 18 frames, then every 4.
const REPEAT_DELAY: u32 = 18;
const REPEAT_EVERY: u32 = 4;
/// Spell items hold a level; other stock counts stop at 99.
const MAX_SPELL: u8 = 3;
const MAX_STOCK: u8 = 99;
/// Frames a result line stays in the footer.
const NOTICE_FRAMES: u32 = 120;

const FORMS: [(u8, &str); 6] = [
    (forms::VALOR, "Valor Form"),
    (forms::WISDOM, "Wisdom Form"),
    (forms::LIMIT, "Limit Form"),
    (forms::MASTER, "Master Form"),
    (forms::FINAL, "Final Form"),
    (forms::ANTI, "Antiform"),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Main,
    Items,
    Forms,
    /// The worlds with installed rooms.
    Warp,
    /// One world's rooms.
    Rooms(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Page(Page),
    Restore,
    LevelUp,
    Invincible,
    Defeat,
    Respawn,
    Freeze,
    FillItems,
    Item(u16),
    Form(u8),
    Warp(u8, u8),
    /// The current room becomes its world's launcher start.
    SaveStart,
    Close,
}

struct Row {
    label: Vec<u8>,
    value: Vec<u8>,
    action: Action,
    enabled: bool,
}

/// What the HUD draws while the menu is open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    heading: Vec<u8>,
    /// Label, value (drawn on the right) and whether the row can be used.
    rows: Vec<(Vec<u8>, Vec<u8>, bool)>,
    selected: Option<usize>,
    footer: Vec<u8>,
}

#[derive(Resource, Default)]
pub struct DebugMenu {
    pub open: bool,
    page: Page,
    cursor: usize,
    top: usize,
    /// The main page's cursor, restored when a sub-page closes.
    main_cursor: usize,
    /// The arrow being held and for how many frames.
    held: Option<(Dir, u32)>,
    /// The current world's saved launcher start (read on opening Warp).
    saved: Option<(u8, u8)>,
    notice: Option<(String, u32)>,
    pub view: View,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Default)]
struct Input {
    toggle: bool,
    confirm: bool,
    back: bool,
    /// This frame's arrow step (a press or a repeat).
    step: Option<Dir>,
}

fn text(s: &str) -> Vec<u8> {
    camp::encode_text(s)
}

fn read_input(
    keys: &ButtonInput<KeyCode>,
    pads: &Query<(&Gamepad, Option<&Name>)>,
    held: &mut Option<(Dir, u32)>,
) -> Input {
    let pad_pressed = |b: GamepadButton| pads.iter().any(|(p, _)| p.pressed(b));
    let pad_just = |b: GamepadButton| pads.iter().any(|(p, _)| p.just_pressed(b));
    let sticks = pad_pressed(GamepadButton::LeftThumb)
        && pad_pressed(GamepadButton::RightThumb)
        && (pad_just(GamepadButton::LeftThumb) || pad_just(GamepadButton::RightThumb));
    let mut input = Input {
        toggle: keys.just_pressed(KeyCode::F3) || sticks,
        confirm: keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::NumpadEnter)
            || keys.just_pressed(KeyCode::Space)
            || pad_just(GamepadButton::South),
        back: keys.just_pressed(KeyCode::Escape)
            || keys.just_pressed(KeyCode::Backspace)
            || pad_just(GamepadButton::East),
        step: None,
    };
    let down = |d: Dir| {
        let (key, button) = match d {
            Dir::Up => (KeyCode::ArrowUp, GamepadButton::DPadUp),
            Dir::Down => (KeyCode::ArrowDown, GamepadButton::DPadDown),
            Dir::Left => (KeyCode::ArrowLeft, GamepadButton::DPadLeft),
            Dir::Right => (KeyCode::ArrowRight, GamepadButton::DPadRight),
        };
        keys.pressed(key) || pad_pressed(button)
    };
    let current = [Dir::Up, Dir::Down, Dir::Left, Dir::Right]
        .into_iter()
        .find(|&d| down(d));
    *held = match (current, *held) {
        (Some(d), Some((h, frames))) if d == h => {
            let frames = frames + 1;
            if frames >= REPEAT_DELAY && (frames - REPEAT_DELAY).is_multiple_of(REPEAT_EVERY) {
                input.step = Some(d);
            }
            Some((d, frames))
        }
        (Some(d), _) => {
            input.step = Some(d);
            Some((d, 0))
        }
        (None, _) => None,
    };
    input
}

/// The items the Items page lists: what the camp's All tab shows, plus the
/// spell items, with their names (empty names are skipped).
fn item_list(camp: Option<&CampRes>, game: &Game) -> Vec<(u16, Vec<u8>)> {
    if let Some(c) = camp {
        return c
            .source
            .items
            .table
            .iter()
            .filter(|r| matches!(r.kind, 0..=17 | 20) || ELEMENT_ITEMS.contains(&r.id))
            .filter(|r| r.name.iter().any(|&b| b >= 0x20))
            .map(|r| (r.id, r.name.clone()))
            .collect();
    }
    game.items
        .as_ref()
        .map(|it| {
            it.stock
                .keys()
                .map(|&id| (id, text(&format!("Item {id}"))))
                .collect()
        })
        .unwrap_or_default()
}

fn stock_of(game: &Game, camp: Option<&CampRes>, id: u16) -> u8 {
    game.items
        .as_ref()
        .map(|it| &it.stock)
        .or(camp.map(|c| &c.stock))
        .and_then(|s| s.get(&id))
        .copied()
        .unwrap_or(0)
}

fn cap_of(id: u16) -> u8 {
    if ELEMENT_ITEMS.contains(&id) {
        MAX_SPELL
    } else {
        MAX_STOCK
    }
}

/// Sets a stock count in the battle items and the camp (the camp shows the
/// battle's stock while one exists), and a spell's level.
fn set_stock(game: &mut Game, camp: Option<&mut CampRes>, id: u16, n: u8) {
    let n = n.min(cap_of(id));
    let set = |s: &mut std::collections::BTreeMap<u16, u8>| {
        if n == 0 {
            s.remove(&id);
        } else {
            s.insert(id, n);
        }
    };
    if let Some(it) = game.items.as_mut() {
        set(&mut it.stock);
    }
    if let Some(c) = camp {
        set(&mut c.stock);
    }
    if let Some(t) = ELEMENT_ITEMS.iter().position(|&e| e == id)
        && let Some(m) = game.magic.as_mut()
    {
        m.levels[t] = n;
    }
}

fn on_off(on: bool) -> Vec<u8> {
    text(if on { "On" } else { "Off" })
}

fn rows(
    page: Page,
    saved: Option<(u8, u8)>,
    game: &Game,
    area: Option<&AreaRes>,
    camp: Option<&CampRes>,
) -> Vec<Row> {
    let row = |label: &str, action: Action, enabled: bool| Row {
        label: text(label),
        value: Vec::new(),
        action,
        enabled,
    };
    let rt = game.enemy.as_ref();
    match page {
        Page::Main => {
            let level = rt.map(|rt| (rt.level, (rt.level as usize) < rt.sora_levels.len()));
            let busy = area.is_some_and(|a| a.transition.is_some() || a.pending.is_some());
            let mut v = vec![
                row(
                    "Items",
                    Action::Page(Page::Items),
                    game.items.is_some() || camp.is_some(),
                ),
                row("Restore HP, MP and Drive", Action::Restore, true),
                Row {
                    value: level.map_or_else(Vec::new, |(l, _)| text(&format!("Lv {l}"))),
                    ..row("Level up", Action::LevelUp, level.is_some_and(|l| l.1))
                },
                row(
                    "Drive form",
                    Action::Page(Page::Forms),
                    game.drive.is_some(),
                ),
                Row {
                    value: on_off(game.cheats.invincible),
                    ..row("Invincible", Action::Invincible, rt.is_some())
                },
                row(
                    "Defeat enemy",
                    Action::Defeat,
                    rt.is_some_and(|rt| rt.enemy.alive()),
                ),
                row("Respawn enemy", Action::Respawn, rt.is_some()),
                Row {
                    value: on_off(game.cheats.enemy_frozen),
                    ..row("Freeze enemy", Action::Freeze, rt.is_some())
                },
                row("Warp", Action::Page(Page::Warp), area.is_some() && !busy),
            ];
            v.push(row("Close", Action::Close, true));
            v
        }
        Page::Items => {
            let mut v = vec![row("Fill every item", Action::FillItems, true)];
            for (id, name) in item_list(camp, game) {
                v.push(Row {
                    label: name,
                    value: text(&stock_of(game, camp, id).to_string()),
                    action: Action::Item(id),
                    enabled: true,
                });
            }
            v
        }
        Page::Forms => {
            let d = game.drive.as_ref();
            let idle = d.is_some_and(|d| !d.active && d.change.is_none());
            let mut v: Vec<Row> = FORMS
                .iter()
                .map(|&(f, name)| row(name, Action::Form(f), idle))
                .collect();
            v.push(row(
                "Revert",
                Action::Form(0),
                d.is_some_and(|d| d.active && d.change.is_none()),
            ));
            v
        }
        Page::Warp => {
            let Some(a) = area else { return Vec::new() };
            let (world, here) = (a.current.world, a.current.area);
            let code = area_sim::progress::WORLDS[world as usize];
            let saved = crate::launcher::field_start(code).map(|_| {
                saved.map_or_else(|| text("Default"), |(n, _)| text(&format!("{code}{n:02}")))
            });
            let mut v = vec![Row {
                value: saved.clone().unwrap_or_default(),
                ..row(
                    &format!("Start here next launch ({code}{here:02})"),
                    Action::SaveStart,
                    saved.is_some(),
                )
            }];
            v.extend(
                (0..area_sim::progress::WORLDS.len() as u8)
                    .filter(|&w| !a.source.rooms(w).is_empty())
                    .map(|w| Row {
                        value: if w == world {
                            text("Here")
                        } else {
                            text(&format!("{} rooms", a.source.rooms(w).len()))
                        },
                        ..row(area::world_name(w), Action::Page(Page::Rooms(w)), true)
                    }),
            );
            v
        }
        Page::Rooms(w) => {
            let Some(a) = area else { return Vec::new() };
            let code = area_sim::progress::WORLDS[w as usize];
            let current = (a.current.world, a.current.area);
            a.source
                .rooms(w)
                .iter()
                .map(|&n| Row {
                    label: a
                        .source
                        .place_name(w, n)
                        .cloned()
                        .unwrap_or_else(|| text(&format!("Room {n:02}"))),
                    value: if (w, n) == current {
                        text("Here")
                    } else {
                        text(&format!("{code}{n:02}"))
                    },
                    action: Action::Warp(w, n),
                    enabled: (w, n) != current,
                })
                .collect()
        }
    }
}

fn heading(page: Page) -> String {
    match page {
        Page::Main => "Debug".into(),
        Page::Items => "Debug - Items".into(),
        Page::Forms => "Debug - Drive form".into(),
        Page::Warp => "Debug - Warp".into(),
        Page::Rooms(w) => format!("Debug - {}", area::world_name(w)),
    }
}

fn footer(page: Page) -> &'static str {
    match page {
        Page::Items => "Left/Right change   Esc back",
        Page::Main => "Enter select   F3 close",
        Page::Rooms(_) => "Enter warp   Esc back",
        _ => "Enter select   Esc back",
    }
}

/// Runs a row's action; returns a line for the footer.
fn act(
    action: Action,
    step: i32,
    menu: &mut DebugMenu,
    game: &mut Game,
    area: Option<&mut AreaRes>,
    mut camp: Option<&mut CampRes>,
    hud: Option<&mut Hud>,
) -> Option<String> {
    match action {
        Action::Page(p) => {
            if menu.page == Page::Main {
                menu.main_cursor = menu.cursor;
            }
            menu.page = p;
            menu.cursor = 0;
            menu.top = 0;
            match (p, area.as_deref()) {
                (Page::Warp, Some(a)) => {
                    let code = area_sim::progress::WORLDS[a.current.world as usize];
                    menu.saved = crate::launcher::saved_start(&crate::launcher::data_dir(), code);
                }
                // A world's rooms open on the current room.
                (Page::Rooms(w), Some(a)) if w == a.current.world => {
                    let rooms = a.source.rooms(w);
                    menu.cursor = rooms.iter().position(|&n| n == a.current.area).unwrap_or(0);
                }
                _ => {}
            }
            None
        }
        Action::SaveStart => {
            let a = area?;
            let code = area_sim::progress::WORLDS[a.current.world as usize];
            let (n, entrance) = (a.current.area, a.current.state.entrance);
            match crate::launcher::save_start(code, n, entrance) {
                Ok(_) => {
                    menu.saved = Some((n, entrance));
                    Some(format!("{code}{n:02} is this world's start"))
                }
                Err(e) => Some(format!("Start not saved: {e}")),
            }
        }
        Action::Close => {
            menu.open = false;
            None
        }
        Action::Restore => {
            if let Some(rt) = game.enemy.as_mut() {
                rt.sora_hp = rt.sora_target.max_hp;
                rt.ko_ticks = 0;
            } else if let Some(h) = hud {
                h.stats.hp = h.stats.max_hp;
            }
            let mp = &mut game.player.mp;
            (mp.cur, mp.remaining, mp.total) = (mp.max, 0., 0.);
            game.player.mp_charging = false;
            // In a form the gauge is the form's timer; leave it running.
            if game.drive.as_ref().is_none_or(|d| !d.active) {
                game.player.drive.refill();
            }
            Some("HP, MP and Drive restored".into())
        }
        Action::LevelUp => {
            let tick = game.tick;
            let Game { enemy, log, .. } = game;
            let rt = enemy.as_mut()?;
            let next = rt.sora_levels.get(rt.level as usize - 1)?.exp;
            let need = next.saturating_sub(rt.exp_total).max(1);
            prize_room::grant_exp(rt, need, log, tick);
            Some(format!("Level {}", rt.level))
        }
        Action::Invincible => {
            game.cheats.invincible = !game.cheats.invincible;
            None
        }
        Action::Freeze => {
            game.cheats.enemy_frozen = !game.cheats.enemy_frozen;
            None
        }
        Action::Defeat => {
            game.enemy.as_mut()?.enemy.defeat();
            Some("Enemy defeated".into())
        }
        Action::Respawn => {
            game.enemy.as_mut()?.enemy.respawn();
            Some("Enemy respawned".into())
        }
        Action::FillItems => {
            for (id, _) in item_list(camp.as_deref(), game) {
                set_stock(game, camp.as_deref_mut(), id, cap_of(id));
            }
            // Empty battle slots take their item back.
            if let Some(it) = game.items.as_mut() {
                for (slot, &auto) in it.slots.iter_mut().zip(&it.originals) {
                    if *slot == 0 {
                        *slot = auto;
                    }
                }
            }
            Some("Every item filled".into())
        }
        Action::Item(id) => {
            let n = stock_of(game, camp.as_deref(), id) as i32 + step;
            set_stock(game, camp, id, n.clamp(0, cap_of(id) as i32) as u8);
            None
        }
        Action::Form(f) => {
            let d = game.drive.as_ref()?;
            let (active, changing) = (d.active, d.change.is_some());
            if changing {
                return None;
            }
            if f == 0 {
                if active {
                    drive_room::start_change(game, 0);
                    menu.open = false;
                }
                return None;
            }
            if active {
                return None;
            }
            // A full gauge gives the form its full time.
            game.player.drive.refill();
            drive_room::start_change(game, f);
            menu.open = false;
            None
        }
        Action::Warp(world, n) => {
            let a = area?;
            if a.transition.is_some() || a.pending.is_some() {
                return None;
            }
            a.transition = Some(Transition {
                stage: Stage::Closing,
                iris: None,
                fade: Fade::default(),
                target: (world, n, 0),
                job: start_area_job(&a.source, (world, n, 0)),
            });
            menu.open = false;
            None
        }
    }
}

/// Opens, drives and closes the menu (Update, so presses are not missed
/// between fixed ticks).
#[allow(clippy::too_many_arguments)]
pub fn tick(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    pads: Query<(&Gamepad, Option<&Name>)>,
    mut menu: ResMut<DebugMenu>,
    mut game: ResMut<Game>,
    mut area: Option<ResMut<AreaRes>>,
    mut camp: Option<ResMut<CampRes>>,
    mut hud: Option<ResMut<Hud>>,
) {
    if game.autoplay.is_some() {
        return;
    }
    let menu = &mut *menu;
    let input = read_input(&keys, &pads, &mut menu.held);
    let was_open = menu.open;
    // Esc backs out of the menu; the room must not also take it as exit.
    if was_open {
        keys.clear_just_pressed(KeyCode::Escape);
    }
    if !menu.open {
        let blocked = camp.as_ref().is_some_and(|c| c.menu_up()) || game.talking;
        if !input.toggle || blocked {
            return;
        }
        menu.open = true;
        menu.page = Page::Main;
        menu.cursor = menu.main_cursor;
        menu.top = 0;
    } else if input.toggle {
        menu.open = false;
    } else {
        let list = rows(
            menu.page,
            menu.saved,
            &game,
            area.as_deref(),
            camp.as_deref(),
        );
        let count = list.len();
        menu.cursor = menu.cursor.min(count.saturating_sub(1));
        let selected = list.get(menu.cursor);
        let mut run = None;
        match input.step {
            Some(Dir::Up) if count > 0 => menu.cursor = (menu.cursor + count - 1) % count,
            Some(Dir::Down) if count > 0 => menu.cursor = (menu.cursor + 1) % count,
            Some(d @ (Dir::Left | Dir::Right)) => {
                if let Some(r) = selected.filter(|r| matches!(r.action, Action::Item(_))) {
                    run = Some((r.action, if d == Dir::Left { -1 } else { 1 }));
                }
            }
            _ => {}
        }
        if input.confirm
            && let Some(r) = selected.filter(|r| r.enabled)
        {
            run = Some((r.action, 1));
        } else if input.back {
            match menu.page {
                Page::Main => menu.open = false,
                // Back to the world list, on the world just left.
                Page::Rooms(w) => {
                    menu.page = Page::Warp;
                    menu.cursor = rows(Page::Warp, menu.saved, &game, area.as_deref(), None)
                        .iter()
                        .position(|r| r.action == Action::Page(Page::Rooms(w)))
                        .unwrap_or(0);
                    menu.top = 0;
                }
                _ => {
                    menu.page = Page::Main;
                    menu.cursor = menu.main_cursor;
                    menu.top = 0;
                }
            }
        }
        if let Some((action, step)) = run {
            let notice = act(
                action,
                step,
                menu,
                &mut game,
                area.as_deref_mut(),
                camp.as_deref_mut(),
                hud.as_deref_mut(),
            );
            if let Some(n) = notice {
                eprintln!("Debug menu: {n}");
                menu.notice = Some((n, NOTICE_FRAMES));
            }
        }
    }
    if let Some((_, frames)) = menu.notice.as_mut() {
        *frames = frames.saturating_sub(1);
        if *frames == 0 {
            menu.notice = None;
        }
    }
    let view = if menu.open {
        build_view(menu, &game, area.as_deref(), camp.as_deref())
    } else {
        View::default()
    };
    if view != menu.view || was_open != menu.open {
        menu.view = view;
        if let Some(h) = hud.as_deref_mut() {
            h.updated = true;
        }
    }
}

fn build_view(
    menu: &mut DebugMenu,
    game: &Game,
    area: Option<&AreaRes>,
    camp: Option<&CampRes>,
) -> View {
    let list = rows(menu.page, menu.saved, game, area, camp);
    menu.cursor = menu.cursor.min(list.len().saturating_sub(1));
    menu.top = scroll(menu.cursor, menu.top);
    let footer = match &menu.notice {
        Some((n, _)) => n.clone(),
        None if list.len() > ROWS => {
            format!("{}   {}/{}", footer(menu.page), menu.cursor + 1, list.len())
        }
        None => footer(menu.page).to_string(),
    };
    View {
        heading: text(&heading(menu.page)),
        rows: list
            .into_iter()
            .skip(menu.top)
            .take(ROWS)
            .map(|r| (r.label, r.value, r.enabled))
            .collect(),
        selected: Some(menu.cursor - menu.top),
        footer: text(&footer),
    }
}

/// The first row shown, moved just enough to keep the cursor in view.
fn scroll(cursor: usize, top: usize) -> usize {
    if cursor < top {
        cursor
    } else if cursor >= top + ROWS {
        cursor + 1 - ROWS
    } else {
        top
    }
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32, rgba: [f32; 4]) -> sequence::Quad {
    sequence::Quad {
        corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
        uv: [0, 0, 1, 1],
        colors: [rgba; 4],
        blend: 0,
    }
}

/// The menu's draws: the box and highlight bar on `white`, the text on the
/// system font sheet `font_sheet`.
pub fn draws(view: &View, white: usize, font_sheet: usize, widths: &[u8]) -> Vec<Draw> {
    let font = command::Font { widths, pad: 1 };
    let [x0, y0, x1, y1] = BOX;
    let mut shapes = vec![rect(x0, y0, x1, y1, [0.04, 0.04, 0.06, 0.85])];
    let mut glyphs = Vec::new();
    let cell = 24. * TEXT_SIZE / 18.;
    let line = |y: f32| y + (ROW_HEIGHT - cell) / 2.;
    glyphs.extend(command::text_quads_sized(
        &view.heading,
        [x0 + MARGIN, line(y0 + 6.)],
        [1.; 4],
        &font,
        TEXT_SIZE + 2.,
    ));
    let top = y0 + 34.;
    for (i, (label, value, enabled)) in view.rows.iter().enumerate() {
        let y = top + i as f32 * ROW_HEIGHT;
        if view.selected == Some(i) {
            shapes.push(rect(
                x0 + 6.,
                y,
                x1 - 6.,
                y + ROW_HEIGHT,
                [1., 1., 1., 0.16],
            ));
        }
        let colour = if *enabled {
            [1.; 4]
        } else {
            [0.5, 0.5, 0.5, 1.]
        };
        glyphs.extend(command::text_quads_sized(
            label,
            [x0 + MARGIN, line(y)],
            colour,
            &font,
            TEXT_SIZE,
        ));
        if !value.is_empty() {
            let w = command::text_width(value, &font, TEXT_SIZE);
            glyphs.extend(command::text_quads_sized(
                value,
                [x1 - MARGIN - w, line(y)],
                colour,
                &font,
                TEXT_SIZE,
            ));
        }
    }
    glyphs.extend(command::text_quads_sized(
        &view.footer,
        [x0 + MARGIN, line(y1 - ROW_HEIGHT - 6.)],
        [0.7, 0.7, 0.7, 1.],
        &font,
        TEXT_SIZE - 2.,
    ));
    vec![
        Draw::Quads {
            quads: shapes,
            z: 0,
            sheet: white,
        },
        Draw::Quads {
            quads: glyphs,
            z: 0,
            sheet: font_sheet,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_keeps_the_cursor_in_view() {
        assert_eq!(scroll(20, 0), 20 + 1 - ROWS);
        assert_eq!(scroll(3, 10), 3);
        assert_eq!(scroll(12, 5), 5);
    }

    #[test]
    fn spell_items_cap_at_their_level() {
        assert_eq!(cap_of(ELEMENT_ITEMS[0]), MAX_SPELL);
        assert_eq!(cap_of(1), MAX_STOCK);
    }

    #[test]
    fn draws_put_text_inside_the_box() {
        let view = View {
            heading: text("Debug"),
            rows: vec![(text("Items"), text("12"), true)],
            selected: Some(0),
            footer: text("F3 close"),
        };
        let widths = [10u8; 224];
        let d = draws(&view, 0, 1, &widths);
        assert_eq!(d.len(), 2);
        let Draw::Quads { quads, .. } = &d[1] else {
            panic!("text draw")
        };
        for q in quads {
            for [x, y] in q.corners {
                assert!((BOX[0]..=BOX[2]).contains(&x) && (BOX[1]..=BOX[3]).contains(&y));
            }
        }
    }
}
