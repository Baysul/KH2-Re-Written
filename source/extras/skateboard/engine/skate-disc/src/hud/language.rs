//! Language label/value string pools (tools/vendor/skate3_ui/language.py).
use super::binary::bad;
use crate::Error;

/// The pool's strings, Latin-1 decoded (byte-preserving).
pub fn parse_language_table(data: &[u8]) -> Result<Vec<String>, Error> {
    if data.len() < 28 {
        return Err(bad("language table is shorter than its header"));
    }
    let le = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) as usize;
    let declared = le(8);
    if le(12) != 28 {
        return Err(bad("unsupported language header size"));
    }
    let strings_offset = le(16) + 8;
    if strings_offset > data.len() {
        return Err(bad("language string pool lies outside the file"));
    }
    let strings: Vec<String> = data[strings_offset..]
        .split(|&b| b == 0)
        .map(|s| s.iter().map(|&b| b as char).collect())
        .collect();
    if strings.len() != declared + 2 {
        return Err(bad("language pool string count differs from its header"));
    }
    Ok(strings)
}

/// (label, value) entries of the paired tables, in pool order.
pub fn pair_language_tables(labels: &[u8], values: &[u8]) -> Result<Vec<(String, String)>, Error> {
    let labels = parse_language_table(labels)?;
    let values = parse_language_table(values)?;
    if labels.len() != values.len() {
        return Err(bad(
            "language label/value pools have different numbers of strings",
        ));
    }
    Ok(labels.into_iter().zip(values).collect())
}
