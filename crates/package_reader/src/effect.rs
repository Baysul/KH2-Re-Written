//! Object effect data (BAR type 18 "PAX_") as the exe loads it: PAX
//! elements select DPX effects, each a particle-data program (PDATA) in a
//! DPD with its shapes, textures, models and vertex sets. Offsets inside a
//! PDATA program block stay raw: the VM reads keyframes and the words after
//! them exactly where the exe does (FUN_1404b06d0 relocates the same
//! fields, FUN_1402c1610 registers shapes/models/VSFs).
use std::{collections::HashMap, io, sync::Arc};

fn invalid(m: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, m.to_owned())
}

pub fn u32_at(b: &[u8], at: usize) -> io::Result<u32> {
    b.get(at..at + 4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .ok_or_else(|| invalid("effect data truncated"))
}
pub fn i32_at(b: &[u8], at: usize) -> io::Result<i32> {
    u32_at(b, at).map(|v| v as i32)
}
pub fn i16_at(b: &[u8], at: usize) -> io::Result<i16> {
    b.get(at..at + 2)
        .map(|w| i16::from_le_bytes([w[0], w[1]]))
        .ok_or_else(|| invalid("effect data truncated"))
}
pub fn f32_at(b: &[u8], at: usize) -> io::Result<f32> {
    u32_at(b, at).map(f32::from_bits)
}

/// One PAX element (0x50 bytes; layout checked against FUN_1402c6b30).
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub effect_number: u16,
    pub id: u16,
    pub group: u8,
    pub fadeout_frame: u8,
    pub bone: i16,
    pub category: u64,
    pub flags: u32,
    pub start_wait: f32,
    pub sound: i32,
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

/// A DPX record: which DPD and PDATA play for an effect number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpxEffect {
    pub dpd: usize,
    pub pdata: usize,
    pub effect_number: u32,
}

#[derive(Debug)]
pub struct Pax {
    pub name: String,
    pub elements: Vec<Element>,
    pub effects: Vec<DpxEffect>,
    pub dpds: Vec<Arc<Dpd>>,
}

impl Pax {
    /// First element whose category contains every bit of `mask`
    /// (FUN_1402c6730).
    pub fn by_category(&self, mask: u64) -> Option<&Element> {
        self.elements.iter().find(|e| e.category & mask == mask)
    }

    /// First element with `id` (FUN_1402c6b30).
    pub fn by_id(&self, id: u16) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id)
    }

    /// The DPX effect for an element's effect number (FUN_1402bd510).
    pub fn effect(&self, number: u16) -> Option<DpxEffect> {
        self.effects
            .iter()
            .copied()
            .find(|e| e.effect_number == number as u32)
    }
}

