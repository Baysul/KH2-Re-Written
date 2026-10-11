//! J: Sora takes the skateboard. Skate 3's riding comes from the user's own
//! Skate 3 Xbox 360 disc: the core riding data is prepared from it once
//! (skate-disc), then the Skate 3 skater simulation (skate-sim, both
//! GPL-3.0-only) runs on the room's collision and drives Sora. Without an
//! admitted disc the key refuses with a notice and nothing changes.
use bevy::prelude::*;
#[cfg(feature = "skate3")]
use skate_sim::{Difficulty, Sim, SimTerrain, XboxState};
use skate_source::{Disc, Rejection};
#[cfg(feature = "skate3")]
use std::thread::JoinHandle;

/// How long a notice stays up (frames at 60 Hz).
const NOTICE_FRAMES: u32 = 300;
/// Scene units per Skate metre (test-room choice: Sora's model is 171
/// units tall, about the Skate 3 rider's height in metres x 100).
#[cfg(feature = "skate3")]
pub const UNITS_PER_METRE: f32 = 100.;
/// A deck's size (scene units): width, thickness, length of a common
/// 8.0 x 31.5 inch deck (the Keyblade board is scaled to its length; the
/// plain board is the fallback), and its top above the deck frame.
pub const DECK_SIZE: [f32; 3] = [20.3, 1.5, 80.];
pub const DECK_TOP: f32 = 0.75;

enum State {
    Off,
    /// The disc is being converted on a worker thread.
    #[cfg(feature = "skate3")]
    Preparing(JoinHandle<(Disc, Result<(), String>)>),
    /// Prepared; the test room starts the ride on its next tick.
    #[cfg(feature = "skate3")]
    Starting,
    #[cfg(feature = "skate3")]
    Riding(Box<Sim>),
}

#[derive(Resource)]
pub struct Skate {
    disc: Option<Disc>,
    state: State,
    /// The deck after the last step (scene coordinates).
    deck: Option<Transform>,
    /// The rider after the last step: its root (scene coordinates) and
    /// bone matrices relative to it (scene units).
    rider: Option<(Transform, Vec<Mat4>)>,
    /// The rider's bones before the first step (its rest pose).
    rider_rest: Vec<Mat4>,
    /// Skate 3's gameplay camera after the last step (scene coordinates)
    /// and its vertical field of view (radians).
    camera: Option<(Transform, f32)>,
    /// The window's aspect ratio, fed to the Skate 3 camera.
    pub aspect: f32,
    /// Skate 3's score HUD, when its data was prepared from the disc.
    #[cfg(feature = "skate3")]
    hud: Option<skate_sim::host_hud::ScoreHud>,
    /// The notice line (system-font bytes) and its remaining frames.
    pub notice: Option<(Vec<u8>, u32)>,
    /// The latest pad state for the simulation (read every frame).
    #[cfg(feature = "skate3")]
    pad: XboxState,
}

impl Default for Skate {
    fn default() -> Self {
        Self {
            disc: None,
            state: State::Off,
            deck: None,
            rider: None,
            rider_rest: Vec::new(),
            camera: None,
            aspect: 16. / 9.,
            #[cfg(feature = "skate3")]
            hud: None,
            notice: None,
            #[cfg(feature = "skate3")]
            pad: XboxState::default(),
        }
    }
}

/// Where the ride puts Sora and the board this tick (scene coordinates).
pub struct Ride {
    pub position: Vec3,
    pub yaw: f32,
}

impl Skate {
    fn say(&mut self, text: &str) {
        self.notice = Some((encode(text), NOTICE_FRAMES));
    }

    pub fn riding(&self) -> bool {
        #[cfg(feature = "skate3")]
        return matches!(self.state, State::Riding(_));
        #[cfg(not(feature = "skate3"))]
        false
    }

    pub fn wants_start(&self) -> bool {
        #[cfg(feature = "skate3")]
        return matches!(self.state, State::Starting);
        #[cfg(not(feature = "skate3"))]
        false
    }

