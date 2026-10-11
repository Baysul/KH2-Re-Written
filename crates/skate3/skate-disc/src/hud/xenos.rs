//! Xbox 360 texture fetch constants, base-mip untiling and DXT/ARGB decoding
//! (tools/vendor/skate3_ui/xenos.py).
use super::binary::{align, bad};
use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fetch {
    pub width: u32,
    pub height: u32,
    pub format_code: u32,
    pub mipmaps: u32,
    pub tiled: bool,
    pub endian: u32,
    pub pitch: u32,
}

impl Fetch {
    pub fn format_name(&self) -> String {
        match self.format_code {
            6 => "A8R8G8B8".into(),
            18 => "DXT1".into(),
            19 => "DXT3".into(),
            20 => "DXT5".into(),
            n => format!("XENOS_{n}"),
        }
    }
}

pub fn reverse_bits(value: u32, size: u32) -> u32 {
    (0..size).fold(0, |r, bit| r | (((value >> bit) & 1) << (size - bit - 1)))
}

/// The 24-byte Xbox 360 texture fetch constant used by RX2.
pub fn parse_fetch_constant(data: &[u8]) -> Result<Fetch, Error> {
    if data.len() != 24 {
        return Err(bad("Xbox texture fetch constant is not 24 bytes"));
    }
    let e: Vec<u32> = data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_be_bytes(*c))
        .collect();
    let words = [
        reverse_bits(e[0], 32),
        reverse_bits(e[1], 32),
        e[2],
        reverse_bits(e[3], 32),
        reverse_bits(e[4], 32),
        reverse_bits(e[5], 32),
    ];
    Ok(Fetch {
        width: (words[2] & 0x1FFF) + 1,
        height: ((words[2] >> 13) & 0x1FFF) + 1,
        format_code: reverse_bits((words[1] >> 26) & 0x3F, 6),
        mipmaps: reverse_bits((words[4] >> 22) & 0xF, 4) + 1,
        tiled: reverse_bits(words[0] & 1, 1) != 0,
        endian: reverse_bits((words[1] >> 24) & 0x3, 2),
        pitch: ((e[0] >> 22) & 0x1FF) << 5,
    })
}

pub fn tiled_offset(x: usize, y: usize, width: usize, log_bpb: u32) -> usize {
    let aligned_width = align(width, 32);
    let macro_ = ((x >> 5) + (y >> 5) * (aligned_width >> 5)) << (log_bpb + 7);
    let micro = ((x & 7) + ((y & 0xE) << 2)) << log_bpb;
    let offset = macro_ + ((micro & !0xF) << 1) + (micro & 0xF) + ((y & 1) << 4);
    let address = ((offset & !0x1FF) << 3)
        + ((y & 16) << 7)
        + ((offset & 0x1C0) << 2)
        + (((((y & 8) >> 2) + (x >> 3)) & 3) << 6)
        + (offset & 0x3F);
    address >> log_bpb
}

/// (texels per block side, bytes per block).
fn layout(fetch: &Fetch) -> Result<(usize, usize), Error> {
    match fetch.format_code {
        18 => Ok((4, 8)),
        19 | 20 => Ok((4, 16)),
        6 => Ok((1, 4)),
        _ => Err(bad("unsupported Xbox texture format")),
    }
}

fn endian_swap(block: &[u8], compressed: bool, out: &mut [u8]) {
    if compressed {
        out.copy_from_slice(block);
        for pair in out.as_chunks_mut::<2>().0 {
            pair.swap(0, 1);
        }
    } else {
        for (o, b) in out.iter_mut().zip(block.iter().rev()) {
            *o = *b;
        }
    }
}

/// Untiles and endian-corrects the base mip.
pub fn linearize_base_level(data: &[u8], fetch: &Fetch) -> Result<Vec<u8>, Error> {
    let (block_size, bpb) = layout(fetch)?;
    let (width, height) = (fetch.width as usize, fetch.height as usize);
    let block_width = width.div_ceil(block_size).max(1);
    let block_height = height.div_ceil(block_size).max(1);
    let mut linear = vec![0u8; block_width * block_height * bpb];
    let compressed = block_size == 4;
    if !fetch.tiled {
        let pitch_texels = if fetch.pitch != 0 {
            fetch.pitch as usize
        } else {
            width
        };
        let pitch_blocks = block_width.max(pitch_texels.div_ceil(block_size));
        let row_pitch = pitch_blocks * bpb;
        let needed = (block_height - 1) * row_pitch + block_width * bpb;
        if data.len() < needed {
            return Err(bad("texture payload is shorter than its base mip"));
        }
        for y in 0..block_height {
            for x in 0..block_width {
                let source = y * row_pitch + x * bpb;
                let target = (y * block_width + x) * bpb;
                endian_swap(
                    &data[source..source + bpb],
                    compressed,
                    &mut linear[target..target + bpb],
                );
            }
        }
        return Ok(linear);
    }
    let tiled_width = align(width, if compressed { 128 } else { 32 }) / block_size;
    let log_bpb = bpb.ilog2();
    for y in 0..block_height {
        for x in 0..block_width {
            let source = tiled_offset(x, y, tiled_width, log_bpb) * bpb;
            let target = (y * block_width + x) * bpb;
            if source + bpb > data.len() {
                return Err(bad("tiled texture address exceeds its payload"));
            }
            endian_swap(
                &data[source..source + bpb],
                compressed,
                &mut linear[target..target + bpb],
            );
        }
    }
    Ok(linear)
}

