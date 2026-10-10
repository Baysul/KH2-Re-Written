//! The launcher window `kh2fm` opens without arguments: pick the game
//! folder (found in Steam's libraries, browsed, or dropped on the window),
//! check it against the supported build, then choose a test. The choice is
//! remembered, so later launches open on the test list.
//!
//! Before a folder is chosen there are no game files, so everything here is
//! drawn from generated shapes and Windows' fonts in a KH2-like style
//! [launcher design, not the original's menus]. A test runs as a child
//! `kh2fm <program> ...` process; this window waits hidden and comes back
//! when the test closes.
mod install;
mod paint;

use bevy::app::AppExit;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::{FileDragAndDrop, PrimaryWindow, WindowResolution};
use bevy::winit::{UpdateMode, WinitSettings};
use install::{Bound, CheckError, Settings, TESTS};
use paint::{Face, Paint, Rgba, rgba, with_alpha};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows_shell::ui2d::{Ui2dFrame, Ui2dPlugin};

pub use install::play_args;

// Palette.
const INK: u32 = 0xeaf3ff;
const INK_SOFT: u32 = 0xa9bfe6;
const INK_DIM: u32 = 0x6f86b3;
const GOLD: u32 = 0xf3cf6b;
const GOLD_DEEP: u32 = 0xc8902e;
const NAVY: u32 = 0x0b1d46;
const BLUE: u32 = 0x2c6be0;
const EDGE: u32 = 0x5d8fe6;

const MARGIN: f32 = 72.;
const LIST_TOP: f32 = 222.;
const LIST_BOTTOM: f32 = 640.;
const LIST_WIDTH: f32 = 540.;
const PANEL: [f32; 4] = [668., 222., 540., 360.];

#[derive(Clone, Debug, PartialEq)]
enum Action {
    UseFolder(PathBuf),
    /// Browse into a folder (None: the drive list).
    Browse(Option<PathBuf>),
    Pick,
    Run(usize),
    Quit,
}

#[derive(Clone)]
struct Item {
    label: String,
    detail: Option<String>,
    /// A gold tag on the right ("Steam", "KH2FM here").
    tag: Option<&'static str>,
    action: Option<Action>,
}

impl Item {
    fn new(label: impl Into<String>, action: Action) -> Self {
        Self {
            label: label.into(),
            detail: None,
            tag: None,
            action: Some(action),
        }
    }
    fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }
    fn tag(mut self, t: &'static str) -> Self {
        self.tag = Some(t);
        self
    }
    fn disabled(mut self) -> Self {
        self.action = None;
        self
    }
}

/// A folder check or scan running on its own thread.
struct Job {
    folder: PathBuf,
    progress: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    scanning: Arc<AtomicBool>,
    done: Arc<Mutex<Option<Result<Bound, String>>>>,
}

/// A test running as a child process.
struct Running {
    child: Child,
    name: &'static str,
    output: Arc<Mutex<VecDeque<String>>>,
    window_open: Arc<AtomicBool>,
    since_window: Option<f32>,
    hidden: bool,
}

enum Screen {
    Boot,
    Pick,
    Browse(Option<PathBuf>),
    Check(Job),
    Tests(Bound),
    Loading(Running, Bound),
    Problem {
        title: String,
        lines: Vec<String>,
        retry: Option<Bound>,
    },
}

#[derive(Resource)]
struct Launcher {
    dir: PathBuf,
    settings: Settings,
    supported: Option<String>,
    detected: Vec<PathBuf>,
    screen: Screen,
    items: Vec<Item>,
    cursor: usize,
    scroll: usize,
    /// Per-row highlight, eased towards the cursor.
    glow: Vec<f32>,
    entered: f32,
    now: f32,
    fade: f32,
    hover_drop: bool,
    repeat: Option<(i32, f32)>,
    mouse: Option<Vec2>,
    sparkles: Vec<[f32; 4]>,
    paint: Option<Paint>,
}

impl Launcher {
    fn go(&mut self, screen: Screen) {
        self.screen = screen;
        self.items = self.build_items();
        self.cursor = self
            .items
            .iter()
            .position(|i| i.action.is_some())
            .unwrap_or(0);
        self.scroll = 0;
        self.glow = vec![0.; self.items.len()];
        self.entered = self.now;
        self.repeat = None;
    }

    fn build_items(&self) -> Vec<Item> {
        match &self.screen {
            Screen::Pick => {
                let mut items: Vec<Item> = self
                    .detected
                    .iter()
                    .map(|p| {
                        let current = self.settings.game_folder.as_ref() == Some(p);
                        Item::new(
                            if current {
                                "Keep the Steam Install"
                            } else {
                                "Use the Steam Install"
                            },
                            Action::UseFolder(p.clone()),
                        )
                        .detail(plain(p))
                        .tag("STEAM")
                    })
                    .collect();
                items.push(
                    Item::new("Browse for the Folder", Action::Browse(None))
                        .detail("Find the collection's install folder yourself"),
                );
                if let Some(remembered) = &self.settings.game_folder
                    && !self.detected.contains(remembered)
                {
                    items.push(
                        Item::new(
                            "Keep the Current Folder",
                            Action::UseFolder(remembered.clone()),
                        )
                        .detail(plain(remembered)),
                    );
                }
                items.push(Item::new("Quit", Action::Quit));
                items
            }
            Screen::Browse(path) => browse_items(path.as_deref()),
            Screen::Tests(_) => {
                let mut items: Vec<Item> = TESTS
                    .iter()
                    .enumerate()
                    .map(|(i, t)| Item::new(t.name, Action::Run(i)))
                    .collect();
                items.push(Item::new("Change Game Folder", Action::Pick));
                items.push(Item::new("Quit", Action::Quit));
                items
            }
            Screen::Problem { retry, .. } => {
                let mut items = Vec::new();
                if let Some(b) = retry {
                    items.push(Item::new(
                        "Back to the Tests",
                        Action::UseFolder(b.folder.clone()),
                    ));
                }
                items.push(Item::new("Choose Another Folder", Action::Pick));
                items.push(Item::new("Quit", Action::Quit));
                items
            }
            Screen::Boot | Screen::Check(_) | Screen::Loading(..) => Vec::new(),
        }
    }
}

