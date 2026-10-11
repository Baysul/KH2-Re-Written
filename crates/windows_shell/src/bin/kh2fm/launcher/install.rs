//! The launcher's non-visual half: where its private files live, finding
//! the Steam install, binding a folder to a source manifest (a fast size and
//! time check against an existing one, or a full scan that writes a new
//! one), and turning a test into the program arguments it runs with.
//!
//! The launcher's check only decides what to offer; every program still
//! admits its source itself (`ProbeSource::bind_session` +
//! `verify_session`) when it starts.
use serde::{Deserialize, Serialize};
use source_identity::{KH2_EXECUTABLE, SourceManifest};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::time::UNIX_EPOCH;

const APP_ID: &str = "2552430";
const SETTINGS: &str = "launcher.json";
/// Schema 2 manifests list ~16k chunk digests (about 1.2 MB).
const MANIFEST_LIMIT: u64 = 8 << 20;
const CONFIG_LIMIT: u64 = 1 << 20;

/// The private folder for launch files, configs, manifests and the
/// launcher's settings: `.local/test-room` beside the checkout the
/// executable was built in, else `%LOCALAPPDATA%\KH2FinalRewrite`.
pub fn data_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        for up in exe.ancestors().skip(1).take(4) {
            let dir = up.join(".local/test-room");
            if dir.is_dir() {
                return dir;
            }
        }
    }
    std::env::var_os("LOCALAPPDATA")
        .map(|local| PathBuf::from(local).join("KH2FinalRewrite"))
        .unwrap_or_else(|| PathBuf::from(".local/test-room"))
}

fn read_limited(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} is larger than expected", path.display()),
        ));
    }
    Ok(bytes)
}

/// What the launcher remembers between runs.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub game_folder: Option<PathBuf>,
    pub manifest: Option<PathBuf>,
    /// The user's Skate 3 Xbox 360 ISO (or extracted `default.xex`).
    #[serde(default)]
    pub skate3_iso: Option<PathBuf>,
}

pub fn load_settings(dir: &Path) -> Settings {
    read_limited(&dir.join(SETTINGS), CONFIG_LIMIT)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Written beside and renamed over, so a crash never leaves half a file.
pub fn save_settings(dir: &Path, settings: &Settings) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let next = dir.join(format!("{SETTINGS}.new"));
    fs::write(&next, serde_json::to_vec_pretty(settings)?)?;
    fs::rename(next, dir.join(SETTINGS))
}

pub fn has_executable(folder: &Path) -> bool {
    folder.join(KH2_EXECUTABLE).is_file()
}

/// Every `"key" "value"` pair named `key` in a Steam VDF text.
fn vdf_values(text: &str, key: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split('"').collect();
            (fields.len() >= 5 && fields[1] == key).then(|| fields[3].replace("\\\\", "\\"))
        })
        .collect()
}

/// Steam library folders: the default install locations and every library
/// they list in `steamapps/libraryfolders.vdf`.
fn steam_libraries() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(p) = std::env::var_os(var) {
            roots.push(PathBuf::from(p).join("Steam"));
        }
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    let mut libraries = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        libraries.push(root.clone());
        if let Ok(bytes) = read_limited(&root.join("steamapps/libraryfolders.vdf"), CONFIG_LIMIT) {
            let text = String::from_utf8_lossy(&bytes);
            libraries.extend(vdf_values(&text, "path").into_iter().map(PathBuf::from));
        }
    }
    libraries
}

/// Installs of the collection (Steam app 2552430) that hold the KH2FM
/// executable, without duplicates.
pub fn detect_installs() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for library in steam_libraries() {
        let apps = library.join("steamapps");
        let Ok(bytes) = read_limited(
            &apps.join(format!("appmanifest_{APP_ID}.acf")),
            CONFIG_LIMIT,
        ) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let Some(dir) = source_identity::vdf_value(&text, "installdir") else {
            continue;
        };
        let folder = apps.join("common").join(dir);
        if !has_executable(&folder) {
            continue;
        }
        let folder = fs::canonicalize(&folder).unwrap_or(folder);
        if !found.contains(&folder) {
            found.push(folder);
        }
    }
    found
}

