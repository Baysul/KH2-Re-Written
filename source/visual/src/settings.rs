//! The rebuild's own settings, shown as extra pages of the camp's Config
//! screen (Graphics, Sound, Other) after the exe's page. They are PC
//! conveniences, not the exe's options: the exe's Config rows stay on page 0
//! with their save bits. Every value applies while the menu is open.
use serde::{Deserialize, Serialize};

/// Page names; page 0 is the exe's Config.
pub const PAGES: [&str; 4] = ["Game", "Graphics", "Sound", "Other"];

/// Render scale steps, percent of the window's size.
pub const RENDER_SCALES: [u16; 6] = [50, 75, 100, 125, 150, 200];

/// Text languages an install may hold (`msg/<code>/sys.bar`): code and the
/// name shown. The caller offers the ones the selected source has.
pub const LANGUAGES: [(&str, &str); 6] = [
    ("us", "English"),
    ("fr", "Francais"),
    ("gr", "Deutsch"),
    ("it", "Italiano"),
    ("sp", "Espanol"),
    ("jp", "Japanese"),
];

/// Display mode: a window, a borderless window over the monitor, or
/// exclusive fullscreen.
pub const DISPLAY_MODES: [&str; 3] = ["Window", "Borderless", "Fullscreen"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Index into `DISPLAY_MODES`.
    pub display: u8,
    pub vsync: bool,
    /// Index into `RENDER_SCALES`.
    pub render_scale: u8,
    /// 4x multisampling on the 3D scene.
    pub anti_aliasing: bool,
    /// 1..=10, 5 unchanged.
    pub brightness: u8,
    /// 0..=10.
    pub master_volume: u8,
    pub music_volume: u8,
    pub effects_volume: u8,
    /// Keep sound playing while the window is not focused.
    pub background_audio: bool,
    /// A `LANGUAGES` code.
    pub language: String,
    pub show_hud: bool,
    /// Mouse camera turn speed, 1..=10, 5 the room's default.
    pub camera_speed: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            display: 0,
            vsync: true,
            render_scale: 2,
            anti_aliasing: false,
            brightness: 5,
            master_volume: 10,
            music_volume: 10,
            effects_volume: 10,
            background_audio: true,
            language: "us".into(),
            show_hud: true,
            camera_speed: 5,
        }
    }
}

/// One settings row as shown: label, choice labels, the chosen one, help.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub label: &'static str,
    pub choices: Vec<String>,
    pub selected: usize,
    pub help: &'static str,
}

fn on_off(on: bool) -> (Vec<String>, usize) {
    (vec!["On".into(), "Off".into()], usize::from(!on))
}

fn numbers(range: std::ops::RangeInclusive<u8>, value: u8) -> (Vec<String>, usize) {
    let start = *range.start();
    (
        range.map(|n| n.to_string()).collect(),
        value.saturating_sub(start) as usize,
    )
}

impl Settings {
    /// Reads `path`; a missing or unreadable file gives the defaults.
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
            .map(Settings::clamped)
            .unwrap_or_default()
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    fn clamped(mut self) -> Self {
        self.display = self.display.min(DISPLAY_MODES.len() as u8 - 1);
        self.render_scale = self.render_scale.min(RENDER_SCALES.len() as u8 - 1);
        self.brightness = self.brightness.clamp(1, 10);
        self.master_volume = self.master_volume.min(10);
        self.music_volume = self.music_volume.min(10);
        self.effects_volume = self.effects_volume.min(10);
        self.camera_speed = self.camera_speed.clamp(1, 10);
        self
    }

    /// The rows of page `page` (1..PAGES.len()); `languages` are the codes
    /// the install has.
    pub fn rows(&self, page: usize, languages: &[String]) -> Vec<Row> {
        let row = |label, (choices, selected), help| Row {
            label,
            choices,
            selected,
            help,
        };
        match page {
            1 => vec![
                row(
                    "Display",
                    (
                        DISPLAY_MODES.iter().map(|s| s.to_string()).collect(),
                        self.display as usize,
                    ),
                    "Play in a window or fill the screen.",
                ),
                row(
                    "V-Sync",
                    on_off(self.vsync),
                    "Sync frames to the monitor to stop tearing.",
                ),
                row(
                    "Resolution",
                    (
                        RENDER_SCALES.iter().map(|s| format!("{s}%")).collect(),
                        self.render_scale as usize,
                    ),
                    "The 3D scene's size against the window.",
                ),
                row(
                    "Anti-Aliasing",
                    on_off(self.anti_aliasing),
                    "Smooth the jagged edges of 3D models.",
                ),
                row(
                    "Brightness",
                    numbers(1..=10, self.brightness),
                    "Make the 3D scene darker or brighter.",
                ),
            ],
            2 => vec![
                row(
                    "Master Volume",
                    numbers(0..=10, self.master_volume),
                    "The volume of all sound.",
                ),
                row(
                    "Music Volume",
                    numbers(0..=10, self.music_volume),
                    "The volume of the music.",
                ),
                row(
                    "Effects Volume",
                    numbers(0..=10, self.effects_volume),
                    "The volume of sound effects.",
                ),
                row(
                    "Background Sound",
                    on_off(self.background_audio),
                    "Keep sound on while the window is inactive.",
                ),
            ],
            3 => vec![
                row(
                    "Text Language",
                    (
                        languages.iter().map(|c| language_name(c)).collect(),
                        languages
                            .iter()
                            .position(|c| *c == self.language)
                            .unwrap_or(0),
                    ),
                    "The language of the menus and commands.",
                ),
                row(
                    "HUD",
                    (
                        vec!["Show".into(), "Hide".into()],
                        usize::from(!self.show_hud),
                    ),
                    "Show the gauges and command menu in the field.",
                ),
                row(
                    "Camera Speed",
                    numbers(1..=10, self.camera_speed),
                    "How fast the mouse turns the camera.",
                ),
            ],
            _ => Vec::new(),
        }
    }

