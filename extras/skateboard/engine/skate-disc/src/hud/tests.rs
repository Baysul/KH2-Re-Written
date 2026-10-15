//! Synthetic bytes only.
use super::actions::Actions;
use super::apt::inspect_apt;
use super::font::parse_bitmap_font;
use super::json::{J, Obj, dumps, py_float, py_strip};
use super::language::pair_language_tables;
use super::xenos::*;

#[test]
fn floats_and_strings_print_as_python_json() {
    let cases = [
        (1.0, "1.0"),
        (-0.0, "-0.0"),
        (0.1f32 as f64, "0.10000000149011612"),
        (1e-5, "1e-05"),
        (0.0001, "0.0001"),
        (1e16, "1e+16"),
        (1e15, "1000000000000000.0"),
        (123.456, "123.456"),
        (-2.5e-300, "-2.5e-300"),
        (f64::NAN, "NaN"),
        (f64::NEG_INFINITY, "-Infinity"),
    ];
    for (value, text) in cases {
        assert_eq!(py_float(value), text);
    }
    let mut o = Obj::new();
    o.set("b", "q\"\\\n\u{e9}\u{7f}\u{1f600}");
    o.set("a", vec![J::Int(-3), J::Null, J::Bool(true)]);
    o.set("b", "x\u{e9}");
    assert_eq!(
        dumps(&J::Obj(o)),
        "{\"b\":\"x\\u00e9\",\"a\":[-3,null,true]}"
    );
    let mut o = Obj::new();
    o.set("s", "\u{7f}\u{1f600}\t");
    assert_eq!(dumps(&J::Obj(o)), "{\"s\":\"\\u007f\\ud83d\\ude00\\t\"}");
    assert_eq!(py_strip("\u{1c} a\u{a0}\u{85}"), "a");
}

fn fetch_words(e: [u32; 6]) -> Vec<u8> {
    e.iter().flat_map(|w| w.to_be_bytes()).collect()
}

#[test]
fn fetch_constant_fields() {
    let bytes = fetch_words([1 << 31 | 3 << 22, 18 | 2 << 6, 63 | 31 << 13, 0, 3 << 6, 0]);
    let f = parse_fetch_constant(&bytes).unwrap();
    assert_eq!(
        f,
        Fetch {
            width: 64,
            height: 32,
            format_code: 18,
            mipmaps: 4,
            tiled: true,
            endian: 2,
            pitch: 96,
        }
    );
    assert_eq!(f.format_name(), "DXT1");
    assert_eq!(reverse_bits(0b1101, 4), 0b1011);
    assert!(parse_fetch_constant(&bytes[..20]).is_err());
}

#[test]
fn tiled_argb_untiles_and_swaps_to_rgba() {
    // 64x40 ARGB, tiled: put each texel's (x, y) at its tiled address.
    let (w, h) = (64usize, 40usize);
    let tiled_width = 64;
    let mut data = vec![0u8; 64 * 64 * 4];
    for y in 0..h {
        for x in 0..w {
            let at = tiled_offset(x, y, tiled_width, 2) * 4;
            // Big-endian ARGB as stored: A, R, G, B -> reversed to B G R A.
            data[at..at + 4].copy_from_slice(&[255, x as u8, y as u8, 7]);
        }
    }
    let fetch = Fetch {
        width: w as u32,
        height: h as u32,
        format_code: 6,
        mipmaps: 1,
        tiled: true,
        endian: 0,
        pitch: 0,
    };
    let rgba = decode_rgba(&data, &fetch).unwrap();
    assert_eq!(rgba.len(), w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            assert_eq!(&rgba[o..o + 4], &[x as u8, y as u8, 7, 255][..]);
        }
    }
    // Distinct texels land at distinct addresses within the padded surface.
    let mut seen = std::collections::HashSet::new();
    for y in 0..64 {
        for x in 0..64 {
            assert!(seen.insert(tiled_offset(x, y, 64, 2)));
        }
    }
    assert!(seen.iter().all(|&a| a < 64 * 64));
    // Truncated payloads fail rather than read out of range.
    assert!(decode_rgba(&data[..100], &fetch).is_err());
}

