//! The UI asset cache of tools/vendor/skate3_ui/project.py (extract_project)
//! and scene_graph.py (AssetCache, SceneFlattener's shape path), held in
//! memory: only the runtime files are ever written.
use super::apt::{Movie, inspect_apt};
use super::binary::{bad, suffix_start, with_suffix};
use super::font::parse_bitmap_font;
use super::geo::{Shape, parse_geo};
use super::json::{J, Obj};
use super::language::pair_language_tables;
use super::rw4::{Texture, read_textures};
use super::xenos::decode_rgba;
use crate::big::Big;
use crate::{DiscFiles, Error, safe_relative};
use std::cell::OnceCell;
use std::collections::{BTreeMap, HashMap};

const CORE_ARCHIVES: [&str; 3] = ["fedata.big", "fetexture.big", "miscboot.big"];
const UI_EXTENSIONS: [&str; 8] = [
    ".apt", ".bmpfont", ".const", ".geo", ".rx2", ".rps3", ".jpg", ".png",
];
const LABELS: &str = "data/fe/languages/labels/language_labels_global_skate3ng.bin";
const ENGLISH: &str = "data/fe/languages/english/language_english_global_skate3ng.bin";
const SUPPORT_PREFIX: &str = "data/fe/fonts/";

fn suffix(path: &str) -> &str {
    suffix_start(path).map_or("", |i| &path[i..])
}

/// `is_selected_ui_path`.
pub fn is_selected(path: &str, prefixes: &[&str]) -> bool {
    let normalized = path.replace('\\', "/").to_lowercase();
    if normalized == LABELS || normalized == ENGLISH {
        return true;
    }
    let ui = UI_EXTENSIONS.contains(&suffix(&normalized));
    if normalized.starts_with(SUPPORT_PREFIX) {
        return ui;
    }
    ui && prefixes
        .iter()
        .any(|p| normalized.starts_with(&p.to_lowercase()))
}

/// `AssetCache.normalize_bundle`.
pub fn normalize_bundle(value: &str) -> String {
    let mut normalized = value.replace('\\', "/").to_lowercase();
    if normalized.starts_with("source/") {
        normalized = format!("data/fe/{normalized}");
    }
    // str(PurePosixPath(...)): empty and "." parts collapse.
    let absolute = normalized.starts_with('/');
    let parts: Vec<&str> = normalized
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let joined = parts.join("/");
    match (absolute, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".into(),
        (false, false) => joined,
    }
}

pub struct Bundle {
    /// Display name (archive case, no suffix).
    pub name: String,
    pub key: String,
    /// Raw files by lower-case suffix.
    pub parts: BTreeMap<String, Vec<u8>>,
    pub timeline: Option<Movie>,
    pub shapes: HashMap<u32, Shape>,
    pub font: Option<Obj>,
    pub textures: Option<Vec<Texture>>,
    decoded: Vec<OnceCell<Option<Vec<u8>>>>,
}

impl Bundle {
    pub fn character(&self, id: i64) -> Option<&Obj> {
        self.timeline
            .as_ref()?
            .characters
            .iter()
            .find(|c| c.get("id").and_then(J::as_i64) == Some(id))
    }
    /// `assets/<name>`: where the arena's outputs were written.
    fn asset_dir(&self) -> String {
        format!("assets/{}", self.name)
    }
    /// RGBA8 of one texture, decoded once; None when decoding fails.
    pub fn rgba(&self, index: usize) -> Option<&[u8]> {
        let texture = self.textures.as_ref()?.get(index)?;
        self.decoded[index]
            .get_or_init(|| {
                texture
                    .fetch
                    .as_ref()
                    .and_then(|f| decode_rgba(&texture.data, f).ok())
            })
            .as_deref()
    }
}

pub struct TextureRef {
    pub bundle: usize,
    pub index: usize,
    pub obj: Obj,
}

pub struct Cache {
    pub bundles: Vec<Bundle>,
    /// normalize_bundle(name) -> bundle (later records replace earlier ones).
    by_key: HashMap<String, usize>,
    /// (label, value) pairs of the English global language table.
    pub language: Option<Vec<(String, String)>>,
}

