use data::{
    image_structure::{
        IMGD_INDEXED8_LINEAR_PROFILE, decode_imgd_indexed8_linear, read_imgd_header, summarize_rgba,
    },
    inspection::read_selected_images,
    original::read_original,
    probe::ProbeSource,
};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs::OpenOptions, io::Write, path::PathBuf};

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if !matches!(args.len(), 4..=5) {
        return Err(
            "usage: image_structure_probe <install> <manifest> <selection> <new-private-report> [existing-private-imgd-pixel-directory]"
                .into(),
        );
    }
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let output = source.output_path(&args[3])?;
    let pixel_directory = args.get(4);
    if let Some(directory) = pixel_directory {
        if !directory.is_dir() {
            return Err("IMGD pixel directory must exist".into());
        }
        source.output_path(&directory.join("imgd-pixel-admission.bmp"))?;
    }
    // Same bounded admission as the viewer: fresh full rehash before decoding.
    let images = read_selected_images(&source, &args[2])?;
    let mut observations = Vec::new();
    let mut seen = HashSet::new();
    let mut internal_images = Vec::new();
    let mut total_original_bytes = 0usize;
    let mut total_imgd_rgba_bytes = 0usize;
    for item in &images {
        observations.push(serde_json::json!({"index":item.index,"ordinal":item.ordinal,
            "replacement_ordinal":item.replacement_ordinal,"width":item.decoded.width,"height":item.decoded.height,
            "pixel_decode_profile":item.decoded.pixel_decode_profile,"pixels_rgba_sha256":hash(&item.decoded.pixels_rgba),
            "channels":summarize_rgba(&item.decoded.pixels_rgba)?}));
        if !seen.insert((&item.index, item.ordinal)) {
            continue;
        }
        let mut checked = source.open_index(&item.index)?;
        let entry = &checked.inventory.entries[item.ordinal];
        let original = read_original(&mut checked.reader, entry)?;
        total_original_bytes += original.bytes.len();
        if total_original_bytes > 128 * 1024 * 1024 {
            return Err("original observation budget exceeded".into());
        }
        let replacements = data::replacement::read_all_replacements(&mut checked.reader, entry)?;
        let bar = data::bar::read_bar(&original.bytes)?;
        for (bar_ordinal, record) in bar.entries.iter().enumerate() {
            if record.offset < 0 || record.size == 0 {
                continue;
            }
            let start = record.offset as usize;
            let bytes = &original.bytes[start..start + record.size as usize];
            if !bytes.starts_with(b"IMGD") {
                continue;
            }
            let header = read_imgd_header(bytes)?;
            let flagged_offset = (start as u64 + header.bitmap_offset as u64) | 0x2000_0000;
            let matches: Vec<_> = replacements
                .iter()
                .filter(|r| {
                    u64::try_from(r.metadata.original_asset_offset).ok() == Some(flagged_offset)
                })
                .map(|r| r.metadata.ordinal)
                .collect();
            let mut observation = serde_json::json!({"index":item.index,"ordinal":item.ordinal,"bar_ordinal":bar_ordinal,
                "bar_entry":record,"original_sha256":hash(&original.bytes),"header_sha256":hash(&bytes[..64]),
                "header":header,"bitmap_sha256":hash(&bytes[header.bitmap_offset as usize..header.bitmap_offset as usize+header.bitmap_length as usize]),
                "clut_sha256":hash(&bytes[header.clut_offset as usize..header.clut_offset as usize+header.clut_length as usize]),
                "flagged_bitmap_offset":flagged_offset,"matching_replacement_ordinals":matches,"pixel_decode":"NOT RUN"});
            if let Some(directory) = pixel_directory {
                if internal_images.len() >= 64 {
                    return Err("IMGD pixel count exceeds 64".into());
                }
                let decoded = decode_imgd_indexed8_linear(bytes)?;
                total_imgd_rgba_bytes += decoded.pixels_rgba.len();
                if total_imgd_rgba_bytes > 128 * 1024 * 1024 {
                    return Err("IMGD RGBA budget exceeded".into());
                }
                let stem = format!("internal-{:03}", internal_images.len());
                let names = ["bmp", "rgba", "indices", "clut"]
                    .map(|extension| format!("{stem}.{extension}"));
                let paths = names
                    .iter()
                    .map(|name| source.output_path(&directory.join(name)))
                    .collect::<Result<Vec<_>, _>>()?;
                let create = |path| OpenOptions::new().write(true).create_new(true).open(path);
                let mut preview = create(&paths[0])?;
                data::font::write_rgba_preview(
                    &mut preview,
                    header.width as usize,
                    header.height as usize,
                    &decoded.pixels_rgba,
                )?;
                preview.sync_all()?;
                for (path, content) in [
                    (&paths[1], decoded.pixels_rgba.as_slice()),
                    (&paths[2], decoded.indices.as_slice()),
                    (&paths[3], decoded.clut_rgba.as_slice()),
                ] {
                    let mut file = create(path)?;
                    if path == &paths[1] {
                        file.write_all(b"RGBA")?;
                        file.write_all(&(header.width as u32).to_le_bytes())?;
                        file.write_all(&(header.height as u32).to_le_bytes())?;
                    }
                    file.write_all(content)?;
                    file.sync_all()?;
                }
                observation["pixel_decode"] = serde_json::json!("CPU_PROFILE_ONLY");
                observation["decoded_image"] = serde_json::json!({"pixel_decode_profile":IMGD_INDEXED8_LINEAR_PROFILE,
                    "indices_sha256":hash(&decoded.indices),"clut_rgba_sha256":hash(&decoded.clut_rgba),
                    "pixels_rgba_sha256":hash(&decoded.pixels_rgba),"rgba_byte_count":decoded.pixels_rgba.len(),
                    "preview_file":names[0],"rgba_file":names[1],"indices_file":names[2],"clut_file":names[3],
                    "channels":summarize_rgba(&decoded.pixels_rgba)?,"installed_semantics":"NOT RUN"});
            }
            internal_images.push(observation);
        }
        checked.verify_unchanged()?;
    }
    let count = observations.len();
    let report = serde_json::to_vec_pretty(&serde_json::json!({"schema_version":1,
        "source_content_identity":source.content_identity(),"fresh_full_scan":true,"qualification":"quarantined_structural_observation_only",
        "images":observations,"internal_images":internal_images,"gameplay_enabled":false,"glyph_shader_semantics":"UNKNOWN"}))?;
    if report.len() > 32 * 1024 * 1024 {
        return Err("structure report exceeds 32 MiB".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&report)?;
    file.sync_all()?;
    println!(
        "Observed {count} image channel distributions and bounded internal headers; no glyph semantics or gameplay qualification."
    );
    Ok(())
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Image structure rejected: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