#[test]
fn linear_dxt1_and_dxt5_blocks() {
    // Untiled DXT1 4x4: red/blue endpoints.
    let mut fetch = Fetch {
        width: 4,
        height: 4,
        format_code: 18,
        mipmaps: 1,
        tiled: false,
        endian: 0,
        pitch: 0,
    };
    // Stored big-endian 16-bit words: undo the byte swap the decoder applies.
    let mut le = Vec::new();
    for v in [0xF800u16, 0x001F] {
        le.extend_from_slice(&v.to_le_bytes());
    }
    le.extend_from_slice(&[0b0001_1011, 0, 0, 0]);
    let swapped: Vec<u8> = le.chunks(2).flat_map(|p| [p[1], p[0]]).collect();
    let rgba = decode_rgba(&swapped, &fetch).unwrap();
    assert_eq!(&rgba[0..4], &[85, 0, 170, 255]); // index 3: (c0 + 2 c1) / 3
    assert_eq!(&rgba[4..8], &[170, 0, 85, 255]); // index 2
    assert_eq!(&rgba[8..12], &[0, 0, 255, 255]); // index 1: blue
    assert_eq!(&rgba[12..16], &[255, 0, 0, 255]); // index 0: red

    // c0 <= c1 selects the punch-through table.
    let mut le = Vec::new();
    for v in [0x001Fu16, 0xF800] {
        le.extend_from_slice(&v.to_le_bytes());
    }
    le.extend_from_slice(&[3, 0, 0, 0]);
    let swapped: Vec<u8> = le.chunks(2).flat_map(|p| [p[1], p[0]]).collect();
    assert_eq!(&decode_rgba(&swapped, &fetch).unwrap()[0..4], &[0, 0, 0, 0]);

    // DXT5: alpha 255..0 with index 1 everywhere except pixel 0 (index 2).
    fetch.format_code = 20;
    let mut le = vec![255u8, 0];
    let bits: u64 = (0..16u64)
        .map(|p| (if p == 0 { 2 } else { 1 }) << (p * 3))
        .sum();
    le.extend_from_slice(&bits.to_le_bytes()[..6]);
    for v in [0xFFFFu16, 0x0000] {
        le.extend_from_slice(&v.to_le_bytes());
    }
    le.extend_from_slice(&[0, 0, 0, 0]);
    let swapped: Vec<u8> = le.chunks(2).flat_map(|p| [p[1], p[0]]).collect();
    let rgba = decode_rgba(&swapped, &fetch).unwrap();
    assert_eq!(&rgba[0..4], &[255, 255, 255, 218]); // (6*255 + 0) / 7
    assert_eq!(rgba[7], 0);
}