    /// Starts riding at Sora's position and facing on the room's collision.
    #[cfg(feature = "skate3")]
    pub fn start(&mut self, triangles: &[[Vec3; 3]], position: Vec3, yaw: f32) {
        let metres = |v: Vec3| (v / UNITS_PER_METRE).to_array();
        let terrain = SimTerrain::Host {
            triangles: triangles.iter().map(|t| t.map(metres)).collect(),
            spawn: metres(position),
            heading: yaw,
        };
        match Sim::load_with(
            &crate::launcher::skate3_cache(),
            terrain,
            Difficulty::default(),
        ) {
            Ok(sim) => {
                self.rider_rest = rider_bones(&sim);
                self.hud = match skate_sim::host_hud::ScoreHud::load(&hud_root(), &sim) {
                    Ok(hud) => Some(hud),
                    Err(e) => {
                        eprintln!("Skate 3 score HUD not loaded: {e}");
                        None
                    }
                };
                self.state = State::Riding(Box::new(sim));
                self.say("Skateboarding. Space pushes, arrows flick tricks, J gets off.");
            }
            Err(e) => {
                eprintln!("Skate 3 simulation refused: {e}");
                self.state = State::Off;
                self.say("The Skate 3 skater could not be set up. See the log.");
            }
        }
    }

    #[cfg(not(feature = "skate3"))]
    pub fn start(&mut self, _: &[[Vec3; 3]], _: Vec3, _: f32) {}

    pub fn stop(&mut self) {
        self.state = State::Off;
        self.deck = None;
        self.rider = None;
        self.camera = None;
    }

    /// Skate 3's score HUD and the folder its textures are in, while riding.
    #[cfg(feature = "skate3")]
    pub fn score_hud(
        &mut self,
    ) -> Option<(&mut skate_sim::host_hud::ScoreHud, std::path::PathBuf)> {
        if !self.riding() {
            return None;
        }
        self.hud.as_mut().map(|hud| (hud, hud_root()))
    }

    /// Whether Skate 3's score HUD is shown in place of the KH2 HUD.
    pub fn replaces_hud(&self) -> bool {
        #[cfg(feature = "skate3")]
        return self.riding() && self.hud.is_some();
        #[cfg(not(feature = "skate3"))]
        false
    }

    /// Skate 3's camera while riding.
    pub fn camera(&self) -> Option<(Transform, f32)> {
        self.camera.filter(|_| self.riding())
    }

    /// The rider's root, bone matrices and rest bones while riding.
    pub fn rider(&self) -> Option<(&Transform, &[Mat4], &[Mat4])> {
        self.rider
            .as_ref()
            .filter(|_| self.riding())
            .map(|(root, bones)| (root, bones.as_slice(), self.rider_rest.as_slice()))
    }

    /// Where the board is while riding.
    pub fn deck(&self) -> Option<Transform> {
        self.riding().then_some(self.deck).flatten()
    }

    /// One 60 Hz simulation step; None when not riding.
    #[cfg(feature = "skate3")]
    pub fn step(&mut self) -> Option<Ride> {
        let pad = self.pad;
        let State::Riding(sim) = &mut self.state else {
            return None;
        };
        sim.set_camera_aspect_ratio(self.aspect);
        if let Err(e) = sim.tick(Some(pad), true) {
            eprintln!("Skate 3 simulation stopped: {e}");
            self.stop();
            self.say("The skateboard stopped. See the log.");
            return None;
        }
        let deck = sim.deck_transform();
        let root = sim.skater_root();
        let forward = deck.rotation * Vec3::Z;
        let deck = Transform {
            translation: deck.translation * UNITS_PER_METRE,
            rotation: deck.rotation,
            scale: Vec3::ONE,
        };
        self.deck = Some(deck);
        let root = Transform {
            translation: root.translation * UNITS_PER_METRE,
            rotation: root.rotation,
            scale: Vec3::ONE,
        };
        self.rider = Some((root, rider_bones(sim)));
        if let Some(hud) = &mut self.hud
            && let Err(e) = hud.update(sim)
        {
            eprintln!("Skate 3 score HUD stopped: {e}");
        }
        self.camera = sim.camera().map(|(mut t, fov)| {
            t.translation *= UNITS_PER_METRE;
            (t, fov)
        });
        Some(Ride {
            position: root.translation,
            yaw: forward.x.atan2(forward.z),
        })
    }