/// A path as people type it (no `\\?\` prefix).
fn plain(p: &Path) -> String {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").map(str::to_owned).unwrap_or(s)
}

fn browse_items(path: Option<&Path>) -> Vec<Item> {
    let Some(path) = path else {
        let mut items: Vec<Item> = ('A'..='Z')
            .map(|d| PathBuf::from(format!("{d}:\\")))
            .filter(|p| p.is_dir())
            .map(|p| Item::new(plain(&p), Action::Browse(Some(p))))
            .collect();
        items.push(Item::new("Back", Action::Pick));
        return items;
    };
    let mut items = Vec::new();
    let here = Item::new("Use This Folder", Action::UseFolder(path.to_path_buf()));
    items.push(if install::has_executable(path) {
        here.detail("KINGDOM HEARTS II FINAL MIX is here")
            .tag("KH2FM")
    } else {
        here.detail("No KINGDOM HEARTS II FINAL MIX.exe in this folder")
            .disabled()
    });
    items.push(Item::new(
        "Up One Level",
        Action::Browse(path.parent().map(Path::to_path_buf)),
    ));
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    dirs.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    for d in dirs {
        let name = d
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let item = Item::new(name, Action::Browse(Some(d.clone())));
        items.push(if install::has_executable(&d) {
            item.tag("KH2FM")
        } else {
            item
        });
    }
    items
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    1. - (1. - t).powi(3)
}

/// Words wrapped to `width`.
fn wrap(paint: &mut Paint, face: Face, size: f32, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let next = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && paint.measure(face, size, &next) > width {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = next;
            }
        }
        lines.push(line);
    }
    lines
}

fn start_check(folder: PathBuf, dir: PathBuf, hint: Option<PathBuf>) -> Job {
    let done = Arc::new(Mutex::new(None));
    let progress = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let scanning = Arc::new(AtomicBool::new(false));
    let (p, t, s, f, tx) = (
        progress.clone(),
        total.clone(),
        scanning.clone(),
        folder.clone(),
        done.clone(),
    );
    std::thread::spawn(move || {
        let result = match install::quick_check(&dir, &f, hint.as_deref()) {
            Ok(b) => Ok(b),
            Err(CheckError::Rejected(why)) => Err(why),
            Err(CheckError::NeedsScan) => {
                t.store(source_identity::required_bytes(&f), Ordering::Relaxed);
                s.store(true, Ordering::Relaxed);
                install::full_scan(&dir, &f, &p)
            }
        };
        if let Ok(mut slot) = tx.lock() {
            *slot = Some(result);
        }
    });
    Job {
        folder,
        progress,
        total,
        scanning,
        done,
    }
}

fn start_test(index: usize, dir: &Path, b: &Bound) -> Result<Running, String> {
    let test = &TESTS[index];
    let args = install::test_args(dir, test.program, b)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = Command::new(exe)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("The test could not start: {e}"))?;
    let output = Arc::new(Mutex::new(VecDeque::new()));
    let window_open = Arc::new(AtomicBool::new(false));
    let pipes: [Box<dyn std::io::Read + Send>; 2] = [
        Box::new(child.stdout.take().ok_or("no test output")?),
        Box::new(child.stderr.take().ok_or("no test output")?),
    ];
    for pipe in pipes {
        let (output, window_open) = (output.clone(), window_open.clone());
        std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                if line.contains("Creating new window") {
                    window_open.store(true, Ordering::Relaxed);
                }
                if let Ok(mut o) = output.lock() {
                    if o.len() >= 80 {
                        o.pop_front();
                    }
                    o.push_back(line);
                }
            }
        });
    }
    Ok(Running {
        child,
        name: test.name,
        output,
        window_open,
        since_window: None,
        hidden: false,
    })
}

/// The lines worth showing from a failed test's output: its own messages,
/// not the engine's log.
fn failure_lines(output: &VecDeque<String>) -> Vec<String> {
    let own: Vec<String> = output
        .iter()
        .filter(|l| !l.contains(" INFO ") && !l.contains(" WARN ") && !l.trim().is_empty())
        .cloned()
        .collect();
    let tail = own.len().saturating_sub(6);
    own[tail..].to_vec()
}

#[derive(Default)]
struct Pressed {
    up: bool,
    down: bool,
    page: i32,
    accept: bool,
    back: bool,
    change: bool,
}

fn read_input(
    l: &mut Launcher,
    keys: &ButtonInput<KeyCode>,
    pads: &Query<&Gamepad>,
    wheel: f32,
) -> Pressed {
    let mut p = Pressed {
        accept: keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]),
        back: keys.any_just_pressed([KeyCode::Escape, KeyCode::Backspace]),
        change: keys.just_pressed(KeyCode::F2),
        page: keys.just_pressed(KeyCode::PageDown) as i32
            - keys.just_pressed(KeyCode::PageUp) as i32,
        ..default()
    };
    let mut held = keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) as i32
        - keys.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) as i32;
    for g in pads.iter() {
        p.accept |= g.just_pressed(GamepadButton::South);
        p.back |= g.just_pressed(GamepadButton::East);
        p.change |= g.just_pressed(GamepadButton::North);
        let stick = g.left_stick().y;
        let dir = g.pressed(GamepadButton::DPadDown) as i32
            - g.pressed(GamepadButton::DPadUp) as i32
            + (stick < -0.5) as i32
            - (stick > 0.5) as i32;
        if held == 0 {
            held = dir.signum();
        }
        p.page += g.just_pressed(GamepadButton::RightTrigger) as i32
            - g.just_pressed(GamepadButton::LeftTrigger) as i32;
    }
    // Held directions repeat after a pause.
    let step = match (held, l.repeat) {
        (0, _) => {
            l.repeat = None;
            0
        }
        (d, Some((was, at))) if was == d => {
            if l.now >= at {
                l.repeat = Some((d, l.now + 0.075));
                d
            } else {
                0
            }
        }
        (d, _) => {
            l.repeat = Some((d, l.now + 0.38));
            d
        }
    };
    // A tap shorter than a frame still moves once.
    let tap = keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) as i32
        - keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) as i32;
    let step = if step == 0 { tap } else { step };
    let wheel_step = if wheel > 0. {
        -1
    } else if wheel < 0. {
        1
    } else {
        0
    };
    p.up = step < 0 || wheel_step < 0;
    p.down = step > 0 || wheel_step > 0;
    p
}