    /// Left (`forward` false) or Right on row `row` of page `page`; wraps
    /// like the exe's Config choices.
    pub fn step(&mut self, page: usize, row: usize, forward: bool, languages: &[String]) {
        let cycle = |v: usize, n: usize| {
            if forward {
                (v + 1) % n
            } else {
                (v + n - 1) % n
            }
        };
        // Numbers stop at their ends instead of wrapping.
        let bump = |v: u8, lo: u8, hi: u8| {
            if forward {
                v.saturating_add(1).min(hi)
            } else {
                v.saturating_sub(1).max(lo)
            }
        };
        match (page, row) {
            (1, 0) => self.display = cycle(self.display as usize, DISPLAY_MODES.len()) as u8,
            (1, 1) => self.vsync = !self.vsync,
            (1, 2) => self.render_scale = bump(self.render_scale, 0, RENDER_SCALES.len() as u8 - 1),
            (1, 3) => self.anti_aliasing = !self.anti_aliasing,
            (1, 4) => self.brightness = bump(self.brightness, 1, 10),
            (2, 0) => self.master_volume = bump(self.master_volume, 0, 10),
            (2, 1) => self.music_volume = bump(self.music_volume, 0, 10),
            (2, 2) => self.effects_volume = bump(self.effects_volume, 0, 10),
            (2, 3) => self.background_audio = !self.background_audio,
            (3, 0) if !languages.is_empty() => {
                let at = languages
                    .iter()
                    .position(|c| *c == self.language)
                    .unwrap_or(0);
                self.language = languages[cycle(at, languages.len())].clone();
            }
            (3, 1) => self.show_hud = !self.show_hud,
            (3, 2) => self.camera_speed = bump(self.camera_speed, 1, 10),
            _ => {}
        }
    }

    /// Render scale as a factor of the window size.
    pub fn scale(&self) -> f32 {
        RENDER_SCALES[self.render_scale as usize] as f32 / 100.
    }

    /// The scene's colour multiplier: 0.6..=1.5, 1 at 5.
    pub fn brightness_factor(&self) -> f32 {
        0.5 + self.brightness as f32 * 0.1
    }

    /// Mixer gains (master × bus); volumes are squared so each step is a
    /// similar loudness change.
    pub fn music_gain(&self) -> f32 {
        gain(self.master_volume) * gain(self.music_volume)
    }

    pub fn effects_gain(&self) -> f32 {
        gain(self.master_volume) * gain(self.effects_volume)
    }

    /// The mouse turn per pixel (the room's 0.1 at 5).
    pub fn camera_factor(&self) -> f32 {
        0.02 * self.camera_speed as f32
    }
}

fn gain(v: u8) -> f32 {
    let x = v as f32 / 10.;
    x * x
}

/// The shown name of a language code (the code itself when unknown).
pub fn language_name(code: &str) -> String {
    LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map_or_else(|| code.to_owned(), |(_, n)| (*n).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs() -> Vec<String> {
        vec!["us".into(), "fr".into(), "gr".into()]
    }

    #[test]
    fn rows_show_the_current_values() {
        let s = Settings::default();
        let g = s.rows(1, &langs());
        assert_eq!(g[0].choices, ["Window", "Borderless", "Fullscreen"]);
        assert_eq!(g[1].selected, 0);
        assert_eq!(g[2].choices[g[2].selected], "100%");
        assert_eq!(g[4].choices[g[4].selected], "5");
        let a = s.rows(2, &langs());
        assert_eq!(a[0].choices[a[0].selected], "10");
        let o = s.rows(3, &langs());
        assert_eq!(o[0].choices, ["English", "Francais", "Deutsch"]);
        assert_eq!(o[0].selected, 0);
        assert!(s.rows(0, &langs()).is_empty());
    }

    #[test]
    fn steps_cycle_choices_and_stop_numbers() {
        let mut s = Settings::default();
        let l = langs();
        s.step(1, 0, false, &l);
        assert_eq!(s.display, 2);
        s.step(1, 1, true, &l);
        assert!(!s.vsync);
        for _ in 0..20 {
            s.step(1, 2, true, &l);
            s.step(2, 0, true, &l);
            s.step(1, 4, false, &l);
        }
        assert_eq!(s.render_scale, 5);
        assert_eq!(s.master_volume, 10);
        assert_eq!(s.brightness, 1);
        s.step(3, 0, false, &l);
        assert_eq!(s.language, "gr");
        s.step(3, 0, true, &l);
        assert_eq!(s.language, "us");
    }

    #[test]
    fn derived_values() {
        let mut s = Settings::default();
        assert_eq!(s.scale(), 1.);
        assert_eq!(s.brightness_factor(), 1.);
        assert_eq!(s.music_gain(), 1.);
        assert!((s.camera_factor() - 0.1).abs() < 1e-6);
        s.master_volume = 5;
        s.effects_volume = 0;
        assert_eq!(s.music_gain(), 0.25);
        assert_eq!(s.effects_gain(), 0.);
    }

    #[test]
    fn loading_clamps_and_fills_defaults() {
        let s: Settings = serde_json::from_str(r#"{"brightness":40,"display":9}"#).unwrap();
        let s = s.clamped();
        assert_eq!(s.brightness, 10);
        assert_eq!(s.display, 2);
        assert!(s.vsync);
        assert_eq!(s.language, "us");
    }
}