    #[cfg(not(feature = "skate3"))]
    pub fn step(&mut self) -> Option<Ride> {
        None
    }
}

/// Where the score HUD's prepared data is.
#[cfg(feature = "skate3")]
fn hud_root() -> std::path::PathBuf {
    crate::launcher::skate3_cache().join("private/hud")
}

/// The rider's model-space bones with translations in scene units.
#[cfg(feature = "skate3")]
fn rider_bones(sim: &Sim) -> Vec<Mat4> {
    sim.pose()
        .iter()
        .map(|b| {
            let mut m = b.model;
            m.w_axis = (m.w_axis.truncate() * UNITS_PER_METRE).extend(1.);
            m
        })
        .collect()
}

/// ASCII into the system font's international encoding: upper case and
/// digits as `camp::encode_number` (FUN_1403adcc0), lower case 0x9a..0xb3
/// and the marks after OpenKH's InternationalSystemDecode table; anything
/// else is a space.
fn encode(text: &str) -> Vec<u8> {
    text.bytes()
        .map(|b| match b {
            b'a'..=b'z' => 0x9a + (b - b'a'),
            b'.' => 0x4f,
            b',' => 0x50,
            b'?' => 0x49,
            b'-' => 0x66,
            b'0'..=b'9' | b'A'..=b'Z' | b'!' | b'\'' => {
                windows_shell::camp::encode_number(&char::from(b).to_string())[0]
            }
            _ => 0x01,
        })
        .collect()
}

/// The on-screen line for a refusal; the full reason goes to the log.
fn refusal(e: &Rejection) -> &'static str {
    match e {
        Rejection::NotConfigured => "Skateboarding needs your Skate 3 ISO. Set it in the launcher.",
        Rejection::Io(..) => "The Skate 3 ISO set in the launcher could not be opened.",
        Rejection::NotXboxImage | Rejection::Malformed(_) => {
            "The Skate 3 ISO set in the launcher is not a readable Xbox 360 disc."
        }
        Rejection::Missing(_) | Rejection::NotXex => "The disc set in the launcher is not Skate 3.",
    }
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut skate: ResMut<Skate>) {
    if let Some((_, frames)) = &mut skate.notice {
        *frames = frames.saturating_sub(1);
        if *frames == 0 {
            skate.notice = None;
        }
    }
    #[cfg(feature = "skate3")]
    {
        skate.pad = read_pad(&keys, &pads);
        poll_preparation(&mut skate);
    }
    #[cfg(not(feature = "skate3"))]
    let _ = pads;
    if !keys.just_pressed(KeyCode::KeyJ) {
        return;
    }
    #[cfg(feature = "skate3")]
    match skate.state {
        State::Off => {}
        State::Preparing(_) => return,
        State::Starting | State::Riding(_) => {
            skate.stop();
            skate.say("Off the skateboard.");
            return;
        }
    }
    let iso = crate::launcher::skate3_iso();
    if skate.disc.is_none() {
        match skate_source::admit(iso.as_deref()) {
            Ok(disc) => skate.disc = Some(disc),
            Err(e) => {
                eprintln!("Skate 3 disc refused: {e}");
                return skate.say(refusal(&e));
            }
        }
    }
    #[cfg(not(feature = "skate3"))]
    skate.say("Skateboarding is not installed in this build.");
    #[cfg(feature = "skate3")]
    prepare_or_start(&mut skate, iso.as_deref());
}

/// A finished preparation starts the ride or reports why it cannot.
#[cfg(feature = "skate3")]
fn poll_preparation(skate: &mut Skate) {
    if matches!(&skate.state, State::Preparing(worker) if worker.is_finished())
        && let State::Preparing(worker) = std::mem::replace(&mut skate.state, State::Off)
    {
        let Ok((disc, result)) = worker.join() else {
            skate.say("The Skate 3 disc could not be read. See the log.");
            return;
        };
        skate.disc = Some(disc);
        match result {
            Ok(()) => skate.state = State::Starting,
            Err(e) => {
                eprintln!("Skate 3 data not prepared: {e}");
                skate.say("The Skate 3 disc could not be read. See the log.");
            }
        }
    }
}

