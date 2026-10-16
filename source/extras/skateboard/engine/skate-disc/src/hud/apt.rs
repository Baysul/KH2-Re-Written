//! APT movie structure: characters, frames and placement controls
//! (tools/vendor/skate3_ui/apt.py), as the inspector's JSON dicts.
use super::binary::{Reader, bad};
use super::json::{J, Obj};
use crate::Error;

fn character_type(t: u32) -> String {
    const NAMES: [&str; 20] = [
        "shape",
        "text",
        "font",
        "button",
        "sprite",
        "sound",
        "bitmap",
        "morph",
        "animation",
        "static_text",
        "none",
        "video",
        "command",
        "command_list",
        "level",
        "custom_control",
        "image",
        "plugin",
        "bitmap_slice",
        "avatar",
    ];
    match t {
        1..=20 => NAMES[t as usize - 1].into(),
        _ => format!("unknown_{t}"),
    }
}

fn control_type(t: u32) -> String {
    const NAMES: [&str; 11] = [
        "do_action",
        "frame_label",
        "place_object2",
        "remove_object2",
        "background_color",
        "start_sound",
        "start_sound_stream",
        "do_init_action",
        "place_object3",
        "place_object4",
        "remove_object3",
    ];
    match t {
        1..=11 => NAMES[t as usize - 1].into(),
        _ => format!("unknown_{t}"),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub file: String,
    pub name: String,
    pub character_id: u32,
}

/// What the cache keeps of an inspected movie.
#[derive(Clone, Debug)]
pub struct Movie {
    /// Non-root characters, ascending id, each the inspector's dict.
    pub characters: Vec<Obj>,
    pub imports: Vec<Import>,
    /// (name, character id), in table order.
    pub exports: Vec<(String, u32)>,
}

struct Inspector<'a> {
    apt: Reader<'a>,
}

fn floats(r: &Reader, at: usize, n: usize) -> Result<Vec<J>, Error> {
    (0..n).map(|i| Ok(J::Float(r.f32be(at + i * 4)?))).collect()
}