/// Rows of the current list: height, gap, label size.
fn row_metrics(screen: &Screen) -> (f32, f32, f32) {
    match screen {
        Screen::Browse(_) => (40., 8., 21.),
        _ => (56., 12., 25.),
    }
}

fn visible_rows(screen: &Screen) -> usize {
    let (h, gap, _) = row_metrics(screen);
    (((LIST_BOTTOM - LIST_TOP + gap) / (h + gap)) as usize).max(1)
}

fn row_rect(l: &Launcher, i: usize) -> [f32; 4] {
    let (h, gap, _) = row_metrics(&l.screen);
    let k = i as f32 - l.scroll as f32;
    [MARGIN, LIST_TOP + k * (h + gap), LIST_WIDTH, h]
}

fn move_cursor(l: &mut Launcher, by: i32) {
    let n = l.items.len() as i32;
    if n == 0 {
        return;
    }
    let mut c = l.cursor as i32;
    for _ in 0..n {
        c = (c + by.signum()).rem_euclid(n);
        if l.items[c as usize].action.is_some() {
            break;
        }
    }
    l.cursor = c as usize;
    keep_visible(l);
}

fn keep_visible(l: &mut Launcher) {
    let rows = visible_rows(&l.screen);
    if l.cursor < l.scroll {
        l.scroll = l.cursor;
    } else if l.cursor >= l.scroll + rows {
        l.scroll = l.cursor + 1 - rows;
    }
}

fn act(l: &mut Launcher, action: Action, exit: &mut MessageWriter<AppExit>) {
    match action {
        Action::Quit => {
            exit.write(AppExit::Success);
        }
        Action::Pick => l.go(Screen::Pick),
        Action::Browse(path) => {
            let from = match &l.screen {
                Screen::Browse(Some(p)) => Some(p.clone()),
                _ => None,
            };
            l.go(Screen::Browse(path));
            // Going up lands on the folder just left.
            if let Some(from) = from
                && let Some(i) = l
                    .items
                    .iter()
                    .position(|it| it.action == Some(Action::Browse(Some(from.clone()))))
            {
                l.cursor = i;
                keep_visible(l);
            }
        }
        Action::UseFolder(folder) => {
            let hint = (l.settings.game_folder.as_deref() == Some(folder.as_path()))
                .then(|| l.settings.manifest.clone())
                .flatten();
            let job = start_check(folder, l.dir.clone(), hint);
            l.go(Screen::Check(job));
        }
        Action::Run(i) => {
            let Screen::Tests(b) = &l.screen else {
                return;
            };
            let b = b.clone();
            match start_test(i, &l.dir, &b) {
                Ok(running) => {
                    l.go(Screen::Loading(running, b));
                }
                Err(why) => l.go(Screen::Problem {
                    title: format!("{} could not start", TESTS[i].name),
                    lines: vec![why],
                    retry: Some(b),
                }),
            }
        }
    }
}

fn back(l: &mut Launcher, exit: &mut MessageWriter<AppExit>) {
    let to = match &l.screen {
        Screen::Browse(Some(p)) => Some(Action::Browse(p.parent().map(Path::to_path_buf))),
        Screen::Browse(None) => Some(Action::Pick),
        Screen::Pick => l.settings.game_folder.clone().map(Action::UseFolder),
        Screen::Problem { retry: Some(b), .. } => Some(Action::UseFolder(b.folder.clone())),
        Screen::Tests(_) => Some(Action::Quit),
        _ => None,
    };
    if let Some(a) = to {
        act(l, a, exit);
    }
}