/// Starts riding from prepared data, or prepares it from the disc first
/// (once per disc, on a worker thread).
#[cfg(feature = "skate3")]
fn prepare_or_start(skate: &mut Skate, iso: Option<&std::path::Path>) {
    let cache = crate::launcher::skate3_cache();
    let identity = disc_identity(iso);
    if skate_disc::is_prepared(&cache, &identity) {
        skate.state = State::Starting;
        return;
    }
    let Some(disc) = skate.disc.take() else {
        return;
    };
    let worker = std::thread::spawn(move || {
        let mut host = DiscHost(disc);
        let result =
            skate_disc::prepare(&mut host, &cache, &identity, |m| eprintln!("Skate 3: {m}"))
                .map_err(|e| e.to_string());
        // The score HUD is optional: riding works without it.
        if result.is_ok() {
            eprintln!("Skate 3: Preparing the score HUD");
            let hud = std::fs::read(cache.join("private/stock/skater-collections.json"))
                .map_err(|e| e.to_string())
                .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
                .and_then(|c| {
                    skate_disc::prepare_hud(&mut host, &c, &cache).map_err(|e| e.to_string())
                });
            if let Err(e) = hud {
                eprintln!("Skate 3 score HUD not prepared: {e}");
            }
        }
        (host.0, result)
    });
    skate.state = State::Preparing(worker);
    skate.say("Preparing Skate 3 data from your disc...");
}

/// The disc's path, size and modification time: a new or changed ISO is
/// prepared again.
#[cfg(feature = "skate3")]
fn disc_identity(iso: Option<&std::path::Path>) -> String {
    let Some(iso) = iso else {
        return String::new();
    };
    let meta = std::fs::metadata(iso).ok();
    let size = meta.as_ref().map_or(0, |m| m.len());
    let modified = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    format!("{}|{size}|{modified}", iso.display())
}

/// skate-disc's file source over the admitted disc.
#[cfg(feature = "skate3")]
struct DiscHost(Disc);

#[cfg(feature = "skate3")]
struct DiscFile<'a> {
    image: &'a mut skate_source::xdvdfs::Image<std::fs::File>,
    entry: skate_source::xdvdfs::Entry,
}

#[cfg(feature = "skate3")]
impl skate_disc::ReadAt for DiscFile<'_> {
    fn len(&self) -> u64 {
        self.entry.size as u64
    }
    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, skate_disc::Error> {
        self.image
            .read(&self.entry, offset, len)
            .map_err(|e| skate_disc::Error::Io(std::io::Error::other(e.to_string())))
    }
}

#[cfg(feature = "skate3")]
struct FolderFile(std::fs::File, u64);

#[cfg(feature = "skate3")]
impl skate_disc::ReadAt for FolderFile {
    fn len(&self) -> u64 {
        self.1
    }
    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, skate_disc::Error> {
        use std::io::{Read, Seek, SeekFrom};
        self.0.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; len];
        self.0.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

#[cfg(feature = "skate3")]
fn io(e: skate_source::xdvdfs::Error) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

#[cfg(feature = "skate3")]
impl skate_disc::DiscFiles for DiscHost {
    fn open(
        &mut self,
        path: &str,
    ) -> Result<Option<Box<dyn skate_disc::ReadAt + '_>>, skate_disc::Error> {
        match &mut self.0 {
            Disc::Image(image) => match image.find(path).map_err(io)? {
                Some(entry) if !entry.directory => Ok(Some(Box::new(DiscFile { image, entry }))),
                _ => Ok(None),
            },
            Disc::Folder(root) => {
                let file = root.join(path);
                if !file.is_file() {
                    return Ok(None);
                }
                let handle = std::fs::File::open(&file)?;
                let len = handle.metadata()?.len();
                Ok(Some(Box::new(FolderFile(handle, len))))
            }
        }
    }

