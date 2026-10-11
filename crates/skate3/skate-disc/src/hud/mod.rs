//! Skate 3's scoring HUD (the trick display movie) from the user's disc:
//! tools/prepare_hud.py followed by install_prepared_hud.py, as
//! tools/prepare_runtime_huds.py runs them. Writes only what the runtime
//! loads: `private/hud/runtime/trickdisplay.json` and the RGBA8 textures it
//! names, under the asset root.
use crate::{DiscFiles, Error, sha256_hex};
use binary::bad;
use json::{J, Obj, dumps, py_strip};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub mod actions;
pub mod apt;
pub mod binary;
pub mod cache;
pub mod font;
pub mod geo;
pub mod json;
pub mod language;
pub mod rw4;
pub mod xenos;

#[cfg(test)]
mod tests;

/// The scoring HUD movie.
pub const BUNDLE: &str = "data/fe/source/screens/hud2/trickdisplay2";
/// Where the runtime looks, relative to the asset root.
pub const INSTALL_DIR: &str = "private/hud";
pub const MANIFEST: &str = "runtime/trickdisplay.json";
/// FontManager82808AE8's AttribSys class.
const FONT_CLASS: u64 = 0xFECF_BCAF_3565_18C4;

/// APT font name -> (native family, provenance): `AssetCache.font_aliases`.
pub type Aliases = HashMap<String, (String, Obj)>;

