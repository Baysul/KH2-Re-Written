//! The "core" asset group of tools/asset_pipeline (asset_exports.core) in
//! Rust, reading the user's own disc through a host-supplied file source:
//! everything a skater needs to ride, written in the prepared layout that
//! skate-data and the simulation read (`private/stock/...`).
//!
//! Disc files read: data/big/miscload.big (all entries), data/big/miscboot.big
//! (data/config/input.cfg), data/big/db.big (the skater schema and
//! collections), and the loose data/anim folder.
#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub mod audio;
pub mod big;
pub mod hud;
pub mod skeleton;
pub mod vlt;

pub use audio::prepare_audio;
pub use hud::prepare_hud;

/// Bumped whenever the prepared output changes.
pub const PREPARED_VERSION: u32 = 2;
const MARKER: &str = "private/stock/prepared.json";

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Format(&'static str),
    Missing(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Format(why) => write!(f, "{why}"),
            Error::Missing(path) => write!(f, "{path} is missing from the disc"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// Random access to one disc file.
pub trait ReadAt {
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, Error>;
}

impl<T: ReadAt + ?Sized> ReadAt for Box<T> {
    fn len(&self) -> u64 {
        (**self).len()
    }
    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, Error> {
        (**self).read_at(offset, len)
    }
}

/// The host's view of the disc (an ISO read in place or an extracted folder).
pub trait DiscFiles {
    /// Opens a file by its disc path (`/`-separated, case-insensitive).
    fn open(&mut self, path: &str) -> Result<Option<Box<dyn ReadAt + '_>>, Error>;
    /// File names directly inside a disc directory (empty if it is absent).
    fn list(&mut self, dir: &str) -> Result<Vec<String>, Error>;
}

pub(crate) fn be16(data: &[u8], at: usize) -> Result<u16, Error> {
    Ok(u16::from_be_bytes(field(data, at)?))
}

pub(crate) fn be32(data: &[u8], at: usize) -> Result<u32, Error> {
    Ok(u32::from_be_bytes(field(data, at)?))
}

pub(crate) fn be64(data: &[u8], at: usize) -> Result<u64, Error> {
    Ok(u64::from_be_bytes(field(data, at)?))
}

fn field<const N: usize>(data: &[u8], at: usize) -> Result<[u8; N], Error> {
    at.checked_add(N)
        .and_then(|end| data.get(at..end))
        .map(|s| s.try_into().unwrap())
        .ok_or(Error::Format("truncated record"))
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// An archive path as a relative path, refusing anything that escapes.
fn safe_relative(path: &str) -> Result<PathBuf, Error> {
    let parts: Vec<&str> = path.split('/').collect();
    if path.starts_with('/')
        || parts.is_empty()
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == "..")
        || parts[0].contains(':')
    {
        return Err(Error::Format("unsafe archive path"));
    }
    Ok(parts.iter().collect())
}

fn read_whole(disc: &mut dyn DiscFiles, path: &str) -> Result<Vec<u8>, Error> {
    let mut file = disc
        .open(path)?
        .ok_or_else(|| Error::Missing(path.into()))?;
    let len = file.len();
    if len > 1 << 30 {
        return Err(Error::Format("disc file is unexpectedly large"));
    }
    file.read_at(0, len as usize)
}

/// Whether `root` already holds output prepared from the disc `identity`.
pub fn is_prepared(root: &Path, identity: &str) -> bool {
    fs::read(root.join(MARKER))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .is_some_and(|v| v["version"] == PREPARED_VERSION && v["disc"].as_str() == Some(identity))
}

/// Prepares the riding data under `root` (built in `root.tmp`, then moved
/// into place). `identity` names the disc for `is_prepared`.
pub fn prepare(
    disc: &mut dyn DiscFiles,
    root: &Path,
    identity: &str,
    mut report: impl FnMut(&str),
) -> Result<(), Error> {
    let staging = root.with_extension("tmp");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    let stock = staging.join("private/stock");
    let write = |relative: &Path, bytes: &[u8]| -> Result<(), Error> {
        let target = stock.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, bytes)?;
        Ok(())
    };

    report("Extracting animation banks, graphs and gameplay inputs");
    {
        let file = disc
            .open("data/big/miscload.big")?
            .ok_or_else(|| Error::Missing("data/big/miscload.big".into()))?;
        let mut archive = big::Big::open(file)?;
        for entry in archive.entries.clone() {
            let bytes = archive.read(&entry)?;
            write(&safe_relative(&entry.path)?, &bytes)?;
        }
    }
    {
        let file = disc
            .open("data/big/miscboot.big")?
            .ok_or_else(|| Error::Missing("data/big/miscboot.big".into()))?;
        let mut archive = big::Big::open(file)?;
        for entry in archive.entries.clone() {
            if entry.path.eq_ignore_ascii_case("data/config/input.cfg") {
                let bytes = archive.read(&entry)?;
                write(&safe_relative(&entry.path)?, &bytes)?;
            }
        }
    }
    // Some disc banks are loose files rather than members of miscload.
    for name in disc.list("data/anim")? {
        let bytes = read_whole(disc, &format!("data/anim/{name}"))?;
        write(&Path::new("data/anim").join(safe_relative(&name)?), &bytes)?;
    }

    report("Converting physics and difficulty settings");
    let mut database = std::collections::HashMap::new();
    {
        let file = disc
            .open("data/big/db.big")?
            .ok_or_else(|| Error::Missing("data/big/db.big".into()))?;
        let mut archive = big::Big::open(file)?;
        for entry in archive.entries.clone() {
            let base = entry.path.rsplit('/').next().unwrap_or("").to_lowercase();
            if [
                "skaterschema.bin",
                "skaterschema.vlt",
                "skatercollections.bin",
                "skatercollections.vlt",
            ]
            .contains(&base.as_str())
            {
                database.insert(base, archive.read(&entry)?);
            }
        }
    }
    let get = |name: &str| {
        database
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| Error::Missing(format!("data/big/db.big {name}")))
    };
    let collections = vlt::convert(
        get("skaterschema.vlt")?,
        get("skaterschema.bin")?,
        get("skatercollections.vlt")?,
        get("skatercollections.bin")?,
    )?;
    write(
        Path::new("skater-collections.json"),
        collections.to_string().as_bytes(),
    )?;
    let onboard = fs::read(stock.join("data/anim/OnBoard.abin"))
        .map_err(|_| Error::Missing("data/anim/OnBoard.abin".into()))?;
    let skeletons = skeleton::convert(&onboard)?;
    write(
        Path::new("physics-skeletons.json"),
        skeletons.to_string().as_bytes(),
    )?;
    let game = serde_json::json!({
        "version": 1,
        "character_scene": "private/skater.glb",
        "initial_animation": "R_IDLE_HCOM_000",
        "action_graph": "private/stock/data/state/ActionGraph_OnBoard.stategraph",
        "motion_graph": "private/stock/data/state/MotionGraph_OnBoard.stategraph",
    });
    fs::write(staging.join("private/game.json"), game.to_string())?;
    write(
        Path::new("prepared.json"),
        serde_json::json!({"version": PREPARED_VERSION, "disc": identity})
            .to_string()
            .as_bytes(),
    )?;
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    fs::rename(&staging, root)?;
    Ok(())
}

#[cfg(test)]
mod tests;
