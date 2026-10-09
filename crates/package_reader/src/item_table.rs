//! 03system.bin `item` table as the menus read it (FUN_1403e3960 iterates
//! the records in file order, FUN_1403e39a0 finds one by id): u32 version,
//! u32 count, then 0x18-byte records.
use std::io;

fn invalid(m: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, m.to_owned())
}

pub const RECORD_SIZE: usize = 0x18;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: u16,
    /// +2: category (0 consumable, 1 boost/Tent/Drive Recovery, 2..13
    /// weapons, 14 armor, 15 accessory, 16 material, 17 key item, ...).
    pub kind: u8,
    /// +3: bit 0 = the count is one bit of the save's flag field.
    pub flags: u8,
    /// +4: consumable effect amount, or the armor/accessory stat entry.
    pub amount: u16,
    /// +6: the weapon stat entry.
    pub weapon_stats: u16,
    /// +8 / +0xa: name and description message ids.
    pub name: u16,
    pub description: u16,
    pub buy: u16,
    /// +0xe: non-zero when it can be thrown away.
    pub sell: u16,
    /// +0x12: the count byte / flag bit index.
    pub slot: u16,
    /// +0x14: `itempic/item-%03d.imd`.
    pub picture: u16,
    /// +0x17: the inline icon glyph before the name.
    pub icon: u8,
}

pub fn read(bytes: &[u8]) -> io::Result<Vec<Item>> {
    let word = |at: usize| -> io::Result<u32> {
        bytes
            .get(at..at + 4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .ok_or_else(|| invalid("item table truncated"))
    };
    let count = word(4)? as usize;
    let end = count
        .checked_mul(RECORD_SIZE)
        .and_then(|n| n.checked_add(8))
        .ok_or_else(|| invalid("item count overflows"))?;
    let records = bytes
        .get(8..end)
        .ok_or_else(|| invalid("item table truncated"))?;
    Ok(records
        .as_chunks::<RECORD_SIZE>()
        .0
        .iter()
        .map(|r| {
            let h = |at: usize| u16::from_le_bytes([r[at], r[at + 1]]);
            Item {
                id: h(0),
                kind: r[2],
                flags: r[3],
                amount: h(4),
                weapon_stats: h(6),
                name: h(8),
                description: h(0xa),
                buy: h(0xc),
                sell: h(0xe),
                slot: h(0x12),
                picture: h(0x14),
                icon: r[0x17],
            }
        })
        .collect())
}

/// An equipment stat record (the table after the items; FUN_14041e8f0
/// binary-searches it by id, 0x10 bytes each).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub id: u16,
    /// +2: the ability the equipment gives (0 = none).
    pub ability: u16,
    pub strength: u8,
    pub magic: u8,
    pub defense: u8,
    pub ap: u8,
}

/// The stat table that follows the item records in the same subfile.
pub fn read_stats(bytes: &[u8]) -> io::Result<Vec<Stats>> {
    let word = |at: usize| -> io::Result<u32> {
        bytes
            .get(at..at + 4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .ok_or_else(|| invalid("item table truncated"))
    };
    let start = 8 + word(4)? as usize * RECORD_SIZE;
    let count = word(start + 4)? as usize;
    let records = bytes
        .get(start + 8..start + 8 + count * 0x10)
        .ok_or_else(|| invalid("stat table truncated"))?;
    Ok(records
        .as_chunks::<0x10>()
        .0
        .iter()
        .map(|r| Stats {
            id: u16::from_le_bytes([r[0], r[1]]),
            ability: u16::from_le_bytes([r[2], r[3]]),
            strength: r[4],
            magic: r[5],
            defense: r[6],
            ap: r[7],
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_records_in_file_order() {
        let mut b = vec![6, 0, 0, 0, 2, 0, 0, 0];
        for (id, kind, icon) in [(7u16, 0u8, 0u8), (0x29, 2, 4)] {
            let mut r = [0u8; RECORD_SIZE];
            r[0..2].copy_from_slice(&id.to_le_bytes());
            r[2] = kind;
            r[4] = 40;
            r[8..10].copy_from_slice(&0x82f8u16.to_le_bytes());
            r[0xe] = 10;
            r[0x12] = 3;
            r[0x17] = icon;
            b.extend(r);
        }
        let items = read(&b).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, 7);
        assert_eq!(items[0].amount, 40);
        assert_eq!(items[0].name, 0x82f8);
        assert_eq!(items[0].sell, 10);
        assert_eq!(items[1].kind, 2);
        assert_eq!(items[1].icon, 4);
        assert!(read(&b[..b.len() - 1]).is_err());
        assert!(read_stats(&b).is_err());
        b.extend([0, 0, 0, 0, 1, 0, 0, 0]);
        b.extend([
            0x50, 0, 0x1e, 0x02, 3, 1, 0, 0, 100, 100, 100, 100, 100, 100, 100, 0,
        ]);
        let stats = read_stats(&b).unwrap();
        assert_eq!(stats[0].id, 0x50);
        assert_eq!(stats[0].ability, 0x21e);
        assert_eq!((stats[0].strength, stats[0].magic), (3, 1));
    }
}
