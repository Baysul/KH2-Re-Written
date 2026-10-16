//! skater-collections.json from Skate 3's big-endian, 64-bit AttribSys
//! schema and collections (tools/asset_pipeline/vlt.py).
use crate::{Error, be16, be32, be64, sha256_hex};
use serde_json::{Map, Value, json};
use skate_data::attrib_hash::hash;
use std::collections::{HashMap, HashSet};

const PTRN: u32 = 0x5074_724e;
const EXPN: u32 = 0x4578_704e;
const ENDC: u32 = 0x456e_6443;
const CLASS: u64 = 0x2A78_95AC_4A87_6152;
const COLLECTION: u64 = 0xAD30_3B8F_42B3_307E;
const TEXT: &str = "EA::Reflection::Text";

fn take(data: &[u8], at: usize, size: usize) -> Result<&[u8], Error> {
    at.checked_add(size)
        .and_then(|end| data.get(at..end))
        .ok_or(Error::Format("truncated AttribSys record"))
}

fn hex(raw: &[u8]) -> String {
    raw.iter().map(|b| format!("{b:02X}")).collect()
}

struct Export {
    kind: u64,
    offset: usize,
}

/// Applies the pointer fixups and returns (vlt, bin, exports).
/// The fixed-up vault and binary, and the vault's exports.
type Vault = (Vec<u8>, Vec<u8>, Vec<Export>);