fn from_serde(value: &serde_json::Value) -> J {
    match value {
        serde_json::Value::Null => J::Null,
        serde_json::Value::Bool(b) => J::Bool(*b),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(J::Int)
            .unwrap_or_else(|| J::Float(n.as_f64().unwrap_or(0.0))),
        serde_json::Value::String(s) => J::Str(s.clone()),
        serde_json::Value::Array(a) => J::Arr(a.iter().map(from_serde).collect()),
        serde_json::Value::Object(o) => J::Obj(Obj(o
            .iter()
            .map(|(k, v)| (k.clone(), from_serde(v)))
            .collect())),
    }
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    digits
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

/// `font_mapping`: FontManager rows (APT name -> native bitmap bank file),
/// resolved against the packaged fonts. Returns (font_mappings, aliases).
fn font_mapping(
    collections: &serde_json::Value,
    cache: &cache::Cache,
) -> Result<(Obj, Aliases), Error> {
    let mut result = Obj::new();
    let mut aliases = HashMap::new();
    let rows = collections["collections"]
        .as_array()
        .ok_or(bad("collections JSON has no collections"))?;
    for row in rows {
        let class = row["class"]
            .as_str()
            .ok_or(bad("collection without a class"))?;
        let class_id = match class.strip_prefix("Hash_") {
            Some(hex) => u64::from_str_radix(hex, 16).map_err(|_| bad("invalid class hash"))?,
            None => skate_data::attrib_hash::hash(class),
        };
        if class_id != FONT_CLASS {
            continue;
        }
        let fields = &row["fields"];
        let field = |name: &str| -> Result<&serde_json::Value, Error> {
            let hashed = format!("Hash_{:016X}", skate_data::attrib_hash::hash(name));
            fields
                .get(name)
                .or_else(|| fields.get(&hashed))
                .filter(|v| !v.is_null())
                .map(|v| &v["data"])
                .ok_or(bad("font row lacks a field"))
        };
        let apt_name = field("AptName")?
            .as_str()
            .ok_or(bad("font AptName is not text"))?
            .to_owned();
        let filename = field("FileName")?
            .as_str()
            .ok_or(bad("font FileName is not text"))?
            .to_owned();
        let matches: Vec<&cache::Bundle> = cache
            .bundles
            .iter()
            .filter(|b| b.font.is_some() && b.name.rsplit('/').next() == Some(filename.as_str()))
            .collect();
        if matches.is_empty() {
            continue; // The debug font is not a packaged front-end bitmap bank.
        }
        if matches.len() != 1 || result.get(&apt_name).is_some() {
            return Err(bad("ambiguous native font record"));
        }
        let mut layout = Obj::new();
        for name in ["ScaleX", "ScaleY", "OffsetX", "OffsetY"] {
            let raw = field(name)?
                .as_str()
                .and_then(unhex)
                .ok_or(bad("font scale is not hex"))?;
            let raw: [u8; 4] = raw
                .try_into()
                .map_err(|_| bad("font scale is not a 4-byte float"))?;
            layout.set(name, f32::from_be_bytes(raw) as f64);
        }
        let target = matches[0]
            .font
            .as_ref()
            .and_then(|f| f.get("family"))
            .and_then(J::as_str)
            .unwrap_or("")
            .to_owned();
        let mut provenance = Obj::new();
        provenance.set("class", from_serde(&row["class"]));
        provenance.set("key", from_serde(&row["key"]));
        provenance.set("source_sha256", from_serde(&row["sha256"]));
        provenance.set("file_name", filename.as_str());
        provenance.set("native_layout", layout);
        aliases.insert(apt_name.clone(), (target, provenance.clone()));
        result.set(apt_name, provenance);
    }
    Ok((result, aliases))
}

/// Prepares the scoring HUD from the disc and installs its runtime files
/// under `out` (the asset root): `out/private/hud/runtime/trickdisplay.json`
/// and every RGBA texture it references. `collections_json` is the
/// skater-collections document `vlt::convert` produced.
pub fn prepare_hud(
    disc: &mut dyn DiscFiles,
    collections_json: &serde_json::Value,
    out: &Path,
) -> Result<(), Error> {
    let cache = cache::Cache::extract(disc, &[BUNDLE])?;
    let (mappings, aliases) = font_mapping(collections_json, &cache)?;
    let bundle = cache.load_bundle(BUNDLE)?;
    let b = &cache.bundles[bundle];
    let apt = b
        .parts
        .get(".apt")
        .ok_or(bad("trick display APT is missing"))?;
    let constants = b
        .parts
        .get(".const")
        .ok_or(bad("trick display CONST is missing"))?;
    let actions = actions::Actions::new(apt, constants)?;
    let characters = &b.timeline.as_ref().unwrap().characters;

    let mut blocks = Obj::new();
    for c in characters {
        for frame in c.get("frames").and_then(J::as_arr).unwrap_or(&[]) {
            for control in frame.get("controls").and_then(J::as_arr).unwrap_or(&[]) {
                let kind = control.get("type_name").and_then(J::as_str);
                if matches!(kind, Some("do_action" | "do_init_action")) {
                    let offset = control
                        .get("actions_offset")
                        .and_then(J::as_i64)
                        .unwrap_or(0);
                    if offset != 0 {
                        blocks.set(
                            offset.to_string(),
                            actions.stream(offset as usize, None, 0)?,
                        );
                    }
                }
            }
        }
    }

    let mut shapes = Obj::new();
    let mut fonts = Obj::new();
    // Installed texture files: relative path -> (bundle, texture index).
    let mut files: Vec<(String, usize, usize)> = Vec::new();
    for c in characters {
        let id = c.get("id").and_then(J::as_i64).unwrap_or(0);
        match c.get("type_name").and_then(J::as_str) {
            Some("shape") => {
                let (primitives, used) = cache.flatten_shape(BUNDLE, id)?;
                for t in used {
                    let rgba = t
                        .obj
                        .get("rgba")
                        .and_then(J::as_str)
                        .unwrap_or("")
                        .to_owned();
                    files.push((rgba, t.bundle, t.index));
                }
                shapes.set(id.to_string(), primitives);
            }
            Some("font") => {
                let family = c
                    .get("font")
                    .and_then(|f| f.get("name"))
                    .and_then(J::as_str)
                    .unwrap_or("")
                    .to_owned();
                match cache.font_asset(&family, &aliases)? {
                    Some((asset, t)) => {
                        let rgba = asset.get("texture").and_then(J::as_str).unwrap_or("");
                        files.push((rgba.to_owned(), t.bundle, t.index));
                        fonts.set(family, asset);
                    }
                    None => fonts.set(family, J::Null),
                }
            }
            _ => {}
        }
    }

    let mut language = Obj::new();
    for (label, value) in cache
        .language
        .as_ref()
        .ok_or(bad("English language table is missing"))?
    {
        language.set(py_strip(label), value.as_str());
    }
    let unresolved: Vec<J> = fonts
        .0
        .iter()
        .filter(|(_, asset)| *asset == J::Null)
        .map(|(family, _)| J::from(family.as_str()))
        .collect();

    let mut source = Obj::new();
    source.set("bundle", BUNDLE);
    source.set("apt_sha256", sha256_hex(apt));
    source.set("const_sha256", sha256_hex(constants));
    // The file skate_disc::prepare writes (private/stock/skater-collections.json).
    source.set(
        "collections_sha256",
        sha256_hex(collections_json.to_string().as_bytes()),
    );
    let mut result = Obj::new();
    result.set("format", "skate3-scoring-hud");
    result.set("version", 1);
    result.set("source", source);
    result.set("characters", characters.clone());
    result.set("shapes", shapes.clone());
    result.set("fonts", fonts.clone());
    result.set("actions", blocks);
    result.set("font_mappings", mappings);
    result.set("language", language);
    result.set("unresolved_fonts", unresolved.clone());
    let manifest = dumps(&J::Obj(result)) + "\n";

    // install_prepared_hud.runtime_files: no unresolved fonts, and every
    // referenced texture a whole RGBA8 image of the size the manifest declares.
    if !unresolved.is_empty() {
        return Err(bad("invalid scoring HUD or unresolved original fonts"));
    }
    let mut textures: Vec<(String, i64, i64)> = Vec::new();
    for (_, shape) in &shapes.0 {
        for primitive in shape.as_arr().unwrap_or(&[]) {
            let t = primitive
                .get("texture")
                .filter(|t| **t != J::Null)
                .ok_or(bad("an untextured HUD shape cannot be installed"))?;
            let get = |k: &str| t.get(k);
            textures.push((
                get("rgba").and_then(J::as_str).unwrap_or("").to_owned(),
                get("width").and_then(J::as_i64).unwrap_or(0),
                get("height").and_then(J::as_i64).unwrap_or(0),
            ));
        }
    }
    for (_, font) in &fonts.0 {
        let size = font
            .get("definition")
            .and_then(|d| d.get("textures"))
            .and_then(J::as_arr)
            .and_then(|t| t.first())
            .ok_or(bad("HUD font declares no texture"))?;
        textures.push((
            font.get("texture")
                .and_then(J::as_str)
                .unwrap_or("")
                .to_owned(),
            size.get("width").and_then(J::as_i64).unwrap_or(0),
            size.get("height").and_then(J::as_i64).unwrap_or(0),
        ));
    }
    let mut payloads: Vec<(PathBuf, &[u8])> = Vec::new();
    for (relative, width, height) in textures {
        let path = contained(&relative)?;
        let &(_, bundle, index) = files
            .iter()
            .find(|(r, _, _)| *r == relative)
            .ok_or(bad("HUD texture was not prepared"))?;
        let rgba = cache.bundles[bundle]
            .rgba(index)
            .ok_or(bad("HUD texture was not decoded"))?;
        if width <= 0 || height <= 0 || rgba.len() as i64 != width * height * 4 {
            return Err(bad("invalid HUD RGBA size"));
        }
        if !payloads.iter().any(|(p, _)| *p == path) {
            payloads.push((path, rgba));
        }
    }

    let root = out.join(INSTALL_DIR);
    let write = |relative: &Path, bytes: &[u8]| -> Result<(), Error> {
        let target = root.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, bytes)?;
        Ok(())
    };
    write(Path::new(MANIFEST), manifest.as_bytes())?;
    for (path, bytes) in payloads {
        write(&path, bytes)?;
    }
    Ok(())
}

/// `contained`: a relative path that stays inside the HUD root.
fn contained(relative: &str) -> Result<PathBuf, Error> {
    crate::safe_relative(relative)
}