/// A finished check: keep the folder if it is the supported build.
fn checked(l: &mut Launcher, result: Result<Bound, String>, folder: &Path) {
    let b = match result {
        Ok(b) => b,
        Err(why) => {
            return l.go(Screen::Problem {
                title: "This folder can't be used".into(),
                lines: vec![plain(folder), why],
                retry: None,
            });
        }
    };
    if let Some(want) = &l.supported
        && *want != b.identity
    {
        return l.go(Screen::Problem {
            title: "This is not the supported build".into(),
            lines: vec![
                plain(&b.folder),
                format!(
                    "The tests were made for the Steam KINGDOM HEARTS II FINAL MIX build {}…; this folder holds {}…. Unknown builds stay quarantined, so the tests won't run on it.",
                    &want[..12.min(want.len())],
                    &b.identity[..12.min(b.identity.len())]
                ),
            ],
            retry: None,
        });
    }
    l.settings = Settings {
        game_folder: Some(b.folder.clone()),
        manifest: Some(b.manifest.clone()),
    };
    if let Err(e) = install::save_settings(&l.dir, &l.settings) {
        eprintln!("Launcher settings not saved: {e}");
    }
    l.go(Screen::Tests(b));
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut l: ResMut<Launcher>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    mut wheel: MessageReader<MouseWheel>,
    mut drops: MessageReader<FileDragAndDrop>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut winit: ResMut<WinitSettings>,
    mut exit: MessageWriter<AppExit>,
) {
    let l = &mut *l;
    let dt = time.delta_secs().min(0.1);
    l.now += dt;
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    if matches!(l.screen, Screen::Boot) {
        l.supported = install::supported_identity(&l.dir);
        l.detected = install::detect_installs();
        match l.settings.game_folder.clone() {
            Some(folder) => act(l, Action::UseFolder(folder), &mut exit),
            None => l.go(Screen::Pick),
        }
    }
    let mut lines = 0.;
    for w in wheel.read() {
        lines += match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y / 40.,
        };
    }
    for d in drops.read() {
        match d {
            FileDragAndDrop::HoveredFile { .. } => l.hover_drop = true,
            FileDragAndDrop::HoveredFileCanceled { .. } => l.hover_drop = false,
            FileDragAndDrop::DroppedFile { path_buf, .. } => {
                l.hover_drop = false;
                let folder = if path_buf.is_dir() {
                    path_buf.clone()
                } else {
                    path_buf.parent().map(Path::to_path_buf).unwrap_or_default()
                };
                if matches!(
                    l.screen,
                    Screen::Pick | Screen::Browse(_) | Screen::Problem { .. } | Screen::Tests(_)
                ) {
                    act(l, Action::UseFolder(folder), &mut exit);
                }
            }
        }
    }

    // Background work.
    let mut finished = None;
    match &mut l.screen {
        Screen::Check(job) => {
            if let Some(result) = job.done.lock().ok().and_then(|mut d| d.take()) {
                finished = Some((result, job.folder.clone()));
            }
        }
        Screen::Loading(run, b) => {
            if run.window_open.load(Ordering::Relaxed) && run.since_window.is_none() {
                run.since_window = Some(l.now);
            }
            // Hide once the test's window has had a moment to draw.
            if !run.hidden && run.since_window.is_some_and(|t| l.now - t > 0.6) {
                run.hidden = true;
                window.visible = false;
                winit.unfocused_mode = UpdateMode::reactive_low_power(Duration::from_millis(100));
            }
            if let Ok(Some(status)) = run.child.try_wait() {
                window.visible = true;
                window.focused = true;
                winit.unfocused_mode =
                    UpdateMode::reactive_low_power(Duration::from_secs_f64(1. / 60.));
                let b = b.clone();
                let name = run.name;
                let lines = run
                    .output
                    .lock()
                    .map(|o| failure_lines(&o))
                    .unwrap_or_default();
                l.fade = 1.;
                if status.success() {
                    let cursor = l.cursor;
                    l.go(Screen::Tests(b));
                    l.cursor = cursor;
                } else {
                    l.go(Screen::Problem {
                        title: format!("{name} stopped with an error"),
                        lines: if lines.is_empty() {
                            vec![format!("Exit status: {status}")]
                        } else {
                            lines
                        },
                        retry: Some(b),
                    });
                }
            }
        }
        _ => {}
    }
    if let Some((result, folder)) = finished {
        checked(l, result, &folder);
    }

    // Input on lists.
    if l.items.is_empty() {
        return;
    }
    let p = read_input(l, &keys, &pads, lines);
    if p.up {
        move_cursor(l, -1);
    }
    if p.down {
        move_cursor(l, 1);
    }
    if p.page != 0 {
        let rows = visible_rows(&l.screen) as i32;
        let n = l.items.len() as i32;
        l.cursor = (l.cursor as i32 + p.page * rows).clamp(0, n - 1) as usize;
        keep_visible(l);
    }
    // Mouse: hover picks (only while it moves, so keys win otherwise),
    // click decides.
    let mut clicked = false;
    let pos = window.cursor_position();
    let moved = l.mouse.is_some() && pos.is_some() && pos != l.mouse;
    l.mouse = pos;
    if let Some(pos) = pos {
        let (x, y) = (
            pos.x * paint::W / window.width(),
            pos.y * paint::H / window.height(),
        );
        let rows = visible_rows(&l.screen);
        for i in l.scroll..(l.scroll + rows).min(l.items.len()) {
            let [rx, ry, rw, rh] = row_rect(l, i);
            let inside = x >= rx && x <= rx + rw + 20. && y >= ry && y <= ry + rh;
            if inside && l.items[i].action.is_some() {
                if moved {
                    l.cursor = i;
                }
                if mouse.just_pressed(MouseButton::Left) {
                    l.cursor = i;
                    clicked = true;
                }
            }
        }
    }
    if mouse.just_pressed(MouseButton::Right) {
        back(l, &mut exit);
        return;
    }
    if p.accept || clicked {
        if let Some(a) = l.items.get(l.cursor).and_then(|i| i.action.clone()) {
            act(l, a, &mut exit);
        }
    } else if p.back {
        back(l, &mut exit);
    } else if p.change && matches!(l.screen, Screen::Tests(_)) {
        act(l, Action::Pick, &mut exit);
    }
}

