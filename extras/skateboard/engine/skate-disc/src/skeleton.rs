//! physics-skeletons.json from OnBoard.abin
//! (tools/asset_pipeline/physics_skeleton.py).
use crate::{Error, be32, sha256_hex};
use serde_json::{Value, json};

const ALPHABET: &[u8; 37] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_";

fn fast_name(data: &[u8], at: usize, words: usize) -> Result<String, Error> {
    let mut out = String::new();
    for i in 0..words {
        let mut word = be32(data, at + i * 4)? as u64;
        let mut divisor = 38u64.pow(5);
        for _ in 0..6 {
            let digit = word / divisor;
            word %= divisor;
            if digit == 0 {
                if word != 0 {
                    return Err(Error::Format("invalid FastString padding"));
                }
                break;
            }
            let ch = ALPHABET
                .get(digit as usize - 1)
                .ok_or(Error::Format("invalid FastString digit"))?;
            out.push(*ch as char);
            divisor /= 38;
        }
    }
    Ok(out)
}

pub fn convert(data: &[u8]) -> Result<Value, Error> {
    let end = be32(data, 0)? as usize;
    if be32(data, 4)? != 1 || end > data.len() {
        return Err(Error::Format("invalid animation bank"));
    }
    let mut skeletons = Vec::new();
    let mut at = 48;
    while at < end {
        let size = be32(data, at)? as usize;
        let kind = be32(data, at + 4)?;
        if size < 48 || !size.is_multiple_of(16) || at + size > end {
            return Err(Error::Format("invalid ABIN record"));
        }
        if kind == 5 {
            let payload = (at + 55) & !15;
            let count = be32(data, payload)? as usize;
            let start = payload + 16;
            if start + count * 112 > data.len() {
                return Err(Error::Format("truncated ABIN skeleton"));
            }
            let mut bones = Vec::with_capacity(count);
            for i in 0..count {
                let pos = start + i * 112;
                let words = (0..28)
                    .map(|w| be32(data, pos + w * 4))
                    .collect::<Result<Vec<u32>, _>>()?;
                bones.push(json!({
                    "name": fast_name(data, pos + 92, 5)?,
                    "source_offset": pos,
                    "words": words,
                }));
            }
            skeletons.push(json!({
                "name": fast_name(data, at + 16, 6)?,
                "source_offset": at,
                "bones": bones,
            }));
        }
        at += size;
    }
    if skeletons.is_empty() {
        return Err(Error::Format("no physics skeletons in animation bank"));
    }
    Ok(json!({"version": 1, "source_sha256": sha256_hex(data), "skeletons": skeletons}))
}
