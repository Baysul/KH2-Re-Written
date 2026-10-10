//! Raw 00objentry records, following pinned OpenKH Objentry.cs
//! (7a3b945c538d32c6a285128c98aefba093f52ceb): a two-word header, then
//! 0x60-byte entries. Only fields used by the test room are exposed.
use serde::Serialize;
use std::io;

pub const ENTRY_SIZE: usize = 0x60;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ObjectEntry {
    pub object_id: u32,
    pub object_type: u8,
    pub sub_type: u8,
    pub draw_priority: u8,
    pub weapon_joint: u8,
    pub model_name: Vec<u8>,
    pub animation_name: Vec<u8>,
    pub neo_moveset: u16,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn name(bytes: &[u8]) -> Vec<u8> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    bytes[..end].to_vec()
}

pub fn read(bytes: &[u8]) -> io::Result<Vec<ObjectEntry>> {
    if bytes.len() < 8 || bytes.len() > 4 * 1024 * 1024 {
        return Err(invalid("object entry table outside inspection bounds"));
    }
    let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let body = bytes
        .get(
            8..8 + count
                .checked_mul(ENTRY_SIZE)
                .ok_or_else(|| invalid("count overflow"))?,
        )
        .ok_or_else(|| invalid("object entry table exceeds payload"))?;
    Ok(body
        .as_chunks::<ENTRY_SIZE>()
        .0
        .iter()
        .map(|e| ObjectEntry {
            object_id: u32::from_le_bytes(e[0..4].try_into().unwrap()),
            object_type: e[4],
            sub_type: e[5],
            draw_priority: e[6],
            weapon_joint: e[7],
            model_name: name(&e[8..40]),
            animation_name: name(&e[40..72]),
            neo_moveset: u16::from_le_bytes(e[78..80].try_into().unwrap()),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_names_and_weapon_joint() {
        let mut bytes = vec![0; 8 + 2 * ENTRY_SIZE];
        bytes[0..4].copy_from_slice(&3u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        let second = 8 + ENTRY_SIZE;
        bytes[second..second + 4].copy_from_slice(&0x54u32.to_le_bytes());
        bytes[second + 7] = 178;
        bytes[second + 8..second + 15].copy_from_slice(b"P_TEST1");
        bytes[second + 40..second + 44].copy_from_slice(b"ANIM");
        bytes[second + 78..second + 80].copy_from_slice(&9u16.to_le_bytes());
        let entries = read(&bytes).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].object_id, 0x54);
        assert_eq!(entries[1].weapon_joint, 178);
        assert_eq!(entries[1].model_name, b"P_TEST1");
        assert_eq!(entries[1].animation_name, b"ANIM");
        assert_eq!(entries[1].neo_moveset, 9);
        assert!(entries[0].model_name.is_empty());
    }

    #[test]
    fn rejects_truncated_tables() {
        let mut bytes = vec![0; 8 + ENTRY_SIZE];
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        assert!(read(&bytes).is_err());
        assert!(read(&bytes[..4]).is_err());
    }
}