#[test]
fn bitmap_font_fields() {
    let text = b"Version: 3\r\nFullName: Test Font\r\nFamily: Test\r\nSize: 12\r\nAscent: 10.5\r\n\
GlyphMetricsMap: 2 0 0 1 2 3 4 1 2 5,\r\n 1 0 10 20 3 4 0 1 6\r\nCharMapSet: 2 65 0, 66 1\r\n\
Texture1: 5 8 8 b.tga\r\nTexture0: 1024 32 16 a b.tga\r\nSmooth: Yes\r\n";
    let font = parse_bitmap_font(text, "raw/x.bmpfont").unwrap();
    let json = dumps(&J::Obj(font));
    assert!(json.starts_with(r#"{"format":"skate3-bitmap-font","source":"raw/x.bmpfont","full_name":"Test Font","family":"Test","style":"","smooth":true,"fixed_pitch":false,"metrics":{"Size":12,"Ascent":10.5},"glyphs":[{"glyph_index":0,"texture_index":0,"x":1,"y":2,"width":3,"height":4,"x_offset":1,"y_offset":2,"x_advance":5,"atlas_bounds":[2,0,5,4]}"#));
    assert!(json.contains(
        r#""characters":[{"codepoint":65,"glyph_index":0},{"codepoint":66,"glyph_index":1}]"#
    ));
    assert!(json.ends_with(r#""textures":[{"index":0,"bytes":1024,"width":32,"height":16,"name":"a b.tga"},{"index":1,"bytes":5,"width":8,"height":8,"name":"b.tga"}]}"#));
    // Declared counts must match; undefined cp1252 bytes fail.
    assert!(parse_bitmap_font(&text.replace_first(b"2 65", b"3 65"), "").is_err());
    assert!(parse_bitmap_font(b"Version: 3\x81", "").is_err());
}

trait ReplaceFirst {
    fn replace_first(&self, from: &[u8], to: &[u8]) -> Vec<u8>;
}
impl ReplaceFirst for [u8] {
    fn replace_first(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
        let at = self.windows(from.len()).position(|w| w == from).unwrap();
        [&self[..at], to, &self[at + from.len()..]].concat()
    }
}

fn put(data: &mut Vec<u8>, at: usize, bytes: &[u8]) {
    if data.len() < at + bytes.len() {
        data.resize(at + bytes.len(), 0);
    }
    data[at..at + bytes.len()].copy_from_slice(bytes);
}
fn put32(data: &mut Vec<u8>, at: usize, v: u32) {
    put(data, at, &v.to_be_bytes());
}

fn movie() -> (Vec<u8>, Vec<u8>) {
    let mut apt = vec![0u8; 16];
    // Root movie (type 9): one empty frame at 240, characters at 100.
    put32(&mut apt, 16, 9);
    put32(&mut apt, 32, 1);
    put32(&mut apt, 36, 240);
    put32(&mut apt, 44, 3);
    put32(&mut apt, 48, 100);
    put(&mut apt, 100, &[0; 4]);
    put32(&mut apt, 104, 120);
    put32(&mut apt, 108, 160);
    // Character 1: a shape with bounds.
    put32(&mut apt, 120, 1);
    for (i, v) in [1.0f32, 2.0, 3.0, 4.5].iter().enumerate() {
        put(&mut apt, 136 + i * 4, &v.to_be_bytes());
    }
    // Character 2: a sprite whose one frame runs actions at 220.
    put32(&mut apt, 160, 5);
    put32(&mut apt, 176, 1);
    put32(&mut apt, 180, 200);
    put32(&mut apt, 200, 1);
    put32(&mut apt, 204, 208);
    put32(&mut apt, 208, 212);
    put32(&mut apt, 212, 1);
    put32(&mut apt, 216, 220);
    // push constant 0; stop; end.
    put(&mut apt, 220, &[0x96]);
    put32(&mut apt, 224, 1);
    put32(&mut apt, 228, 300);
    put(&mut apt, 232, &[0x07, 0x00]);
    put32(&mut apt, 240, 0);
    put32(&mut apt, 300, 0);
    let mut constants = vec![0u8; 32];
    put32(&mut constants, 20, 16);
    put32(&mut constants, 24, 1);
    put32(&mut constants, 28, 32);
    put32(&mut constants, 32, 1);
    put32(&mut constants, 36, 48);
    put(&mut constants, 48, b"hi\0");
    (apt, constants)
}

#[test]
fn apt_characters_frames_and_actions() {
    let (apt, constants) = movie();
    let m = inspect_apt(&apt, &constants).unwrap();
    assert_eq!(m.characters.len(), 2);
    assert_eq!(
        dumps(&J::Obj(m.characters[0].clone())),
        r#"{"id":1,"offset":120,"type":1,"type_name":"shape","bounds":[1.0,2.0,3.0,4.5]}"#
    );
    assert_eq!(
        dumps(&J::Obj(m.characters[1].clone())),
        r#"{"id":2,"offset":160,"type":5,"type_name":"sprite","movie":{"frame_count":1,"frames_offset":200,"labels_offset":0},"frames":[{"index":0,"controls":[{"offset":212,"type":1,"type_name":"do_action","actions_offset":220,"first_action_opcode":150}]}]}"#
    );
    let actions = Actions::new(&apt, &constants).unwrap();
    let rows = actions.stream(220, None, 0).unwrap();
    assert_eq!(
        dumps(&J::Arr(rows)),
        r#"[{"offset":220,"opcode":150,"values":[{"kind":1,"value":"hi"}],"next":232},{"offset":232,"opcode":7,"next":233},{"offset":233,"opcode":0}]"#
    );
    // A constant index past the CONST table is refused.
    let mut bad = apt.clone();
    put32(&mut bad, 300, 5);
    assert!(
        Actions::new(&bad, &constants)
            .unwrap()
            .stream(220, None, 0)
            .is_err()
    );
    // A frame table outside the file fails the whole movie.
    let mut bad = apt.clone();
    put32(&mut bad, 180, 0x10000);
    assert!(inspect_apt(&bad, &constants).is_err());
}

fn table(strings: &[&[u8]]) -> Vec<u8> {
    let mut data = vec![0u8; 28];
    data[8..12].copy_from_slice(&((strings.len() - 2) as u32).to_le_bytes());
    data[12..16].copy_from_slice(&28u32.to_le_bytes());
    data[16..20].copy_from_slice(&28u32.to_le_bytes());
    data.extend_from_slice(&[0; 8]);
    data.extend_from_slice(&strings.join(&b"\0"[..]));
    data
}

#[test]
fn language_pairs_latin1() {
    let labels = table(&[b"", b"A ", b"B"]);
    let values = table(&[b"", b"caf\xe9", b"x"]);
    let pairs = pair_language_tables(&labels, &values).unwrap();
    assert_eq!(pairs[1], ("A ".into(), "caf\u{e9}".into()));
    assert!(pair_language_tables(&labels, &table(&[b"", b"", b"", b""])).is_err());
}