/// A folder bound to a manifest, as the launcher shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Bound {
    pub folder: PathBuf,
    pub manifest: PathBuf,
    pub identity: String,
    pub build_id: Option<String>,
    pub language: Option<String>,
}

/// Why a folder was not bound.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckError {
    /// No manifest of this folder matches what is on disk (a new folder, or
    /// Steam changed files since the last scan): a full scan is needed.
    NeedsScan,
    /// Not usable; the text says why.
    Rejected(String),
}

fn modified_nanos(meta: &fs::Metadata) -> Option<u128> {
    Some(
        meta.modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos(),
    )
}

fn bound(folder: PathBuf, manifest_path: PathBuf, m: SourceManifest) -> Bound {
    Bound {
        folder,
        manifest: manifest_path,
        identity: m.content_identity,
        build_id: m.observed_build_id,
        language: m.observed_steam_language,
    }
}

/// The fast check: the manifest is for this folder and self-consistent,
/// and every required file has the size and time it was hashed at. No
/// file is read.
fn quick_match(folder: &Path, manifest_path: &Path) -> Option<Bound> {
    let bytes = read_limited(manifest_path, MANIFEST_LIMIT).ok()?;
    let m: SourceManifest = serde_json::from_slice(&bytes).ok()?;
    let required = source_identity::required_files();
    if m.canonical_source_path != folder
        || m.qualification != "quarantined_unqualified"
        || source_identity::content_identity(&m.files) != m.content_identity
        || m.files.len() != required.len()
    {
        return None;
    }
    for (relative, _) in &required {
        let expected = m.files.iter().find(|f| &f.relative_path == relative)?;
        let meta = fs::metadata(folder.join(relative)).ok()?;
        if meta.len() != expected.size || modified_nanos(&meta)? != expected.modified_unix_nanos {
            return None;
        }
    }
    Some(bound(folder.to_path_buf(), manifest_path.to_path_buf(), m))
}

/// `source-manifest-NN.json` files in `dir`, newest number first.
fn manifests_in(dir: &Path) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("source-manifest-") && n.ends_with(".json"))
        })
        .collect();
    list.sort();
    list.reverse();
    list
}

/// Binds `folder` to an existing manifest (`hint` first, then the data
/// folder's) without hashing.
pub fn quick_check(dir: &Path, folder: &Path, hint: Option<&Path>) -> Result<Bound, CheckError> {
    let folder = fs::canonicalize(folder).map_err(|e| CheckError::Rejected(e.to_string()))?;
    if !has_executable(&folder) {
        return Err(CheckError::Rejected(format!(
            "There is no \"{KH2_EXECUTABLE}\" in this folder. Pick the folder Steam installed the collection in."
        )));
    }
    hint.map(Path::to_path_buf)
        .into_iter()
        .chain(manifests_in(dir))
        .find_map(|path| quick_match(&folder, &path))
        .ok_or(CheckError::NeedsScan)
}