pub fn read_pax(bytes: &[u8]) -> io::Result<Pax> {
    if bytes.get(..4) != Some(b"PAX_") {
        return Err(invalid("not a PAX"));
    }
    let name_at = u32_at(bytes, 4)? as usize;
    let count = u32_at(bytes, 8)? as usize;
    let dpx = u32_at(bytes, 12)? as usize;
    if count > 1024 {
        return Err(invalid("PAX element count outside inspection bounds"));
    }
    let name = bytes
        .get(name_at..(name_at + 0x80).min(bytes.len()))
        .map(|n| {
            let end = n.iter().position(|&b| b == 0).unwrap_or(n.len());
            String::from_utf8_lossy(&n[..end]).into_owned()
        })
        .unwrap_or_default();
    let mut elements = Vec::with_capacity(count);
    for i in 0..count {
        let o = 16 + i * 0x50;
        let v3 = |at: usize| -> io::Result<[f32; 3]> {
            Ok([
                f32_at(bytes, at)?,
                f32_at(bytes, at + 4)?,
                f32_at(bytes, at + 8)?,
            ])
        };
        elements.push(Element {
            effect_number: i16_at(bytes, o)? as u16,
            id: i16_at(bytes, o + 2)? as u16,
            group: *bytes.get(o + 4).ok_or_else(|| invalid("PAX truncated"))?,
            fadeout_frame: *bytes.get(o + 5).ok_or_else(|| invalid("PAX truncated"))?,
            bone: i16_at(bytes, o + 6)?,
            category: u32_at(bytes, o + 8)? as u64 | (u32_at(bytes, o + 12)? as u64) << 32,
            flags: u32_at(bytes, o + 0x10)?,
            start_wait: f32_at(bytes, o + 0x14)?,
            sound: i32_at(bytes, o + 0x18)?,
            translation: v3(o + 0x1c)?,
            rotation: v3(o + 0x28)?,
            scale: v3(o + 0x34)?,
        });
    }
    let x = bytes.get(dpx..).ok_or_else(|| invalid("PAX DPX offset"))?;
    let effect_count = u32_at(x, 12)? as usize;
    if effect_count > 1024 {
        return Err(invalid("DPX effect count outside inspection bounds"));
    }
    let table = 16 + effect_count * 32;
    let dpd_count = u32_at(x, table)? as usize;
    if dpd_count > 256 {
        return Err(invalid("DPX DPD count outside inspection bounds"));
    }
    let offsets: Vec<usize> = (0..dpd_count)
        .map(|i| u32_at(x, table + 4 + 4 * i).map(|o| o as usize))
        .collect::<io::Result<_>>()?;
    let mut effects = Vec::with_capacity(effect_count);
    for i in 0..effect_count {
        let r = 16 + i * 32;
        let dpd_offset = u32_at(x, r)? as usize;
        effects.push(DpxEffect {
            dpd: offsets
                .iter()
                .position(|&o| o == dpd_offset)
                .ok_or_else(|| invalid("DPX record names no DPD"))?,
            pdata: u32_at(x, r + 4)? as usize,
            effect_number: u32_at(x, r + 8)?,
        });
    }
    let mut dpds = Vec::with_capacity(dpd_count);
    for (j, &start) in offsets.iter().enumerate() {
        let end = offsets.get(j + 1).copied().unwrap_or(x.len());
        let slice = x
            .get(start..end)
            .ok_or_else(|| invalid("DPD range outside DPX"))?;
        dpds.push(Arc::new(read_dpd(slice)?));
    }
    Ok(Pax {
        name,
        elements,
        effects,
        dpds,
    })
}

/// A program record (0x10 bytes): type, keyframe size and count, and the
/// keyframe and work-variable list positions (absolute in the DPD bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Program {
    pub kind: u32,
    pub param_size: usize,
    pub keys: usize,
    pub params: usize,
    pub use_val: usize,
}

/// One particle entry (part type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub flag: u32,
    pub status: i32,
    pub start_frame: i32,
    pub total_frame: i32,
    pub loop_start: i32,
    pub loop_end: i32,
    /// Work block size including the 0xa0-byte header; the cursor array
    /// follows it.
    pub work_size: usize,
    pub priority: i16,
    pub programs: Vec<Program>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParticleData {
    pub offset: [f32; 4],
    pub angle: [i32; 4],
    pub scale: [f32; 4],
    pub use_shape: [u8; 16],
    pub character_scale: u8,
    /// Instance end time (VM frames × 0x1000) and loop flag (block +0/+4).
    pub end_time: i32,
    pub looping: bool,
    pub entries: Vec<Entry>,
}

/// Shape (sprite flip-book) registered by its id: anim header position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeRef {
    pub anim: usize,
    pub packet_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vsf {
    pub model: i16,
    pub indices: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpdTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug)]
pub struct Dpd {
    pub bytes: Vec<u8>,
    pub pdata: Vec<ParticleData>,
    pub shapes: HashMap<i16, ShapeRef>,
    /// Model vertex arrays (3×i16 per vertex) by model id.
    pub model_vertices: HashMap<i16, usize>,
    pub vsf: HashMap<i32, Vsf>,
    pub textures: Vec<DpdTexture>,
}

impl Dpd {
    pub fn vertex(&self, model: i16, index: u16) -> Option<[i16; 3]> {
        let base = *self.model_vertices.get(&model)? + index as usize * 6;
        Some([
            i16_at(&self.bytes, base).ok()?,
            i16_at(&self.bytes, base + 2).ok()?,
            i16_at(&self.bytes, base + 4).ok()?,
        ])
    }
}

