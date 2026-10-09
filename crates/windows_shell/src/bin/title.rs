//! The KH2FM title screen from the selected Steam installation:
//! `menu/us/title.2ld` played by the translated layout player and title
//! task (`windows_shell::title`), drawn on the GPU in the original's 2D
//! space, with the system sounds from se000 and the exe's pad/keyboard
//! mapping (pad mode 7).
//!
//! Every launch rehashes the complete selected source once. Not built yet:
//! the title music (needs a Vorbis decoder), the file menu, the Theater and
//! the Game Settings overlay. Esc quits (on PC it opens Game Settings).
use bevy::{
    app::AppExit,
    input::mouse::MouseWheel,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use package_reader::probe::ProbeSource;
use package_reader::sequence::Quad;
use serde::Deserialize;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use windows_shell::command::Font;
use windows_shell::fade::{self, Fade};
use windows_shell::hud::{self, SystemFont};
use windows_shell::input;
use windows_shell::layout2d::{self, Menu2ld};
use windows_shell::newgame::{self, NewGame, Texts};
use windows_shell::pad::{self, Pad};
use windows_shell::sound::{self, Banks};
use windows_shell::sprite2d::Sheet;
use windows_shell::title::{Action, Event, State, Title};
use windows_shell::ui2d::{Ui2dFrame, Ui2dPlugin};

#[derive(Deserialize, Clone)]
struct Locator {
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
}

#[derive(Deserialize)]
struct Config {
    schema_version: u32,
    source_content_identity: String,
    /// menu/us/title.2ld.
    title: Locator,
    /// se/se000.win32.scd (system sounds).
    #[serde(default)]
    system_sounds: Option<Locator>,
    /// Play the first-boot splash (scene 8) before the title.
    #[serde(default)]
    first_boot: bool,
    /// msg/us/sys.bar, msg/us/fontimage.bar and msg/us/fontinfo.bar for the
    /// New Game dialog.
    messages: Locator,
    font_image: Locator,
    font_info: Locator,
}

/// The dialog's art: title.2ld image 0.
const DIALOG_IMAGE: usize = 0;
/// The fade quad at 16:9 (FUN_140156940: margin 85).
const FADE_RECT: [f32; 4] = [-86., -1., 598., 417.];

/// Title image 8's device-glyph alternates (FUN_1402b40d0): replacement N
/// for keyboard, Xbox, PlayStation and other pads.
const GLYPH_IMAGE: usize = 8;
const GLYPHS: [usize; 4] = [8, 9, 10, 11];

/// Unattended plan: presses by tick, captures by tick. Rows down and up,
/// New Game, Proud, vibration ON, back out of the confirm page, confirm.
const AUTOPLAY_PRESSES: &[(u64, u64)] = &[
    (700, pad_bits::DOWN),
    (740, pad_bits::UP),
    (780, pad_bits::DECIDE),
    (840, pad_bits::DOWN),
    (880, pad_bits::DECIDE),
    (940, pad_bits::DECIDE),
    (1000, pad_bits::CANCEL),
    (1060, pad_bits::DECIDE),
    (1120, pad_bits::DECIDE),
];
const AUTOPLAY_CAPTURES: &[u64] = &[
    60, 240, 450, 690, 750, 790, 800, 830, 845, 900, 970, 1010, 1080, 1125, 1135, 1200, 1300,
];
const AUTOPLAY_END: u64 = 1450;
const AUTOPLAY_WARMUP_FRAMES: u64 = 30;

mod pad_bits {
    pub use windows_shell::title::bits::*;
}

#[derive(Resource)]
struct Session {
    menu: Menu2ld,
    first_boot: bool,
    title: Option<Title>,
    dialog: Option<NewGame>,
    texts: Texts,
    font: SystemFont,
    font_sheet: usize,
    /// This frame's 2D draw (taken before the title task's logic, as the
    /// sprite draw task runs before it).
    snapshot: Vec<(usize, Vec<Quad>)>,
    fade: Fade,
    fade_drawn: bool,
    pad: Pad,
    wheel: f32,
    keyboard_last: bool,
    glyph: usize,
    /// Sheet slot per layout image (image 8 follows the device).
    image_sheets: Vec<usize>,
    glyph_sheets: [Option<usize>; 4],
    white: usize,
    banks: Banks,
    output: Option<audio_out::Output>,
    handoff: Option<Action>,
    tick: u64,
    rendered_frames: u64,
    log: Option<File>,
    limit: Option<u64>,
    autoplay: Option<PathBuf>,
    captures_taken: usize,
    capture_pending: bool,
    exit_code: Option<u8>,
}

impl Session {
    fn event(&mut self, value: serde_json::Value) {
        if let Some(log) = &mut self.log {
            serde_json::to_writer(&mut *log, &value).expect("title log write");
            writeln!(log).expect("title log newline");
            log.flush().expect("title log flush");
        }
    }

    fn play(&mut self, se: u32, cause: &str) {
        let resolved = self.banks.request(se).unwrap_or_else(|e| {
            eprintln!("Sound {se} failed to decode: {e}");
            None
        });
        let tick = self.tick;
        self.event(
            serde_json::json!({"event":"sound","tick":tick,"id":se,"cause":cause,
            "layers":resolved.as_ref().map(|r| r.layers.len())}),
        );
        if let (Some(output), Some(r)) = (self.output.as_ref(), resolved)
            && let Ok(mut mixer) = output.mixer.lock()
        {
            for layer in r.layers {
                mixer.play(layer, se, r.volume, 1.);
            }
        }
    }
}

fn read_limited(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(1_048_577).read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        return Err("selection exceeds 1 MiB".into());
    }
    Ok(bytes)
}

