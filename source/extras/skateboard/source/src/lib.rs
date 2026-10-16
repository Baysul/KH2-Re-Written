//! The user's own Skate 3 (Xbox 360) disc: an ISO read in place, or an
//! extracted folder holding `default.xex`. Read-only; nothing is extracted
//! to disk.
//!
//! Admission checks the files the Skate 3 engine's own setup requires
//! (`tools/asset_pipeline/install.py` in SK8-ENGINE/skate-3-rust-engine):
//! `default.xex` (with the XEX2 magic), the three core `.big` archives and
//! `createacharacter.big`. That is a layout check, not build qualification:
//! no reference digest of a retail disc is established here.
#![forbid(unsafe_code)]

use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub mod xdvdfs;

/// Files a Skate 3 disc must hold (paths relative to the game root).
pub const REQUIRED_FILES: [&str; 5] = [
    "default.xex",
    "data/big/miscload.big",
    "data/big/miscboot.big",
    "data/big/db.big",
    "data/content/createacharacter.big",
];

/// Why a path was not admitted as a Skate 3 disc.
#[derive(Debug)]
pub enum Rejection {
    NotConfigured,
    Io(PathBuf, io::Error),
    NotXboxImage,
    Malformed(String),
    Missing(&'static str),
    NotXex,
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rejection::NotConfigured => write!(f, "no Skate 3 ISO is set"),
            Rejection::Io(p, e) => write!(f, "{}: {e}", p.display()),
            Rejection::NotXboxImage => write!(f, "not an Xbox 360 disc image"),
            Rejection::Malformed(why) => write!(f, "disc image is damaged: {why}"),
            Rejection::Missing(file) => write!(f, "not a Skate 3 disc: missing {file}"),
            Rejection::NotXex => write!(f, "default.xex is not an Xbox 360 executable"),
        }
    }
}

impl std::error::Error for Rejection {}

/// An admitted disc.
pub enum Disc {
    Image(xdvdfs::Image<File>),
    Folder(PathBuf),
}

impl Disc {
    /// Reads one whole file from the disc, refusing anything over `limit`.
    pub fn read(&mut self, path: &str, limit: u64) -> Result<Vec<u8>, Rejection> {
        match self {
            Disc::Image(image) => {
                let entry = image
                    .find(path)
                    .map_err(|e| Rejection::Malformed(e.to_string()))?
                    .filter(|e| !e.directory)
                    .ok_or_else(|| Rejection::Malformed(format!("{path} not found")))?;
                if entry.size as u64 > limit {
                    return Err(Rejection::Malformed(format!("{path} is too large")));
                }
                image
                    .read(&entry, 0, entry.size as usize)
                    .map_err(|e| Rejection::Malformed(e.to_string()))
            }
            Disc::Folder(root) => {
                let file = root.join(path);
                let mut bytes = Vec::new();
                File::open(&file)
                    .and_then(|f| f.take(limit + 1).read_to_end(&mut bytes))
                    .map_err(|e| Rejection::Io(file.clone(), e))?;
                if bytes.len() as u64 > limit {
                    return Err(Rejection::Malformed(format!("{path} is too large")));
                }
                Ok(bytes)
            }
        }
    }
}

/// Opens `path` (an ISO, or `default.xex` / the folder holding it) and
/// checks the Skate 3 layout.
pub fn admit(path: Option<&Path>) -> Result<Disc, Rejection> {
    let path = path.ok_or(Rejection::NotConfigured)?;
    let meta = std::fs::metadata(path).map_err(|e| Rejection::Io(path.into(), e))?;
    let folder = if meta.is_dir() {
        Some(path.to_path_buf())
    } else if path
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("default.xex"))
    {
        path.parent().map(Path::to_path_buf)
    } else {
        None
    };
    let mut disc = match folder {
        Some(root) => {
            for file in REQUIRED_FILES {
                if !root.join(file).is_file() {
                    return Err(Rejection::Missing(file));
                }
            }
            Disc::Folder(root)
        }
        None => {
            let file = File::open(path).map_err(|e| Rejection::Io(path.into(), e))?;
            let mut image = xdvdfs::Image::open(file).map_err(|e| match e {
                xdvdfs::Error::NotXdvdfs => Rejection::NotXboxImage,
                e => Rejection::Malformed(e.to_string()),
            })?;
            check_image(&mut image)?;
            Disc::Image(image)
        }
    };
    let xex = disc.read("default.xex", 64 << 20)?;
    if !xex.starts_with(b"XEX2") {
        return Err(Rejection::NotXex);
    }
    Ok(disc)
}

fn check_image<R: Read + Seek>(image: &mut xdvdfs::Image<R>) -> Result<(), Rejection> {
    for file in REQUIRED_FILES {
        match image.find(file) {
            Ok(Some(e)) if !e.directory => {}
            Ok(_) => return Err(Rejection::Missing(file)),
            Err(e) => return Err(Rejection::Malformed(e.to_string())),
        }
    }
    Ok(())
}

/// Reads `len` bytes at `offset`, failing on a short read.
fn read_at<R: Read + Seek>(r: &mut R, offset: u64, len: usize) -> io::Result<Vec<u8>> {
    r.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; len];
    r.read_exact(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests;