fn expand_565(v: u16) -> [u32; 3] {
    let (r, g, b) = (
        (v >> 11) as u32 & 0x1F,
        (v >> 5) as u32 & 0x3F,
        v as u32 & 0x1F,
    );
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

fn color_table(c0: u16, c1: u16, allow_transparency: bool) -> [[u8; 4]; 4] {
    let (a, b) = (expand_565(c0), expand_565(c1));
    let rgb = |c: [u32; 3]| [c[0] as u8, c[1] as u8, c[2] as u8, 255];
    let mix = |f: &dyn Fn(u32, u32) -> u32| rgb([f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])]);
    if !allow_transparency || c0 > c1 {
        [
            rgb(a),
            rgb(b),
            mix(&|x, y| (2 * x + y) / 3),
            mix(&|x, y| (x + 2 * y) / 3),
        ]
    } else {
        [rgb(a), rgb(b), mix(&|x, y| (x + y) / 2), [0, 0, 0, 0]]
    }
}

pub fn decode_dxt(
    width: usize,
    height: usize,
    data: &[u8],
    format_code: u32,
) -> Result<Vec<u8>, Error> {
    let mut out = vec![0u8; width * height * 4];
    let block_bytes = if format_code == 18 { 8 } else { 16 };
    let blocks_wide = width.div_ceil(4).max(1);
    let blocks_high = height.div_ceil(4).max(1);
    let mut source = 0;
    for by in 0..blocks_high {
        for bx in 0..blocks_wide {
            if source + block_bytes > data.len() {
                return Err(bad("compressed texture base mip is truncated"));
            }
            let mut alphas: Option<[u8; 16]> = None;
            let mut color_offset = source;
            if format_code == 19 {
                let mut a = [0u8; 16];
                for (i, v) in data[source..source + 8].iter().enumerate() {
                    a[i * 2] = (v & 0xF) * 17;
                    a[i * 2 + 1] = (v >> 4) * 17;
                }
                alphas = Some(a);
                color_offset += 8;
            } else if format_code == 20 {
                let (a0, a1) = (data[source] as u32, data[source + 1] as u32);
                let mut palette = [a0, a1, 0, 0, 0, 0, 0, 0];
                if a0 > a1 {
                    for i in 1..7 {
                        palette[i as usize + 1] = ((7 - i) * a0 + i * a1) / 7;
                    }
                } else {
                    for i in 1..5 {
                        palette[i as usize + 1] = ((5 - i) * a0 + i * a1) / 5;
                    }
                    palette[6] = 0;
                    palette[7] = 255;
                }
                let mut bits = 0u64;
                for (i, b) in data[source + 2..source + 8].iter().enumerate() {
                    bits |= (*b as u64) << (8 * i);
                }
                let mut a = [0u8; 16];
                for (pixel, slot) in a.iter_mut().enumerate() {
                    *slot = palette[((bits >> (pixel * 3)) & 7) as usize] as u8;
                }
                alphas = Some(a);
                color_offset += 8;
            }
            let c = &data[color_offset..color_offset + 8];
            let c0 = u16::from_le_bytes([c[0], c[1]]);
            let c1 = u16::from_le_bytes([c[2], c[3]]);
            let color_bits = u32::from_le_bytes([c[4], c[5], c[6], c[7]]);
            let colors = color_table(c0, c1, format_code == 18);
            for py in 0..4 {
                for px in 0..4 {
                    let (x, y) = (bx * 4 + px, by * 4 + py);
                    if x >= width || y >= height {
                        continue;
                    }
                    let pixel = py * 4 + px;
                    let color = colors[((color_bits >> (pixel * 2)) & 3) as usize];
                    let alpha = alphas.map_or(color[3], |a| a[pixel]);
                    let o = (y * width + x) * 4;
                    out[o..o + 4].copy_from_slice(&[color[0], color[1], color[2], alpha]);
                }
            }
            source += block_bytes;
        }
    }
    Ok(out)
}

/// RGBA8 pixels of the base mip.
pub fn decode_rgba(data: &[u8], fetch: &Fetch) -> Result<Vec<u8>, Error> {
    let linear = linearize_base_level(data, fetch)?;
    let (width, height) = (fetch.width as usize, fetch.height as usize);
    match fetch.format_code {
        18..=20 => decode_dxt(width, height, &linear, fetch.format_code),
        6 => {
            let mut out = vec![0u8; width * height * 4];
            for (o, l) in out
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(linear.as_chunks::<4>().0)
            {
                o.copy_from_slice(&[l[2], l[1], l[0], l[3]]);
            }
            Ok(out)
        }
        _ => Err(bad("unsupported Xbox texture format")),
    }
}
