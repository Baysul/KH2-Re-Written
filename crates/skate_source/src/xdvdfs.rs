//! XDVDFS, the Xbox disc filesystem, read in place from an ISO.
//!
//! The volume descriptor sits at sector 32 of the game partition: the
//! 20-byte magic `MICROSOFT*XBOX*MEDIA`, then the root directory's sector
//! and byte size (little-endian u32). A directory is a binary tree of
//! entries: left and right child offsets (u16, in 4-byte units, 0xFFFF in
//! the left slot marks padding), start sector (u32), size (u32), attributes
//! (u8, 0x10 = directory), name length (u8), name. Sectors are 2048 bytes
//! and count from the partition start.
//!
//! The game partition starts at 0 for a rebuilt image and at 0x2080000,
//! 0xFD90000 or 0x18300000 for XGD3, XGD2 and XGD1 dumps.
use std::fmt;
use std::io::{self, Read, Seek};

use crate::read_at;

pub const SECTOR: u64 = 2048;
const MAGIC: &[u8; 20] = b"MICROSOFT*XBOX*MEDIA";
const PARTITIONS: [u64; 4] = [0, 0x0208_0000, 0x0FD9_0000, 0x1830_0000];
/// Bounds on what a directory may hold.
const DIRECTORY_LIMIT: u32 = 16 << 20;
const ENTRY_LIMIT: usize = 1 << 16;
const DEPTH_LIMIT: usize = 32;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    NotXdvdfs,
    Malformed(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::NotXdvdfs => write!(f, "no Xbox volume descriptor"),
            Error::Malformed(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// A file or directory on the disc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub sector: u32,
    pub size: u32,
    pub directory: bool,
}

pub struct Image<R> {
    reader: R,
    partition: u64,
    length: u64,
    root: Entry,
}

impl<R: Read + Seek> Image<R> {
    pub fn open(mut reader: R) -> Result<Self, Error> {
        let length = reader.seek(io::SeekFrom::End(0))?;
        for partition in PARTITIONS {
            let at = partition + 32 * SECTOR;
            if at + 28 > length {
                continue;
            }
            let head = read_at(&mut reader, at, 28)?;
            if &head[..20] != MAGIC {
                continue;
            }
            let sector = u32::from_le_bytes(head[20..24].try_into().unwrap());
            let size = u32::from_le_bytes(head[24..28].try_into().unwrap());
            let root = Entry {
                name: String::new(),
                sector,
                size,
                directory: true,
            };
            return Ok(Image {
                reader,
                partition,
                length,
                root,
            });
        }
        Err(Error::NotXdvdfs)
    }

    /// Lists a directory's entries in tree order.
    pub fn list(&mut self, dir: &Entry) -> Result<Vec<Entry>, Error> {
        if !dir.directory {
            return Err(Error::Malformed("not a directory"));
        }
        if dir.size == 0 {
            return Ok(Vec::new());
        }
        if dir.size > DIRECTORY_LIMIT {
            return Err(Error::Malformed("directory too large"));
        }
        let bytes = self.read(dir, 0, dir.size as usize)?;
        let mut out = Vec::new();
        let mut stack = vec![0usize];
        while let Some(offset) = stack.pop() {
            if out.len() >= ENTRY_LIMIT {
                return Err(Error::Malformed("directory has too many entries"));
            }
            let Some(head) = bytes.get(offset..offset + 14) else {
                return Err(Error::Malformed("directory entry out of range"));
            };
            let left = u16::from_le_bytes([head[0], head[1]]);
            if left == 0xFFFF {
                continue;
            }
            let right = u16::from_le_bytes([head[2], head[3]]);
            let sector = u32::from_le_bytes(head[4..8].try_into().unwrap());
            let size = u32::from_le_bytes(head[8..12].try_into().unwrap());
            let directory = head[12] & 0x10 != 0;
            let length = head[13] as usize;
            let Some(name) = bytes.get(offset + 14..offset + 14 + length) else {
                return Err(Error::Malformed("directory name out of range"));
            };
            out.push(Entry {
                name: String::from_utf8_lossy(name).into_owned(),
                sector,
                size,
                directory,
            });
            // Children sit after their parent, which bounds the walk.
            for child in [right, left] {
                let at = child as usize * 4;
                if child != 0 {
                    if at <= offset {
                        return Err(Error::Malformed("directory tree loops"));
                    }
                    stack.push(at);
                }
            }
        }
        Ok(out)
    }

    /// Finds `path` (`/`-separated, case-insensitive).
    pub fn find(&mut self, path: &str) -> Result<Option<Entry>, Error> {
        let mut current = self.root.clone();
        let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
        if parts.len() > DEPTH_LIMIT {
            return Err(Error::Malformed("path too deep"));
        }
        for part in parts {
            if !current.directory {
                return Ok(None);
            }
            match self
                .list(&current)?
                .into_iter()
                .find(|e| e.name.eq_ignore_ascii_case(part))
            {
                Some(e) => current = e,
                None => return Ok(None),
            }
        }
        Ok(Some(current))
    }

    /// Reads `len` bytes of `entry` from `offset`.
    pub fn read(&mut self, entry: &Entry, offset: u64, len: usize) -> Result<Vec<u8>, Error> {
        if offset + len as u64 > entry.size as u64 {
            return Err(Error::Malformed("read past the end of a file"));
        }
        let at = self.partition + entry.sector as u64 * SECTOR + offset;
        if at + len as u64 > self.length {
            return Err(Error::Malformed("file extends past the image"));
        }
        Ok(read_at(&mut self.reader, at, len)?)
    }

    pub fn root(&self) -> &Entry {
        &self.root
    }
}