fn draw(
    mut l: ResMut<Launcher>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time>,
) {
    let l = &mut *l;
    if l.paint.is_none() {
        match Paint::new(&mut images, &mut frame) {
            Ok(p) => l.paint = Some(p),
            Err(e) => {
                eprintln!("Launcher drawing unavailable: {e}");
                return;
            }
        }
    }
    let dt = time.delta_secs().min(0.1);
    l.fade = (l.fade - dt * 3.).max(0.);
    for (i, g) in l.glow.iter_mut().enumerate() {
        let want = (i == l.cursor) as u8 as f32;
        *g += (want - *g) * (1. - (-dt * 14.).exp());
    }
    let mut paint = l.paint.take().expect("paint");
    let f = &mut *frame;
    background(l, &mut paint, f, dt);
    header(l, &mut paint, f);
    match &l.screen {
        Screen::Check(job) => check_screen(l, job, &mut paint, f),
        Screen::Loading(run, _) => loading_screen(l, run.name, &mut paint, f),
        Screen::Boot => {}
        _ => {
            list(l, &mut paint, f);
            side_panel(l, &mut paint, f);
            footer(l, &mut paint, f);
        }
    }
    if l.hover_drop {
        paint.rect(
            f,
            [0., 0., paint::W, paint::H],
            [rgba(0x000000, 0.55); 4],
            false,
        );
        paint.plate(
            f,
            [340., 300., 600., 120.],
            22.,
            0.,
            [rgba(NAVY, 0.95); 4],
            false,
        );
        paint.text(
            f,
            Face::Heading,
            30.,
            [640., 340.],
            0.5,
            "Drop to use this game folder",
            [rgba(INK, 1.), rgba(INK_SOFT, 1.)],
            0.6,
        );
    }
    if l.fade > 0. {
        paint.rect(
            f,
            [0., 0., paint::W, paint::H],
            [rgba(0x000000, ease(l.fade)); 4],
            false,
        );
    }
    paint.flush(&mut images, f);
    l.paint = Some(paint);
}

fn background(l: &mut Launcher, p: &mut Paint, f: &mut Ui2dFrame, dt: f32) {
    let (top, bottom) = (rgba(0x081536, 1.), rgba(0x010208, 1.));
    p.rect(
        f,
        [0., 0., paint::W, paint::H],
        [top, top, bottom, bottom],
        false,
    );
    let breathe = 0.85 + 0.15 * (l.now * 0.6).sin();
    p.blob(
        f,
        p.glow,
        [330., 300.],
        [1100., 760.],
        rgba(0x1d4fc4, 0.30 * breathe),
    );
    p.blob(f, p.glow, [1080., 620.], [900., 560.], rgba(0x2a7fe0, 0.16));
    p.blob(f, p.glow, [640., -60.], [1400., 260.], rgba(0x6aa8ff, 0.10));
    // Drifting sparkles: [x, y, size, phase].
    if l.sparkles.is_empty() {
        let mut seed = 0x9e37_79b9_u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 10_000) as f32 / 10_000.
        };
        l.sparkles = (0..46)
            .map(|_| {
                [
                    next() * paint::W,
                    next() * paint::H,
                    6. + next() * 16.,
                    next() * std::f32::consts::TAU,
                ]
            })
            .collect();
    }
    for s in &mut l.sparkles {
        s[1] -= dt * (6. + s[2] * 0.9);
        s[0] += dt * 3. * (l.now * 0.3 + s[3]).sin();
        if s[1] < -20. {
            s[1] = paint::H + 20.;
        }
        let twinkle = 0.5 + 0.5 * (l.now * 1.7 + s[3] * 3.).sin();
        let size = s[2] * (0.6 + 0.4 * twinkle);
        p.blob(
            f,
            p.star,
            [s[0], s[1]],
            [size, size],
            rgba(0x9cc8ff, 0.25 + 0.55 * twinkle),
        );
    }
}

fn header(l: &Launcher, p: &mut Paint, f: &mut Ui2dFrame) {
    let title_w = p.text(
        f,
        Face::Title,
        40.,
        [MARGIN, 34.],
        0.,
        "KINGDOM HEARTS II",
        [rgba(0xffffff, 1.), rgba(0xb9d4ff, 1.)],
        0.7,
    );
    p.text(
        f,
        Face::Heading,
        22.,
        [MARGIN + title_w + 14., 52.],
        0.,
        "FINAL MIX",
        [rgba(GOLD, 1.), rgba(GOLD_DEEP, 1.)],
        0.6,
    );
    p.text(
        f,
        Face::Heading,
        24.,
        [paint::W - MARGIN, 48.],
        1.,
        "Final Re:WRITE",
        [rgba(INK, 0.95), rgba(INK_SOFT, 0.95)],
        0.6,
    );
    // Gold rule fading out at both ends.
    let y = 104.;
    let (c, e) = (rgba(GOLD, 0.85), rgba(GOLD, 0.));
    p.rect(f, [MARGIN - 20., y, 360., 2.], [e, c, e, c], false);
    p.rect(
        f,
        [MARGIN + 340., y, paint::W - 2. * MARGIN - 320., 2.],
        [c, e, c, e],
        false,
    );
    p.blob(
        f,
        p.glow,
        [MARGIN + 340., y + 1.],
        [420., 26.],
        rgba(GOLD, 0.35),
    );
    let (title, sub): (&str, String) = match &l.screen {
        Screen::Boot | Screen::Pick => (
            "SELECT GAME FOLDER",
            "Where is KINGDOM HEARTS -HD 1.5+2.5 ReMIX- installed?".into(),
        ),
        Screen::Browse(path) => (
            "BROWSE",
            path.as_deref()
                .map_or_else(|| "Choose a drive".into(), plain),
        ),
        Screen::Check(_) => ("CHECKING", "Making sure this is the supported game".into()),
        Screen::Tests(_) | Screen::Loading(..) => {
            ("TEST MENU", "How do you want to test the game?".into())
        }
        Screen::Problem { title, .. } => ("NOTICE", title.clone()),
    };
    let t = ease((l.now - l.entered) * 3.);
    p.text(
        f,
        Face::Heading,
        32.,
        [MARGIN - 24. * (1. - t), 128.],
        0.,
        title,
        [
            with_alpha(rgba(INK, 1.), t),
            with_alpha(rgba(0x9fc2ff, 1.), t),
        ],
        0.7,
    );
    let sub = p.fit(Face::Body, 19., &sub, paint::W - 2. * MARGIN);
    p.text(
        f,
        Face::Body,
        19.,
        [MARGIN + 2., 172.],
        0.,
        &sub,
        [with_alpha(rgba(INK_SOFT, 1.), t); 2],
        0.5,
    );
}

