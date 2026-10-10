//! `menu/<lang>/jiminy.bar` tables (Tz::MenuFile, FUN_1402f9a70): each BAR
//! entry has a 16-byte header (magic `JM..`, version, record count at +8)
//! and fixed-size records from +0x10.
use std::io;

/// A `worl` record (16 B): a journal world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    /// +0: journal world index (0..15; 0x0f Collection, 0x10 Other).
    pub world: i8,
    /// +1: the page file code (`jm_top_<code>.2ld`).
    pub code: String,
    /// +4 / +6: names; +0xa / +0xc the alternate names.
    pub names: [u16; 2],
    pub alt_names: [u16; 2],
    /// +8: the story flag that lists the world; +0xe the one that switches
    /// to the alternate names.
    pub unlock: u16,
    pub alt_flag: u16,
}

/// A `stor` record (12 B): one story step of a world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Story {
    pub world: i8,
    /// +4 Summary, +6 Objective, +8 the Story page entry.
    pub summary: u16,
    pub objective: u16,
    pub entry: u16,
    /// +0xa: the story flag that reaches this step.
    pub flag: u16,
}

/// A `limi` record (8 B): a Limit's command id and texts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub id: u16,
    pub name: u16,
    pub description: u16,
}

fn records(bytes: &[u8], size: usize) -> io::Result<impl Iterator<Item = &[u8]>> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "journal table truncated");
    let count = bytes
        .get(8..12)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()) as usize)
        .ok_or_else(invalid)?;
    let body = bytes.get(0x10..0x10 + count * size).ok_or_else(invalid)?;
    Ok(body.chunks_exact(size))
}

fn u16_at(r: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([r[at], r[at + 1]])
}

pub fn read_worlds(bytes: &[u8]) -> io::Result<Vec<World>> {
    Ok(records(bytes, 16)?
        .map(|r| World {
            world: r[0] as i8,
            code: r[1..4]
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as char)
                .collect(),
            names: [u16_at(r, 4), u16_at(r, 6)],
            unlock: u16_at(r, 8),
            alt_names: [u16_at(r, 0xa), u16_at(r, 0xc)],
            alt_flag: u16_at(r, 0xe),
        })
        .collect())
}

pub fn read_stories(bytes: &[u8]) -> io::Result<Vec<Story>> {
    Ok(records(bytes, 12)?
        .map(|r| Story {
            world: r[0] as i8,
            summary: u16_at(r, 4),
            objective: u16_at(r, 6),
            entry: u16_at(r, 8),
            flag: u16_at(r, 0xa),
        })
        .collect())
}

pub fn read_limits(bytes: &[u8]) -> io::Result<Vec<Limit>> {
    Ok(records(bytes, 8)?
        .map(|r| Limit {
            id: u16_at(r, 0),
            name: u16_at(r, 2),
            description: u16_at(r, 4),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(count: u32, records: &[u8]) -> Vec<u8> {
        let mut b = b"JM\0\0".to_vec();
        b.extend(18u32.to_le_bytes());
        b.extend(count.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend_from_slice(records);
        b
    }

    #[test]
    fn reads_worlds_stories_and_limits() {
        let mut w = vec![0u8; 16];
        w[0] = 1;
        w[1..3].copy_from_slice(b"hb");
        w[4..6].copy_from_slice(&0x26b4u16.to_le_bytes());
        w[8..10].copy_from_slice(&0x1002u16.to_le_bytes());
        w[0xe..0x10].copy_from_slice(&0x108bu16.to_le_bytes());
        let worlds = read_worlds(&table(1, &w)).unwrap();
        assert_eq!(worlds[0].code, "hb");
        assert_eq!(worlds[0].names[0], 0x26b4);
        assert_eq!(worlds[0].unlock, 0x1002);
        assert_eq!(worlds[0].alt_flag, 0x108b);
        let mut s = vec![0u8; 12];
        s[0] = 4;
        s[4..6].copy_from_slice(&0x29a4u16.to_le_bytes());
        s[0xa..0xc].copy_from_slice(&0x1c24u16.to_le_bytes());
        let stories = read_stories(&table(1, &s)).unwrap();
        assert_eq!((stories[0].world, stories[0].summary), (4, 0x29a4));
        assert_eq!(stories[0].flag, 0x1c24);
        let l = [0x1a, 0x01, 0xd4, 0x4a, 0xd5, 0x4a, 0, 0];
        assert_eq!(read_limits(&table(1, &l)).unwrap()[0].id, 0x11a);
        assert!(read_limits(&table(2, &l)).is_err());
    }
}
