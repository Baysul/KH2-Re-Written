//! Source-bound HUD sheets: a SEQD sequence and the HD replacement of its
//! IMGD, e.g. the gauge from `field2d/<lang>/<world>0field.2dd` or a
//! character face from `obj/<model>.a.<lang>`.
use package_reader::{
    bar, hd_image, image_structure, original, probe::ProbeSource, replacement, sequence,
};
use std::io;

pub struct Sheet {
    pub sequence: sequence::Sequence,
    /// Straight RGBA of the HD replacement for the IMGD.
    pub texture_rgba: Vec<u8>,
    pub texture_size: [u32; 2],
    /// HD texels per original texel.
    pub texel_scale: u32,
}

const IMGD: u16 = 24;
const SEQD: u16 = 25;

/// Reads the named sequence and its IMGD from the archive at
/// (`index`, `ordinal`, `name_hash`). The archive's IMGD replacements are
/// the last ones, in IMGD order (other textures, such as effect TIM2s, come
/// first); the matching one must be an integer upscale.
pub fn load(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    name: &[u8; 4],
) -> Result<Sheet, Box<dyn std::error::Error>> {
    let invalid = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_owned());
    let mut checked = source.open_index(index)?;
    let entry = checked
        .inventory
        .entries
        .get(ordinal)
        .filter(|e| e.name_hash == name_hash)
        .ok_or("HUD archive entry mismatch")?;
    let archive = original::read_original(&mut checked.reader, entry)?.bytes;
    let replacements = replacement::read_all_replacements(&mut checked.reader, entry)?;
    checked.verify_unchanged()?;
    let table = bar::read_bar(&archive)?;
    let slice = |e: &bar::BarEntry| -> io::Result<&[u8]> {
        archive
            .get(e.offset as usize..e.offset as usize + e.size as usize)
            .ok_or_else(|| invalid("BAR entry exceeds archive"))
    };
    let images: Vec<_> = table
        .entries
        .iter()
        .filter(|e| e.entry_type == IMGD)
        .collect();
    let image_slot = images
        .iter()
        .position(|e| &e.name_bytes == name)
        .ok_or("HUD IMGD absent")?;
    let sequences: Vec<_> = table
        .entries
        .iter()
        .filter(|e| e.entry_type == SEQD && &e.name_bytes == name)
        .collect();
    let [seq_entry] = sequences.as_slice() else {
        return Err("expected one HUD sequence".into());
    };
    let sequence = sequence::read(slice(seq_entry)?)?;
    let header = image_structure::read_imgd_header(slice(images[image_slot])?)?;
    let first_image = replacements
        .len()
        .checked_sub(images.len())
        .ok_or("fewer HD replacements than IMGDs")?;
    let hd = &replacements[first_image + image_slot];
    let decoded =
        hd_image::decode_hd_image(&hd.bytes, hd.metadata.declared_decompressed_length as usize)?;
    let (w, h) = (header.width as u32, header.height as u32);
    let scale = decoded.width / w;
    if scale == 0 || decoded.width != w * scale || decoded.height != h * scale {
        return Err("HD replacement is not an integer upscale of the IMGD".into());
    }
    Ok(Sheet {
        sequence,
        texture_size: [decoded.width, decoded.height],
        texture_rgba: decoded.pixels_rgba,
        texel_scale: scale,
    })
}

/// Sora's starting HP and MP from the battle archive's PLRP table (public
/// OpenKH 00battle layout: 128-byte entries after an 8-byte header; id u16,
/// character u8, HP u8, MP u8). Entry id 0 of character 1 (Sora).
pub fn starting_stats(plrp: &[u8]) -> Result<(i32, i32), Box<dyn std::error::Error>> {
    let count = u32::from_le_bytes(plrp.get(4..8).ok_or("PLRP header")?.try_into()?) as usize;
    if plrp.len() < 8 + count * 128 {
        return Err("PLRP table truncated".into());
    }
    (0..count)
        .map(|i| &plrp[8 + i * 128..8 + i * 128 + 5])
        .find(|e| u16::from_le_bytes([e[0], e[1]]) == 0 && e[2] == 1)
        .map(|e| (e[3] as i32, e[4] as i32))
        .ok_or_else(|| "PLRP entry for Sora absent".into())
}