impl Cache {
    /// `extract_project(game, output, prefixes=prefixes)`, without the
    /// intermediate files.
    pub fn extract(disc: &mut dyn DiscFiles, prefixes: &[&str]) -> Result<Self, Error> {
        // key -> (archive, entry index), later archives preferred.
        let mut selected: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut entries = Vec::new();
        for (archive_index, name) in CORE_ARCHIVES.iter().enumerate() {
            let path = format!("data/big/{name}");
            let file = disc
                .open(&path)?
                .ok_or_else(|| Error::Missing(path.clone()))?;
            let archive = Big::open(file)?;
            for (entry_index, entry) in archive.entries.iter().enumerate() {
                if is_selected(&entry.path, prefixes) {
                    let key = entry.path.replace('\\', "/").to_lowercase();
                    selected.insert(key, (archive_index, entry_index));
                }
            }
            entries.push(archive.entries);
        }
        // Read each archive's chosen entries.
        let mut extracted: BTreeMap<String, (String, Vec<u8>)> = BTreeMap::new();
        for (archive_index, name) in CORE_ARCHIVES.iter().enumerate() {
            let wanted: Vec<(&String, usize)> = selected
                .iter()
                .filter(|(_, (a, _))| *a == archive_index)
                .map(|(k, (_, e))| (k, *e))
                .collect();
            if wanted.is_empty() {
                continue;
            }
            let path = format!("data/big/{name}");
            let file = disc
                .open(&path)?
                .ok_or_else(|| Error::Missing(path.clone()))?;
            let mut archive = Big::open(file)?;
            for (key, entry_index) in wanted {
                let entry = entries[archive_index][entry_index].clone();
                safe_relative(&entry.path)?;
                let data = archive.read(&entry)?;
                extracted.insert(key.clone(), (entry.path, data));
            }
        }

        // Bundles group the files of one stem.
        let mut grouped: BTreeMap<String, (String, BTreeMap<String, Vec<u8>>)> = BTreeMap::new();
        let mut language_files: HashMap<&str, Vec<u8>> = HashMap::new();
        for (key, (path, data)) in &extracted {
            if key == LABELS || key == ENGLISH {
                language_files.insert(if key == LABELS { LABELS } else { ENGLISH }, data.clone());
            }
            let stem = with_suffix(key, "");
            let display = with_suffix(path, "");
            let group = grouped
                .entry(stem)
                .or_insert_with(|| (display, BTreeMap::new()));
            group.1.insert(suffix(path).to_lowercase(), data.clone());
        }

        let mut bundles = Vec::new();
        for (_, (name, parts)) in grouped {
            safe_relative(&name)?;
            let timeline = match (parts.get(".apt"), parts.get(".const")) {
                (Some(apt), Some(constants)) => inspect_apt(apt, constants).ok(),
                _ => None,
            };
            let mut shapes = HashMap::new();
            if let Some(geo) = parts.get(".geo")
                && let Ok(list) = parse_geo(geo)
            {
                for shape in list {
                    shapes.insert(shape.id, shape);
                }
            }
            let font = parts.get(".bmpfont").and_then(|raw| {
                let source = format!("raw/{}", font_source(&extracted, &name));
                parse_bitmap_font(raw, &source).ok()
            });
            let textures = parts
                .get(".rx2")
                .or_else(|| parts.get(".rps3"))
                .and_then(|arena| read_textures(arena).ok());
            let decoded = (0..textures.as_ref().map_or(0, Vec::len))
                .map(|_| OnceCell::new())
                .collect();
            bundles.push(Bundle {
                key: normalize_bundle(&name),
                name,
                parts,
                timeline,
                shapes,
                font,
                textures,
                decoded,
            });
        }
        let by_key = bundles
            .iter()
            .enumerate()
            .map(|(i, b)| (b.key.clone(), i))
            .collect();
        let language = match (language_files.get(LABELS), language_files.get(ENGLISH)) {
            (Some(labels), Some(values)) => pair_language_tables(labels, values).ok(),
            _ => None,
        };
        Ok(Cache {
            bundles,
            by_key,
            language,
        })
    }

