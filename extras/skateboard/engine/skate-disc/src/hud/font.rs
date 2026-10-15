//! EA::Text bitmap-font descriptions, .bmpfont (tools/vendor/skate3_ui/bitmap_font.py).
use super::binary::bad;
use super::json::{J, Obj, py_int, py_split, py_strip};
use crate::Error;

/// Windows-1252 text; the five undefined bytes fail as Python's strict codec does.
pub fn decode_cp1252(bytes: &[u8]) -> Result<String, Error> {
    const HIGH: [u16; 32] = [
        0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
        0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC,
        0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
    ];
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => match HIGH[b as usize - 0x80] {
                0 => Err(bad("bitmap font is not Windows-1252 text")),
                c => Ok(char::from_u32(c as u32).unwrap()),
            },
            _ => Ok(b as char),
        })
        .collect()
}

/// `^([A-Za-z][A-Za-z0-9]*):[ \t]*(.*)$` fields (multiline); each value runs
/// to the next field line, newlines folded to spaces, stripped.
fn fields(text: &str) -> Vec<(String, String)> {
    // (key, value start) for every matching line.
    let mut matches: Vec<(usize, String, usize)> = Vec::new();
    let mut line_start = 0;
    for line in text.split('\n') {
        let bytes = line.as_bytes();
        if bytes.first().is_some_and(u8::is_ascii_alphabetic) {
            let key_len = bytes
                .iter()
                .take_while(|b| b.is_ascii_alphanumeric())
                .count();
            if bytes.get(key_len) == Some(&b':') {
                let mut value = key_len + 1;
                while matches!(bytes.get(value), Some(b' ' | b'\t')) {
                    value += 1;
                }
                matches.push((line_start, line[..key_len].to_owned(), line_start + value));
            }
        }
        line_start += line.len() + 1;
    }
    let mut result: Vec<(String, String)> = Vec::new();
    for (i, (_, key, value_start)) in matches.iter().enumerate() {
        let end = matches.get(i + 1).map_or(text.len(), |m| m.0);
        let value = text[*value_start..end].replace('\r', "").replace('\n', " ");
        let value = py_strip(&value).to_owned();
        match result.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value,
            None => result.push((key.clone(), value)),
        }
    }
    result
}

fn number(value: &str) -> Result<J, Error> {
    if value.contains(['.', 'e', 'E']) {
        let t = py_strip(value);
        let ok = !t.contains("__") && !t.starts_with('_') && !t.ends_with('_');
        t.replace('_', "")
            .parse::<f64>()
            .ok()
            .filter(|_| ok)
            .map(J::Float)
            .ok_or(bad("bitmap-font metric is not a number"))
    } else {
        py_int(value)
            .map(J::Int)
            .ok_or(bad("bitmap-font metric is not an integer"))
    }
}

fn ints(chunk: &str) -> Result<Vec<i64>, Error> {
    py_split(chunk)
        .into_iter()
        .map(|v| py_int(v).ok_or(bad("bitmap-font value is not an integer")))
        .collect()
}

const SCALARS: [&str; 16] = [
    "Weight",
    "Stretch",
    "Size",
    "HAdvanceXMax",
    "VAdvanceYMax",
    "Ascent",
    "Descent",
    "Leading",
    "Baseline",
    "LineHeight",
    "XHeight",
    "CapsHeight",
    "UnderlinePosition",
    "UnderlineThickness",
    "LinethroughPosition",
    "LinethroughThickness",
];