fn vault(vlt: &[u8], bin: &[u8]) -> Result<Vault, Error> {
    let mut v = vlt.to_vec();
    let mut b = bin.to_vec();
    let mut exports = Vec::new();
    let mut at = 0;
    while at < v.len() {
        let tag = be32(&v, at)?;
        let size = be32(&v, at + 4)? as usize;
        if size < 8 {
            return Err(Error::Format("invalid AttribSys chunk length"));
        }
        take(&v, at, size)?;
        let (body, body_size) = (at + 8, size - 8);
        if tag == PTRN {
            let mut target_is_vlt = false;
            let mut p = body;
            while p < body + body_size {
                let offset = be32(&v, p)? as usize;
                let kind = be16(&v, p + 4)?;
                let index = be16(&v, p + 6)?;
                let dest = be64(&v, p + 8)?;
                match kind {
                    0 => break,
                    2 => target_is_vlt = index == 0,
                    1 | 3 => {
                        let value = if kind == 1 { 0 } else { dest as u32 };
                        let target = if target_is_vlt { &mut v } else { &mut b };
                        take(target, offset, 4)?;
                        target[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
                    }
                    4 => {}
                    _ => return Err(Error::Format("unknown VLT pointer kind")),
                }
                p += 16;
            }
        } else if tag == EXPN {
            let count = be64(&v, body)? as usize;
            if count > (body_size.saturating_sub(8)) / 24 {
                return Err(Error::Format("invalid VLT export count"));
            }
            exports = (0..count)
                .map(|i| {
                    let r = body + 8 + i * 24;
                    Ok(Export {
                        kind: be64(&v, r + 8)?,
                        offset: be32(&v, r + 20)? as usize,
                    })
                })
                .collect::<Result<_, Error>>()?;
        }
        at += size;
        if tag == ENDC {
            break;
        }
    }
    Ok((v, b, exports))
}

/// Non-overlapping matches of `[A-Za-z_][A-Za-z0-9_:./ -]{1,180}\0`.
fn strings(data: &[u8], out: &mut HashSet<String>) {
    let first = |b: u8| b.is_ascii_alphabetic() || b == b'_';
    let rest = |b: u8| b.is_ascii_alphanumeric() || b"_:./ -".contains(&b);
    let mut p = 0;
    while p < data.len() {
        if first(data[p]) {
            let mut n = 0;
            while n <= 180 && p + 1 + n < data.len() && rest(data[p + 1 + n]) {
                n += 1;
            }
            if (1..=180).contains(&n) && data.get(p + 1 + n) == Some(&0) {
                out.insert(String::from_utf8_lossy(&data[p..p + 1 + n]).into_owned());
                p += n + 2;
                continue;
            }
        }
        p += 1;
    }
}

#[derive(Clone, Copy)]
struct FieldDef {
    kind: u64,
    offset: usize,
    size: usize,
    flags: u8,
    alignment: u8,
}

fn cstr(data: &[u8], ptr: usize) -> Result<String, Error> {
    let tail = data
        .get(ptr..)
        .ok_or(Error::Format("VLT text out of range"))?;
    let end = tail
        .iter()
        .position(|&b| b == 0)
        .ok_or(Error::Format("unterminated VLT text"))?;
    String::from_utf8(tail[..end].to_vec()).map_err(|_| Error::Format("VLT text is not UTF-8"))
}

fn array_items(data: &[u8], pos: usize, size: usize, alignment: u8) -> Result<Value, Error> {
    let capacity = be16(data, pos)? as usize;
    let count = be16(data, pos + 2)? as usize;
    let stride = be16(data, pos + 4)? as usize;
    if count > capacity || stride != size || stride == 0 || alignment > 16 {
        return Err(Error::Format("invalid VLT array"));
    }
    let boundary = 1usize << alignment;
    let mut cursor = pos + 8;
    let mut items = Vec::new();
    for index in 0..capacity {
        cursor = cursor.div_ceil(boundary) * boundary;
        let raw = take(data, cursor, stride)?;
        if index < count {
            items.push(Value::String(hex(raw)));
        }
        cursor += stride;
    }
    Ok(json!({"capacity": capacity, "element_size": stride, "alignment": boundary, "items": items}))
}

pub fn convert(
    schema_vlt: &[u8],
    schema_bin: &[u8],
    collections_vlt: &[u8],
    collections_bin: &[u8],
) -> Result<Value, Error> {
    let (sv, sb, se) = vault(schema_vlt, schema_bin)?;
    let (cv, cb, ce) = vault(collections_vlt, collections_bin)?;
    let mut set: HashSet<String> = include_str!("names.txt")
        .lines()
        .map(str::to_owned)
        .collect();
    strings(&sb, &mut set);
    strings(&cb, &mut set);
    let mut lookup: HashMap<u64, String> = set.into_iter().map(|s| (hash(&s), s)).collect();
    let name = |lookup: &HashMap<u64, String>, key: u64| -> String {
        if key == 0 {
            return String::new();
        }
        lookup
            .get(&key)
            .cloned()
            .unwrap_or_else(|| format!("Hash_{key:016X}"))
    };
    // Schema classes: field definitions in declaration order.
    let mut classes: HashMap<u64, Vec<(u64, FieldDef)>> = HashMap::new();
    for e in se.iter().filter(|e| e.kind == CLASS) {
        let at = e.offset;
        let key = be64(&sv, at)?;
        let count = be32(&sv, at + 12)? as usize;
        let defs = be32(&sv, at + 16)? as usize;
        let mut fields: Vec<(u64, FieldDef)> = Vec::with_capacity(count);
        for i in 0..count {
            let d = defs + i * 24;
            let fkey = be64(&sb, d)?;
            let def = FieldDef {
                kind: be64(&sb, d + 8)?,
                offset: be16(&sb, d + 16)? as usize,
                size: be16(&sb, d + 18)? as usize,
                flags: take(&sb, d + 22, 1)?[0],
                alignment: take(&sb, d + 23, 1)?[0],
            };
            match fields.iter_mut().find(|(k, _)| *k == fkey) {
                Some(slot) => slot.1 = def,
                None => fields.push((fkey, def)),
            }
        }
        classes.insert(key, fields);
    }
    let source = sha256_hex(collections_vlt);
    let mut rows: Vec<(u64, u64, Value)> = Vec::new();
    for e in ce.iter().filter(|e| e.kind == COLLECTION) {
        let at = e.offset;
        let key = be64(&cv, at)?;
        let class = be64(&cv, at + 8)?;
        let parent = be64(&cv, at + 16)?;
        let count = be32(&cv, at + 32)? as usize;
        let typeslen = be16(&cv, at + 38)? as usize;
        let layout = be32(&cv, at + 40)? as usize;
        let fields = classes
            .get(&class)
            .ok_or(Error::Format("collection of an unknown class"))?;
        let mut out = Map::new();
        let mut value = |lookup: &mut HashMap<u64, String>,
                         fkey: u64,
                         from_vlt: bool,
                         pos: usize,
                         inline: bool|
         -> Result<(), Error> {
            let def = fields
                .iter()
                .find(|(k, _)| *k == fkey)
                .map(|(_, d)| *d)
                .ok_or(Error::Format("unknown schema field"))?;
            let data: &[u8] = if from_vlt { &cv } else { &cb };
            let t = name(lookup, def.kind);
            if t == TEXT && def.flags & 1 == 0 {
                let ptr = be32(data, pos)? as usize;
                let raw = if ptr != 0 {
                    cstr(&cb, ptr)?
                } else {
                    String::new()
                };
                if raw.is_ascii() {
                    lookup.insert(hash(&raw), raw.clone());
                }
                // Named after the insert, as the converter does.
                out.insert(name(lookup, fkey), json!({"type": t, "data": raw}));
            } else {
                let length = if inline { 4 } else { def.size };
                let mut entry = Map::new();
                entry.insert("type".into(), Value::String(t.clone()));
                entry.insert("data".into(), Value::String(hex(take(data, pos, length)?)));
                if def.flags & 1 != 0 {
                    let mut array = array_items(data, pos, def.size, def.alignment)?;
                    if t == TEXT {
                        let texts = array["items"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|item| {
                                let raw = item.as_str().unwrap_or_default();
                                let ptr = u32::from_str_radix(&raw[..8.min(raw.len())], 16)
                                    .map_err(|_| Error::Format("invalid VLT text item"))?;
                                if ptr == 0 {
                                    Ok(String::new())
                                } else {
                                    cstr(&cb, ptr as usize)
                                }
                            })
                            .collect::<Result<Vec<_>, Error>>()?;
                        array["text_items"] = json!(texts);
                    }
                    entry.insert("array".into(), array);
                }
                out.insert(name(lookup, fkey), Value::Object(entry));
            }
            Ok(())
        };
        for (fkey, def) in fields {
            if def.flags & 2 != 0 && layout != 0 {
                value(&mut lookup, *fkey, false, layout + def.offset, false)?;
            }
        }
        let entries = at + 48 + typeslen * 8;
        for i in 0..count {
            let pos = entries + i * 16;
            let fkey = be64(&cv, pos)?;
            let ptr = be32(&cv, pos + 8)? as usize;
            let def = fields
                .iter()
                .find(|(k, _)| *k == fkey)
                .map(|(_, d)| *d)
                .ok_or(Error::Format("unknown schema field"))?;
            if def.size <= 4 && def.flags & 1 == 0 {
                value(&mut lookup, fkey, true, pos + 8, true)?;
            } else {
                value(&mut lookup, fkey, false, ptr, false)?;
            }
        }
        let row = json!({
            "class": name(&lookup, class),
            "fields": out,
            "source": "skatercollections.vlt",
            "sha256": source,
        });
        rows.push((key, parent, row));
    }
    let collections: Vec<Value> = rows
        .into_iter()
        .map(|(key, parent, mut row)| {
            row["key"] = Value::String(name(&lookup, key));
            row["parent"] = Value::String(name(&lookup, parent));
            row
        })
        .collect();
    Ok(json!({"version": 1, "collections": collections}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_match_the_python_pattern() {
        let mut out = HashSet::new();
        strings(b"\x01ab\0_x.y z\0 9bad\0q\0Ok-1\0", &mut out);
        let mut got: Vec<_> = out.into_iter().collect();
        got.sort();
        assert_eq!(got, ["Ok-1", "_x.y z", "ab", "bad"]);
    }
}
