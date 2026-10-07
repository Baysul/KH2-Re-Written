use package_reader::{
    image_structure::{read_imgd_header, summarize_rgba},
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
    if args.len() != 4 {
        return Err(
            "usage: image_structure_probe <install> <manifest> <selection> <new-private-report>"
                .into(),
        );
    }
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let output = source.output_path(&args[3])?;
    // Same bounded admission as the viewer: fresh full rehash before decoding.
    let images = read_selected_images(&source, &args[2])?;
    let mut observations = Vec::new();
    let mut seen = HashSet::new();
    let mut internal_images = Vec::new();
    let mut total_original_bytes = 0usize;
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
        let replacements =
            package_reader::replacement::read_all_replacements(&mut checked.reader, entry)?;
        let bar = package_reader::bar::read_bar(&original.bytes)?;
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
            internal_images.push(serde_json::json!({"index":item.index,"ordinal":item.ordinal,"bar_ordinal":bar_ordinal,
                "bar_entry":record,"original_sha256":hash(&original.bytes),"header_sha256":hash(&bytes[..64]),
                "header":header,"bitmap_sha256":hash(&bytes[header.bitmap_offset as usize..header.bitmap_offset as usize+header.bitmap_length as usize]),
                "clut_sha256":hash(&bytes[header.clut_offset as usize..header.clut_offset as usize+header.clut_length as usize]),
                "flagged_bitmap_offset":flagged_offset,"matching_replacement_ordinals":matches,"pixel_decode":"NOT RUN"}));
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