fn list(l: &Launcher, p: &mut Paint, f: &mut Ui2dFrame) {
    let (h, _, label_size) = row_metrics(&l.screen);
    let rows = visible_rows(&l.screen);
    let since = l.now - l.entered;
    for (k, i) in (l.scroll..(l.scroll + rows).min(l.items.len())).enumerate() {
        let item = &l.items[i];
        let g = l.glow.get(i).copied().unwrap_or(0.);
        let intro = ease((since - k as f32 * 0.035) * 4.5);
        let [x, y, w, _] = row_rect(l, i);
        let x = x - 40. * (1. - intro) + 14. * g;
        let a = intro * if item.action.is_some() { 1. } else { 0.55 };
        // Border, body, then the highlight.
        p.plate(
            f,
            [x - 2., y - 2., w + 4., h + 4.],
            h * 0.32,
            12.,
            [with_alpha(rgba(EDGE, 0.55 + 0.4 * g), a); 4],
            false,
        );
        let (l0, l1) = (rgba(NAVY, 0.96), rgba(0x050c22, 0.92));
        p.plate(
            f,
            [x, y, w, h],
            h * 0.3,
            12.,
            [l0, l1, l0, l1].map(|c| with_alpha(c, a)),
            false,
        );
        if g > 0.01 {
            let (b0, b1) = (rgba(BLUE, 0.95 * g * a), rgba(0x123a8c, 0.55 * g * a));
            let (b2, b3) = (rgba(0x1b4fb8, 0.95 * g * a), rgba(0x0a2260, 0.4 * g * a));
            p.plate(f, [x, y, w, h], h * 0.3, 12., [b0, b1, b2, b3], false);
            // Sheen on the top half.
            let s = rgba(0xffffff, 0.16 * g * a);
            p.plate(
                f,
                [x + 4., y + 3., w - 8., h * 0.42],
                h * 0.2,
                7.,
                [s, with_alpha(s, 0.2), with_alpha(s, 0.), with_alpha(s, 0.)],
                true,
            );
        }
        let ink = if item.action.is_some() {
            [rgba(0xffffff, a), rgba(0xcfe2ff, a)]
        } else {
            [rgba(INK_DIM, a); 2]
        };
        let text_x = x + 30.;
        let has_detail = item.detail.is_some() && h > 48.;
        let label_y = if has_detail {
            y + 6.
        } else {
            y + (h - label_size * 1.32) / 2.
        };
        let tag_w = item
            .tag
            .map_or(0., |t| p.measure(Face::Heading, 15., t) + 30.);
        let label = p.fit(Face::Heading, label_size, &item.label, w - 50. - tag_w);
        p.text(
            f,
            Face::Heading,
            label_size,
            [text_x, label_y],
            0.,
            &label,
            ink,
            0.6,
        );
        if has_detail && let Some(d) = &item.detail {
            let d = p.fit(Face::Body, 15., d, w - 50. - tag_w);
            p.text(
                f,
                Face::Body,
                15.,
                [text_x + 2., y + 33.],
                0.,
                &d,
                [rgba(INK_SOFT, a * (0.75 + 0.25 * g)); 2],
                0.,
            );
        }
        if let Some(tag) = item.tag {
            let tw = tag_w - 30.;
            let tx = x + w - tw - 24.;
            let ty = y + (h - 22.) / 2.;
            p.plate(
                f,
                [tx - 8., ty, tw + 16., 22.],
                8.,
                4.,
                [
                    rgba(GOLD_DEEP, 0.85 * a),
                    rgba(GOLD, 0.85 * a),
                    rgba(0x7a5414, 0.85 * a),
                    rgba(GOLD_DEEP, 0.85 * a),
                ],
                false,
            );
            p.text(
                f,
                Face::Heading,
                15.,
                [tx, ty + 1.],
                0.,
                tag,
                [rgba(0x2a1a02, a); 2],
                0.,
            );
        }
        if i == l.cursor && item.action.is_some() {
            cursor(l, p, f, [x - 30., y + h / 2.], a);
        }
    }
    // Scroll hints.
    if l.scroll > 0 {
        arrow(p, f, [MARGIN + LIST_WIDTH / 2., LIST_TOP - 16.], -1., l.now);
    }
    if l.scroll + rows < l.items.len() {
        arrow(
            p,
            f,
            [MARGIN + LIST_WIDTH / 2., LIST_BOTTOM + 12.],
            1.,
            l.now,
        );
    }
}

/// The pointer: a glowing gold triangle that bobs.
fn cursor(l: &Launcher, p: &mut Paint, f: &mut Ui2dFrame, [x, y]: [f32; 2], a: f32) {
    let bob = (l.now * 6.).sin() * 3.;
    let x = x + bob;
    p.blob(f, p.glow, [x, y], [70., 48.], rgba(0x7fb2ff, 0.55 * a));
    triangle(
        p,
        f,
        [[x - 11., y - 12.], [x + 12., y], [x - 11., y + 12.]],
        [rgba(0xfff4c8, a), rgba(GOLD, a), rgba(GOLD_DEEP, a)],
    );
    p.blob(f, p.star, [x - 2., y], [26., 26.], rgba(0xffffff, 0.7 * a));
}

fn arrow(p: &mut Paint, f: &mut Ui2dFrame, [x, y]: [f32; 2], dir: f32, now: f32) {
    let y = y + dir * (now * 5.).sin().abs() * 3.;
    let c = rgba(INK_SOFT, 0.8);
    triangle(
        p,
        f,
        [
            [x - 10., y - 5. * dir],
            [x + 10., y - 5. * dir],
            [x, y + 6. * dir],
        ],
        [c; 3],
    );
}