/// The parsed font as the Python `parse_bitmap_font` dict. `source` is the
/// value written to its "source" field.
pub fn parse_bitmap_font(raw: &[u8], source: &str) -> Result<Obj, Error> {
    // Path.read_text: cp1252 with universal newlines.
    let text = decode_cp1252(raw)?
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let fields = fields(&text);
    let get = |k: &str| {
        fields
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    };
    for key in [
        "CharMapSet",
        "Family",
        "FullName",
        "GlyphMetricsMap",
        "Version",
    ] {
        if get(key).is_none() {
            return Err(bad("missing bitmap-font fields"));
        }
    }
    if get("Version") != Some("3") {
        return Err(bad("unsupported bitmap-font version"));
    }

    let mut metric_chunks: Vec<String> = get("GlyphMetricsMap")
        .unwrap()
        .split(',')
        .map(|c| py_strip(c).to_owned())
        .collect();
    let mut first = py_split(&metric_chunks[0]);
    if first.len() != 10 {
        return Err(bad("malformed first glyph metric"));
    }
    let declared_glyphs = py_int(first.remove(0)).ok_or(bad("malformed glyph count"))?;
    metric_chunks[0] = first.join(" ");
    let mut glyphs = Vec::new();
    for chunk in &metric_chunks {
        let v = ints(chunk)?;
        if v.len() != 9 {
            return Err(bad("malformed glyph metric"));
        }
        let mut glyph = Obj::new();
        for (key, value) in [
            "glyph_index",
            "texture_index",
            "x",
            "y",
            "width",
            "height",
            "x_offset",
            "y_offset",
            "x_advance",
        ]
        .iter()
        .zip(&v)
        {
            glyph.set(*key, *value);
        }
        // Pen origin plus bearing gives the atlas rectangle (EATextBmpFont.cpp).
        let left = v[2] + v[6];
        let top = v[3] - v[7];
        glyph.set("atlas_bounds", vec![left, top, left + v[4], top + v[5]]);
        glyphs.push(J::Obj(glyph));
    }
    if glyphs.len() as i64 != declared_glyphs {
        return Err(bad("bitmap-font glyph count differs from its declaration"));
    }

    let mut char_chunks: Vec<String> = get("CharMapSet")
        .unwrap()
        .split(',')
        .map(|c| py_strip(c).to_owned())
        .collect();
    let mut first = py_split(&char_chunks[0]);
    if first.len() != 3 {
        return Err(bad("malformed first character mapping"));
    }
    let declared_chars = py_int(first.remove(0)).ok_or(bad("malformed character count"))?;
    char_chunks[0] = first.join(" ");
    let mut characters = Vec::new();
    for chunk in &char_chunks {
        let v = ints(chunk)?;
        if v.len() != 2 {
            return Err(bad("malformed character mapping"));
        }
        let mut c = Obj::new();
        c.set("codepoint", v[0]);
        c.set("glyph_index", v[1]);
        characters.push(J::Obj(c));
    }
    if characters.len() as i64 != declared_chars {
        return Err(bad(
            "bitmap-font character count differs from its declaration",
        ));
    }

    let mut metrics = Obj::new();
    for key in SCALARS {
        if let Some(v) = get(key) {
            metrics.set(key, number(v)?);
        }
    }
    let mut textures: Vec<(i64, Obj)> = Vec::new();
    for (key, value) in &fields {
        let Some(index) = key.strip_prefix("Texture") else {
            continue;
        };
        let parts = py_split(value);
        if parts.len() < 4 {
            return Err(bad("malformed texture declaration"));
        }
        let index = py_int(index).ok_or(bad("malformed texture index"))?;
        let int = |s: &str| py_int(s).ok_or(bad("malformed texture declaration"));
        let mut t = Obj::new();
        t.set("index", index);
        t.set("bytes", int(parts[0])?);
        t.set("width", int(parts[1])?);
        t.set("height", int(parts[2])?);
        t.set("name", parts[3..].join(" "));
        textures.push((index, t));
    }
    textures.sort_by_key(|(index, _)| *index);

    let mut out = Obj::new();
    out.set("format", "skate3-bitmap-font");
    out.set("source", source);
    out.set("full_name", get("FullName").unwrap());
    out.set("family", get("Family").unwrap());
    out.set("style", get("Style").unwrap_or(""));
    out.set("smooth", get("Smooth") == Some("Yes"));
    out.set("fixed_pitch", get("FixedPitch") == Some("Yes"));
    out.set("metrics", metrics);
    out.set("glyphs", glyphs);
    out.set("characters", characters);
    out.set(
        "textures",
        textures
            .into_iter()
            .map(|(_, t)| J::Obj(t))
            .collect::<Vec<_>>(),
    );
    Ok(out)
}