    fn list(&mut self, dir: &str) -> Result<Vec<String>, skate_disc::Error> {
        match &mut self.0 {
            Disc::Image(image) => match image.find(dir).map_err(io)? {
                Some(entry) if entry.directory => Ok(image
                    .list(&entry)
                    .map_err(io)?
                    .into_iter()
                    .filter(|e| !e.directory)
                    .map(|e| e.name)
                    .collect()),
                _ => Ok(Vec::new()),
            },
            Disc::Folder(root) => Ok(std::fs::read_dir(root.join(dir))
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()),
        }
    }
}

/// Keyboard and pad as an XInput pad. Keyboard: WASD the left stick, arrows
/// the right stick (flicks), Space A (push), Left Shift B, F X, R Y, Z / C
/// the bumpers, Q / E the triggers.
#[cfg(feature = "skate3")]
fn read_pad(keys: &ButtonInput<KeyCode>, pads: &Query<&Gamepad>) -> XboxState {
    let mut s = XboxState::default();
    let axis = |neg: KeyCode, pos: KeyCode| {
        ((keys.pressed(pos) as i32 - keys.pressed(neg) as i32) * i16::MAX as i32) as i16
    };
    s.left = [
        axis(KeyCode::KeyA, KeyCode::KeyD),
        axis(KeyCode::KeyS, KeyCode::KeyW),
    ];
    s.right = [
        axis(KeyCode::ArrowLeft, KeyCode::ArrowRight),
        axis(KeyCode::ArrowDown, KeyCode::ArrowUp),
    ];
    for (key, bit) in [
        (KeyCode::Space, 0x1000),
        (KeyCode::ShiftLeft, 0x2000),
        (KeyCode::KeyF, 0x4000),
        (KeyCode::KeyR, 0x8000),
        (KeyCode::KeyZ, 0x0100),
        (KeyCode::KeyC, 0x0200),
    ] {
        if keys.pressed(key) {
            s.buttons |= bit;
        }
    }
    s.triggers = [KeyCode::KeyQ, KeyCode::KeyE].map(|k| if keys.pressed(k) { 255 } else { 0 });
    if let Some(pad) = pads.iter().next() {
        let stick =
            |a: GamepadAxis| (pad.get(a).unwrap_or(0.).clamp(-1., 1.) * i16::MAX as f32) as i16;
        let merge = |k: i16, p: i16| if k != 0 { k } else { p };
        s.left = [
            merge(s.left[0], stick(GamepadAxis::LeftStickX)),
            merge(s.left[1], stick(GamepadAxis::LeftStickY)),
        ];
        s.right = [
            merge(s.right[0], stick(GamepadAxis::RightStickX)),
            merge(s.right[1], stick(GamepadAxis::RightStickY)),
        ];
        for (button, bit) in [
            (GamepadButton::DPadUp, 0x0001),
            (GamepadButton::DPadDown, 0x0002),
            (GamepadButton::DPadLeft, 0x0004),
            (GamepadButton::DPadRight, 0x0008),
            (GamepadButton::Start, 0x0010),
            (GamepadButton::Select, 0x0020),
            (GamepadButton::LeftThumb, 0x0040),
            (GamepadButton::RightThumb, 0x0080),
            (GamepadButton::LeftTrigger, 0x0100),
            (GamepadButton::RightTrigger, 0x0200),
            (GamepadButton::South, 0x1000),
            (GamepadButton::East, 0x2000),
            (GamepadButton::West, 0x4000),
            (GamepadButton::North, 0x8000),
        ] {
            if pad.pressed(button) {
                s.buttons |= bit;
            }
        }
        let trigger = |b: GamepadButton| (pad.get(b).unwrap_or(0.).clamp(0., 1.) * 255.) as u8;
        s.triggers = [
            s.triggers[0].max(trigger(GamepadButton::LeftTrigger2)),
            s.triggers[1].max(trigger(GamepadButton::RightTrigger2)),
        ];
    }
    s
}