impl Inspector<'_> {
    fn character(&self, offset: usize, id: usize) -> Result<Option<Obj>, Error> {
        if offset == 0 {
            return Ok(None);
        }
        let r = &self.apt;
        r.require(offset, 16)?;
        let type_id = r.u32be(offset)?;
        let body = offset + 16;
        let mut c = Obj::new();
        c.set("id", id);
        c.set("offset", offset);
        c.set("type", type_id);
        c.set("type_name", character_type(type_id));
        if matches!(type_id, 1 | 2 | 10) {
            r.require(body, 16)?;
            c.set("bounds", floats(r, body, 4)?);
        }
        match type_id {
            2 => {
                r.require(body, 52)?;
                let mut t = Obj::new();
                t.set("font_id", r.u32be(body + 16)?);
                t.set("alignment", r.u32be(body + 20)?);
                t.set("color_argb", format!("#{:08x}", r.u32be(body + 24)?));
                t.set("font_height", r.f32be(body + 28)?);
                t.set("read_only", r.u32be(body + 32)? != 0);
                t.set("multiline", r.u32be(body + 36)? != 0);
                t.set("word_wrap", r.u32be(body + 40)? != 0);
                t.set("initial_text", r.cstring(r.at(body + 44)?));
                t.set("variable", r.cstring(r.at(body + 48)?));
                c.set("text", t);
            }
            3 => {
                r.require(body, 12)?;
                let glyph_count = r.at(body + 4)?;
                let glyphs_offset = r.at(body + 8)?;
                if glyph_count > 100_000 {
                    return Err(bad("unreasonable APT font glyph count"));
                }
                let mut glyphs = Vec::new();
                if glyph_count != 0 {
                    r.require(glyphs_offset, glyph_count * 4)?;
                    for i in 0..glyph_count {
                        glyphs.push(J::from(r.u32be(glyphs_offset + i * 4)?));
                    }
                }
                let mut f = Obj::new();
                f.set("name", r.cstring(r.at(body)?));
                f.set("glyph_character_ids", glyphs);
                c.set("font", f);
            }
            10 => {
                r.require(body, 48)?;
                c.set("matrix", floats(r, body + 16, 6)?);
                let record_count = r.at(body + 40)?;
                let records_offset = r.at(body + 44)?;
                if record_count > 100_000 {
                    return Err(bad("unreasonable APT static-text record count"));
                }
                r.require(records_offset, record_count * 56)?;
                let mut records = Vec::new();
                for i in 0..record_count {
                    let at = records_offset + i * 56;
                    let glyph_count = r.at(at + 48)?;
                    let glyphs_offset = r.at(at + 52)?;
                    if glyph_count > 1_000_000 {
                        return Err(bad("unreasonable APT static-text glyph count"));
                    }
                    r.require(glyphs_offset, glyph_count * 4)?;
                    let mut glyphs = Vec::new();
                    for g in 0..glyph_count {
                        let ga = glyphs_offset + g * 4;
                        let mut glyph = Obj::new();
                        glyph.set("index", r.u16be(ga)?);
                        glyph.set("advance", r.i16be(ga + 2)? as i32);
                        glyphs.push(J::Obj(glyph));
                    }
                    let mut record = Obj::new();
                    record.set("offset", at);
                    record.set("font_id", r.u32be(at)?);
                    record.set("color_transform", floats(r, at + 4, 8)?);
                    record.set("x_offset", r.f32be(at + 36)?);
                    record.set("y_offset", r.f32be(at + 40)?);
                    record.set("scale", r.f32be(at + 44)?);
                    record.set("glyphs", glyphs);
                    records.push(J::Obj(record));
                }
                let mut s = Obj::new();
                s.set("records", records);
                c.set("static_text", s);
            }
            5 | 9 => {
                r.require(body, 12)?;
                let mut m = Obj::new();
                let frame_count = r.u32be(body)?;
                m.set("frame_count", frame_count);
                m.set("frames_offset", r.u32be(body + 4)?);
                m.set("labels_offset", r.u32be(body + 8)?);
                if frame_count > 100_000 {
                    return Err(bad("unreasonable APT frame count"));
                }
                if type_id == 9 {
                    r.require(body, 52)?;
                    for (i, key) in [
                        "character_count",
                        "characters_offset",
                        "width",
                        "height",
                        "milliseconds_per_frame",
                        "import_count",
                        "imports_offset",
                        "export_count",
                        "exports_offset",
                        "current_constant_index",
                    ]
                    .iter()
                    .enumerate()
                    {
                        m.set(*key, r.u32be(body + 12 + i * 4)?);
                    }
                }
                c.set("movie", m);
            }
            7 => {
                r.require(body, 4)?;
                let mut b = Obj::new();
                b.set("texture_id", r.u32be(body)?);
                c.set("bitmap", b);
            }
            _ => {}
        }
        Ok(Some(c))
    }

    fn first_opcode(&self, c: &mut Obj, offset: usize) -> Result<(), Error> {
        let action = self.apt.at(offset)?;
        c.set("actions_offset", action);
        if action != 0 {
            self.apt.require(action, 1)?;
            c.set("first_action_opcode", self.apt.u8(action)?);
        }
        Ok(())
    }

    fn control(&self, offset: usize) -> Result<Obj, Error> {
        let r = &self.apt;
        r.require(offset, 4)?;
        let type_id = r.u32be(offset)?;
        let mut c = Obj::new();
        c.set("offset", offset);
        c.set("type", type_id);
        c.set("type_name", control_type(type_id));
        match type_id {
            3 | 9 => {
                r.require(offset, if type_id == 9 { 72 } else { 64 })?;
                let name_pointer = r.at(offset + 52)?;
                c.set("flags", r.u32be(offset + 4)?);
                c.set("depth", r.i32be(offset + 8)?);
                c.set("character_id", r.i32be(offset + 12)?);
                c.set("matrix", floats(r, offset + 16, 6)?);
                c.set(
                    "color_transform",
                    r.bytes(offset + 40, 8)?
                        .iter()
                        .map(|&b| J::from(b))
                        .collect::<Vec<_>>(),
                );
                c.set("ratio", r.f32be(offset + 48)?);
                c.set("name", r.cstring(name_pointer));
                c.set("clip_depth", r.i32be(offset + 56)?);
                c.set("actions_offset", r.u32be(offset + 60)?);
                if type_id == 9 {
                    c.set("blend_mode", r.i32be(offset + 64)?);
                    c.set("filter_pointer", r.u32be(offset + 68)?);
                }
            }
            2 => {
                r.require(offset, 8)?;
                c.set("label", r.cstring(r.at(offset + 4)?));
            }
            5 => {
                r.require(offset, 8)?;
                c.set("color", format!("#{:08x}", r.u32be(offset + 4)?));
            }
            1 => {
                r.require(offset, 8)?;
                self.first_opcode(&mut c, offset + 4)?;
            }
            8 => {
                r.require(offset, 12)?;
                c.set("sprite_id", r.u32be(offset + 4)?);
                self.first_opcode(&mut c, offset + 8)?;
            }
            4 | 11 => {
                r.require(offset, 8)?;
                c.set("depth", r.i32be(offset + 4)?);
            }
            6 => {
                r.require(offset, 8)?;
                c.set("sound_id", r.u32be(offset + 4)?);
            }
            _ => {}
        }
        Ok(c)
    }

    fn frames(&self, movie: &Obj) -> Result<Vec<J>, Error> {
        let field = |k: &str| movie.get(k).and_then(J::as_i64).unwrap_or(0) as usize;
        let count = field("frame_count");
        let frames_offset = field("frames_offset");
        let r = &self.apt;
        r.require(frames_offset, count * 8)?;
        let mut frames = Vec::with_capacity(count);
        for index in 0..count {
            let frame = frames_offset + index * 8;
            let control_count = r.at(frame)?;
            let controls_offset = r.at(frame + 4)?;
            if control_count > 100_000 {
                return Err(bad("unreasonable APT control count"));
            }
            r.require(controls_offset, control_count * 4)?;
            let mut controls = Vec::new();
            for i in 0..control_count {
                let pointer = r.at(controls_offset + i * 4)?;
                if pointer != 0 {
                    controls.push(J::Obj(self.control(pointer)?));
                }
            }
            let mut f = Obj::new();
            f.set("index", index);
            f.set("controls", controls);
            frames.push(J::Obj(f));
        }
        Ok(frames)
    }
}

