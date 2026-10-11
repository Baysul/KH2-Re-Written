//! EB BIG v3 archives with RefPack and chunkref payloads
//! (tools/owned_game/big.py, refpack.py). All fields big-endian.
use crate::{Error, ReadAt, be16, be32};
use std::io::Read;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub offset: u64,
    pub stored_size: u32,
    pub unpacked_size: u32,
    pub compression: u8,
}

pub struct Big<R> {
    reader: R,
    pub entries: Vec<Entry>,
}

fn align(value: usize, to: usize) -> usize {
    value.div_ceil(to) * to
}

fn bytes(data: &[u8], at: usize, len: usize) -> Result<&[u8], Error> {
    at.checked_add(len)
        .and_then(|end| data.get(at..end))
        .ok_or(Error::Format("truncated BIG metadata"))
}

impl<R: ReadAt> Big<R> {
    pub fn open(mut reader: R) -> Result<Self, Error> {
        let file_size = reader.len();
        let header = reader.read_at(0, 48)?;
        let index_size = be32(&header, 12)? as u64;
        let names_size = be32(&header, 16)? as u64;
        let metadata_size = index_size + names_size;
        if metadata_size > file_size || metadata_size > 64 << 20 {
            return Err(Error::Format("BIG index/name blocks exceed file"));
        }
        let meta = reader.read_at(0, metadata_size as usize)?;
        let entries = parse(&meta, file_size)?;
        Ok(Big { reader, entries })
    }

    pub fn find(&self, path: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|e| e.path.eq_ignore_ascii_case(path))
    }

    pub fn read(&mut self, entry: &Entry) -> Result<Vec<u8>, Error> {
        let packed = self
            .reader
            .read_at(entry.offset, entry.stored_size as usize)?;
        let expected = entry.unpacked_size as usize;
        let decoded = match entry.compression {
            0 => packed,
            1 => refpack(&packed, Some(expected))?,
            2..=4 => chunkref(&packed, expected)?,
            _ => return Err(Error::Format("unsupported BIG compression")),
        };
        if decoded.len() != expected {
            return Err(Error::Format("BIG entry decoded to the wrong size"));
        }
        Ok(decoded)
    }
}

fn parse(m: &[u8], file_size: u64) -> Result<Vec<Entry>, Error> {
    if be16(m, 0)? != 0x4542 {
        return Err(Error::Format("not an EB BIG archive"));
    }
    if be16(m, 2)? != 3 {
        return Err(Error::Format("unsupported EB BIG version"));
    }
    let count = be32(m, 4)? as usize;
    if count > 1_000_000 {
        return Err(Error::Format("unreasonable BIG entry count"));
    }
    let flags = be16(m, 8)?;
    let shift = bytes(m, 10, 1)?[0] as u32;
    if shift > 31 {
        return Err(Error::Format("invalid BIG alignment shift"));
    }
    let index_size = be32(m, 12)? as usize;
    let names_size = be32(m, 16)? as usize;
    let name_record = bytes(m, 20, 1)?[0] as usize;
    let dir_record = bytes(m, 21, 1)?[0] as usize;
    if name_record < 3 || dir_record < 2 {
        return Err(Error::Format("invalid BIG name record sizing"));
    }
    let entry_size = if flags & 1 != 0 { 20 } else { 16 };
    let entries_start = 48;
    let compression_start = entries_start + align(entry_size * count, 16);
    let names_start = index_size;
    let directories_start = names_start + align(name_record * count, 16);
    bytes(m, entries_start, entry_size * count)?;
    bytes(m, compression_start, count)?;
    bytes(m, names_start, name_record * count)?;
    let text = |raw: &[u8]| {
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        String::from_utf8_lossy(&raw[..end]).into_owned()
    };
    let mut directories = Vec::new();
    let mut cursor = directories_start;
    while cursor + dir_record <= index_size + names_size {
        directories.push(text(bytes(m, cursor, dir_record)?));
        cursor += dir_record;
    }
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let at = entries_start + index * entry_size;
        let offset = (be32(m, at)? as u64) << shift;
        let declared = be32(m, at + 4)?;
        let unpacked_size = match be32(m, at + 8)? {
            0 => declared,
            n => n,
        };
        let stored_size = if declared == 0 {
            unpacked_size
        } else {
            declared
        };
        let compression = m[compression_start + index];
        let record = names_start + index * name_record;
        let dir = be16(m, record)? as usize;
        let file = text(bytes(m, record + 2, name_record - 2)?);
        let dir = directories.get(dir).map_or(".", String::as_str);
        let path = if dir.is_empty() || dir == "." {
            file
        } else {
            format!("{dir}/{file}")
        };
        if offset > file_size.saturating_sub(stored_size as u64) {
            return Err(Error::Format("BIG payload exceeds archive"));
        }
        out.push(Entry {
            path: path.replace('\\', "/"),
            offset,
            stored_size,
            unpacked_size,
            compression,
        });
    }
    Ok(out)
}