fn offset_list(b: &[u8], at: usize) -> io::Result<(Vec<usize>, usize)> {
    let n = u32_at(b, at)? as usize;
    if n > 4096 {
        return Err(invalid("DPD list outside inspection bounds"));
    }
    let list = (0..n)
        .map(|i| u32_at(b, at + 4 + 4 * i).map(|o| o as usize))
        .collect::<io::Result<_>>()?;
    Ok((list, at + 4 + 4 * n))
}

pub fn read_dpd(bytes: &[u8]) -> io::Result<Dpd> {
    if u32_at(bytes, 0)? != 0x96 {
        return Err(invalid("DPD version other than 0x96"));
    }
    let mut at = 4;
    let mut lists = Vec::new();
    for _ in 0..5 {
        let (l, next) = offset_list(bytes, at)?;
        lists.push(l);
        at = next;
    }
    let pdata = lists[0]
        .iter()
        .map(|&p| read_pdata(bytes, p))
        .collect::<io::Result<_>>()?;
    let textures = lists[1]
        .iter()
        .map(|&t| read_texture(bytes, t))
        .collect::<io::Result<_>>()?;
    let mut shapes = HashMap::new();
    for &s in &lists[2] {
        shapes.insert(
            i16_at(bytes, s)?,
            ShapeRef {
                anim: s + 0x10,
                packet_size: u32_at(bytes, s + 8)?,
            },
        );
    }
    let mut model_vertices = HashMap::new();
    for &m in &lists[3] {
        let base = m + 0x10;
        model_vertices.insert(i16_at(bytes, m)?, base + i32_at(bytes, base + 8)? as usize);
    }
    let mut vsf = HashMap::new();
    for &v in &lists[4] {
        let n = i32_at(bytes, v + 8)? as usize;
        if n > 65536 {
            return Err(invalid("VSF count outside inspection bounds"));
        }
        vsf.insert(
            i32_at(bytes, v)?,
            Vsf {
                model: i32_at(bytes, v + 4)? as i16,
                indices: (0..n)
                    .map(|i| i16_at(bytes, v + 12 + 2 * i).map(|x| x as u16))
                    .collect::<io::Result<_>>()?,
            },
        );
    }
    Ok(Dpd {
        bytes: bytes.to_vec(),
        pdata,
        shapes,
        model_vertices,
        vsf,
        textures,
    })
}

fn read_pdata(b: &[u8], p: usize) -> io::Result<ParticleData> {
    let v4f = |at: usize| -> io::Result<[f32; 4]> {
        Ok([
            f32_at(b, at)?,
            f32_at(b, at + 4)?,
            f32_at(b, at + 8)?,
            f32_at(b, at + 12)?,
        ])
    };
    let block = p + 0x110;
    let mut use_shape = [0; 16];
    use_shape.copy_from_slice(b.get(p + 0xc0..p + 0xd0).ok_or_else(|| invalid("PDATA"))?);
    // The entry list (block +0xc → count, then block-relative offsets) is
    // the group table FUN_1404b1600 sizes from.
    let (offsets, _) = offset_list(b, block + u32_at(b, block + 0xc)? as usize)?;
    let mut entries = Vec::with_capacity(offsets.len());
    for e in offsets.into_iter().map(|o| block + o) {
        let count = i16_at(b, e + 0x26)?;
        if !(0..=64).contains(&count) {
            return Err(invalid("PDATA program count"));
        }
        let programs = (0..count as usize)
            .map(|j| {
                let r = e + 0x28 + 0x10 * j;
                Ok(Program {
                    kind: u32_at(b, r)?,
                    param_size: i16_at(b, r + 4)? as u16 as usize,
                    keys: i16_at(b, r + 6)? as u16 as usize,
                    params: block + u32_at(b, r + 8)? as usize,
                    use_val: block + u32_at(b, r + 12)? as usize,
                })
            })
            .collect::<io::Result<_>>()?;
        entries.push(Entry {
            flag: u32_at(b, e + 8)?,
            status: i32_at(b, e + 0xc)?,
            start_frame: i32_at(b, e + 0x10)?,
            total_frame: i32_at(b, e + 0x14)?,
            loop_start: i32_at(b, e + 0x18)?,
            loop_end: i32_at(b, e + 0x1c)?,
            work_size: i32_at(b, e + 0x20)? as usize,
            priority: i16_at(b, e + 0x24)?,
            programs,
        });
    }
    Ok(ParticleData {
        offset: v4f(p + 0x80)?,
        angle: [
            i32_at(b, p + 0x90)?,
            i32_at(b, p + 0x94)?,
            i32_at(b, p + 0x98)?,
            i32_at(b, p + 0x9c)?,
        ],
        scale: v4f(p + 0xa0)?,
        use_shape,
        character_scale: *b.get(p + 0xf9).ok_or_else(|| invalid("PDATA"))?,
        end_time: i32_at(b, block)?,
        looping: u32_at(b, block + 4)? & 1 != 0,
        entries,
    })
}

