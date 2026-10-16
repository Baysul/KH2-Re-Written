//! RenderWare 4 texture arenas, .rx2/.rps3 (tools/vendor/skate3_ui/rw4.py).
use super::binary::{Reader, bad};
use super::xenos::{Fetch, parse_fetch_constant};
use crate::Error;

const TOC_TYPE: u32 = 0x00EB_000B;

#[derive(Clone, Debug)]
pub struct Texture {
    /// Index in the arena's table of contents.
    pub index: usize,
    pub name: String,
    pub data: Vec<u8>,
    /// Xbox 360 only; PS3 arenas carry no fetch constant (and no RGBA preview).
    pub fetch: Option<Fetch>,
}

impl Texture {
    /// `_safe_name`: the leaf of the resource name, filtered for a file name.
    pub fn safe_name(&self) -> String {
        let name = self.name.replace('\\', "/");
        // pathlib drops empty and "." parts before taking the name.
        let leaf = name
            .split('/')
            .rfind(|p| !p.is_empty() && *p != ".")
            .unwrap_or("");
        let filtered: String = leaf
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || "._-".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if filtered.is_empty() {
            format!("texture_{:04}", self.index)
        } else {
            filtered
        }
    }
    /// `{index:04d}_{name}` file stem used for the .rgba/.png outputs.
    pub fn file_stem(&self) -> String {
        format!("{:04}_{}", self.index, self.safe_name())
    }
}

struct Dict {
    pointer: usize,
    size: usize,
    type_id: u32,
}

pub fn read_textures(raw: &[u8]) -> Result<Vec<Texture>, Error> {
    let r = Reader::new(raw);
    r.require(0, 68)?;
    if r.bytes(0, 4)? != b"\x89RW4" {
        return Err(bad("not a RenderWare 4 arena"));
    }
    let xbox = match r.bytes(4, 4)? {
        b"ps3\0" => false,
        b"xb2\0" => true,
        _ => return Err(bad("unknown RW4 platform magic")),
    };
    let arena = 32;
    let entry_count = r.at(arena)?;
    let dictionary_offset = r.at(arena + 16)?;
    let resource_header_size = r.at(arena + 36)?;
    if entry_count > 1_000_000 {
        return Err(bad("unreasonable RW4 dictionary count"));
    }
    r.require(dictionary_offset, entry_count * 24)?;
    let mut dictionary = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        let at = dictionary_offset + index * 24;
        dictionary.push(Dict {
            pointer: r.at(at)?,
            size: r.at(at + 8)?,
            type_id: r.u32be(at + 20)?,
        });
    }
    let toc = dictionary
        .iter()
        .find(|d| d.type_id == TOC_TYPE)
        .ok_or(bad("RW4 arena has no table of contents"))?;
    let toc_offset = toc.pointer;
    r.require(toc_offset, 20)?;
    let item_count = r.at(toc_offset)?;
    let item_array = r.at(toc_offset + 4)?;
    if item_count > 1_000_000 {
        return Err(bad("unreasonable RW4 TOC count"));
    }
    r.require(toc_offset + item_array, item_count * 24)?;
    let mut textures = Vec::new();
    for item_index in 0..item_count {
        let item = toc_offset + item_array + item_index * 24;
        let name_offset = r.at(item)?;
        let name = r.cstring(toc_offset + name_offset);
        let _guid = r.u64be(item + 8)?;
        let dictionary_index = r.u32be(item + 20)? as i64 - 1;
        if dictionary_index < 0 || dictionary_index as usize + 1 >= dictionary.len() {
            continue;
        }
        let payload = &dictionary[dictionary_index as usize];
        let info = &dictionary[dictionary_index as usize + 1];
        let data = r
            .bytes(payload.pointer + resource_header_size, payload.size)?
            .to_vec();
        let fetch = if xbox {
            Some(parse_fetch_constant(r.bytes(info.pointer + 28, 24)?)?)
        } else {
            r.require(info.pointer, 40)?;
            None
        };
        textures.push(Texture {
            index: item_index,
            name,
            data,
            fetch,
        });
    }
    let _arena_id = r.u32be(28)?;
    Ok(textures)
}
