//! Message record spans, following pinned OpenKH Msg.Read/GetMsgData.
//! Copyright 2019-2022 OpenKH Team and Contributors; Apache-2.0, see NOTICE.
//! Changes: preserve source offsets, bounded scans, opaque encoded bytes/hashes.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io,
};

pub const MAX_MESSAGE_RECORDS: usize = 100_000;
pub const MAX_ENCODED_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_SCANNED_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct MessageEntry {
    pub id: i32,
    pub source_offset: u32,
    pub encoded_length: u32,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct MessageInventory {
    pub table_end: u32,
    pub duplicate_ids: usize,
    pub repeated_offsets: usize,
    pub table_overlapping_records: usize,
    pub entries: Vec<MessageEntry>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn encoded_length(bytes: &[u8]) -> io::Result<usize> {
    let mut cursor = 0;
    loop {
        if cursor >= MAX_ENCODED_MESSAGE_BYTES {
            return Err(invalid("encoded message exceeds inspection limit"));
        }
        let opcode = *bytes
            .get(cursor)
            .ok_or_else(|| invalid("unterminated message"))?;
        cursor += 1;
        let parameters = match opcode {
            0x04 | 0x06 | 0x09..=0x0c | 0x0e | 0x16 | 0x19..=0x1f => 1,
            0x12 | 0x14 | 0x15 | 0x18 => 2,
            0x08 => 3,
            0x07 | 0x11 | 0x13 => 4,
            0x0f => 5,
            0x05 => 6,
            _ => 0,
        };
        cursor += parameters;
        if cursor > bytes.len() {
            return Err(invalid("truncated message command parameters"));
        }
        if cursor > MAX_ENCODED_MESSAGE_BYTES {
            return Err(invalid("encoded message exceeds inspection limit"));
        }
        if opcode == 0 {
            return Ok(cursor);
        }
    }
}

/// Inspect structure and exact encoded ranges; no glyph/Unicode mapping is asserted.
pub fn read_messages(bytes: &[u8]) -> io::Result<MessageInventory> {
    if bytes.len() < 8 || bytes[..4] != 1u32.to_le_bytes() {
        return Err(invalid("invalid message header"));
    }
    let count = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if count < 0 || count as usize > MAX_MESSAGE_RECORDS {
        return Err(invalid("invalid/oversized message table count"));
    }
    let table_end = 8 + count as usize * 8;
    if table_end > bytes.len() {
        return Err(invalid("truncated message table"));
    }
    let mut entries = Vec::with_capacity(count as usize);
    let mut ids = HashSet::new();
    let mut ranges: HashMap<usize, (usize, String)> = HashMap::new();
    let mut duplicate_ids = 0;
    let mut repeated_offsets = 0;
    let mut table_overlapping_records = 0;
    let mut scanned = 0;
    for record in bytes[8..table_end].as_chunks::<8>().0 {
        let id = i32::from_le_bytes(record[..4].try_into().unwrap());
        let offset = i32::from_le_bytes(record[4..8].try_into().unwrap());
        if offset < 0 || offset as usize >= bytes.len() {
            return Err(invalid("message offset exceeds source"));
        }
        let start = offset as usize;
        if !ids.insert(id) {
            duplicate_ids += 1;
        }
        if start < table_end {
            table_overlapping_records += 1;
        }
        let (length, hash) = if let Some((length, hash)) = ranges.get(&start) {
            repeated_offsets += 1;
            (*length, hash.clone())
        } else {
            let length = encoded_length(&bytes[start..])?;
            scanned += length;
            if scanned > MAX_SCANNED_BYTES {
                return Err(invalid("message scan budget exceeded"));
            }
            let hash = format!("{:x}", Sha256::digest(&bytes[start..start + length]));
            ranges.insert(start, (length, hash.clone()));
            (length, hash)
        };
        entries.push(MessageEntry {
            id,
            source_offset: offset as u32,
            encoded_length: length as u32,
            sha256: hash,
        });
    }
    Ok(MessageInventory {
        table_end: table_end as u32,
        duplicate_ids,
        repeated_offsets,
        table_overlapping_records,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(records: &[(i32, i32)], payload: &[u8]) -> Vec<u8> {
        let mut bytes = 1u32.to_le_bytes().to_vec();
        bytes.extend((records.len() as i32).to_le_bytes());
        for &(id, offset) in records {
            bytes.extend(id.to_le_bytes());
            bytes.extend(offset.to_le_bytes());
        }
        bytes.extend(payload);
        bytes
    }

    #[test]
    fn preserves_signed_ids_shared_offsets_suffixes_and_parameter_zeroes() {
        let bytes = fixture(&[(-4, 32), (-4, 32), (7, 35)], &[4, 0, 0x90, 0x91, 0]);
        let result = read_messages(&bytes).unwrap();
        assert_eq!(result.entries.len(), 3);
        assert_eq!(result.duplicate_ids, 1);
        assert_eq!(result.repeated_offsets, 1);
        assert_eq!(result.entries[0].id, -4);
        assert_eq!(result.entries[0].encoded_length, 5);
        assert_eq!(result.entries[2].encoded_length, 2);
        assert_eq!(result.entries[0].sha256, result.entries[1].sha256);
        assert_ne!(result.entries[0].sha256, result.entries[2].sha256);
    }

    #[test]
    fn skips_complete_parameters_for_each_command_arity() {
        for (opcode, arity) in [(4, 1), (0x12, 2), (8, 3), (7, 4), (0x0f, 5), (5, 6)] {
            let mut payload = vec![opcode];
            payload.extend(vec![0; arity]);
            payload.extend([0x90, 0]);
            assert_eq!(
                read_messages(&fixture(&[(1, 16)], &payload))
                    .unwrap()
                    .entries[0]
                    .encoded_length as usize,
                arity + 3
            );
        }
    }

    #[test]
    fn rejects_bad_counts_offsets_truncated_parameters_and_missing_terminators() {
        assert!(read_messages(&[]).is_err());
        for count in [-1, 1, i32::MAX] {
            let mut bytes = fixture(&[], &[]);
            bytes[4..8].copy_from_slice(&count.to_le_bytes());
            assert!(read_messages(&bytes).is_err());
        }
        for offset in [-1, 17, i32::MAX] {
            assert!(read_messages(&fixture(&[(1, offset)], &[0])).is_err());
        }
        for payload in [
            vec![4],
            vec![5, 0, 0, 0, 0, 0],
            vec![0x90],
            vec![0x90; MAX_ENCODED_MESSAGE_BYTES + 1],
        ] {
            assert!(read_messages(&fixture(&[(1, 16)], &payload)).is_err());
        }
    }
}