/// "TEX@" PSMT8 texture: 0x20-byte header, w·h indices, then a 256-colour
/// CSM1 palette with PS2 alpha (0x80 = opaque).
fn read_texture(b: &[u8], t: usize) -> io::Result<DpdTexture> {
    let psm = i16_at(b, t + 6)?;
    let w = i16_at(b, t + 12)? as usize;
    let h = i16_at(b, t + 14)? as usize;
    if psm != 0x13 || w == 0 || h == 0 || w > 1024 || h > 1024 {
        return Err(invalid("DPD texture outside the PSMT8 profile"));
    }
    let pixels = b
        .get(t + 0x20..t + 0x20 + w * h)
        .ok_or_else(|| invalid("DPD texture truncated"))?;
    let palette = b
        .get(t + 0x20 + w * h..t + 0x20 + w * h + 1024)
        .ok_or_else(|| invalid("DPD palette truncated"))?;
    Ok(DpdTexture {
        width: w as u32,
        height: h as u32,
        rgba: decode_psmt8(pixels, palette),
    })
}

/// Indexed pixels through a CSM1 palette (index bits 3 and 4 swapped),
/// alpha scaled from 0..0x80 to 0..255.
pub fn decode_psmt8(pixels: &[u8], palette: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 4);
    for &i in pixels {
        let i = i as usize;
        let swapped = (i & !0x18) | ((i & 0x08) << 1) | ((i & 0x10) >> 1);
        let c = &palette[swapped * 4..swapped * 4 + 4];
        out.extend([c[0], c[1], c[2], (c[3] as u32 * 255 / 128).min(255) as u8]);
    }
    out
}

/// One sprite of a shape frame, with its eight colour/blend bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    /// Corners in drawing order (v0, v1, v2, v3), raw 12.4 units.
    pub corners: [[i16; 2]; 4],
    /// S, T, W, H raw texture coordinates.
    pub uv: [i16; 4],
    /// R, G, B, A, GS ALPHA byte, FIX, unknown, CLUT offset.
    pub colour: [u8; 8],
}

/// Sprite frame `index` of the shape whose anim header is at `anim`
/// (FUN_1404ad7b0 axis-aligned / FUN_1404ada10 four-corner).
pub fn shape_frame(b: &[u8], anim: usize, index: usize) -> io::Result<Vec<Sprite>> {
    let address = i16_at(b, anim + 0x10 + index * 8)? as isize;
    let f = (anim as isize + address) as usize;
    let vuflag = i16_at(b, f)?;
    let count = i16_at(b, f + 2)?.max(0) as usize;
    let colours = f + i32_at(b, f + 4)? as usize;
    let four = vuflag & 8 != 0;
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        let (stride, base) = if four { (24, f + 0x20) } else { (16, f + 0x20) };
        let s = base + k * stride;
        let w = |i: usize| i16_at(b, s + 2 * i);
        let corners = if four {
            // v0 = (X0,Y0), v1 = (X1,Y1), v2 = (X3,Y3), v3 = (X2,Y2).
            [
                [w(0)?, w(1)?],
                [w(2)?, w(3)?],
                [w(6)?, w(7)?],
                [w(4)?, w(5)?],
            ]
        } else {
            [
                [w(0)?, w(1)?],
                [w(2)?, w(1)?],
                [w(2)?, w(3)?],
                [w(0)?, w(3)?],
            ]
        };
        let uv_at = if four { 8 } else { 4 };
        let mut colour = [0; 8];
        colour.copy_from_slice(
            b.get(colours + k * 8..colours + k * 8 + 8)
                .ok_or_else(|| invalid("shape colours truncated"))?,
        );
        out.push(Sprite {
            corners,
            uv: [w(uv_at)?, w(uv_at + 1)?, w(uv_at + 2)?, w(uv_at + 3)?],
            colour,
        });
    }
    Ok(out)
}

