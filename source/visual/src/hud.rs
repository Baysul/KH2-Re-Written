//! Source-bound HUD sheets: a SEQD sequence and the HD replacement of its
//! IMGD, e.g. the gauge from `field2d/<lang>/<world>0field.2dd` or a
//! character face from `obj/<model>.a.<lang>`.
use data::{bar, hd_image, image_structure, original, probe::ProbeSource, replacement, sequence};
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

/// System font: the HD replacement of fontimage.bar's 'sys' plane and the
/// 'sys' width table from fontinfo.bar (one byte per glyph).
pub struct SystemFont {
    pub rgba: Vec<u8>,
    pub size: [u32; 2],
    /// HD texels per original font texel (original plane 512 wide).
    pub texel_scale: u32,
    pub widths: Vec<u8>,
}

fn read_entry(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    replacements: bool,
) -> Result<(Vec<u8>, Vec<replacement::ReplacementSample>), Box<dyn std::error::Error>> {
    let mut checked = source.open_index(index)?;
    let entry = checked
        .inventory
        .entries
        .get(ordinal)
        .filter(|e| e.name_hash == name_hash)
        .ok_or("archive entry mismatch")?;
    let archive = original::read_original(&mut checked.reader, entry)?.bytes;
    let hd = if replacements {
        replacement::read_all_replacements(&mut checked.reader, entry)?
    } else {
        Vec::new()
    };
    checked.verify_unchanged()?;
    Ok((archive, hd))
}

fn named<'a>(
    archive: &'a [u8],
    kind: u16,
    name: &[u8; 4],
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let table = bar::read_bar(archive)?;
    let e = table
        .entries
        .iter()
        .find(|e| e.entry_type == kind && &e.name_bytes == name)
        .ok_or("BAR entry absent")?;
    archive
        .get(e.offset as usize..e.offset as usize + e.size as usize)
        .ok_or_else(|| "BAR entry exceeds archive".into())
}

/// `image` = msg/<lang>/fontimage.bar, `info` = msg/<lang>/fontinfo.bar,
/// as (index, ordinal, name hash). The font planes (type 36) are replaced
/// in archive order; 'sys' is the first.
pub fn load_system_font(
    source: &ProbeSource,
    image: (&str, usize, [u8; 16]),
    info: (&str, usize, [u8; 16]),
) -> Result<SystemFont, Box<dyn std::error::Error>> {
    load_font(source, image, info, b"sys\0")
}

/// Any font plane of fontimage.bar (e.g. 'sys', 'evt') with its width
/// table of the same name in fontinfo.bar.
pub fn load_font(
    source: &ProbeSource,
    image: (&str, usize, [u8; 16]),
    info: (&str, usize, [u8; 16]),
    plane: &[u8; 4],
) -> Result<SystemFont, Box<dyn std::error::Error>> {
    let (images, hd) = read_entry(source, image.0, image.1, image.2, true)?;
    let planes: Vec<_> = bar::read_bar(&images)?
        .entries
        .into_iter()
        .filter(|e| e.entry_type == 36)
        .collect();
    let slot = planes
        .iter()
        .position(|e| &e.name_bytes == plane)
        .ok_or("font plane absent")?;
    if hd.len() != planes.len() {
        return Err("font HD replacements do not match the planes".into());
    }
    let decoded = hd_image::decode_hd_image(
        &hd[slot].bytes,
        hd[slot].metadata.declared_decompressed_length as usize,
    )?;
    let scale = decoded.width / 512;
    if scale == 0 || decoded.width != 512 * scale {
        return Err("system font HD plane is not an integer upscale".into());
    }
    let (infos, _) = read_entry(source, info.0, info.1, info.2, false)?;
    let widths = named(&infos, 2, plane)?.to_vec();
    Ok(SystemFont {
        size: [decoded.width, decoded.height],
        rgba: decoded.pixels_rgba,
        texel_scale: scale,
        widths,
    })
}