fn chunkref(data: &[u8], expected: usize) -> Result<Vec<u8>, Error> {
    if data.get(..8) != Some(b"chunkref".as_slice()) || be32(data, 8)? != 2 {
        return Err(Error::Format("invalid chunkref header"));
    }
    let total = be32(data, 12)? as usize;
    let chunk_size = be32(data, 16)? as usize;
    let chunk_count = be32(data, 20)? as usize;
    let alignment = be32(data, 24)? as usize;
    if total != expected
        || chunk_size == 0
        || chunk_count == 0
        || alignment == 0
        || alignment > 4096
        || !alignment.is_power_of_two()
    {
        return Err(Error::Format("invalid chunkref dimensions"));
    }
    let mut out = Vec::with_capacity(total);
    let mut cursor = 28;
    for _ in 0..chunk_count {
        let payload = align(cursor + 8, alignment);
        let packed_size = be32(data, payload - 8)? as usize;
        let method = be32(data, payload - 4)?;
        let packed = bytes(data, payload, packed_size)?;
        let expected_chunk = chunk_size.min(total.saturating_sub(out.len()));
        let decoded = match method {
            0 | 4 => packed.to_vec(),
            2 => refpack(packed, Some(expected_chunk))?,
            3 => {
                let mut d = Vec::new();
                flate2::read::ZlibDecoder::new(packed)
                    .take(expected_chunk as u64 + 1)
                    .read_to_end(&mut d)
                    .map_err(|_| Error::Format("chunkref zlib stream is invalid"))?;
                d
            }
            _ => return Err(Error::Format("unsupported chunkref method")),
        };
        if decoded.len() != expected_chunk {
            return Err(Error::Format("chunkref chunk decoded to the wrong size"));
        }
        out.extend_from_slice(&decoded);
        cursor = payload + packed_size;
    }
    if out.len() != total {
        return Err(Error::Format("chunkref produced the wrong size"));
    }
    Ok(out)
}

/// An EA RefPack stream, with or without its 0x10FB / 0x90FB header.
pub fn refpack(data: &[u8], expected: Option<usize>) -> Result<Vec<u8>, Error> {
    if data.len() < 2 {
        return Err(Error::Format("RefPack stream is too short"));
    }
    let mut pos = 0;
    let mut header = None;
    if data[1] == 0xFB && (data[0] == 0x10 || data[0] == 0x90) {
        let n = if data[0] & 0x80 != 0 { 4 } else { 3 };
        let raw = bytes(data, 2, n).map_err(|_| Error::Format("truncated RefPack header"))?;
        header = Some(raw.iter().fold(0usize, |v, &b| v << 8 | b as usize));
        pos = 2 + n;
    }
    let expected = match (expected, header) {
        (Some(e), Some(h)) if e != h => {
            return Err(Error::Format(
                "RefPack header size differs from archive size",
            ));
        }
        (Some(e), _) => Some(e),
        (None, h) => h,
    };
    let mut out: Vec<u8> = Vec::with_capacity(expected.unwrap_or(data.len() * 2));
    let literal = |out: &mut Vec<u8>, pos: &mut usize, n: usize| -> Result<(), Error> {
        let raw = bytes(data, *pos, n).map_err(|_| Error::Format("truncated RefPack literal"))?;
        out.extend_from_slice(raw);
        *pos += n;
        Ok(())
    };
    let backref = |out: &mut Vec<u8>, distance: usize, count: usize| -> Result<(), Error> {
        if distance == 0 || distance > out.len() {
            return Err(Error::Format("RefPack back-reference out of range"));
        }
        for _ in 0..count {
            out.push(out[out.len() - distance]);
        }
        Ok(())
    };
    let byte = |at: usize| {
        data.get(at)
            .copied()
            .map(usize::from)
            .ok_or(Error::Format("truncated RefPack command"))
    };
    while pos < data.len() {
        let c = data[pos] as usize;
        pos += 1;
        if c < 0x80 {
            let b1 = byte(pos)?;
            pos += 1;
            literal(&mut out, &mut pos, c & 3)?;
            backref(&mut out, ((c & 0x60) << 3) + b1 + 1, ((c >> 2) & 7) + 3)?;
        } else if c < 0xC0 {
            let (b1, b2) = (byte(pos)?, byte(pos + 1)?);
            pos += 2;
            literal(&mut out, &mut pos, b1 >> 6)?;
            backref(&mut out, ((b1 & 0x3F) << 8) + b2 + 1, (c & 0x3F) + 4)?;
        } else if c < 0xE0 {
            let (b1, b2, b3) = (byte(pos)?, byte(pos + 1)?, byte(pos + 2)?);
            pos += 3;
            literal(&mut out, &mut pos, c & 3)?;
            backref(
                &mut out,
                ((c & 0x10) << 12) + (b1 << 8) + b2 + 1,
                ((c & 0x0C) << 6) + b3 + 5,
            )?;
        } else if c < 0xFC {
            literal(&mut out, &mut pos, ((c & 0x1F) << 2) + 4)?;
        } else {
            literal(&mut out, &mut pos, c & 3)?;
            break;
        }
        if expected.is_some_and(|e| out.len() > e) {
            return Err(Error::Format("RefPack output exceeds declared size"));
        }
    }
    if expected.is_some_and(|e| out.len() != e) {
        return Err(Error::Format("RefPack produced the wrong size"));
    }
    Ok(out)
}
