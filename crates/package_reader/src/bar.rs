//! BAR table inspection, based on the pinned OpenKH reader; no entry type is guessed.
use serde::Serialize;
use std::io;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct BarEntry {
    pub entry_type: u16,
    pub index: i16,
    pub name_bytes: [u8; 4],
    pub offset: i32,
    pub size: u32,
}

#[derive(Debug, Serialize)]
pub struct BarInventory {
    pub header_field_8: i32,
    pub header_field_12: i32,
    pub absent_offsets: usize,
    pub table_overlapping_entries: usize,
    pub entries: Vec<BarEntry>,
}

pub fn read_bar(bytes: &[u8]) -> io::Result<BarInventory> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    if bytes.len() < 16 || !bytes.starts_with(b"BAR\x01") {
        return Err(invalid("invalid BAR signature/header"));
    }
    let count = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if count < 0 {
        return Err(invalid("negative BAR entry count"));
    }
    let table_end = 16u64 + count as u64 * 16;
    if table_end > bytes.len() as u64 {
        return Err(invalid("truncated BAR table"));
    }
    let mut entries = Vec::with_capacity(count as usize);
    let mut absent_offsets = 0;
    let mut table_overlapping_entries = 0;
    for record in bytes[16..table_end as usize].as_chunks::<16>().0 {
        let offset = i32::from_le_bytes(record[8..12].try_into().unwrap());
        let size = i32::from_le_bytes(record[12..16].try_into().unwrap());
        if offset < -1 || size < 0 {
            return Err(invalid("invalid signed BAR range"));
        }
        if offset == -1 {
            absent_offsets += 1;
        } else {
            if offset as u64 + size as u64 > bytes.len() as u64 {
                return Err(invalid("BAR payload range exceeds original"));
            }
            if size != 0 && (offset as u64) < table_end {
                table_overlapping_entries += 1;
            }
        }
        entries.push(BarEntry {
            entry_type: u16::from_le_bytes(record[..2].try_into().unwrap()),
            index: i16::from_le_bytes(record[2..4].try_into().unwrap()),
            name_bytes: record[4..8].try_into().unwrap(),
            offset,
            size: size as u32,
        });
    }
    Ok(BarInventory {
        header_field_8: i32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        header_field_12: i32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        absent_offsets,
        table_overlapping_entries,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(offset: i32, size: i32) -> Vec<u8> {
        let mut bytes = b"BAR\x01".to_vec();
        bytes.extend(1i32.to_le_bytes());
        bytes.extend(7i32.to_le_bytes());
        bytes.extend(2i32.to_le_bytes());
        bytes.extend(65535u16.to_le_bytes());
        bytes.extend((-2i16).to_le_bytes());
        bytes.extend(b"test");
        bytes.extend(offset.to_le_bytes());
        bytes.extend(size.to_le_bytes());
        bytes.extend(b"DATA");
        bytes
    }
    #[test]
    fn preserves_unknown_types_signed_indices_and_absent_ranges() {
        let result = read_bar(&fixture(32, 4)).unwrap();
        assert_eq!(
            result.entries[0],
            BarEntry {
                entry_type: 65535,
                index: -2,
                name_bytes: *b"test",
                offset: 32,
                size: 4
            }
        );
        assert_eq!(result.header_field_8, 7);
        assert_eq!(result.header_field_12, 2);
        assert_eq!(read_bar(&fixture(-1, 4)).unwrap().absent_offsets, 1);
        assert_eq!(
            read_bar(&fixture(0, 4)).unwrap().table_overlapping_entries,
            1
        );
    }
    #[test]
    fn rejects_bad_signature_counts_and_ranges() {
        assert!(read_bar(&[]).is_err());
        let mut bytes = fixture(32, 4);
        bytes[0] = 0;
        assert!(read_bar(&bytes).is_err());
        for count in [-1, i32::MAX] {
            let mut bytes = fixture(32, 4);
            bytes[4..8].copy_from_slice(&count.to_le_bytes());
            assert!(read_bar(&bytes).is_err());
        }
        for (offset, size) in [(-2, 1), (32, -1), (33, 4), (i32::MAX, i32::MAX)] {
            assert!(read_bar(&fixture(offset, size)).is_err());
        }
    }
}
