use package_reader::original::read_original;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Deserialize)]
struct SelectionFile {
    schema_version: u32,
    source_content_identity: String,
    selections: Vec<Selection>,
    #[serde(default)]
    inspect_hd_images: bool,
    #[serde(default)]
    inspect_model_skeletons: bool,
    #[serde(default)]
    dds_pixel_decode_profile: package_reader::hd_image::HdPixelDecodeProfile,
}
#[derive(Deserialize)]
struct Selection {
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !matches!(args.len(), 3..=5) {
        return Err("usage: original_probe <selected-install-root> <private-source-manifest.json> <new-private-report.json> [private-explicit-selection.json] [existing-private-preview-directory]".into());
    }
    let source = package_reader::probe::ProbeSource::bind(
        &PathBuf::from(&args[0]),
        &PathBuf::from(&args[1]),
    )?;
    let output = source.output_path(&PathBuf::from(&args[2]))?;
    let (selections, inspect_images, dds_profile, inspect_models) = if let Some(path) = args.get(3)
    {
        let mut bytes = Vec::new();
        File::open(path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err("original selection exceeds 1 MiB limit".into());
        }
        let selection: SelectionFile = serde_json::from_slice(&bytes)?;
        if selection.schema_version != 1
            || selection.source_content_identity != source.content_identity()
            || selection.selections.is_empty()
            || selection.selections.len() > 64
        {
            return Err("original selection schema/source identity/count is invalid".into());
        }
        let mut seen = HashSet::new();
        for item in &selection.selections {
            if !source_identity::required_files()
                .iter()
                .any(|(name, _)| name == &item.index && name.ends_with(".hed"))
                || !seen.insert((&item.index, item.ordinal))
            {
                return Err(
                    "original selection contains unsupported index or duplicate identity".into(),
                );
            }
        }
        if !selection.inspect_hd_images
            && selection.dds_pixel_decode_profile
                != package_reader::hd_image::HdPixelDecodeProfile::default()
        {
            return Err("DDS decoding profile requires explicit HD image inspection".into());
        }
        (
            Some(selection.selections),
            selection.inspect_hd_images,
            selection.dds_pixel_decode_profile,
            selection.inspect_model_skeletons,
        )
    } else {
        (
            None,
            false,
            package_reader::hd_image::HdPixelDecodeProfile::default(),
            false,
        )
    };
    if inspect_models {
        source.verify_full_source()?;
    }
    let preview_dir = args.get(4).map(PathBuf::from);
    if preview_dir
        .as_ref()
        .is_some_and(|p| !inspect_images || !p.is_dir())
    {
        return Err(
            "HD previews require explicit image inspection and an existing private directory"
                .into(),
        );
    }
    if let Some(directory) = &preview_dir {
        source.output_path(&directory.join("preview-admission-check.bmp"))?;
    }
    let mut samples = Vec::new();
    let mut total_bytes = 0usize;
    let mut image_bytes = 0usize;
    for (relative, _) in source_identity::required_files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".hed"))
    {
        if selections
            .as_ref()
            .is_some_and(|items| !items.iter().any(|s| s.index == relative))
        {
            continue;
        }
        let mut checked = source.open_index(&relative)?;
        if let Some(items) = &selections {
            for item in items.iter().filter(|s| s.index == relative) {
                if checked
                    .inventory
                    .entries
                    .get(item.ordinal)
                    .is_none_or(|e| e.name_hash != item.name_hash)
                {
                    return Err("selected ordinal/name hash differs from bound HED".into());
                }
            }
        }
        let mut selected = [0usize; 3];
        for (ordinal, entry) in checked.inventory.entries.iter().enumerate() {
            if selections.as_ref().is_some_and(|items| {
                !items
                    .iter()
                    .any(|s| s.index == relative && s.ordinal == ordinal)
            }) {
                continue;
            }
            use std::io::{Seek, SeekFrom};
            checked.reader.seek(SeekFrom::Start(entry.package_offset))?;
            let mut header = [0; 16];
            checked.reader.read_exact(&mut header)?;
            let header = package_reader::read_asset_header(&header, entry.stored_length)?;
            let category = if header.compression_tag == -2 {
                0
            } else if header.compression_tag == -1 {
                1
            } else {
                2
            };
            if selections.is_none() && selected[category] >= 2 {
                continue;
            }
            let original = read_original(&mut checked.reader, entry)?;
            selected[category] += 1;
            total_bytes += original.bytes.len();
            let replacement_samples = if selections.is_some() {
                package_reader::replacement::read_all_replacements(&mut checked.reader, entry)?
            } else {
                package_reader::replacement::read_first_replacements(&mut checked.reader, entry)?
            };
            total_bytes += replacement_samples
                .iter()
                .map(|s| s.bytes.len())
                .sum::<usize>();
            if total_bytes > 128 * 1024 * 1024 {
                return Err("original probe exceeds 128 MiB cumulative decoded limit".into());
            }
            let mut replacements = Vec::new();
            for sample in replacement_samples {
                let decoded_image = if inspect_images {
                    let image = package_reader::hd_image::decode_hd_image_with_profile(
                        &sample.bytes,
                        sample.metadata.declared_decompressed_length as usize,
                        dds_profile,
                    )?;
                    image_bytes += image.pixels_rgba.len();
                    if image_bytes > 128 * 1024 * 1024 {
                        return Err("HD image probe exceeds 128 MiB cumulative RGBA limit".into());
                    }
                    let preview = if let Some(directory) = &preview_dir {
                        let name = format!(
                            "sample-{:02}-replacement-{:02}.bmp",
                            samples.len(),
                            sample.metadata.ordinal
                        );
                        let path = source.output_path(&directory.join(&name))?;
                        let mut file =
                            OpenOptions::new().write(true).create_new(true).open(path)?;
                        image.write_preview(&mut file)?;
                        file.sync_all()?;
                        let pixel_name = name.replace(".bmp", ".rgba");
                        let pixel_path = source.output_path(&directory.join(pixel_name))?;
                        let mut pixel_file = OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(pixel_path)?;
                        pixel_file.write_all(b"RGBA")?;
                        pixel_file.write_all(&image.width.to_le_bytes())?;
                        pixel_file.write_all(&image.height.to_le_bytes())?;
                        pixel_file.write_all(&image.pixels_rgba)?;
                        pixel_file.sync_all()?;
                        Some(name)
                    } else {
                        None
                    };
                    Some(
                        serde_json::json!({"format":image.format,"width":image.width,"height":image.height,
                        "encoded_length":image.encoded_length,"storage_padding":image.storage_padding,
                        "rgba_byte_count":image.pixels_rgba.len(),
                        "pixels_rgba_sha256":format!("{:x}",Sha256::digest(&image.pixels_rgba)),
                        "preview_file":preview,"pixel_decode_profile":image.pixel_decode_profile,
                        "color_profile_transform_applied":false,"independent_pixel_check":"NOT RUN","runtime_verified":false}),
                    )
                } else {
                    None
                };
                replacements.push(serde_json::json!({
                    "metadata":sample.metadata,"decoded_size":sample.bytes.len(),
                    "format_hint":if sample.bytes.starts_with(b"DDS ") {"DDS"} else if sample.bytes.starts_with(b"\x89PNG\r\n\x1a\n") {"PNG"} else {"unknown"},
                    "inspection_prefix_hex":if selections.is_some() {Some(sample.bytes.iter().take(128).map(|b| format!("{b:02x}")).collect::<String>())} else {None},
                    "decoded_image":decoded_image,
                    "sha256":format!("{:x}",Sha256::digest(&sample.bytes))
                }));
            }
            let format_hint = if original.bytes.starts_with(b"BAR\x01") {
                "BAR"
            } else if original.bytes.starts_with(b"DDS ") {
                "DDS"
            } else if original.bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                "PNG"
            } else if original.bytes.starts_with(b"TIM2") {
                "TIM2"
            } else if original.bytes.is_empty() {
                "empty"
            } else {
                "unknown"
            };
            let bar = if format_hint == "BAR" {
                Some(package_reader::bar::read_bar(&original.bytes)?)
            } else {
                None
            };
            let mut sample_report = serde_json::json!({"index":relative,"ordinal":ordinal,
                "package_offset":entry.package_offset,"name_hash":entry.name_hash,
                "compression_tag":original.header.compression_tag,"compressed_bytes_consumed":original.compressed_bytes_consumed,
                "original_size":original.bytes.len(),"replacement_count":original.header.remastered_asset_count,
                "format_hint":format_hint,"bar":bar,"replacement_samples":replacements,
                "sha256":format!("{:x}",Sha256::digest(&original.bytes))});
            if inspect_models {
                let inventory = bar
                    .as_ref()
                    .ok_or("model inspection requires BAR original")?;
                let mut models = Vec::new();
                for (bar_ordinal, entry) in inventory
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.entry_type == 4)
                {
                    if entry.offset < 0 || entry.size == 0 {
                        return Err("model entry has no payload".into());
                    }
                    let start = entry.offset as usize;
                    let payload = &original.bytes[start..start + entry.size as usize];
                    let structure = package_reader::model_skeleton::read_model_structure(payload)?;
                    models
                        .push(serde_json::json!({"bar_ordinal":bar_ordinal,"structure":structure}));
                }
                if models.is_empty() {
                    return Err("model inspection requires a type-4 BAR entry".into());
                }
                sample_report["model_skeletons"] = serde_json::to_value(models)?;
            }
            samples.push(sample_report);
        }
        checked.verify_unchanged()?;
    }
    let count = samples.len();
    let mut result = serde_json::json!({"schema_version":1,"source_content_identity":source.content_identity(),
        "qualification":"bounded_original_sample_inspection_only","source_check":"HED content hashes; package size/mtime; full rescan required for mutation assurance",
        "selection_policy":if selections.is_some() {"explicit identities with every replacement; any decoding error aborts"} else {"first two originals per storage category per index, with their first two replacements; any decoding error aborts"},
        "all_declared_replacements_requested":selections.is_some(),
        "hd_image_decoding_requested":inspect_images,
        "original_asset_files_exported":false,"samples":samples});
    if inspect_models {
        result["model_skeleton_inspection_requested"] = true.into();
        result["source_check"] = "fresh full bound-source hashes before model inspection; HED content hashes and package size/mtime during reads; final rescan remains required".into();
    }
    let report_bytes = serde_json::to_vec_pretty(&result)?;
    if report_bytes.len() > 32 * 1024 * 1024 {
        return Err("original report exceeds 32 MiB limit".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&report_bytes)?;
    file.sync_all()?;
    println!(
        "Inspected {count} original storage samples; metadata/hashes saved; no original asset files exported."
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Original inspection failed: {error}");
        std::process::exit(1);
    }
}