fn setup(
    mut session: ResMut<Session>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
) {
    let s = &mut *session;
    let count = s.menu.image_sizes.len();
    for k in 0..count {
        let t = s.menu.replacements[k].take().expect("checked at load");
        s.image_sheets
            .push(frame.add_sheet(&mut images, t.rgba, t.size, t.texel_scale));
    }
    for (slot, n) in GLYPHS.into_iter().enumerate() {
        if n == GLYPH_IMAGE {
            s.glyph_sheets[slot] = Some(s.image_sheets[GLYPH_IMAGE]);
        } else if let Some((t, scale)) = s.menu.texture_for(GLYPH_IMAGE, n) {
            let (rgba, size) = (t.rgba.clone(), t.size);
            s.glyph_sheets[slot] = Some(frame.add_sheet(&mut images, rgba, size, scale));
        }
    }
    s.white = frame.add_sheet(&mut images, vec![255; 4], [1, 1], 1.);
    s.font_sheet = frame.add_font_sheet(
        &mut images,
        std::mem::take(&mut s.font.rgba),
        s.font.size,
        s.font.texel_scale as f32,
    );
    // FUN_1402b4c60: fade.out(0), near-instant black; the task then fades
    // in over 0x3c.
    s.fade.fade_out(0, false);
    while s.fade.state != fade::State::Held {
        s.fade.update(1.);
    }
    s.fade.fade_in(0x3c, false);
}

fn collect_wheel(mut wheel: MessageReader<MouseWheel>, mut session: ResMut<Session>) {
    for w in wheel.read() {
        session.wheel += w.y;
    }
}

