//! Bounds-checked big-endian reads (tools/vendor/skate3_ui/binary.py).
use crate::Error;

pub fn bad(why: &'static str) -> Error {
    Error::Format(why)
}

#[derive(Clone, Copy)]
pub struct Reader<'a> {
    pub data: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data }
    }
    pub fn require(&self, offset: usize, size: usize) -> Result<(), Error> {
        match offset.checked_add(size) {
            Some(end) if end <= self.data.len() => Ok(()),
            _ => Err(bad("range lies outside the file")),
        }
    }
    pub fn bytes(&self, offset: usize, size: usize) -> Result<&'a [u8], Error> {
        self.require(offset, size)?;
        Ok(&self.data[offset..offset + size])
    }
    fn array<const N: usize>(&self, offset: usize) -> Result<[u8; N], Error> {
        Ok(self.bytes(offset, N)?.try_into().unwrap())
    }
    pub fn u8(&self, offset: usize) -> Result<u8, Error> {
        Ok(self.array::<1>(offset)?[0])
    }
    pub fn u16be(&self, offset: usize) -> Result<u16, Error> {
        Ok(u16::from_be_bytes(self.array(offset)?))
    }
    pub fn i16be(&self, offset: usize) -> Result<i16, Error> {
        Ok(i16::from_be_bytes(self.array(offset)?))
    }
    pub fn u32be(&self, offset: usize) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.array(offset)?))
    }
    /// A u32 used as an offset or count.
    pub fn at(&self, offset: usize) -> Result<usize, Error> {
        Ok(self.u32be(offset)? as usize)
    }
    pub fn i32be(&self, offset: usize) -> Result<i32, Error> {
        Ok(i32::from_be_bytes(self.array(offset)?))
    }
    pub fn u64be(&self, offset: usize) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.array(offset)?))
    }
    /// A big-endian f32 widened to a Python float.
    pub fn f32be(&self, offset: usize) -> Result<f64, Error> {
        Ok(f32::from_be_bytes(self.array(offset)?) as f64)
    }
    /// NUL-terminated UTF-8 (invalid bytes replaced); "" for offset 0 or past the end.
    pub fn cstring(&self, offset: usize) -> String {
        if offset == 0 || offset >= self.data.len() {
            return String::new();
        }
        let tail = &self.data[offset..];
        let end = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
        String::from_utf8_lossy(&tail[..end]).into_owned()
    }
}

pub fn align(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

/// `pathlib.PurePath(name).suffix`: the last dot-part of a name, if any.
pub fn suffix_start(name: &str) -> Option<usize> {
    let leaf_start = name.rfind('/').map_or(0, |i| i + 1);
    let leaf = &name[leaf_start..];
    match leaf.rfind('.') {
        Some(i) if i > 0 && i < leaf.len() - 1 => Some(leaf_start + i),
        _ => None,
    }
}

/// `str(PurePath(path).with_suffix(new))` for a `/`-separated path.
pub fn with_suffix(path: &str, new: &str) -> String {
    let stem = suffix_start(path).map_or(path, |i| &path[..i]);
    format!("{stem}{new}")
}
