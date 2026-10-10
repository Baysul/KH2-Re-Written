//! Source-bound 2LD menus (`menu/<lang>/*.2ld`): a BAR with an IMGZ (type
//! 29) of IMGDs and a LAYD (type 28), plus the PKG entry's HD replacements
//! `<LANG>_<name>_2ld<N>.png`. N below the IMGD count replaces that IMGD;
//! higher N are alternates the exe swaps in (e.g. title image 8's device
//! glyph variants, FUN_14012eb70).
use package_reader::{
    bar, hd_image, image_structure, layout, original,
    probe::ProbeSource,
    replacement,
    sequence::{self, Modifiers},
};

pub const IMGZ: u16 = 29;
pub const LAYD: u16 = 28;
pub const IMGD: u16 = 24;

/// An HD image and its texels per original sequence texel.
pub struct Texture {
    pub rgba: Vec<u8>,
    pub size: [u32; 2],
    pub texel_scale: f32,
}

pub struct Menu2ld {
    pub layout: layout::Layout,
    /// Original IMGD sizes, by image index.
    pub image_sizes: Vec<[u32; 2]>,
    /// Every replacement by its number N (`..._2ld<N>.png`).
    pub replacements: Vec<Option<Texture>>,
}

impl Menu2ld {
    /// The replacement N drawn as image `image` (scaled to that IMGD).
    pub fn texture_for(&self, image: usize, n: usize) -> Option<(&Texture, f32)> {
        let t = self.replacements.get(n)?.as_ref()?;
        let w = self.image_sizes.get(image)?[0];
        Some((t, t.size[0] as f32 / w as f32))
    }
}

/// Number N of a `..._2ld<N>.png` (or `.dds`) replacement name.
fn replacement_number(name: &[u8; 32]) -> Option<usize> {
    let end = name.iter().position(|&b| b == 0).unwrap_or(32);
    let name = std::str::from_utf8(&name[..end]).ok()?;
    let stem = name
        .strip_suffix(".png")
        .or_else(|| name.strip_suffix(".dds"))?;
    let digits = stem.rsplit_once("_2ld")?.1;
    digits.parse().ok()
}

/// A 2LD of bare IMGDs (e.g. `menu/jm_world.2ld`, `menu/jm_top_<code>.2ld`):
/// each IMGD's HD replacement, the last replacements in IMGD order.
pub fn load_images(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
) -> Result<Vec<Texture>, Box<dyn std::error::Error>> {
    let mut checked = source.open_index(index)?;
    let entry = checked
        .inventory
        .entries
        .get(ordinal)
        .filter(|e| e.name_hash == name_hash)
        .ok_or("2LD archive entry mismatch")?;
    let archive = original::read_original(&mut checked.reader, entry)?.bytes;
    let hd = replacement::read_all_replacements(&mut checked.reader, entry)?;
    checked.verify_unchanged()?;
    let table = bar::read_bar(&archive)?;
    let images: Vec<_> = table
        .entries
        .iter()
        .filter(|e| e.entry_type == IMGD)
        .collect();
    let first = hd
        .len()
        .checked_sub(images.len())
        .ok_or("fewer HD replacements than IMGDs")?;
    images
        .iter()
        .zip(&hd[first..])
        .map(|(e, r)| {
            let imgd = archive
                .get(e.offset as usize..e.offset as usize + e.size as usize)
                .ok_or("2LD entry exceeds archive")?;
            let h = image_structure::read_imgd_header(imgd)?;
            let (w, h) = (h.width as u32, h.height as u32);
            let d = hd_image::decode_hd_image(
                &r.bytes,
                r.metadata.declared_decompressed_length as usize,
            )?;
            let scale = d.width / w;
            if scale == 0 || d.width != w * scale || d.height != h * scale {
                return Err("2LD HD image is not an integer upscale".into());
            }
            Ok(Texture {
                size: [d.width, d.height],
                rgba: d.pixels_rgba,
                texel_scale: scale as f32,
            })
        })
        .collect()
}

pub fn load(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
) -> Result<Menu2ld, Box<dyn std::error::Error>> {
    let mut checked = source.open_index(index)?;
    let entry = checked
        .inventory
        .entries
        .get(ordinal)
        .filter(|e| e.name_hash == name_hash)
        .ok_or("2LD archive entry mismatch")?;
    let archive = original::read_original(&mut checked.reader, entry)?.bytes;
    let hd = replacement::read_all_replacements(&mut checked.reader, entry)?;
    checked.verify_unchanged()?;
    let table = bar::read_bar(&archive)?;
    let slice = |kind: u16| -> Result<&[u8], Box<dyn std::error::Error>> {
        let e = table
            .entries
            .iter()
            .find(|e| e.entry_type == kind)
            .ok_or("2LD entry absent")?;
        archive
            .get(e.offset as usize..e.offset as usize + e.size as usize)
            .ok_or_else(|| "2LD entry exceeds archive".into())
    };
    let layout = layout::read_layd(slice(LAYD)?)?;
    let image_sizes = layout::read_imgz(slice(IMGZ)?)?
        .into_iter()
        .map(|imgd| {
            image_structure::read_imgd_header(imgd).map(|h| [h.width as u32, h.height as u32])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut replacements: Vec<Option<Texture>> = Vec::new();
    for r in &hd {
        let Some(n) = replacement_number(&r.metadata.name_bytes) else {
            continue;
        };
        let decoded =
            hd_image::decode_hd_image(&r.bytes, r.metadata.declared_decompressed_length as usize)?;
        if replacements.len() <= n {
            replacements.resize_with(n + 1, || None);
        }
        replacements[n] = Some(Texture {
            size: [decoded.width, decoded.height],
            rgba: decoded.pixels_rgba,
            texel_scale: 1.,
        });
    }
    for (k, size) in image_sizes.iter().enumerate() {
        let t = replacements
            .get_mut(k)
            .and_then(Option::as_mut)
            .ok_or("2LD image without HD replacement")?;
        let scale = t.size[0] / size[0];
        if scale == 0 || t.size != [size[0] * scale, size[1] * scale] {
            return Err("2LD HD image is not an integer upscale".into());
        }
        t.texel_scale = scale as f32;
    }
    Ok(Menu2ld {
        layout,
        image_sizes,
        replacements,
    })
}

/// One run of quads on one image (FUN_1401948c0 binds an image per prop).
#[derive(Clone, Debug, PartialEq)]
pub struct Batch {
    pub image: usize,
    pub quads: Vec<sequence::Quad>,
}

/// A layout task's draw (FUN_1401948c0): each shown prop's sequence frame at
/// the prop position plus `offset`, in layout units.
pub fn draw(menu: &Menu2ld, task: &layout::LayoutTask, offset: [i32; 2]) -> Vec<Batch> {
    if task.dead {
        return Vec::new();
    }
    let l = &menu.layout;
    task.player
        .visible(l, offset)
        .map(|(prop, player, origin)| Batch {
            image: prop.texture,
            quads: l.sequences[prop.sequence].quads(player, origin, &Modifiers::default()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_numbers() {
        let name = |s: &str| {
            let mut n = [0u8; 32];
            n[..s.len()].copy_from_slice(s.as_bytes());
            n
        };
        assert_eq!(replacement_number(&name("US_title_2ld0.png")), Some(0));
        assert_eq!(replacement_number(&name("US_title_2ld11.png")), Some(11));
        assert_eq!(replacement_number(&name("US_camp_2ld3.png")), Some(3));
        assert_eq!(replacement_number(&name("US_jmemo_2ld0.dds")), Some(0));
        assert_eq!(replacement_number(&name("title.dds")), None);
    }
}
