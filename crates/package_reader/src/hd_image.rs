//! Bounded HD texture samples. Pixels are stored-channel straight RGBA8, with
//! no color-profile transform or claim about installed shader interpretation.
use image::{ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use std::io::{self, Cursor, Write};

const MAX_INPUT: usize = 32 * 1024 * 1024;
const MAX_RGBA: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HdImageFormat {
    Png,
    DdsBc3,
}

/// Explicit CPU arithmetic; neither profile qualifies installed GPU behavior.
/// PNG always uses the pinned image codec, even when the BC3 override is chosen.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HdPixelDecodeProfile {
    #[default]
    #[serde(rename = "image_0_25_10_stored_rgba8")]
    Image02510StoredRgba8,
    #[serde(rename = "bc3_normalized_nearest_rgba8_v1")]
    Bc3NormalizedNearestRgba8V1,
}

pub struct DecodedHdImage {
    pub format: HdImageFormat,
    pub pixel_decode_profile: HdPixelDecodeProfile,
    pub width: u32,
    pub height: u32,
    pub encoded_length: usize,
    pub storage_padding: usize,
    pub pixels_rgba: Vec<u8>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn unsupported(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, message)
}
fn dimensions(width: u32, height: u32) -> io::Result<()> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(invalid(
            "HD image dimensions exceed 4096-pixel inspection bounds",
        ));
    }
    Ok(())
}