pub fn inspect_apt(apt: &[u8], constants: &[u8]) -> Result<Movie, Error> {
    let constants = Reader::new(constants);
    let inspector = Inspector {
        apt: Reader::new(apt),
    };
    constants.require(0, 32)?;
    inspector.apt.require(0, 16)?;
    let main = constants.at(20)?;
    let root = inspector.character(main, 0)?;
    let mut movie = Movie {
        characters: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
    };
    let Some(root) = root else {
        return Ok(movie);
    };
    if !matches!(root.get("type").and_then(J::as_i64), Some(5 | 9)) {
        return Ok(movie);
    }
    let Some(J::Obj(m)) = root.get("movie") else {
        return Ok(movie);
    };
    let field = |k: &str| m.get(k).and_then(J::as_i64).unwrap_or(0) as usize;
    let r = &inspector.apt;
    let import_count = field("import_count");
    let imports_offset = field("imports_offset");
    if import_count > 100_000 {
        return Err(bad("unreasonable APT import count"));
    }
    if import_count != 0 {
        r.require(imports_offset, import_count * 16)?;
        for index in 0..import_count {
            let record = imports_offset + index * 16;
            movie.imports.push(Import {
                file: r.cstring(r.at(record)?),
                name: r.cstring(r.at(record + 4)?),
                character_id: r.u32be(record + 8)?,
            });
        }
    }
    let export_count = field("export_count");
    let exports_offset = field("exports_offset");
    if export_count > 100_000 {
        return Err(bad("unreasonable APT export count"));
    }
    if export_count != 0 {
        r.require(exports_offset, export_count * 8)?;
        for index in 0..export_count {
            let record = exports_offset + index * 8;
            movie
                .exports
                .push((r.cstring(r.at(record)?), r.u32be(record + 4)?));
        }
    }
    let count = field("character_count");
    let table = field("characters_offset");
    if count > 100_000 {
        return Err(bad("unreasonable APT character count"));
    }
    if count != 0 {
        r.require(table, count * 4)?;
        for id in 0..count {
            if let Some(mut c) = inspector.character(r.at(table + id * 4)?, id)? {
                if let Some(J::Obj(m)) = c.get("movie") {
                    let frames = inspector.frames(m)?;
                    c.set("frames", frames);
                }
                movie.characters.push(c);
            }
        }
    }
    // The root timeline is parsed too: a malformed one fails the movie.
    inspector.frames(m)?;
    Ok(movie)
}