/// Shape sequence table: (frame wait, loop flag) for each sequence.
pub fn shape_sequence(b: &[u8], anim: usize, index: usize) -> io::Result<(i16, u8)> {
    let r = anim + 0x10 + index * 8;
    Ok((
        i16_at(b, r + 2)?,
        *b.get(r + 4).ok_or_else(|| invalid("shape"))?,
    ))
}

pub fn shape_sequence_count(b: &[u8], anim: usize) -> io::Result<i16> {
    i16_at(b, anim + 6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_csm1_palette_swizzle_and_alpha() {
        let mut palette = vec![0u8; 1024];
        palette[16 * 4..16 * 4 + 4].copy_from_slice(&[1, 2, 3, 0x80]);
        palette[8 * 4..8 * 4 + 4].copy_from_slice(&[9, 9, 9, 0x40]);
        // Index 8 reads palette slot 16 and index 16 reads slot 8.
        let rgba = decode_psmt8(&[8, 16], &palette);
        assert_eq!(&rgba[..4], &[1, 2, 3, 255]);
        assert_eq!(&rgba[4..], &[9, 9, 9, 127]);
    }

    fn synthetic_dpd() -> Vec<u8> {
        // Version, then lists: 1 PDATA, 0 textures, 1 shape, 1 model, 1 VSF.
        let mut b = vec![0u8; 0x400];
        let w =
            |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        w(&mut b, 0, 0x96);
        w(&mut b, 4, 1);
        w(&mut b, 8, 0x40);
        w(&mut b, 12, 0);
        w(&mut b, 16, 1);
        w(&mut b, 20, 0x300);
        w(&mut b, 24, 1);
        w(&mut b, 28, 0x340);
        w(&mut b, 32, 1);
        w(&mut b, 36, 0x380);
        // PDATA at 0x40: block at 0x150, end time 0x4000, one entry at +0x10.
        let block = 0x40 + 0x110;
        w(&mut b, block, 0x4000);
        w(&mut b, block + 0xc, 0x100);
        w(&mut b, block + 0x100, 1);
        w(&mut b, block + 0x104, 0x10);
        let e = block + 0x10;
        w(&mut b, e + 0x14, 0x2000);
        w(&mut b, e + 0x20, 0xb0);
        b[e + 0x26] = 1;
        w(&mut b, e + 0x28, 40);
        b[e + 0x2c] = 16;
        b[e + 0x2e] = 1;
        w(&mut b, e + 0x30, 0x80);
        w(&mut b, e + 0x34, 0x90);
        // Shape id 3, model id 1 (vertices at +0x10+0x20), VSF 0 → model 1.
        b[0x300] = 3;
        b[0x340] = 1;
        w(&mut b, 0x340 + 0x18, 0x20);
        b[0x340 + 0x30..0x340 + 0x36].copy_from_slice(&[5, 0, 0xfb, 0xff, 7, 0]);
        w(&mut b, 0x380 + 4, 1);
        w(&mut b, 0x380 + 8, 2);
        b[0x380 + 12] = 0;
        b[0x380 + 14] = 0;
        b
    }

    #[test]
    fn reads_synthetic_dpd_lists_and_entries() {
        let dpd = read_dpd(&synthetic_dpd()).unwrap();
        let p = &dpd.pdata[0];
        assert_eq!((p.end_time, p.looping), (0x4000, false));
        let e = &p.entries[0];
        assert_eq!((e.total_frame, e.work_size), (0x2000, 0xb0));
        assert_eq!(e.programs[0].kind, 40);
        assert_eq!(e.programs[0].params, 0x150 + 0x80);
        assert_eq!(dpd.shapes[&3].anim, 0x310);
        assert_eq!(dpd.vsf[&0].indices, vec![0, 0]);
        assert_eq!(dpd.vertex(1, 0), Some([5, -5, 7]));
        assert!(read_dpd(&[0; 8]).is_err());
    }
}