    /// `load_bundle`: a bundle with a parsed APT timeline.
    pub fn load_bundle(&self, name: &str) -> Result<usize, Error> {
        self.by_key
            .get(&normalize_bundle(name))
            .copied()
            .filter(|&i| self.bundles[i].timeline.is_some())
            .ok_or(bad("APT bundle is absent from cache"))
    }

    /// `resolve_character`: a local character, or the export it imports.
    pub fn resolve_character(&self, bundle: &str, id: i64) -> Result<(usize, &Obj), Error> {
        let b = self.load_bundle(bundle)?;
        if let Some(c) = self.bundles[b].character(id) {
            return Ok((b, c));
        }
        let movie = self.bundles[b].timeline.as_ref().unwrap();
        // {character_id: import}: the last record for an id wins.
        let imported = movie
            .imports
            .iter()
            .rev()
            .find(|i| i64::from(i.character_id) == id)
            .ok_or(bad("character is neither local nor imported"))?;
        let ib = self.load_bundle(&imported.file)?;
        let exports = &self.bundles[ib].timeline.as_ref().unwrap().exports;
        // exports.setdefault: the first record for a name wins.
        let exported = exports
            .iter()
            .find(|(name, _)| *name == imported.name)
            .ok_or(bad("imported export is absent"))?
            .1;
        let c = self.bundles[ib]
            .character(exported.into())
            .ok_or(bad("exported character is absent"))?;
        Ok((ib, c))
    }

