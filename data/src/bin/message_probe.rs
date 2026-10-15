use data::{
    bar::read_bar,
    font::{FontSlot, PaletteLane, decode_font},
    message::{read_messages, system_message_tokens},
    message_text::{
        InternationalContent, decode_international_system, international_fingerprint,
        international_reencoding_fingerprint, reencode_international_system,
    },
    original::read_original,
    probe::ProbeSource,
};
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
}
#[derive(Deserialize)]
struct Selection {
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
    candidate_name: String,
    purpose: String,
    text_profile: Option<String>,
    #[serde(default)]
    inspect_reencoding: bool,
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !matches!(args.len(), 4 | 5) {
        return Err("usage: message_probe <source-root> <private-manifest> <private-selection> <new-private-report> [existing-private-preview-directory]".into());
    }
    let source = ProbeSource::bind(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    let output = source.output_path(&PathBuf::from(&args[3]))?;
    let preview_dir = args.get(4).map(PathBuf::from);
    if preview_dir.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err("preview directory must already exist".into());
    }
    let mut previews = Vec::new();
    let mut preview_bytes = 0;
    let mut bytes = Vec::new();
    File::open(&args[2])?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("selection file exceeds inspection limit".into());
    }
    let selection: SelectionFile = serde_json::from_slice(&bytes)?;
    if selection.schema_version != 1
        || selection.source_content_identity != source.content_identity()
        || selection.selections.is_empty()
        || selection.selections.len() > 64
    {
        return Err("selection schema/source identity/count is invalid".into());
    }
    let indexes: Vec<_> = validation::required_files()
        .into_iter()
        .filter(|(p, _)| p.ends_with(".hed"))
        .map(|(p, _)| p)
        .collect();
    let mut seen = HashSet::new();
    for item in &selection.selections {
        if !indexes.contains(&item.index)
            || !matches!(item.purpose.as_str(), "message" | "font")
            || !seen.insert((&item.index, item.ordinal))
            || item.candidate_name.len() > 1024
            || item.text_profile.as_deref().is_some_and(|profile| {
                profile != "international_system" || item.purpose != "message"
            })
            || (item.inspect_reencoding
                && item.text_profile.as_deref() != Some("international_system"))
        {
            return Err(
                "selection contains unsupported index/purpose or duplicate identity".into(),
            );
        }
    }
    let mut samples = Vec::new();
    for relative in indexes {
        let items: Vec<_> = selection
            .selections
            .iter()
            .filter(|s| s.index == relative)
            .collect();
        if items.is_empty() {
            continue;
        }
        let mut checked = source.open_index(&relative)?;
        for item in items {
            let entry = checked
                .inventory
                .entries
                .get(item.ordinal)
                .ok_or("selected ordinal exceeds bound index")?;
            if entry.name_hash != item.name_hash {
                return Err("selected name hash differs from bound HED entry".into());
            }
            let original = read_original(&mut checked.reader, entry)?;
            let bar = read_bar(&original.bytes)?;
            let mut children = Vec::new();
            let mut message_tables = Vec::new();
            let mut font_images = Vec::new();
            let mut spacing_tables = Vec::new();
            for (ordinal, child) in bar.entries.iter().enumerate() {
                if child.offset < 0 {
                    continue;
                }
                let data = &original.bytes
                    [child.offset as usize..child.offset as usize + child.size as usize];
                children.push(serde_json::json!({"bar_ordinal":ordinal,"sha256":format!("{:x}",Sha256::digest(data))}));
                let slot = match &child.name_bytes {
                    b"sys\0" => Some(FontSlot::System),
                    b"evt\0" => Some(FontSlot::Event),
                    b"icon" => Some(FontSlot::Icon),
                    _ => None,
                };
                if item.purpose == "font" && child.entry_type == 2 && slot.is_some() {
                    spacing_tables.push(serde_json::json!({"bar_ordinal":ordinal,
                        "byte_count":data.len(),"sha256":format!("{:x}",Sha256::digest(data)),
                        "spacing_units_and_glyph_mapping_verified":false}));
                }
                if item.purpose == "font"
                    && child.entry_type == 36
                    && let Some(slot) = slot
                {
                    let lanes = if slot == FontSlot::Icon {
                        &[PaletteLane::First][..]
                    } else {
                        &[PaletteLane::First, PaletteLane::Second][..]
                    };
                    for (lane_index, &lane) in lanes.iter().enumerate() {
                        let image = decode_font(data, slot, lane)?;
                        let preview = if let Some(dir) = &preview_dir {
                            let index_name = relative.rsplit('/').next().unwrap();
                            let name =
                                format!("{index_name}-{}-{ordinal}-{lane_index}.bmp", item.ordinal);
                            let path = source.output_path(&dir.join(&name))?;
                            let mut bytes = Vec::new();
                            image.write_preview(&mut bytes)?;
                            preview_bytes += bytes.len();
                            if preview_bytes > 32 * 1024 * 1024 {
                                return Err(
                                    "private previews exceed 32 MiB aggregate inspection limit"
                                        .into(),
                                );
                            }
                            previews.push((path, bytes));
                            Some(name)
                        } else {
                            None
                        };
                        font_images.push(serde_json::json!({"bar_ordinal":ordinal,"palette_lane":lane_index,
                                "width":image.width,"height":image.height,"bits_per_pixel":image.bits_per_pixel,
                                "embedded_palette":image.embedded_palette,
                                "packed_pixel_sha256":format!("{:x}",Sha256::digest(&image.packed_pixels)),
                                "clut_rgba_sha256":format!("{:x}",Sha256::digest(&image.clut_rgba)),
                                "pixels_rgba_sha256":format!("{:x}",Sha256::digest(&image.pixels_rgba)),
                                "preview_file":preview,"runtime_rendering_verified":false}));
                    }
                }
                // List + Msg magic is a structural lead only, restricted to selected message containers.
                if item.purpose == "message"
                    && child.entry_type == 2
                    && data.starts_with(&1u32.to_le_bytes())
                {
                    let inventory = read_messages(data)?;
                    let mut token_records = Vec::new();
                    let mut opcode_counts = vec![0u64; 256];
                    let mut tokenization_failures = 0;
                    let mut token_bytes = 0;
                    let mut international_records = Vec::new();
                    let mut reencoding_records = Vec::new();
                    for entry in &inventory.entries {
                        token_bytes += entry.encoded_length as usize;
                        if token_bytes > 128 * 1024 * 1024 {
                            return Err("system token scan exceeds 128 MiB table budget".into());
                        }
                        let start = entry.source_offset as usize;
                        let encoded = &data[start..start + entry.encoded_length as usize];
                        let observation = match system_message_tokens(encoded) {
                            Ok(tokens) => {
                                let mut fingerprint = Sha256::new();
                                for token in &tokens {
                                    opcode_counts[token.opcode as usize] += 1;
                                    fingerprint.update((token.source_offset as u32).to_le_bytes());
                                    fingerprint.update([token.opcode]);
                                    fingerprint
                                        .update((token.parameters.len() as u32).to_le_bytes());
                                }
                                serde_json::json!({"status":"PASS", "token_count":tokens.len(),
                                    "boundaries_sha256":format!("{:x}",fingerprint.finalize())})
                            }
                            Err(error) => {
                                tokenization_failures += 1;
                                serde_json::json!({"status":"FAIL", "reason":error.to_string()})
                            }
                        };
                        token_records.push(serde_json::json!({"id":entry.id,
                            "source_offset":entry.source_offset,"observation":observation}));
                        if item.text_profile.as_deref() == Some("international_system") {
                            let observation = match decode_international_system(encoded) {
                                Ok(segments) => {
                                    let mut characters = 0;
                                    let mut complex_symbols = 0;
                                    let mut unsupported = 0;
                                    let mut unmapped = 0;
                                    for segment in &segments {
                                        match &segment.content {
                                            InternationalContent::Text(text) => {
                                                characters += text.chars().count();
                                            }
                                            InternationalContent::Complex(text) => {
                                                characters += text.chars().count();
                                                complex_symbols += 1;
                                            }
                                            InternationalContent::Unsupported { .. } => {
                                                unsupported += 1
                                            }
                                            InternationalContent::UnmappedTable => unmapped += 1,
                                            InternationalContent::Command => {}
                                        }
                                    }
                                    serde_json::json!({"status":"PASS", "segment_count":segments.len(),
                                        "content_sha256":international_fingerprint(&segments),
                                        "unicode_scalar_count":characters,"complex_symbols":complex_symbols,
                                        "unsupported_segments":unsupported,"unmapped_tables":unmapped})
                                }
                                Err(error) => {
                                    serde_json::json!({"status":"FAIL", "reason":error.to_string()})
                                }
                            };
                            international_records.push(serde_json::json!({"id":entry.id,
                                "source_offset":entry.source_offset,"observation":observation}));
                        }
                        if item.inspect_reencoding {
                            let observation = match reencode_international_system(encoded) {
                                Ok(parts) => {
                                    let mut canonical = Vec::with_capacity(encoded.len());
                                    let mut changed = 0;
                                    let mut text_changed = 0;
                                    let mut unsupported = 0;
                                    let mut unmapped = 0;
                                    for part in &parts {
                                        if matches!(
                                            part.source.content,
                                            InternationalContent::Unsupported { .. }
                                        ) {
                                            unsupported += 1;
                                        }
                                        if let Some(bytes) = &part.canonical {
                                            canonical.extend_from_slice(bytes);
                                            if bytes != part.source.encoded {
                                                changed += 1;
                                                if matches!(
                                                    part.source.content,
                                                    InternationalContent::Text(_)
                                                        | InternationalContent::Complex(_)
                                                ) {
                                                    text_changed += 1;
                                                }
                                            }
                                        } else {
                                            unmapped += 1;
                                        }
                                    }
                                    serde_json::json!({"status":"PASS","segment_count":parts.len(),
                                        "ordered_encoding_sha256":international_reencoding_fingerprint(&parts),
                                        "canonical_byte_count":canonical.len(),
                                        "canonical_sha256":if unmapped==0 {Some(format!("{:x}",Sha256::digest(&canonical)))} else {None},
                                        "whole_record_exact":unmapped==0 && canonical==encoded,
                                        "changed_segments":changed,"text_route_changed_segments":text_changed,
                                        "unsupported_segments":unsupported,"unmapped_tables":unmapped})
                                }
                                Err(error) => {
                                    serde_json::json!({"status":"FAIL","reason":error.to_string()})
                                }
                            };
                            reencoding_records.push(serde_json::json!({"id":entry.id,
                                "source_offset":entry.source_offset,"observation":observation}));
                        }
                    }
                    message_tables.push(serde_json::json!({"bar_ordinal":ordinal,
                        "bar_payload_offset":child.offset,"bar_payload_size":child.size,
                        "inventory":inventory,
                        "system_decoder_tokens":{"qualification":"pinned_decoder_lengths_only",
                            "runtime_semantics_verified":false,"unicode_decoded":false,
                            "record_reader_opcode_17_arity_conflict":true,
                            "failed_records":tokenization_failures,
                            "opcode_counts_for_successful_records":opcode_counts,
                            "records":token_records},
                        "international_text":{"requested":item.text_profile.as_deref()==Some("international_system"),
                            "qualification":"explicit_public_profile_only_not_runtime_language",
                            "runtime_semantics_verified":false,"original_bytes_retained":true,
                            "records":international_records},
                        "international_reencoding":{"requested":item.inspect_reencoding,
                            "qualification":"explicit_public_encoder_only_not_installed_rendering",
                            "unsupported_slots_are_not_text_draw_commands":true,
                            "original_bytes_retained":true,"runtime_semantics_verified":false,
                            "records":reencoding_records}}));
                }
            }
            if item.purpose == "message" && message_tables.is_empty() {
                return Err("selected message container has no recognized message table".into());
            }
            samples.push(serde_json::json!({"index":relative,"ordinal":item.ordinal,"candidate_name":item.candidate_name,
                "candidate_name_semantics_verified":false,"purpose":item.purpose,"name_hash":entry.name_hash,
                "package_offset":entry.package_offset,"compression_tag":original.header.compression_tag,
                "original_size":original.bytes.len(),"sha256":format!("{:x}",Sha256::digest(&original.bytes)),
                "replacement_count":original.header.remastered_asset_count,"bar":bar,"child_hashes":children,
                "message_tables":message_tables,"font_images":font_images,"spacing_tables":spacing_tables}));
        }
        checked.verify_unchanged()?;
    }
    let count = samples.len();
    let result = serde_json::json!({"schema_version":1,"source_content_identity":source.content_identity(),
        "qualification":"message_font_structure_inspection_only","source_check":"HED hashes and package size/mtime; full rescan required for mutation assurance",
        "asset_files_exported":!previews.is_empty(),
        "text_decoded_to_unicode":selection.selections.iter().any(|item|item.text_profile.as_deref()==Some("international_system")),
        "font_pixels_rendered":!previews.is_empty(),"font_preview_kind":"checkerboard_composited_inspection_only",
        "samples":samples});
    for (path, bytes) in &previews {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&serde_json::to_vec_pretty(&result)?)?;
    file.sync_all()?;
    println!(
        "Inspected {count} selected message/font containers; {} private font previews.",
        previews.len()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Message/font inspection failed: {error}");
        std::process::exit(1);
    }
}