fn png_length(bytes: &[u8]) -> io::Result<usize> {
    let mut cursor = 8usize;
    let mut has_data = false;
    let mut data_ended = false;
    let mut has_palette = false;
    while cursor < bytes.len() {
        let header = bytes
            .get(cursor..cursor + 8)
            .ok_or_else(|| invalid("truncated PNG chunk"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = cursor
            .checked_add(12)
            .and_then(|v| v.checked_add(length))
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| invalid("PNG chunk exceeds source"))?;
        let kind = &header[4..8];
        if !kind.iter().all(u8::is_ascii_alphabetic) || !kind[2].is_ascii_uppercase() {
            return Err(invalid("invalid PNG chunk type"));
        }
        if crc32fast::hash(&bytes[cursor + 4..end - 4])
            != u32::from_be_bytes(bytes[end - 4..end].try_into().unwrap())
        {
            return Err(invalid("PNG chunk checksum mismatch"));
        }
        if cursor == 8 {
            if kind != b"IHDR" || length != 13 {
                return Err(invalid("PNG must start with a 13-byte IHDR"));
            }
            dimensions(
                u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
                u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
            )?;
            if bytes[24] == 16 {
                return Err(unsupported(
                    "16-bit PNG channels require a lossless higher-depth representation",
                ));
            }
        } else if kind == b"IHDR" {
            return Err(invalid("duplicate PNG IHDR"));
        }
        match kind {
            b"acTL" | b"fcTL" | b"fdAT" => {
                return Err(unsupported("animated PNG is not a static HD texture"));
            }
            b"IDAT" => {
                if data_ended {
                    return Err(invalid("non-contiguous PNG image data"));
                }
                has_data = true;
            }
            b"IEND" => {
                if length != 0 || !has_data || end != bytes.len() {
                    return Err(invalid("invalid PNG end or non-storage trailing data"));
                }
                return Ok(end);
            }
            b"IHDR" => {}
            b"PLTE" => {
                if has_palette
                    || has_data
                    || length == 0
                    || length > 768
                    || !length.is_multiple_of(3)
                {
                    return Err(invalid("invalid PNG palette length/order"));
                }
                has_palette = true;
            }
            _ if kind[0].is_ascii_uppercase() => {
                return Err(unsupported("unknown critical PNG chunk"));
            }
            _ => {}
        }
        if has_data && kind != b"IDAT" {
            data_ended = true;
        }
        cursor = end;
    }
    Err(invalid("PNG end chunk is missing"))
}

fn dds_length(bytes: &[u8]) -> io::Result<usize> {
    if bytes.len() < 128 {
        return Err(invalid("truncated DDS header"));
    }
    let value = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    if value(4) != 124 || value(76) != 32 || value(8) & 0x1007 != 0x1007 {
        return Err(invalid("invalid DDS header sizes/required flags"));
    }
    let (width, height) = (value(16), value(12));
    dimensions(width, height)?;
    if value(80) & 4 == 0
        || &bytes[84..88] != b"DXT5"
        || value(112) != 0
        || value(24) > 1
        || value(28) > 1
        || value(108) & 0x1000 == 0
    {
        return Err(unsupported(
            "HD inspection currently admits only single-level 2D legacy BC3 DDS",
        ));
    }
    if !width.is_multiple_of(4) || !height.is_multiple_of(4) {
        return Err(unsupported(
            "the pinned BC3 decoder requires complete 4x4 dimensions",
        ));
    }
    let expected = 128 + width as usize / 4 * (height as usize / 4) * 16;
    if bytes.len() != expected {
        return Err(invalid(
            "DDS base-level length differs or extra surfaces are present",
        ));
    }
    Ok(expected)
}

/// `declared_length` comes from validated replacement metadata. Uncompressed
/// alignment bytes remain an opaque source span; their values are not guessed.
pub fn decode_hd_image(bytes: &[u8], declared_length: usize) -> io::Result<DecodedHdImage> {
    decode_hd_image_with_profile(bytes, declared_length, HdPixelDecodeProfile::default())
}

/// Common admission bounds precede either CPU decoder. The override applies
/// only to BC3; the result records the decoder actually used for each image.
pub fn decode_hd_image_with_profile(
    bytes: &[u8],
    declared_length: usize,
    dds_profile: HdPixelDecodeProfile,
) -> io::Result<DecodedHdImage> {
    if bytes.len() > MAX_INPUT {
        return Err(invalid("HD image exceeds 32 MiB input limit"));
    }
    if declared_length == 0
        || declared_length > bytes.len()
        || bytes.len() - declared_length > 15
        || (bytes.len() != declared_length && bytes.len() != declared_length.next_multiple_of(16))
    {
        return Err(invalid(
            "HD image logical length/padded storage range is inconsistent",
        ));
    }
    let storage_padding = bytes.len() - declared_length;
    let bytes = &bytes[..declared_length];
    let (format, codec, encoded_length) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        (HdImageFormat::Png, ImageFormat::Png, png_length(bytes)?)
    } else if bytes.starts_with(b"DDS ") {
        (HdImageFormat::DdsBc3, ImageFormat::Dds, dds_length(bytes)?)
    } else {
        return Err(unsupported("unknown HD image signature"));
    };
    if format == HdImageFormat::DdsBc3
        && dds_profile == HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1
    {
        let width = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        return Ok(DecodedHdImage {
            format,
            pixel_decode_profile: dds_profile,
            width,
            height,
            encoded_length,
            storage_padding,
            pixels_rgba: normalized_bc3(bytes, width as usize, height as usize),
        });
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(MAX_RGBA);
    let mut reader = ImageReader::with_format(Cursor::new(&bytes[..encoded_length]), codec);
    reader.limits(limits);
    let pixels = reader
        .decode()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .into_rgba8();
    let (width, height) = pixels.dimensions();
    Ok(DecodedHdImage {
        format,
        pixel_decode_profile: HdPixelDecodeProfile::Image02510StoredRgba8,
        width,
        height,
        encoded_length,
        storage_padding,
        pixels_rgba: pixels.into_raw(),
    })
}

// Public BC3 layout/normalized interpolation, followed by nearest UNORM8.
// Qualification: T024 pinned DirectXTex methods + WIC, finite scalar corpus.
// Called only after strict single-level DDS admission above, preserving hidden
// RGB and straight alpha. No gamma transform, premultiplication or filtering.
fn normalized_bc3(bytes: &[u8], width: usize, height: usize) -> Vec<u8> {
    let nearest = |value: u32, divisor: u32| ((value + divisor / 2) / divisor) as u8;
    let mut rgba = vec![0; width * height * 4];
    for (block_index, block) in bytes[128..].as_chunks::<16>().0.iter().enumerate() {
        let first = u16::from_le_bytes(block[8..10].try_into().unwrap()) as u32;
        let second = u16::from_le_bytes(block[10..12].try_into().unwrap()) as u32;
        let mut colors = [[0u8; 3]; 4];
        for (channel, (mask, shift)) in [(31, 11), (63, 5), (31, 0)].into_iter().enumerate() {
            let (a, b) = ((first >> shift) & mask, (second >> shift) & mask);
            for (selector, (n, m, divisor)) in [(1, 0, 1), (0, 1, 1), (2, 1, 3), (1, 2, 3)]
                .into_iter()
                .enumerate()
            {
                colors[selector][channel] = nearest((n * a + m * b) * 255, divisor * mask);
            }
        }
        let mut alphas = [0u8; 8];
        alphas[..2].copy_from_slice(&block[..2]);
        let (divisor, end) = if block[0] > block[1] { (7, 8) } else { (5, 6) };
        for (selector, alpha) in alphas.iter_mut().enumerate().take(end).skip(2) {
            *alpha = nearest(
                (divisor + 1 - selector as u32) * block[0] as u32
                    + (selector as u32 - 1) * block[1] as u32,
                divisor,
            );
        }
        if end == 6 {
            alphas[7] = 255;
        }
        let colors_index = u32::from_le_bytes(block[12..16].try_into().unwrap());
        let mut alpha_bytes = [0u8; 8];
        alpha_bytes[..6].copy_from_slice(&block[2..8]);
        let alphas_index = u64::from_le_bytes(alpha_bytes);
        for p in 0..16 {
            let x = (block_index % (width / 4)) * 4 + p % 4;
            let y = (block_index / (width / 4)) * 4 + p / 4;
            let position = (y * width + x) * 4;
            rgba[position..position + 3]
                .copy_from_slice(&colors[((colors_index >> (p * 2)) & 3) as usize]);
            rgba[position + 3] = alphas[((alphas_index >> (p * 3)) & 7) as usize];
        }
    }
    rgba
}

impl DecodedHdImage {
    pub fn write_preview(&self, writer: &mut impl Write) -> io::Result<()> {
        crate::font::write_rgba_preview(
            writer,
            self.width as usize,
            self.height as usize,
            &self.pixels_rgba,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

    fn decode(bytes: &[u8]) -> io::Result<DecodedHdImage> {
        decode_hd_image(bytes, bytes.len())
    }

    fn png() -> Vec<u8> {
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(
                &[1, 2, 3, 0, 200, 100, 50, 128],
                2,
                1,
                ExtendedColorType::Rgba8,
            )
            .unwrap();
        bytes
    }
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = (data.len() as u32).to_be_bytes().to_vec();
        bytes.extend(kind);
        bytes.extend(data);
        bytes.extend(crc32fast::hash(&bytes[4..]).to_be_bytes());
        bytes
    }
    fn dds() -> Vec<u8> {
        let mut bytes = vec![0; 144];
        bytes[..4].copy_from_slice(b"DDS ");
        for (offset, value) in [
            (4, 124u32),
            (8, 0x81007),
            (12, 4),
            (16, 4),
            (20, 16),
            (28, 1),
            (76, 32),
            (80, 4),
            (108, 0x1000),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[84..88].copy_from_slice(b"DXT5");
        bytes[128] = 255;
        bytes[136..138].copy_from_slice(&0xf800u16.to_le_bytes());
        bytes
    }

    #[test]
    fn png_preserves_straight_samples_and_separates_storage_padding() {
        let mut bytes = png();
        let encoded = bytes.len();
        bytes.resize(encoded.next_multiple_of(16), 0xab);
        let decoded = decode_hd_image(&bytes, encoded).unwrap();
        assert_eq!(decoded.format, HdImageFormat::Png);
        assert_eq!(
            (
                decoded.width,
                decoded.height,
                decoded.encoded_length,
                decoded.storage_padding
            ),
            (2, 1, encoded, bytes.len() - encoded)
        );
        assert_eq!(decoded.pixels_rgba, [1, 2, 3, 0, 200, 100, 50, 128]);
        assert!(decode_hd_image(&bytes, bytes.len() + 1).is_err());
        bytes.extend([0; 16]);
        assert!(decode_hd_image(&bytes, encoded).is_err());
    }

    #[test]
    fn png_rejects_crc_truncation_animation_unknown_critical_and_oversized_headers() {
        let source = png();
        for prefix in [0, 7, 15, 32, source.len() - 1] {
            assert!(decode(&source[..prefix]).is_err());
        }
        let mut corrupt = source.clone();
        corrupt[29] ^= 1;
        assert!(decode(&corrupt).is_err());
        for kind in [b"acTL", b"ABCD"] {
            let mut bytes = source[..33].to_vec();
            bytes.extend(chunk(kind, &[0; 8]));
            bytes.extend(&source[33..]);
            assert_eq!(
                decode(&bytes).err().unwrap().kind(),
                io::ErrorKind::Unsupported
            );
        }
        let mut oversized = source.clone();
        oversized[16..20].copy_from_slice(&4097u32.to_be_bytes());
        let crc = crc32fast::hash(&oversized[12..29]);
        oversized[29..33].copy_from_slice(&crc.to_be_bytes());
        assert!(decode(&oversized).is_err());
        let mut high_depth = source;
        high_depth[24] = 16;
        let crc = crc32fast::hash(&high_depth[12..29]);
        high_depth[29..33].copy_from_slice(&crc.to_be_bytes());
        assert_eq!(
            decode(&high_depth).err().unwrap().kind(),
            io::ErrorKind::Unsupported
        );
    }

    #[test]
    fn bc3_block_and_container_admission_are_separate() {
        let source = dds();
        let decoded = decode(&source).unwrap();
        assert_eq!(
            (decoded.width, decoded.height, decoded.storage_padding),
            (4, 4, 0)
        );
        assert_eq!(decoded.pixels_rgba, [255, 0, 0, 255].repeat(16));
        assert!(decode(&source[..143]).is_err());
        let mut tail = source.clone();
        tail.push(0);
        assert!(decode(&tail).is_err());
        for (offset, value) in [
            (4, 123u32),
            (12, 0),
            (16, u32::MAX),
            (28, 2),
            (112, 0x200),
            (16, 3),
        ] {
            let mut bytes = source.clone();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&bytes).is_err());
        }
        let mut unknown = source;
        unknown[84..88].copy_from_slice(b"DX10");
        assert_eq!(
            decode(&unknown).err().unwrap().kind(),
            io::ErrorKind::Unsupported
        );
    }

    #[test]
    fn explicit_bc3_profile_distinguishes_endpoint_and_alpha_quantization() {
        let mut bytes = dds();
        bytes[128] = 1;
        bytes[129] = 0;
        let mut indices = 0u64;
        for p in 0..16 {
            indices |= (p as u64 % 8) << (p * 3);
        }
        bytes[130..136].copy_from_slice(&indices.to_le_bytes()[..6]);
        bytes[136..138].copy_from_slice(&(3u16 << 11).to_le_bytes());
        bytes[138..140].copy_from_slice(&(3u16 << 11).to_le_bytes());
        let legacy = decode(&bytes).unwrap();
        let normalized = decode_hd_image_with_profile(
            &bytes,
            bytes.len(),
            HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1,
        )
        .unwrap();
        assert_eq!(&legacy.pixels_rgba[8..12], &[24, 0, 0, 0]);
        assert_eq!(&normalized.pixels_rgba[8..12], &[25, 0, 0, 1]);
        assert_eq!(
            legacy.pixel_decode_profile,
            HdPixelDecodeProfile::Image02510StoredRgba8
        );
        assert_eq!(
            normalized.pixel_decode_profile,
            HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1
        );
    }

    #[test]
    fn normalized_bc3_preserves_four_colors_hidden_rgb_and_equal_alpha_branch() {
        let mut bytes = dds();
        bytes[128..144].copy_from_slice(&[
            0, 0, 0x88, 0xc6, 0xfa, 0x88, 0xc6, 0xfa, 0, 0, 0, 0xf8, 0xe4, 0xe4, 0xe4, 0xe4,
        ]);
        let normalized = decode_hd_image_with_profile(
            &bytes,
            bytes.len(),
            HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1,
        )
        .unwrap();
        for (p, pixel) in normalized.pixels_rgba.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                *pixel,
                [
                    [0, 0, 0, 0],
                    [255, 0, 0, 0],
                    [85, 0, 0, 0],
                    [170, 0, 0, 0],
                    [0, 0, 0, 0],
                    [255, 0, 0, 0],
                    [85, 0, 0, 0],
                    [170, 0, 0, 255]
                ][p % 8]
            );
        }
    }

    #[test]
    fn profiles_share_admission_and_png_always_reports_its_actual_decoder() {
        let profile = HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1;
        let mut bytes = png();
        let logical = bytes.len();
        bytes.resize(logical.next_multiple_of(16), 0xab);
        let legacy = decode_hd_image(&bytes, logical).unwrap();
        let overridden = decode_hd_image_with_profile(&bytes, logical, profile).unwrap();
        assert_eq!(legacy.pixels_rgba, overridden.pixels_rgba);
        assert_eq!(
            overridden.pixel_decode_profile,
            HdPixelDecodeProfile::default()
        );
        let mut invalid_dds = dds();
        invalid_dds[28..32].copy_from_slice(&2u32.to_le_bytes());
        for bytes in [&invalid_dds[..], &invalid_dds[..143], b"unknown".as_slice()] {
            let legacy = decode_hd_image(bytes, bytes.len()).err().unwrap();
            let normalized = decode_hd_image_with_profile(bytes, bytes.len(), profile)
                .err()
                .unwrap();
            assert_eq!(legacy.kind(), normalized.kind());
        }
        assert!(serde_json::from_str::<HdPixelDecodeProfile>("\"unqualified_gpu\"").is_err());
    }
}