    /// `texture_for_id`: the one arena texture named `<texture_id>.*`.
    pub fn texture_for_id(&self, bundle: usize, texture_id: i64) -> Result<TextureRef, Error> {
        let b = &self.bundles[bundle];
        let textures = b.textures.as_deref().unwrap_or(&[]);
        let wanted = texture_id.to_string();
        let matches: Vec<usize> = textures
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                let name = t.name.replace('\\', "/");
                let basename = name.rsplit('/').next().unwrap_or("");
                let stem = basename.rsplit_once('.').map_or(basename, |(s, _)| s);
                stem == wanted
            })
            .map(|(i, _)| i)
            .collect();
        if matches.len() != 1 {
            return Err(bad("expected exactly one texture resource for a bitmap"));
        }
        let index = matches[0];
        let texture = &textures[index];
        let fetch = texture.fetch.ok_or(bad("texture has no RGBA preview"))?;
        b.rgba(index).ok_or(bad("texture has no RGBA preview"))?;
        let stem = texture.file_stem();
        let mut obj = Obj::new();
        obj.set("resource_name", texture.name.as_str());
        obj.set("width", fetch.width);
        obj.set("height", fetch.height);
        obj.set("preview", format!("{}/{stem}.png", b.asset_dir()));
        obj.set("rgba", format!("{}/{stem}.rgba", b.asset_dir()));
        Ok(TextureRef { bundle, index, obj })
    }

    /// `font_asset`: the one packaged bitmap font of a family (through its
    /// alias), with its first texture. Ok(None) when not exactly one matches.
    pub fn font_asset(
        &self,
        family: &str,
        aliases: &super::Aliases,
    ) -> Result<Option<(Obj, TextureRef)>, Error> {
        let alias = aliases.get(family);
        let target = alias.map_or(family, |(t, _)| t.as_str());
        let matches: Vec<usize> = self
            .bundles
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.font
                    .as_ref()
                    .is_some_and(|f| f.get("family").and_then(J::as_str) == Some(target))
            })
            .map(|(i, _)| i)
            .collect();
        if matches.len() != 1 {
            return Ok(None);
        }
        let bundle = matches[0];
        let b = &self.bundles[bundle];
        let textures = b
            .textures
            .as_ref()
            .ok_or(bad("font bundle has no texture arena"))?;
        let texture = textures.first().ok_or(bad("font arena has no textures"))?;
        b.rgba(0).ok_or(bad("font texture has no RGBA preview"))?;
        let stem = texture.file_stem();
        let mut result = Obj::new();
        result.set("bundle", b.name.as_str());
        result.set(
            "metadata",
            format!("metadata/{}", with_suffix(&b.name, ".font.json")),
        );
        result.set("texture", format!("{}/{stem}.rgba", b.asset_dir()));
        result.set("preview", format!("{}/{stem}.png", b.asset_dir()));
        result.set("definition", b.font.clone().unwrap());
        if let Some((target, provenance)) = alias {
            result.set("requested_family", family);
            let mut a = Obj::new();
            a.set("target", target.as_str());
            a.set("provenance", provenance.clone());
            result.set("alias", a);
        }
        Ok(Some((
            result,
            TextureRef {
                bundle,
                index: 0,
                obj: Obj::new(),
            },
        )))
    }

    /// `SceneFlattener(cache, lambda *_: {}).flatten(bundle, shape_id)` for a
    /// shape character: its primitives, or an error where any would be unresolved.
    pub fn flatten_shape(
        &self,
        bundle_name: &str,
        id: i64,
    ) -> Result<(Vec<J>, Vec<TextureRef>), Error> {
        const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let (bundle, character) = self.resolve_character(bundle_name, id)?;
        if character.get("type_name").and_then(J::as_str) != Some("shape") {
            return Err(bad("flattened character is not a shape"));
        }
        let b = &self.bundles[bundle];
        let character_id = character.get("id").and_then(J::as_i64).unwrap_or(0);
        let shape = u32::try_from(character_id)
            .ok()
            .and_then(|id| b.shapes.get(&id))
            .ok_or(bad("shape has no GEO record"))?;
        let mut primitives = Vec::new();
        let mut used = Vec::new();
        for (unit_index, unit) in shape.units.iter().enumerate() {
            let render_type = unit.render_type_name();
            let mut texture = None;
            if render_type.starts_with("texture") {
                let (_, bitmap) = self.resolve_character(&b.key, unit.texture_id.into())?;
                let texture_id = bitmap
                    .get("bitmap")
                    .and_then(|m| m.get("texture_id"))
                    .and_then(J::as_i64)
                    .ok_or(bad("shape texture is not a bitmap character"))?;
                texture = Some(self.texture_for_id(bundle, texture_id)?);
            }
            let size = texture.as_ref().map(|t| {
                (
                    t.obj.get("width").and_then(J::as_f64).unwrap_or(0.0),
                    t.obj.get("height").and_then(J::as_f64).unwrap_or(0.0),
                )
            });
            let mut triangles = Vec::new();
            for triangle in &unit.triangles {
                let mut vertices = Vec::new();
                for point in triangle {
                    let mut vertex = Obj::new();
                    vertex.set("position", transform(&IDENTITY, point));
                    if let Some((w, h)) = size {
                        let uv = transform(&unit.uv_matrix, point);
                        vertex.set("uv", vec![uv[0] / w, uv[1] / h]);
                    }
                    vertices.push(J::Obj(vertex));
                }
                triangles.push(J::Arr(vertices));
            }
            let mut p = Obj::new();
            p.set("draw_order", unit_index);
            p.set("path", "/");
            p.set("bundle", b.key.as_str());
            p.set("character_id", character_id);
            p.set("unit", unit_index);
            p.set("render_type", render_type);
            p.set("matrix", IDENTITY.to_vec());
            let alpha = 1.0;
            let c = unit.color;
            p.set("color", vec![c[0], c[1], c[2], c[3] * alpha]);
            p.set("texture", texture.as_ref().map(|t| t.obj.clone()));
            p.set("triangles", triangles);
            primitives.push(J::Obj(p));
            used.extend(texture);
        }
        Ok((primitives, used))
    }
}

fn transform(m: &[f64; 6], p: &[f64; 2]) -> Vec<f64> {
    vec![
        m[0] * p[0] + m[2] * p[1] + m[4],
        m[1] * p[0] + m[3] * p[1] + m[5],
    ]
}

/// The archive path of a bundle's .bmpfont file.
fn font_source(extracted: &BTreeMap<String, (String, Vec<u8>)>, name: &str) -> String {
    extracted
        .values()
        .map(|(path, _)| path)
        .find(|path| with_suffix(path, "") == name && suffix(path).eq_ignore_ascii_case(".bmpfont"))
        .cloned()
        .unwrap_or_default()
}
