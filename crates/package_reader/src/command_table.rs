//! 03system.bin `cmd` table (FUN_1403b68b0 finds a command by id): u32
//! version, u32 count, then 0x30-byte records.
use std::io;

pub const RECORD_SIZE: usize = 0x30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub id: u16,
    /// +8: the command's name message.
    pub name: u32,
}

pub fn read(bytes: &[u8]) -> io::Result<Vec<Command>> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "command table truncated");
    let count = bytes
        .get(4..8)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()) as usize)
        .ok_or_else(invalid)?;
    let records = bytes.get(8..8 + count * RECORD_SIZE).ok_or_else(invalid)?;
    Ok(records
        .as_chunks::<RECORD_SIZE>()
        .0
        .iter()
        .map(|r| Command {
            id: u16::from_le_bytes([r[0], r[1]]),
            name: u32::from_le_bytes([r[8], r[9], r[10], r[11]]),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ids_and_names() {
        let mut b = vec![2, 0, 0, 0, 1, 0, 0, 0];
        let mut r = [0u8; RECORD_SIZE];
        r[0] = 0x17;
        r[8..12].copy_from_slice(&0x81e0u32.to_le_bytes());
        b.extend(r);
        let c = read(&b).unwrap();
        assert_eq!(
            c,
            vec![Command {
                id: 0x17,
                name: 0x81e0
            }]
        );
        assert!(read(&b[..20]).is_err());
    }
}
