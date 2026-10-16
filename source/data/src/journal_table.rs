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

/// A `char` record (36 B): a Journal character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Char {
    /// +0: journal world (0..14, 0x10 Character Links, 0x11 Heartless, 0x12
    /// Nobodies).
    pub world: i8,
    /// +1: the face's IMGD in its `jmface` file.
    pub image: u8,
    /// +2: the portrait frame type 0..5.
    pub frame: u8,
    /// +4: the seen / NEW bit.
    pub bit: u16,
    /// +6 / +8: name and description messages.
    pub name: u16,
    pub description: u16,
    /// +0xa: the origin note message (0 = none).
    pub origin: u16,
    /// +0x10: flag 0x800 = a kill counter exists.
    pub flags: u16,
}

pub fn read_chars(bytes: &[u8]) -> io::Result<Vec<Char>> {
    Ok(records(bytes, 36)?
        .map(|r| Char {
            world: r[0] as i8,
            image: r[1],
            frame: r[2],
            bit: u16_at(r, 4),
            name: u16_at(r, 6),
            description: u16_at(r, 8),
            origin: u16_at(r, 0xa),
            flags: u16_at(r, 0x10),
        })
        .collect())
}

/// One `diag` entry (8 B): a character placed on a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagEntry {
    /// `char` row.
    pub character: u16,
    pub at: [i16; 2],
    /// 0 = only feeds the availability bitmap.
    pub visible: bool,
}

/// A `diag` record (12 B): the layout of a world's Characters page, or an
/// extra that joins it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diag {
    /// +0: the story flag (kind 0 / 1) or item id (kind 2) that activates it.
    pub cond: u16,
    /// +2: a story flag that switches it off (0 = none).
    pub off: u16,
    /// +4: journal world, 0x10 Character Links.
    pub world: i8,
    /// +6: 0 base layout, 1 extra by flag, 2 extra by item.
    pub kind: i8,
    pub entries: Vec<DiagEntry>,
}

pub fn read_diag(bytes: &[u8]) -> io::Result<Vec<Diag>> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "diag table truncated");
    let mut out = Vec::new();
    for r in records(bytes, 12)? {
        let count = r[5] as usize;
        let at = u32::from_le_bytes([r[8], r[9], r[10], r[11]]) as usize;
        let body = bytes.get(at..at + count * 8).ok_or_else(invalid)?;
        out.push(Diag {
            cond: u16_at(r, 0),
            off: u16_at(r, 2),
            world: r[4] as i8,
            kind: r[6] as i8,
            entries: body
                .as_chunks::<8>()
                .0
                .iter()
                .map(|e| DiagEntry {
                    character: u16_at(e, 0),
                    at: [u16_at(e, 2) as i16, u16_at(e, 4) as i16],
                    visible: e[6] != 0,
                })
                .collect(),
        });
    }
    Ok(out)
}

/// An `albu` record (12 B): an Album photo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Album {
    /// +0: journal world.
    pub world: i8,
    /// +1: the two-digit photo suffix (`menu/<lang>/jm_photo/<code><nn>.bin`).
    pub suffix: String,
    /// +6: the story flag that unlocks it.
    pub flag: u16,
    /// +8 / +0xa: title and caption messages.
    pub title: u16,
    pub caption: u16,
}

pub fn read_albums(bytes: &[u8]) -> io::Result<Vec<Album>> {
    Ok(records(bytes, 12)?
        .map(|r| Album {
            world: r[0] as i8,
            suffix: r[1..3].iter().map(|&b| b as char).collect(),
            flag: u16_at(r, 6),
            title: u16_at(r, 8),
            caption: u16_at(r, 0xa),
        })
        .collect())
}
/// An `anse` record (8 B): an Ansem Report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    /// +0: the report item.
    pub item: u16,
    /// +2 / +4: title and text messages.
    pub title: u16,
    pub text: u16,
}

pub fn read_reports(bytes: &[u8]) -> io::Result<Vec<Report>> {
    Ok(records(bytes, 8)?
        .map(|r| Report {
            item: u16_at(r, 0),
            title: u16_at(r, 2),
            text: u16_at(r, 4),
        })
        .collect())
}