/// The message window art: the first SEQD of msg/<lang>/fontinfo.bar
/// ('md_m', FUN_1403e2230) and the 'md_m' IMGD of the world message bar
/// (FUN_1403adc10), whose IMGD replacements come last in IMGD order.
pub fn load_window_sheet(
    source: &ProbeSource,
    info: (&str, usize, [u8; 16]),
    world: (&str, usize, [u8; 16]),
) -> Result<Sheet, Box<dyn std::error::Error>> {
    let (infos, _) = read_entry(source, info.0, info.1, info.2, false)?;
    let table = bar::read_bar(&infos)?;
    let e = table
        .entries
        .iter()
        .find(|e| e.entry_type == SEQD)
        .ok_or("window SEQD absent")?;
    let sequence = sequence::read(
        infos
            .get(e.offset as usize..e.offset as usize + e.size as usize)
            .ok_or("window SEQD exceeds archive")?,
    )?;
    let (archive, hd) = read_entry(source, world.0, world.1, world.2, true)?;
    let table = bar::read_bar(&archive)?;
    let images: Vec<_> = table
        .entries
        .iter()
        .filter(|e| e.entry_type == IMGD)
        .collect();
    let slot = images
        .iter()
        .position(|e| &e.name_bytes == b"md_m" && e.size > 0)
        .ok_or("window IMGD absent")?;
    let image = images[slot];
    let header = image_structure::read_imgd_header(
        archive
            .get(image.offset as usize..image.offset as usize + image.size as usize)
            .ok_or("window IMGD exceeds archive")?,
    )?;
    let first = hd
        .len()
        .checked_sub(images.len())
        .ok_or("fewer HD replacements than IMGDs")?;
    let r = &hd[first + slot];
    let decoded =
        hd_image::decode_hd_image(&r.bytes, r.metadata.declared_decompressed_length as usize)?;
    let (w, h) = (header.width as u32, header.height as u32);
    let scale = decoded.width / w;
    if scale == 0 || decoded.width != w * scale || decoded.height != h * scale {
        return Err("window HD image is not an integer upscale".into());
    }
    Ok(Sheet {
        sequence,
        texture_size: [decoded.width, decoded.height],
        texture_rgba: decoded.pixels_rgba,
        texel_scale: scale,
    })
}

/// 03system 'ftst' text colour: after {u32 1, u32 count}, 0x50-byte records
/// {u32 id, u32 colour[19 worlds]} (FUN_1403ea860); PS2 0xAABBGGRR.
pub fn ftst_colour(
    system: &[u8],
    id: u32,
    world: usize,
) -> Result<u32, Box<dyn std::error::Error>> {
    let table = bar::read_bar(system)?;
    let e = table
        .entries
        .iter()
        .find(|e| &e.name_bytes == b"ftst")
        .ok_or("ftst absent")?;
    let d = system
        .get(e.offset as usize..e.offset as usize + e.size as usize)
        .ok_or("ftst exceeds archive")?;
    let word = |at: usize| {
        d.get(at..at + 4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
    };
    // Header: u32 version (1), u32 record count.
    let count = word(4).ok_or("ftst header")? as usize;
    (0..count)
        .find(|&k| word(8 + k * 0x50) == Some(id))
        .and_then(|k| word(8 + k * 0x50 + 4 + world * 4))
        .ok_or_else(|| "ftst colour absent".into())
}

/// Encoded strings of a message archive (e.g. msg/<lang>/sys.bar) by id &
/// 0x7fff, up to the terminating zero.
/// Messages by id from a message archive; `None` for an absent id.
pub fn load_messages_optional(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    ids: &[u32],
) -> Result<Vec<Option<Vec<u8>>>, Box<dyn std::error::Error>> {
    let (archive, _) = read_entry(source, index, ordinal, name_hash, false)?;
    let table = bar::read_bar(&archive)?;
    let e = table
        .entries
        .iter()
        .find(|e| e.entry_type == 2)
        .ok_or("message table absent")?;
    let d = archive
        .get(e.offset as usize..e.offset as usize + e.size as usize)
        .ok_or("message table exceeds archive")?;
    let word = |at: usize| -> Result<u32, Box<dyn std::error::Error>> {
        Ok(u32::from_le_bytes(
            d.get(at..at + 4)
                .ok_or("message table truncated")?
                .try_into()?,
        ))
    };
    if word(0)? != 1 {
        return Err("message table header".into());
    }
    let count = word(4)? as usize;
    ids.iter()
        .map(|&id| {
            for k in 0..count {
                if word(8 + k * 8)? & 0x7fff == id & 0x7fff {
                    let start = word(12 + k * 8)? as usize;
                    let rest = d.get(start..).ok_or("message offset outside table")?;
                    // The terminator is a 0 in code position: a code's
                    // parameters (e.g. 0x07's colour) may hold zeros.
                    let mut end = 0;
                    loop {
                        match rest.get(end) {
                            None => return Err("unterminated message".into()),
                            Some(0) => break,
                            Some(&b) if b < 0x20 => end += crate::dialogue::code_length(b),
                            Some(_) => end += 1,
                        }
                    }
                    return Ok(Some(rest[..end.min(rest.len())].to_vec()));
                }
            }
            Ok(None)
        })
        .collect()
}

pub fn load_messages(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    ids: &[u32],
) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    load_messages_optional(source, index, ordinal, name_hash, ids)?
        .into_iter()
        .zip(ids)
        .map(|(m, id)| m.ok_or_else(|| format!("message {id:#x} absent").into()))
        .collect()
}