/// One 60 Hz title frame: input, the title task, layouts and fade.
fn tick(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<(&Gamepad, Option<&Name>)>,
    mut session: ResMut<Session>,
) {
    let s = &mut *session;
    s.tick += 1;
    let tick = s.tick;
    let (data, keyboard, pad_glyph) = input::read_devices(&keys, &mouse, s.wheel, &pads);
    let pad_glyph = pad_glyph.map(usize::from);
    s.wheel = 0.;
    let mut logical = pad::logical(pad::raw(&data), pad::MODE_TITLE);
    if s.autoplay.is_some() {
        logical = AUTOPLAY_PRESSES
            .iter()
            .filter(|(at, _)| *at == tick)
            .fold(0, |acc, (_, b)| acc | b);
    }
    s.pad.update(logical, 1.);
    let trig = s.pad.pressed;
    if trig != 0 {
        s.event(serde_json::json!({"event":"input","tick":tick,"trig":trig}));
    }
    // FUN_1402b40d0: keyboard activity selects the keyboard sheet.
    if keyboard {
        s.keyboard_last = true;
    } else if pad_glyph.is_some() {
        s.keyboard_last = false;
    }
    let glyph = if s.keyboard_last {
        0
    } else {
        pad_glyph.unwrap_or(s.glyph)
    };
    if glyph != s.glyph && s.glyph_sheets[glyph].is_some() {
        s.glyph = glyph;
        s.event(
            serde_json::json!({"event":"device_glyph","tick":tick,"replacement":GLYPHS[glyph]}),
        );
    }
    s.image_sheets[GLYPH_IMAGE] = s.glyph_sheets[s.glyph].expect("glyph sheet");

    // Sprite and layout tasks (priority ~0x7532) run first, then the 2D
    // draw (103000), then the title task (300000), then the fade (330000).
    if let Some(t) = s.title.as_mut() {
        t.update_layouts(&s.menu.layout, 1.);
    }
    let dialog_seq = &s.menu.layout.sequences[0];
    if let Some(d) = s.dialog.as_mut() {
        d.update_sprites(dialog_seq);
    }
    s.snapshot.clear();
    if let Some(t) = &s.title {
        for task in std::iter::once(&t.main).chain(t.cursor_layout.as_ref()) {
            for b in layout2d::draw(&s.menu, task, [0, 0]) {
                s.snapshot.push((s.image_sheets[b.image], b.quads));
            }
        }
    }
    if let Some(d) = &s.dialog {
        let font = Font {
            widths: &s.font.widths,
        };
        for (sheet, quads) in d.sprites.draw(dialog_seq, &font) {
            let slot = match sheet {
                Sheet::Art(image) => s.image_sheets[image],
                Sheet::Font => s.font_sheet,
            };
            s.snapshot.push((slot, quads));
        }
    }

    // FUN_1402b4590: the layouts start once the opening fade-in is done.
    if s.title.is_none() && s.handoff.is_none() && s.fade.state == fade::State::Idle {
        let t = Title::new(&s.menu.layout, s.first_boot, false);
        let scene = t.main.scene;
        s.title = Some(t);
        s.event(serde_json::json!({"event":"title_start","tick":tick,"scene":scene}));
        s.event(
            serde_json::json!({"event":"bgm","tick":tick,"path":"vagstream/Title.win32.scd",
            "played":false,"note":"no Vorbis decoder yet"}),
        );
    }
    if let Some(mut t) = s.title.take() {
        let before = (
            t.main.scene,
            t.cursor_layout.as_ref().map(|c| c.scene),
            t.state,
        );
        let mut events = Vec::new();
        if t.state == State::NewGameDialog {
            // FUN_1402b7420 runs inside the title task: the menu frame
            // pauses until the dialog returns.
            match s.dialog.as_mut() {
                None => {
                    // The modal starts the frame after the decide.
                    let mode = 1;
                    let mut d = NewGame::new(s.texts.clone(), DIALOG_IMAGE, mode, true);
                    let seq = &s.menu.layout.sequences[0];
                    for n in d.frame(seq, s.pad.pressed, s.pad.repeat) {
                        events.push(Event::Sound(n));
                    }
                    s.dialog = Some(d);
                }
                Some(d) => {
                    let seq = &s.menu.layout.sequences[0];
                    for n in d.frame(seq, s.pad.pressed, s.pad.repeat) {
                        events.push(Event::Sound(n));
                    }
                    if d.finished {
                        let (confirmed, values) = (d.confirmed, d.values);
                        s.dialog = None;
                        s.event(serde_json::json!({"event":"new_game_dialog","tick":tick,
                            "confirmed":confirmed,"mode":values[0],"vibration_on":values[1] == 0}));
                        events.extend(t.new_game_result(confirmed));
                    }
                }
            }
        } else {
            events = t.frame(&s.menu.layout, trig, 1.);
        }
        for e in events {
            match e {
                Event::Sound(n) => s.play(n, "title"),
                Event::FadeOut(n) => s.fade.fade_out(n, false),
                Event::Leave(a) => {
                    s.handoff = Some(a);
                    s.event(serde_json::json!({"event":"leave","tick":tick,"action":format!("{a:?}")}));
                }
                other => s.event(serde_json::json!({"event":"title_event","tick":tick,"what":format!("{other:?}")})),
            }
        }
        let after = (
            t.main.scene,
            t.cursor_layout.as_ref().map(|c| c.scene),
            t.state,
        );
        if after != before {
            s.event(
                serde_json::json!({"event":"scene","tick":tick,"main":after.0,"cursor":after.1,
                "state":format!("{:?}",after.2),"cursor_row":t.cursor}),
            );
        }
        // Back (state 3) waits for black, then returns to the launcher.
        let black = s.fade.state == fade::State::Held;
        let back_done = t.state == State::Back && black;
        let left = match s.handoff {
            Some(Action::Demo) => black,
            Some(_) => t.main.dead,
            None => false,
        };
        if back_done || left {
            let a = s.handoff.unwrap_or(Action::Back);
            s.event(
                serde_json::json!({"event":"handoff","tick":tick,"action":format!("{a:?}"),
                "note":"next screen not built"}),
            );
            s.handoff = Some(a);
        } else {
            s.title = Some(t);
        }
    }
    s.fade_drawn = s.fade.update(1.);
}