fn triangle(p: &Paint, f: &mut Ui2dFrame, pts: [[f32; 2]; 3], colors: [Rgba; 3]) {
    let at = |[x, y]: [f32; 2]| {
        [
            256. - windows_shell::ui2d::WIDE_WIDTH / 2.
                + x * windows_shell::ui2d::WIDE_WIDTH / paint::W,
            y * windows_shell::ui2d::HEIGHT / paint::H,
        ]
    };
    f.draws.push((
        p.white,
        vec![package_reader::sequence::Quad {
            corners: [at(pts[0]), at(pts[1]), at(pts[2]), at(pts[1])],
            uv: [1, 1, 3, 3],
            colors: [colors[0], colors[1], colors[2], colors[1]],
            blend: 0,
        }],
    ));
}

fn panel_frame(p: &mut Paint, f: &mut Ui2dFrame, r: [f32; 4], a: f32) {
    let [x, y, w, h] = r;
    p.plate(
        f,
        [x - 2., y - 2., w + 4., h + 4.],
        20.,
        0.,
        [with_alpha(rgba(EDGE, 0.5), a); 4],
        false,
    );
    let (t, b) = (rgba(0x0a1a40, 0.92 * a), rgba(0x030918, 0.92 * a));
    p.plate(f, r, 18., 0., [t, t, b, b], false);
    let s = rgba(0x7fb2ff, 0.10 * a);
    p.plate(
        f,
        [x + 3., y + 3., w - 6., 46.],
        16.,
        0.,
        [s, s, with_alpha(s, 0.), with_alpha(s, 0.)],
        true,
    );
}

fn side_panel(l: &Launcher, p: &mut Paint, f: &mut Ui2dFrame) {
    let a = ease((l.now - l.entered) * 3.5);
    let [x, y, w, h] = PANEL;
    let (heading, body): (String, Vec<String>) = match &l.screen {
        Screen::Pick => (
            "The Game Folder".into(),
            vec![
                "Pick the folder Steam installed KINGDOM HEARTS -HD 1.5+2.5 ReMIX- in; it holds \"KINGDOM HEARTS II FINAL MIX.exe\".".into(),
                "You can also drop that folder onto this window.".into(),
                "The game files are only read, never changed. The first check reads every file once; after that, starting is quick.".into(),
            ],
        ),
        Screen::Browse(_) => (
            "Browsing".into(),
            vec![
                "Open folders until you reach the collection's install folder, then choose Use This Folder.".into(),
                "Folders marked KH2FM hold the game.".into(),
                "Steam usually installs it in steamapps\\common\\KINGDOM HEARTS -HD 1.5+2.5 ReMIX-.".into(),
            ],
        ),
        Screen::Tests(b) => match l.items.get(l.cursor).and_then(|i| i.action.clone()) {
            Some(Action::Run(i)) => (TESTS[i].name.into(), vec![TESTS[i].lines.join(" ")]),
            Some(Action::Pick) => (
                "Change Game Folder".into(),
                vec![format!("Currently: {}", plain(&b.folder))],
            ),
            _ => ("Quit".into(), vec!["Close the launcher.".into()]),
        },
        Screen::Problem { lines, .. } => ("What happened".into(), lines.clone()),
        _ => return,
    };
    panel_frame(p, f, PANEL, a);
    p.text(
        f,
        Face::Heading,
        26.,
        [x + 28., y + 18.],
        0.,
        &heading,
        [
            with_alpha(rgba(GOLD, 1.), a),
            with_alpha(rgba(GOLD_DEEP, 1.), a),
        ],
        0.6,
    );
    let mut ty = y + 70.;
    for para in &body {
        for line in wrap(p, Face::Body, 18., para, w - 56.) {
            if ty > y + h - 30. {
                break;
            }
            p.text(
                f,
                Face::Body,
                18.,
                [x + 28., ty],
                0.,
                &line,
                [with_alpha(rgba(0xd3e1fa, 1.), a); 2],
                0.4,
            );
            ty += 27.;
        }
        ty += 12.;
    }
    if let Screen::Tests(_) = l.screen {
        let controls = "Closing a test's window (or Esc) brings you back here.";
        let mut cy = y + h - 64.;
        for line in wrap(p, Face::Body, 15., controls, w - 56.) {
            p.text(
                f,
                Face::Body,
                15.,
                [x + 28., cy],
                0.,
                &line,
                [with_alpha(rgba(INK_DIM, 1.), a); 2],
                0.,
            );
            cy += 21.;
        }
    }
}

fn footer(l: &Launcher, p: &mut Paint, f: &mut Ui2dFrame) {
    let y = 668.;
    let (c, e) = (rgba(EDGE, 0.45), rgba(EDGE, 0.));
    p.rect(
        f,
        [
            MARGIN - 20.,
            y - 14.,
            (paint::W - 2. * MARGIN) / 2. + 20.,
            1.,
        ],
        [e, c, e, c],
        false,
    );
    p.rect(
        f,
        [
            paint::W / 2.,
            y - 14.,
            (paint::W - 2. * MARGIN) / 2. + 20.,
            1.,
        ],
        [c, e, c, e],
        false,
    );
    let hints = match l.screen {
        Screen::Tests(_) => "Enter or A  Start      F2 or Y  Folder      Esc or B  Quit",
        _ => "Enter or A  Select      Esc or B  Back",
    };
    p.text(
        f,
        Face::Body,
        17.,
        [paint::W - MARGIN, y],
        1.,
        hints,
        [rgba(INK_SOFT, 0.9); 2],
        0.,
    );
    let status = match &l.screen {
        Screen::Tests(b) => {
            let build = b
                .build_id
                .as_deref()
                .map_or_else(String::new, |id| format!("  ·  Steam build {id}"));
            format!("{}{build}", plain(&b.folder))
        }
        _ => String::new(),
    };
    let status = p.fit(Face::Body, 15., &status, 560.);
    p.text(
        f,
        Face::Body,
        15.,
        [MARGIN, y + 2.],
        0.,
        &status,
        [rgba(INK_DIM, 1.); 2],
        0.,
    );
}