/// A `trsr` record (03system.bin, 12 B at +4): a treasure chest or an
/// event gift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Treasure {
    /// +2: the item inside.
    pub item: u16,
    /// +4: 0 chest, 1 event gift.
    pub kind: u8,
    /// +5: the game world.
    pub game_world: u8,
    /// +8: the story flag that makes it available.
    pub flag: u16,
    /// +10: the opened bit (save+0x23ac); 0 = none.
    pub opened: u16,
}

pub fn read_treasures(bytes: &[u8]) -> io::Result<Vec<Treasure>> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "trsr table truncated");
    let count = u16::from_le_bytes(bytes.get(2..4).ok_or_else(invalid)?.try_into().unwrap());
    let body = bytes.get(4..4 + count as usize * 12).ok_or_else(invalid)?;
    Ok(body
        .as_chunks::<12>()
        .0
        .iter()
        .map(|r| Treasure {
            item: u16_at(r, 2),
            kind: r[4],
            game_world: r[5],
            flag: u16_at(r, 8),
            opened: u16_at(r, 10),
        })
        .collect())
}

/// FUN_1402f23d0: the journal world of a game world, -1 for none.
pub fn journal_world(game_world: u8) -> i8 {
    match game_world {
        1 | 6 => 3,
        2 => 0,
        4 => 1,
        5 => 2,
        7 => 4,
        8 => 5,
        9 => 6,
        10 => 7,
        11 => 8,
        12 => 9,
        13 => 10,
        14 => 11,
        16 => 12,
        17 => 13,
        18 => 14,
        _ => -1,
    }
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

    #[test]
    fn reads_chars_diag_and_albums() {
        let mut c = vec![0u8; 36];
        c[0] = 4;
        c[1] = 2;
        c[2] = 1;
        c[4..6].copy_from_slice(&9u16.to_le_bytes());
        c[6..8].copy_from_slice(&0x2e9eu16.to_le_bytes());
        c[0xa..0xc].copy_from_slice(&0x2868u16.to_le_bytes());
        c[0x10..0x12].copy_from_slice(&0x800u16.to_le_bytes());
        let chars = read_chars(&table(1, &c)).unwrap();
        assert_eq!((chars[0].world, chars[0].image, chars[0].frame), (4, 2, 1));
        assert_eq!((chars[0].bit, chars[0].name), (9, 0x2e9e));
        assert_eq!((chars[0].origin, chars[0].flags), (0x2868, 0x800));
        // One record at +0x10 + 12 whose single entry follows it.
        let mut d = vec![0u8; 12];
        d[0..2].copy_from_slice(&0x895u16.to_le_bytes());
        d[4] = 0;
        d[5] = 1;
        d[8..12].copy_from_slice(&0x1cu32.to_le_bytes());
        d.extend([0x8b, 0, 0xed, 0, 0xfd, 0, 1, 0]);
        let diag = read_diag(&table(1, &d)).unwrap();
        assert_eq!(diag[0].cond, 0x895);
        assert_eq!(diag[0].entries[0].character, 0x8b);
        assert_eq!(diag[0].entries[0].at, [237, 253]);
        assert!(diag[0].entries[0].visible);
        let mut a = vec![0u8; 12];
        a[0] = 4;
        a[1..3].copy_from_slice(b"11");
        a[6..8].copy_from_slice(&0x1c14u16.to_le_bytes());
        a[8..10].copy_from_slice(&0x2c46u16.to_le_bytes());
        let albums = read_albums(&table(1, &a)).unwrap();
        assert_eq!(albums[0].suffix, "11");
        assert_eq!(albums[0].flag, 0x1c14);
        let mut s = vec![0u8; 4 + 12];
        s[2] = 1;
        s[6..8].copy_from_slice(&130u16.to_le_bytes());
        s[8] = 1;
        s[9] = 8;
        s[12..14].copy_from_slice(&0x2040u16.to_le_bytes());
        s[14..16].copy_from_slice(&20u16.to_le_bytes());
        let t = read_treasures(&s).unwrap();
        assert_eq!((t[0].item, t[0].kind, t[0].game_world), (130, 1, 8));
        assert_eq!((t[0].flag, t[0].opened), (0x2040, 20));
        assert_eq!(journal_world(8), 5);
    }
}