/// This frame's layouts and dialog, then the fade on top.
fn draw(session: Res<Session>, mut frame: ResMut<Ui2dFrame>) {
    frame.draws.extend(session.snapshot.iter().cloned());
    if session.fade_drawn {
        let ([r, g, b, a], additive) = session.fade.colour();
        let [x0, y0, x1, y1] = FADE_RECT;
        frame.draws.push((
            session.white,
            vec![Quad {
                corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
                uv: [0, 0, 1, 1],
                colors: [[r, g, b, a]; 4],
                blend: additive as i32,
            }],
        ));
    }
}
fn finish(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut time: ResMut<Time<Virtual>>,
    mut closed: MessageReader<bevy::window::WindowCloseRequested>,
    mut exit: MessageWriter<AppExit>,
) {
    session.rendered_frames += 1;
    if session.autoplay.is_some()
        && time.is_paused()
        && !session.capture_pending
        && session.rendered_frames >= AUTOPLAY_WARMUP_FRAMES
    {
        time.unpause();
    }
    if let Some(dir) = session.autoplay.clone()
        && !session.capture_pending
        && let Some(&at) = AUTOPLAY_CAPTURES.get(session.captures_taken)
        && session.tick >= at
    {
        session.capture_pending = true;
        time.pause();
        let index = session.captures_taken;
        let path = dir.join(format!("title-{index:02}.png"));
        let tick = session.tick;
        commands.spawn(Screenshot::primary_window()).observe(
            move |event: On<ScreenshotCaptured>,
                  mut session: ResMut<Session>,
                  mut time: ResMut<Time<Virtual>>| {
                let image = event
                    .image
                    .clone()
                    .try_into_dynamic()
                    .expect("title capture");
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .expect("new private capture");
                image
                    .write_to(&mut file, image::ImageFormat::Png)
                    .expect("capture encode");
                file.sync_all().expect("capture flush");
                session.capture_pending = false;
                session.captures_taken += 1;
                time.unpause();
                session.event(serde_json::json!({"event":"capture","index":index,"tick":tick}));
            },
        );
    }
    let autoplay_done = session.autoplay.is_some()
        && session.tick >= AUTOPLAY_END
        && session.captures_taken == AUTOPLAY_CAPTURES.len();
    let window_closed = closed.read().count() > 0;
    let handed_off =
        session.handoff.is_some() && session.title.is_none() && session.autoplay.is_none();
    if keys.just_pressed(KeyCode::Escape)
        || window_closed
        || autoplay_done
        || handed_off
        || session.limit.is_some_and(|n| session.tick >= n)
    {
        let code = u8::from(session.autoplay.is_some() && !autoplay_done);
        if session.exit_code.is_none() {
            session.exit_code = Some(code);
            let tick = session.tick;
            session.event(serde_json::json!({"event":"exit","tick":tick,"code":code}));
            exit.write(if code == 0 {
                AppExit::Success
            } else {
                AppExit::Error(std::num::NonZeroU8::new(code).unwrap())
            });
        }
    }
}

fn pause_for_autoplay(session: Res<Session>, mut time: ResMut<Time<Virtual>>) {
    if session.autoplay.is_some() {
        time.pause();
    }
}

fn launch_file_args() -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    #[derive(Deserialize)]
    struct Launch {
        steam_root: PathBuf,
        manifest: PathBuf,
        config: PathBuf,
        #[serde(default)]
        options: Vec<String>,
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        for up in exe.ancestors().skip(1).take(4) {
            candidates.push(up.join(".local/test-room/launch-title.json"));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("KH2FinalRewrite/test-room-launch-title.json"));
    }
    let path = candidates
        .iter()
        .find(|p| p.is_file())
        .ok_or("no arguments and no launch-title.json launch file")?;
    let launch: Launch = serde_json::from_slice(&read_limited(path)?)?;
    let mut args = vec![launch.steam_root, launch.manifest, launch.config];
    args.extend(launch.options.into_iter().map(PathBuf::from));
    Ok(args)
}

