//! Owned numerical diagnostics, not an installed-rendering compatibility profile.
//! Generate authored BC3 bytes, then compare explicit arithmetic hypotheses with
//! independent RGBA evidence. All outputs are new files outside source and Git.
use package_reader::hd_image::{HdImageFormat, decode_hd_image};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
fn bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    if fs::metadata(path)?.len() > limit {
        return Err(io::Error::other("diagnostic input exceeds bounds"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::other("diagnostic input grew beyond bounds"));
    }
    Ok(bytes)
}
fn block(a: u8, b: u8, c: u16, d: u16) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[0] = a;
    bytes[1] = b;
    let mut alpha = 0u64;
    let mut color = 0u32;
    for p in 0..16 {
        alpha |= (p as u64 % 8) << (p * 3);
        color |= (p as u32 % 4) << (p * 2);
    }
    bytes[2..8].copy_from_slice(&alpha.to_le_bytes()[..6]);
    bytes[8..10].copy_from_slice(&c.to_le_bytes());
    bytes[10..12].copy_from_slice(&d.to_le_bytes());
    bytes[12..16].copy_from_slice(&color.to_le_bytes());
    bytes
}
fn generate(root: &Path, directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let outputs: Vec<_> = ["boundaries.dds", "image-rs.rgba", "corpus.json"]
        .iter()
        .map(|name| source_identity::safe_output(root, &directory.join(name)))
        .collect::<io::Result<_>>()?;
    let (width, height) = (2048u32, 1072u32);
    let mut bytes = vec![0u8; 128];
    bytes[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [
        (4, 124),
        (8, 0x1007),
        (12, height),
        (16, width),
        (24, 1),
        (76, 32),
        (80, 4),
        (108, 0x1000),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[84..88].copy_from_slice(b"DXT5");
    for endpoint in 0..=u16::MAX {
        bytes.extend(block(255, 255, endpoint, endpoint));
    }
    for a in 0..=u8::MAX {
        for b in 0..=u8::MAX {
            bytes.extend(block(a, b, 0xf800, 0xf800));
        }
    }
    for (count, shift) in [(32u16, 11), (64, 5), (32, 0)] {
        for c in 0..count {
            for d in 0..count {
                bytes.extend(block(255, 255, c << shift, d << shift));
            }
        }
    }
    let decoded = decode_hd_image(&bytes, bytes.len())?;
    let mut rgba = b"RGBA".to_vec();
    rgba.extend(width.to_le_bytes());
    rgba.extend(height.to_le_bytes());
    rgba.extend(&decoded.pixels_rgba);
    let report = json!({"schema_version":1, "fixture":"authored_bc3_scalar_boundaries_v1",
        "width":width, "height":height, "blocks":(bytes.len()-128)/16,
        "pixels":width as u64*height as u64,
        "sections":[{"kind":"rgb565_equal_endpoints","first_block":0,"blocks":65536},
            {"kind":"alpha_endpoint_pairs","first_block":65536,"blocks":65536},
            {"kind":"red_endpoint_pairs","first_block":131072,"blocks":1024},
            {"kind":"green_endpoint_pairs","first_block":132096,"blocks":4096},
            {"kind":"blue_endpoint_pairs","first_block":136192,"blocks":1024}],
        "color_selector":"pixel_modulo_4", "alpha_selector":"pixel_modulo_8",
        "dds_sha256":format!("{:x}",Sha256::digest(&bytes)),
        "pixel_decode_profile":"image_0_25_10_stored_rgba8",
        "rgba_sha256":format!("{:x}",Sha256::digest(&decoded.pixels_rgba)),
        "installed_rendering_qualified":false});
    write_new(&outputs[0], &bytes)?;
    write_new(&outputs[1], &rgba)?;
    write_new(&outputs[2], &serde_json::to_vec_pretty(&report)?)?;
    println!(
        "Generated 137216 authored blocks / {} pixels.",
        width as u64 * height as u64
    );
    Ok(())
}
fn round_div(value: u32, divisor: u32, nearest: bool) -> u8 {
    ((value + if nearest { divisor / 2 } else { 0 }) / divisor) as u8
}
fn expansion(value: u32, bits: u32, mode: u32) -> u8 {
    match mode {
        0 => round_div(value * 255, (1 << bits) - 1, false),
        1 => ((value << (8 - bits)) | (value >> (2 * bits - 8))) as u8,
        _ => round_div(value * 255, (1 << bits) - 1, true),
    }
}
fn hypothesize(
    bytes: &[u8],
    width: usize,
    height: usize,
    mode: u32,
    nearest_rgb: bool,
    nearest_alpha: bool,
) -> Vec<u8> {
    let mut rgba = vec![0; width * height * 4];
    for (i, b) in bytes[128..].as_chunks::<16>().0.iter().enumerate() {
        let c = u16::from_le_bytes(b[8..10].try_into().unwrap()) as u32;
        let d = u16::from_le_bytes(b[10..12].try_into().unwrap()) as u32;
        let mut colors = [[0u8; 3]; 4];
        for (channel, (bits, shift)) in [(5, 11), (6, 5), (5, 0)].into_iter().enumerate() {
            let mask = (1 << bits) - 1;
            let (c, d) = ((c >> shift) & mask, (d >> shift) & mask);
            let x = expansion(c, bits, mode);
            let y = expansion(d, bits, mode);
            colors[0][channel] = x;
            colors[1][channel] = y;
            for selector in [2usize, 3] {
                let (n, m) = if selector == 2 { (2, 1) } else { (1, 2) };
                colors[selector][channel] = if mode == 3 {
                    // Exact rational normalization before UNORM8 quantization:
                    // diagnostic hypothesis, not a DirectXTex executable result.
                    round_div((n * c + m * d) * 255, 3 * mask, nearest_rgb)
                } else {
                    round_div(n * x as u32 + m * y as u32, 3, nearest_rgb)
                };
            }
        }
        let mut alpha = [0u8; 8];
        alpha[..2].copy_from_slice(&b[..2]);
        let (divisor, end) = if b[0] > b[1] { (7, 8) } else { (5, 6) };
        for (selector, value) in alpha.iter_mut().enumerate().take(end).skip(2) {
            *value = round_div(
                (divisor + 1 - selector as u32) * b[0] as u32 + (selector as u32 - 1) * b[1] as u32,
                divisor,
                nearest_alpha,
            );
        }
        if end == 6 {
            alpha[7] = 255;
        }
        let indices = u32::from_le_bytes(b[12..16].try_into().unwrap());
        let mut alpha_indices = [0u8; 8];
        alpha_indices[..6].copy_from_slice(&b[2..8]);
        let alpha_indices = u64::from_le_bytes(alpha_indices);
        for p in 0..16 {
            let position =
                (((i / (width / 4)) * 4 + p / 4) * width + (i % (width / 4)) * 4 + p % 4) * 4;
            let color = colors[((indices >> (p * 2)) & 3) as usize];
            rgba[position..position + 3].copy_from_slice(&color);
            rgba[position + 3] = alpha[((alpha_indices >> (p * 3)) & 7) as usize];
        }
    }
    rgba
}
fn difference(left: &[u8], right: &[u8]) -> serde_json::Value {
    let mut channels = [0usize; 4];
    let mut maxima = [0u8; 4];
    let mut first = None;
    let mut pixels = 0;
    for (p, (a, b)) in left
        .as_chunks::<4>()
        .0
        .iter()
        .zip(right.as_chunks::<4>().0.iter())
        .enumerate()
    {
        if a != b {
            pixels += 1;
        }
        for c in 0..4 {
            if a[c] != b[c] {
                channels[c] += 1;
                if first.is_none() {
                    first = Some(json!({"byte":p*4+c,"expected":a[c],"observed":b[c]}));
                }
            }
            maxima[c] = maxima[c].max(a[c].abs_diff(b[c]));
        }
    }
    json!({"exact":first.is_none(),"different_pixels":pixels,
        "differing_channels_rgba":channels,"max_absolute_difference_rgba":maxima,
        "first_difference":first})
}
fn compare(
    root: &Path,
    dds: &Path,
    rust: &Path,
    oracle: &Path,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = source_identity::safe_output(root, output)?;
    let bytes = bounded(dds, 32 * 1024 * 1024)?;
    let decoded = decode_hd_image(&bytes, bytes.len())?;
    if decoded.format != HdImageFormat::DdsBc3 {
        return Err("BC3 comparison requires legacy DDS".into());
    }
    let (width, height) = (decoded.width as usize, decoded.height as usize);
    let mut header = b"RGBA".to_vec();
    header.extend(decoded.width.to_le_bytes());
    header.extend(decoded.height.to_le_bytes());
    let mut inputs = Vec::new();
    for path in [rust, oracle] {
        let input = bounded(path, 64 * 1024 * 1024 + 12)?;
        if input.len() != width * height * 4 + 12 || input[..12] != header {
            return Err("RGBA header/dimensions/length mismatch".into());
        }
        inputs.push(input);
    }
    if decoded.pixels_rgba != inputs[0][12..] {
        return Err("Rust evidence differs from pinned decoder".into());
    }
    let mut profiles = Vec::new();
    for (mode, name) in [
        "floor_normalized_endpoints",
        "bit_replicated_endpoints",
        "nearest_normalized_endpoints",
        "rational_before_quantization",
    ]
    .into_iter()
    .enumerate()
    {
        for nearest_rgb in [false, true] {
            for nearest_alpha in [false, true] {
                let pixels = hypothesize(
                    &bytes,
                    width,
                    height,
                    mode as u32,
                    nearest_rgb,
                    nearest_alpha,
                );
                profiles.push(json!({"endpoint_profile":name,"nearest_rgb":nearest_rgb,
                    "nearest_alpha":nearest_alpha,"rust":difference(&pixels,&inputs[0][12..]),
                    "independent":difference(&pixels,&inputs[1][12..])}));
            }
        }
    }
    let report = json!({"schema_version":1,"width":width,"height":height,
        "dds_sha256":format!("{:x}",Sha256::digest(&bytes)),"profiles":profiles,
        "rust_rgba_sha256":format!("{:x}",Sha256::digest(&inputs[0][12..])),
        "independent_rgba_sha256":format!("{:x}",Sha256::digest(&inputs[1][12..])),
        "rust_vs_independent":difference(&inputs[0][12..],&inputs[1][12..]),
        "installed_rendering_qualified":false,
        "profile_selection":"none; diagnostic hypotheses only"});
    write_new(&output, &serde_json::to_vec_pretty(&report)?)?;
    println!(
        "Compared 16 explicit numerical hypotheses / {} pixels.",
        width * height
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [mode,root,directory] if mode=="generate" => generate(Path::new(root),Path::new(directory)),
        [mode,root,dds,rust,oracle,output] if mode=="compare" =>
            compare(Path::new(root),Path::new(dds),Path::new(rust),Path::new(oracle),Path::new(output)),
        _ => Err("usage: bc3_boundaries generate <install-root> <existing-output-dir> | compare <install-root> <dds> <image-rs-rgba> <independent-rgba> <new-report>".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_block(block: [u8; 16]) -> Vec<u8> {
        let mut bytes = vec![0; 128];
        bytes.extend(block);
        bytes
    }

    #[test]
    fn boundary_values_distinguish_endpoint_and_alpha_quantization() {
        // Authored 5-bit red=3 has normalized UNORM8 24.677...; alpha
        // selector 2 has 6/7. Neither is a tolerance or a vendor fixture.
        let bytes = one_block(block(1, 0, 3 << 11, 3 << 11));
        let normalized = hypothesize(&bytes, 4, 4, 3, true, true);
        let early = hypothesize(&bytes, 4, 4, 0, true, false);
        assert_eq!(&normalized[..4], &[25, 0, 0, 1]);
        assert_eq!(&early[..4], &[24, 0, 0, 1]);
        assert_eq!(&normalized[8..12], &[25, 0, 0, 1]);
        assert_eq!(&early[8..12], &[24, 0, 0, 0]);
    }

    #[test]
    fn bc3_uses_four_colors_for_reversed_endpoints_and_equal_alpha_branch() {
        let bytes = one_block(block(0, 0, 0, 0xf800));
        let rgba = hypothesize(&bytes, 4, 4, 3, true, true);
        for p in 0..16 {
            assert_eq!(rgba[p * 4], [0, 255, 85, 170][p % 4]);
            assert_eq!(rgba[p * 4 + 3], if p % 8 == 7 { 255 } else { 0 });
        }
    }
}