fn spinner(l: &Launcher, p: &mut Paint, f: &mut Ui2dFrame, [cx, cy]: [f32; 2], r: f32) {
    for k in 0..12 {
        let ang = k as f32 / 12. * std::f32::consts::TAU;
        let head = (l.now * 1.6).fract();
        let d = ((k as f32 / 12. - head).rem_euclid(1.)) * 12.;
        let a = (1. - d / 12.).powi(2);
        let (x, y) = (cx + ang.cos() * r, cy + ang.sin() * r);
        p.blob(
            f,
            p.star,
            [x, y],
            [14. + 10. * a, 14. + 10. * a],
            rgba(0xbfe0ff, 0.15 + 0.85 * a),
        );
    }
}

fn check_screen(l: &Launcher, job: &Job, p: &mut Paint, f: &mut Ui2dFrame) {
    let a = ease((l.now - l.entered) * 3.);
    let r = [240., 250., 800., 300.];
    panel_frame(p, f, r, a);
    let scanning = job.scanning.load(Ordering::Relaxed);
    let title = if scanning {
        "Reading the game files"
    } else {
        "Checking the game folder"
    };
    p.text(
        f,
        Face::Heading,
        28.,
        [640., 276.],
        0.5,
        title,
        [
            with_alpha(rgba(INK, 1.), a),
            with_alpha(rgba(0x9fc2ff, 1.), a),
        ],
        0.6,
    );
    let path = p.fit(Face::Body, 16., &plain(&job.folder), 720.);
    p.text(
        f,
        Face::Body,
        16.,
        [640., 322.],
        0.5,
        &path,
        [with_alpha(rgba(INK_SOFT, 1.), a); 2],
        0.,
    );
    if scanning {
        let (done, total) = (
            job.progress.load(Ordering::Relaxed),
            job.total.load(Ordering::Relaxed).max(1),
        );
        let t = (done as f32 / total as f32).clamp(0., 1.);
        let bar = [320., 380., 640., 18.];
        p.plate(f, bar, 9., 0., [rgba(0x02060f, 0.95 * a); 4], false);
        if t > 0.005 {
            let fill = [bar[0], bar[1], (bar[2] * t).max(18.), bar[3]];
            p.plate(
                f,
                fill,
                9.,
                0.,
                [
                    rgba(0x6fb0ff, a),
                    rgba(BLUE, a),
                    rgba(0x2f74e6, a),
                    rgba(0x123a8c, a),
                ],
                false,
            );
            p.blob(
                f,
                p.glow,
                [bar[0] + bar[2] * t, bar[1] + 9.],
                [90., 50.],
                rgba(0x9cd0ff, 0.6 * a),
            );
        }
        let gb = |b: u64| b as f64 / (1u64 << 30) as f64;
        let line = format!(
            "{:.0}%   ·   {:.1} of {:.1} GB",
            t * 100.,
            gb(done),
            gb(total)
        );
        p.text(
            f,
            Face::Body,
            17.,
            [640., 410.],
            0.5,
            &line,
            [with_alpha(rgba(INK_SOFT, 1.), a); 2],
            0.,
        );
        p.text(
            f,
            Face::Body,
            15.,
            [640., 476.],
            0.5,
            "This happens once per game folder (and again after Steam updates the game).",
            [with_alpha(rgba(INK_DIM, 1.), a); 2],
            0.,
        );
    } else {
        spinner(l, p, f, [640., 440.], 34.);
    }
}

fn loading_screen(l: &Launcher, name: &str, p: &mut Paint, f: &mut Ui2dFrame) {
    let a = ease((l.now - l.entered) * 3.);
    p.text(
        f,
        Face::Heading,
        30.,
        [640., 300.],
        0.5,
        name,
        [
            with_alpha(rgba(INK, 1.), a),
            with_alpha(rgba(0x9fc2ff, 1.), a),
        ],
        0.6,
    );
    p.text(
        f,
        Face::Body,
        17.,
        [640., 348.],
        0.5,
        "Checking the game files and loading the test…",
        [with_alpha(rgba(INK_SOFT, 1.), a); 2],
        0.,
    );
    spinner(l, p, f, [640., 450.], 30.);
    // KH2's corner "NOW LOADING".
    let pulse = 0.65 + 0.35 * (l.now * 3.).sin();
    p.text(
        f,
        Face::Heading,
        26.,
        [paint::W - MARGIN, 640.],
        1.,
        "NOW LOADING",
        [
            with_alpha(rgba(INK, pulse), a),
            with_alpha(rgba(0x8fb6ff, pulse), a),
        ],
        0.6,
    );
}

/// Opens the launcher window. Returns when it closes.
pub fn main() -> std::process::ExitCode {
    let dir = install::data_dir();
    let settings = install::load_settings(&dir);
    App::new()
        .insert_resource(Launcher {
            dir,
            settings,
            supported: None,
            detected: Vec::new(),
            screen: Screen::Boot,
            items: Vec::new(),
            cursor: 0,
            scroll: 0,
            glow: Vec::new(),
            entered: 0.,
            now: 0.,
            fade: 1.,
            hover_drop: false,
            repeat: None,
            mouse: None,
            sparkles: Vec::new(),
            paint: None,
        })
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(WinitSettings::game())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "KINGDOM HEARTS II FINAL MIX - Final Re:WRITE".into(),
                resolution: WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(Ui2dPlugin {
            order: 0,
            overlay: false,
        })
        .add_systems(Update, (update, draw).chain())
        .run();
    std::process::ExitCode::SUCCESS
}