fn run() -> Result<AppExit, Box<dyn std::error::Error>> {
    let mut args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.is_empty() {
        args = launch_file_args()?;
    }
    if args.len() < 3 {
        return Err("usage: title <Steam-root> <manifest> <title-config> [--log new.jsonl] [--frames N] [--autoplay <new-capture-dir>]".into());
    }
    let (mut log_path, mut limit, mut autoplay) = (None, None, None);
    let mut options = args[3..].iter();
    while let Some(option) = options.next() {
        match option.to_str() {
            Some("--log") if log_path.is_none() => log_path = options.next().cloned(),
            Some("--frames") if limit.is_none() => {
                limit = Some(
                    options
                        .next()
                        .and_then(|v| v.to_str())
                        .ok_or("frame limit absent")?
                        .parse::<u64>()?,
                )
            }
            Some("--autoplay") if autoplay.is_none() => autoplay = options.next().cloned(),
            _ => return Err("unknown/duplicate title option".into()),
        }
    }
    let source = ProbeSource::bind_session(&args[0], &args[1])?;
    let log_path = match log_path {
        Some(path) => {
            let path = source.output_path(&path)?;
            if path.exists() {
                return Err("title log must be new".into());
            }
            Some(path)
        }
        None => None,
    };
    if let Some(dir) = &autoplay {
        source.output_path(&dir.join("title-00.png"))?;
        if !dir.is_dir() || std::fs::read_dir(dir)?.next().is_some() {
            return Err("autoplay capture directory must exist and be empty".into());
        }
    }
    eprintln!("Validating the selected KH2FM source (full rehash), then decoding the title...");
    let config: Config = serde_json::from_slice(&read_limited(&args[2])?)?;
    if config.schema_version != 1 || config.source_content_identity != source.content_identity() {
        return Err("title config does not match the selected source".into());
    }
    let l = &config.title;
    let menu = layout2d::load(&source, &l.index, l.ordinal, l.name_hash)?;
    let mut banks = Banks::default();
    if let Some(l) = &config.system_sounds {
        banks.insert(sound::load_bank(
            &source,
            &l.index,
            l.ordinal,
            l.name_hash,
            None,
        )?)?;
    }
    let m = &config.messages;
    let texts = Texts::from_messages(hud::load_messages(
        &source,
        &m.index,
        m.ordinal,
        m.name_hash,
        &newgame::message_ids(),
    )?);
    let (fi, fo) = (&config.font_image, &config.font_info);
    let font = hud::load_font(
        &source,
        (&fi.index, fi.ordinal, fi.name_hash),
        (&fo.index, fo.ordinal, fo.name_hash),
        b"sys\0",
    )?;
    let mut log = match log_path {
        Some(path) => Some(OpenOptions::new().write(true).create_new(true).open(path)?),
        None => None,
    };
    if let Some(log) = &mut log {
        let summary = serde_json::json!({"event":"loaded","props":menu.layout.props.len(),
            "scenes":menu.layout.scenes.len(),"sequences":menu.layout.sequences.len(),
            "images":menu.image_sizes,"replacements":menu.replacements.iter()
                .map(|r| r.as_ref().map(|t| t.size)).collect::<Vec<_>>(),
            "banks":banks.numbers()});
        serde_json::to_writer(&mut *log, &summary)?;
        writeln!(log)?;
    }
    let output = match audio_out::Output::open() {
        Ok(o) => Some(o),
        Err(e) => {
            eprintln!("Sound output unavailable: {e}");
            None
        }
    };
    let session = Session {
        menu,
        first_boot: config.first_boot,
        title: None,
        dialog: None,
        texts,
        font,
        font_sheet: 0,
        snapshot: Vec::new(),
        fade: Fade::default(),
        fade_drawn: false,
        pad: Pad::default(),
        wheel: 0.,
        keyboard_last: true,
        glyph: 0,
        image_sheets: Vec::new(),
        glyph_sheets: [None; 4],
        white: 0,
        banks,
        output,
        handoff: None,
        tick: 0,
        rendered_frames: 0,
        log,
        limit,
        autoplay,
        captures_taken: 0,
        capture_pending: false,
        exit_code: None,
    };
    eprintln!(
        "Title ready. Arrows/W/S or the D-pad move, Enter/left-click/A decides, Space/Backspace/B backs out, M/Start skips the intro, Esc quits."
    );
    Ok(App::new()
        .insert_resource(session)
        .insert_resource(Time::<Fixed>::from_hz(60.))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "KH2FM title (quarantined source)".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            exit_condition: bevy::window::ExitCondition::DontExit,
            close_when_requested: false,
            ..default()
        }))
        .add_plugins(Ui2dPlugin {
            order: 0,
            overlay: false,
        })
        .add_systems(Startup, (setup, pause_for_autoplay))
        .add_systems(PreUpdate, collect_wheel.after(bevy::input::InputSystems))
        .add_systems(FixedUpdate, tick)
        .add_systems(Update, (draw, finish).chain())
        .run())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(AppExit::Success) => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(e) => {
            eprintln!("Title rejected: {e}");
            if std::env::args_os().len() == 1 {
                eprintln!("Press Enter to close.");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
            std::process::ExitCode::FAILURE
        }
    }
}