/// Hashes the folder in full and writes a new manifest into `dir`.
pub fn full_scan(dir: &Path, folder: &Path, progress: &AtomicU64) -> Result<Bound, String> {
    let folder = fs::canonicalize(folder).map_err(|e| e.to_string())?;
    let manifest =
        source_identity::scan_with_progress(&folder, true, progress).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dir = fs::canonicalize(dir).map_err(|e| e.to_string())?;
    if dir.starts_with(&folder) {
        return Err("The launcher's files cannot live inside the game folder.".into());
    }
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    for n in 1..100 {
        let path = dir.join(format!("source-manifest-{n:02}.json"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(&bytes)
                    .and_then(|()| file.sync_all())
                    .map_err(|e| e.to_string())?;
                return Ok(bound(folder, path, manifest));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("No free manifest name in the launcher's folder.".into())
}

/// A program launch file (`launch*.json`): the program's config and extra
/// options. Its source paths are replaced by the chosen folder.
#[derive(Debug, Deserialize)]
struct LaunchFile {
    steam_root: Option<PathBuf>,
    manifest: Option<PathBuf>,
    config: PathBuf,
    #[serde(default)]
    options: Vec<String>,
}

fn launch_file(dir: &Path, name: &str) -> Result<LaunchFile, String> {
    let path = dir.join(format!("{name}.json"));
    let bytes = read_limited(&path, CONFIG_LIMIT)
        .map_err(|_| format!("The test's launch file {} is missing.", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn config_identity(path: &Path) -> Option<String> {
    let bytes = read_limited(path, CONFIG_LIMIT).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    Some(v.get("source_content_identity")?.as_str()?.to_owned())
}

/// One entry of the test list.
pub struct Test {
    pub name: &'static str,
    pub blurb: &'static str,
    /// The `kh2fm` command it runs.
    pub program: &'static str,
}

pub const TESTS: [Test; 4] = [
    Test {
        name: "Title screen",
        blurb: "Disney, Square Enix and GLURBUS logos, intro, title menu",
        program: "title",
    },
    Test {
        name: "Test room",
        blurb: "Sora, a Shadow to fight and Axel to talk to",
        program: "play",
    },
    Test {
        name: "Twilight Town",
        blurb: "Station Plaza and the areas its exits lead to",
        program: "tt",
    },
    Test {
        name: "Cutscene: Roxas and Axel",
        blurb: "Betwixt and Between, before the fight (event 613)",
        program: "event",
    },
];

/// Where the Twilight Town test starts: Station Plaza (tt08) entrance 0,
/// with the story flags 0x800..=0x895 set (Sora's first visit; area-01 §6).
const TT_START: (u8, u8) = (8, 0);
const TT_STORY: [u32; 2] = [0x800, 0x895];

/// The Twilight Town config: the arena config's actor, HUD, menu and sound
/// sources with a field area in place of the arena and its enemy.
pub fn tt_room_config(arena: &Path) -> Result<PathBuf, String> {
    let bytes =
        read_limited(arena, CONFIG_LIMIT).map_err(|e| format!("{}: {e}", arena.display()))?;
    let mut room: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", arena.display()))?;
    let o = room
        .as_object_mut()
        .ok_or("The arena config is not an object.")?;
    for key in ["arena", "enemy", "spawn", "npc", "room_selection"] {
        o.remove(key);
    }
    // KH2FM_TT_STORY=<last flag, e.g. 0x860> sets tt flags 0x800..=last
    // (default: the whole first-visit range); KH2FM_TT_AREA=<area>[:<entrance>]
    // picks the starting area.
    let story = std::env::var("KH2FM_TT_STORY")
        .ok()
        .and_then(|v| {
            let v = v.trim();
            let v = v.strip_prefix("0x").map_or_else(
                || v.parse::<u32>().ok(),
                |h| u32::from_str_radix(h, 16).ok(),
            )?;
            (0x800..0x900).contains(&v).then_some([0x800, v])
        })
        .unwrap_or(TT_STORY);
    let start = std::env::var("KH2FM_TT_AREA")
        .ok()
        .and_then(|v| {
            let mut p = v.split(':');
            let area = p.next()?.trim().parse::<u8>().ok()?;
            let entrance = p.next().map_or(Some(0), |e| e.trim().parse::<u8>().ok())?;
            Some((area, entrance))
        })
        .unwrap_or(TT_START);
    o.insert(
        "area".into(),
        serde_json::json!({"world":"tt","area":start.0,"entrance":start.1,"story":[story]}),
    );
    let dir = arena.parent().unwrap_or(Path::new("."));
    let (out, next) = (
        dir.join("test-room-tt.json"),
        dir.join("test-room-tt.json.new"),
    );
    let bytes = serde_json::to_vec_pretty(&room).map_err(|e| e.to_string())?;
    fs::write(&next, bytes)
        .and_then(|()| fs::rename(&next, &out))
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// A developer viewer over one private selection file.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewer {
    pub label: String,
    /// The sibling executable it runs.
    pub program: &'static str,
    pub selection: PathBuf,
}

/// Viewers for the model and image selections in `dir`, sorted by label.
pub fn viewers(dir: &Path) -> Vec<Viewer> {
    let mut list: Vec<Viewer> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_owned();
            let (who, _) = name.strip_suffix(".json")?.split_once("-selection-")?;
            let v: serde_json::Value =
                serde_json::from_slice(&read_limited(&path, CONFIG_LIMIT).ok()?).ok()?;
            let flag = |k: &str| v.get(k).and_then(serde_json::Value::as_bool) == Some(true);
            let (kind, program) = if flag("inspect_model_geometry") {
                ("Model viewer", "model_inspector")
            } else if flag("inspect_images") {
                ("Image viewer", "image_inspector")
            } else {
                return None;
            };
            // Names for the selections in use [launcher labels].
            let what = match who {
                "actor" => "Sora".to_owned(),
                "weapon" => "Kingdom Key".to_owned(),
                other => {
                    let mut c = other.chars();
                    c.next()
                        .map(|f| f.to_uppercase().chain(c).collect())
                        .unwrap_or_default()
                }
            };
            Some(Viewer {
                label: format!("{kind}: {what}"),
                program,
                selection: path,
            })
        })
        .collect();
    list.sort_by(|a, b| a.label.cmp(&b.label));
    list
}

/// A new private log path for a viewer: outside any checkout, under
/// `%LOCALAPPDATA%\KH2FinalRewrite\logs`.
pub fn viewer_log(program: &str) -> Result<PathBuf, String> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is not set")?;
    let logs = PathBuf::from(local).join("KH2FinalRewrite").join("logs");
    fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    Ok(logs.join(format!("{program}-{stamp}.jsonl")))
}

/// The content identity the tests' configs were made for (the title
/// config's), if the configs are present.
pub fn supported_identity(dir: &Path) -> Option<String> {
    let launch = launch_file(dir, "launch-title").ok()?;
    config_identity(&dir.join(launch.config))
}

/// Where Axel stands in the combined room (from Sora's spawn; the Shadow
/// is straight ahead) and his facing [launcher choice].
const COMBINED_NPC_OFFSET: [f32; 3] = [-450.0, 0.0, 650.0];
const COMBINED_NPC_YAW: f32 = 2.5;

/// The combined room's config: the arena config with the talk config's
/// NPC added, written beside the arena config (its selections are relative
/// to it). Rebuilt on every launch so it follows both.
pub fn combined_room_config(arena: &Path, talk: &Path) -> Result<PathBuf, String> {
    let read = |path: &Path| -> Result<serde_json::Value, String> {
        let bytes =
            read_limited(path, CONFIG_LIMIT).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    };
    let (mut room, talk_config) = (read(arena)?, read(talk)?);
    if room.get("source_content_identity") != talk_config.get("source_content_identity") {
        return Err("The arena and talk configs are for different builds.".into());
    }
    let mut npc = talk_config
        .get("npc")
        .cloned()
        .ok_or("The talk config has no NPC.")?;
    let (arena_dir, talk_dir) = (
        arena.parent().unwrap_or(Path::new(".")),
        talk.parent().unwrap_or(Path::new(".")),
    );
    if arena_dir != talk_dir
        && let Some(sel) = npc.get_mut("model_selection")
        && let Some(name) = sel.as_str()
    {
        *sel = talk_dir.join(name).to_string_lossy().into_owned().into();
    }
    npc["offset"] = serde_json::json!(COMBINED_NPC_OFFSET);
    npc["yaw"] = serde_json::json!(COMBINED_NPC_YAW);
    room.as_object_mut()
        .ok_or("The arena config is not an object.")?
        .insert("npc".into(), npc);
    let out = arena_dir.join("test-room-combined.json");
    let next = arena_dir.join("test-room-combined.json.new");
    let bytes = serde_json::to_vec_pretty(&room).map_err(|e| e.to_string())?;
    fs::write(&next, bytes)
        .and_then(|()| fs::rename(&next, &out))
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// `kh2fm` arguments for a test on a bound folder: the program, the folder,
/// its manifest, the config and the launch file's options.
pub fn test_args(dir: &Path, program: &str, b: &Bound) -> Result<Vec<String>, String> {
    // The event player reads everything by name from the bound folder and
    // checks the source itself; it needs no private config.
    if program == "event" {
        return Ok(vec![
            "event".into(),
            b.folder.to_string_lossy().into_owned(),
            b.manifest.to_string_lossy().into_owned(),
        ]);
    }
    let (command, config, options) = match program {
        "title" => {
            let l = launch_file(dir, "launch-title")?;
            ("title", dir.join(l.config), l.options)
        }
        "play" => {
            let (arena, talk) = (
                launch_file(dir, "launch")?,
                launch_file(dir, "launch-talk")?,
            );
            let config = combined_room_config(&dir.join(arena.config), &dir.join(talk.config))?;
            ("room", config, arena.options)
        }
        "tt" => {
            let arena = launch_file(dir, "launch")?;
            (
                "room",
                tt_room_config(&dir.join(arena.config))?,
                arena.options,
            )
        }
        other => return Err(format!("unknown test {other}")),
    };
    if config_identity(&config).as_deref() != Some(b.identity.as_str()) {
        return Err(
            "This test's config was made for a different build than the chosen folder.".into(),
        );
    }
    let mut args = vec![
        command.to_owned(),
        b.folder.to_string_lossy().into_owned(),
        b.manifest.to_string_lossy().into_owned(),
        config.to_string_lossy().into_owned(),
    ];
    args.extend(options);
    Ok(args)
}

/// `kh2fm tt` without arguments: the Twilight Town config with the arena
/// launch file's source paths.
pub fn tt_args() -> Result<Vec<PathBuf>, String> {
    let dir = data_dir();
    let arena = launch_file(&dir, "launch")?;
    let (Some(root), Some(manifest)) = (arena.steam_root, arena.manifest) else {
        return Err("launch.json names no steam_root/manifest".into());
    };
    let config = tt_room_config(&dir.join(arena.config))?;
    let mut args = vec![root, manifest, config];
    args.extend(arena.options.into_iter().map(PathBuf::from));
    Ok(args)
}

/// `kh2fm play` without arguments: the combined room with the arena launch
/// file's source paths (as `kh2fm room` uses them).
pub fn play_args() -> Result<Vec<PathBuf>, String> {
    let dir = data_dir();
    let (arena, talk) = (
        launch_file(&dir, "launch")?,
        launch_file(&dir, "launch-talk")?,
    );
    let (Some(root), Some(manifest)) = (arena.steam_root, arena.manifest) else {
        return Err("launch.json names no steam_root/manifest".into());
    };
    let config = combined_room_config(&dir.join(arena.config), &dir.join(talk.config))?;
    let mut args = vec![root, manifest, config];
    args.extend(arena.options.into_iter().map(PathBuf::from));
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64 as Counter, Ordering};
    static NEXT: Counter = Counter::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "kh2-launcher-synthetic-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn populate(root: &Path) {
        for (file, _) in source_identity::required_files() {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"synthetic fixture only").unwrap();
        }
    }

    #[test]
    fn scan_then_quick_check_then_change_needs_scan() {
        let f = Fixture::new();
        let (game, data) = (f.0.join("game"), f.0.join("data"));
        populate(&game);
        assert_eq!(quick_check(&data, &game, None), Err(CheckError::NeedsScan));
        let progress = AtomicU64::new(0);
        let scanned = full_scan(&data, &game, &progress).unwrap();
        assert!(scanned.manifest.ends_with("source-manifest-01.json"));
        assert_eq!(quick_check(&data, &game, None).unwrap(), scanned);
        // A hint that is not a manifest of this folder is skipped.
        let other = quick_check(&data, &game, Some(&data.join("missing.json"))).unwrap();
        assert_eq!(other, scanned);
        fs::write(game.join(KH2_EXECUTABLE), b"a different synthetic exe").unwrap();
        assert_eq!(quick_check(&data, &game, None), Err(CheckError::NeedsScan));
        let rescanned = full_scan(&data, &game, &progress).unwrap();
        assert!(rescanned.manifest.ends_with("source-manifest-02.json"));
        assert_ne!(rescanned.identity, scanned.identity);
    }

    #[test]
    fn folder_without_executable_is_rejected() {
        let f = Fixture::new();
        assert!(matches!(
            quick_check(&f.0, &f.0, None),
            Err(CheckError::Rejected(_))
        ));
    }

    #[test]
    fn settings_round_trip() {
        let f = Fixture::new();
        assert_eq!(load_settings(&f.0), Settings::default());
        let s = Settings {
            game_folder: Some(f.0.join("game")),
            manifest: Some(f.0.join("m.json")),
            skate3_iso: Some(f.0.join("skate.iso")),
        };
        save_settings(&f.0, &s).unwrap();
        assert_eq!(load_settings(&f.0), s);
        // Settings written before the Skate 3 field still load.
        fs::write(
            f.0.join(SETTINGS),
            r#"{"game_folder":null,"manifest":null}"#,
        )
        .unwrap();
        assert_eq!(load_settings(&f.0), Settings::default());
    }

    #[test]
    fn viewers_come_from_model_and_image_selections() {
        let f = Fixture::new();
        let write = |name: &str, v: serde_json::Value| {
            fs::write(f.0.join(name), v.to_string()).unwrap();
        };
        write(
            "actor-selection-01.json",
            serde_json::json!({"inspect_model_geometry": true}),
        );
        write(
            "shadow-selection-01.json",
            serde_json::json!({"inspect_model_geometry": true}),
        );
        write(
            "tex-selection-01.json",
            serde_json::json!({"inspect_images": true}),
        );
        write("other-selection-01.json", serde_json::json!({}));
        write(
            "launch.json",
            serde_json::json!({"inspect_model_geometry": true}),
        );
        let labels: Vec<_> = viewers(&f.0)
            .into_iter()
            .map(|v| (v.label, v.program))
            .collect();
        assert_eq!(
            labels,
            vec![
                ("Image viewer: Tex".to_owned(), "image_inspector"),
                ("Model viewer: Shadow".to_owned(), "model_inspector"),
                ("Model viewer: Sora".to_owned(), "model_inspector"),
            ]
        );
    }

    #[test]
    fn library_paths_unescape() {
        let text =
            "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}";
        assert_eq!(
            vdf_values(text, "path"),
            vec![r"D:\SteamLibrary".to_owned()]
        );
    }

    #[test]
    fn combined_config_adds_the_npc_and_checks_identity() {
        let f = Fixture::new();
        let id = "a".repeat(64);
        let arena = f.0.join("arena.json");
        let talk = f.0.join("talk.json");
        fs::write(
            &arena,
            serde_json::json!({"source_content_identity": id, "enemy": {"offset": [0, 0, 250]}})
                .to_string(),
        )
        .unwrap();
        fs::write(
            &talk,
            serde_json::json!({"source_content_identity": id,
                "npc": {"model_selection": "npc.json", "offset": [0, 0, 180], "yaw": 3.0}})
            .to_string(),
        )
        .unwrap();
        let out = combined_room_config(&arena, &talk).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
        assert!(v.get("enemy").is_some());
        assert_eq!(v["npc"]["model_selection"], "npc.json");
        assert_eq!(v["npc"]["offset"], serde_json::json!(COMBINED_NPC_OFFSET));
        fs::write(
            &talk,
            serde_json::json!({"source_content_identity": "b", "npc": {}}).to_string(),
        )
        .unwrap();
        assert!(combined_room_config(&arena, &talk).is_err());
    }
}
