//! First two replacement payloads, located by pinned OpenKH sequential alignment.
//! Declared offsets remain opaque; this is a compatibility sample, not a name map.
use crate::{
    HedEntry,
    original::{MAX_ORIGINAL_BYTES, decode_payload},
    read_asset_header,
};
use serde::Serialize;
use std::io::{self, Read, Seek, SeekFrom};

#[derive(Debug, Serialize)]
pub struct ReplacementMetadata {
    pub ordinal: u32,
    pub name_bytes: [u8; 32],
    pub declared_offset: i32,
    pub original_asset_offset: i32,
    pub declared_decompressed_length: u32,
    pub compression_tag: i32,
    pub package_offset: u64,
    pub padded_stored_length: u64,
    pub compressed_bytes_consumed: Option<u64>,
    pub uncompressed_padding_retained: bool,
}

pub struct ReplacementSample {
    pub metadata: ReplacementMetadata,
    pub bytes: Vec<u8>,
}

pub fn read_first_replacements<R: Read + Seek>(
    reader: &mut R,
    entry: &HedEntry,
) -> io::Result<Vec<ReplacementSample>> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    let package_end = reader.seek(SeekFrom::End(0))?;
    let indexed_end = entry
        .package_offset
        .checked_add(entry.stored_length as u64)
        .filter(|end| *end <= package_end)
        .ok_or_else(|| invalid("replacement indexed range exceeds package"))?;
    if entry.stored_length < 16 {
        return Err(invalid("replacement missing original header"));
    }
    reader.seek(SeekFrom::Start(entry.package_offset))?;
    let mut seed = [0; 16];
    reader.read_exact(&mut seed)?;
    let header = read_asset_header(&seed, entry.stored_length)?;
    let original_stored = if header.compression_tag >= 0 {
        header.compression_tag as u64
    } else {
        header.decompressed_length as u64
    };
    let mut payload_start =
        entry.package_offset + 16 + header.remastered_asset_count as u64 * 48 + original_stored;
    let mut samples = Vec::new();
    for ordinal in 0..header.remastered_asset_count.min(2) {
        reader.seek(SeekFrom::Start(
            entry.package_offset + 16 + ordinal as u64 * 48,
        ))?;
        let mut record = [0; 48];
        reader.read_exact(&mut record)?;
        let expected = i32::from_le_bytes(record[40..44].try_into().unwrap());
        let tag = i32::from_le_bytes(record[44..48].try_into().unwrap());
        if expected < 0 || tag < -2 {
            return Err(invalid("invalid replacement size/tag"));
        }
        let stored = if tag >= 0 {
            tag as u64
        } else {
            expected as u64
        };
        let padded = stored.div_ceil(16) * 16;
        if padded > MAX_ORIGINAL_BYTES as u64 || expected as u32 > MAX_ORIGINAL_BYTES {
            return Err(invalid("replacement exceeds 32 MiB inspection limit"));
        }
        payload_start = payload_start
            .checked_add(15)
            .ok_or_else(|| invalid("replacement alignment overflow"))?
            / 16
            * 16;
        let payload_end = payload_start
            .checked_add(padded)
            .filter(|end| *end <= indexed_end)
            .ok_or_else(|| invalid("replacement payload exceeds indexed range"))?;
        reader.seek(SeekFrom::Start(payload_start))?;
        let mut bytes = vec![0; padded as usize];
        reader.read_exact(&mut bytes)?;
        let (bytes, consumed) = decode_payload(&seed, tag, expected as u32, bytes)?;
        samples.push(ReplacementSample {
            metadata: ReplacementMetadata {
                ordinal,
                name_bytes: record[..32].try_into().unwrap(),
                declared_offset: i32::from_le_bytes(record[32..36].try_into().unwrap()),
                original_asset_offset: i32::from_le_bytes(record[36..40].try_into().unwrap()),
                declared_decompressed_length: expected as u32,
                compression_tag: tag,
                package_offset: payload_start,
                padded_stored_length: padded,
                compressed_bytes_consumed: consumed,
                uncompressed_padding_retained: tag < 0,
            },
            bytes,
        });
        payload_start = payload_end;
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::original::transform_prefix;
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::{Cursor, Write};

    fn fixture() -> (Cursor<Vec<u8>>, HedEntry) {
        let mut seed = [0; 16];
        seed[..4].copy_from_slice(&3i32.to_le_bytes());
        seed[4..8].copy_from_slice(&2i32.to_le_bytes());
        seed[8..12].copy_from_slice(&(-2i32).to_le_bytes());
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"synthetic replacement").unwrap();
        let mut compressed = encoder.finish().unwrap();
        let compressed_size = compressed.len() as i32;
        compressed.resize(compressed.len().div_ceil(16) * 16, 0);
        transform_prefix(&seed, &mut compressed);
        let mut bytes = seed.to_vec();
        for (expected, tag) in [(3i32, -2i32), (21, compressed_size)] {
            let mut record = [0; 48];
            record[..4].copy_from_slice(b"name");
            record[32..36].copy_from_slice(&(-123i32).to_le_bytes());
            record[40..44].copy_from_slice(&expected.to_le_bytes());
            record[44..48].copy_from_slice(&tag.to_le_bytes());
            bytes.extend(record);
        }
        bytes.extend(b"src");
        bytes.resize(128, 0);
        bytes.extend(b"abc");
        bytes.extend([0x42; 13]);
        bytes.extend(compressed);
        let entry = HedEntry {
            name_hash: [0; 16],
            package_offset: 0,
            stored_length: bytes.len() as u32,
            original_length: 3,
        };
        (Cursor::new(bytes), entry)
    }

    #[test]
    fn locates_aligned_samples_preserves_offsets_and_plain_padding() {
        let (mut reader, entry) = fixture();
        let samples = read_first_replacements(&mut reader, &entry).unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].metadata.package_offset, 128);
        assert_eq!(samples[0].metadata.declared_offset, -123);
        assert_eq!(samples[0].bytes.len(), 16);
        assert_eq!(&samples[0].bytes[..3], b"abc");
        assert_eq!(&samples[0].bytes[3..], &[0x42; 13]);
        assert!(samples[0].metadata.uncompressed_padding_retained);
        assert_eq!(samples[1].metadata.package_offset, 144);
        assert_eq!(samples[1].bytes, b"synthetic replacement");
    }

    #[test]
    fn rejects_bad_tags_lengths_and_truncated_aligned_storage() {
        for (field, value) in [(40, -1i32), (44, -3), (40, i32::MAX)] {
            let (mut reader, entry) = fixture();
            reader.get_mut()[16 + field..20 + field].copy_from_slice(&value.to_le_bytes());
            assert!(read_first_replacements(&mut reader, &entry).is_err());
        }
        let (mut reader, mut entry) = fixture();
        entry.stored_length = 130;
        assert!(read_first_replacements(&mut reader, &entry).is_err());
    }
}
