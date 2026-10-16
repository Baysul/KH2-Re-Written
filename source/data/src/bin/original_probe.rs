use data::original_payload::read_original;
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
    inspect_hd_mip_chains: bool,
    #[serde(default)]
    inspect_model_skeletons: bool,
    #[serde(default)]
    inspect_model_packets: bool,
    #[serde(default)]
    inspect_model_geometry: bool,
    #[serde(default)]
    inspect_model_textures: bool,
    #[serde(default)]
    inspect_model_texture_pixels: bool,
    #[serde(default)]
    inspect_model_texture_routing: bool,
    #[serde(default)]
    inspect_model_texture_animations: bool,
    #[serde(default)]
    model_pose_profile: Option<data::model_pose::PoseProfile>,
    #[serde(default)]
    dds_pixel_decode_profile: data::hd_image::HdPixelDecodeProfile,
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
    let source =
        data::probe_source::ProbeSource::bind(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    let output = source.output_path(&PathBuf::from(&args[2]))?;
    let (
        selections,
        inspect_images,
        inspect_mips,
        dds_profile,
        inspect_models,
        inspect_packets,
        inspect_geometry,
        pose_profile,
        inspect_textures,
        inspect_texture_pixels,
        inspect_texture_routing,
        inspect_texture_animations,
    ) = if let Some(path) = args.get(3) {
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
            if !validation::required_files()
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
            && selection.dds_pixel_decode_profile != data::hd_image::HdPixelDecodeProfile::default()
        {
            return Err("DDS decoding profile requires explicit HD image inspection".into());
        }
        if selection.inspect_hd_mip_chains
            && (!selection.inspect_hd_images
                || selection.dds_pixel_decode_profile
                    != data::hd_image::HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1)
        {
            return Err(
                "mip chains require explicit HD inspection and normalized BC3 profile".into(),
            );
        }
        if selection.inspect_model_packets && !selection.inspect_model_skeletons {
            return Err("packet inspection requires explicit model structure inspection".into());
        }
        if selection.inspect_model_geometry && !selection.inspect_model_packets {
            return Err("geometry inspection requires explicit packet inspection".into());
        }
        if selection.model_pose_profile.is_some() && !selection.inspect_model_geometry {
            return Err("pose profile requires explicit geometry inspection".into());
        }
        if selection.inspect_model_texture_pixels
            && (!selection.inspect_model_textures || args.get(4).is_none())
        {
            return Err(
                "model texture pixels require layout opt-in and existing private pixel directory"
                    .into(),
            );
        }
        if selection.inspect_model_texture_routing
            && (!selection.inspect_model_textures || !selection.inspect_model_geometry)
        {
            return Err(
                "texture routing requires explicit texture layout and geometry inspection".into(),
            );
        }
        if selection.inspect_model_texture_animations && !selection.inspect_model_textures {
            return Err("TEXA records require explicit model texture layout inspection".into());
        }
        (
            Some(selection.selections),
            selection.inspect_hd_images,
            selection.inspect_hd_mip_chains,
            selection.dds_pixel_decode_profile,
            selection.inspect_model_skeletons,
            selection.inspect_model_packets,
            selection.inspect_model_geometry,
            selection.model_pose_profile,
            selection.inspect_model_textures,
            selection.inspect_model_texture_pixels,
            selection.inspect_model_texture_routing,
            selection.inspect_model_texture_animations,
        )
    } else {
        (
            None,
            false,
            false,
            data::hd_image::HdPixelDecodeProfile::default(),
            false,
            false,
            false,
            None,
            false,
            false,
            false,
            false,
        )
    };
    if inspect_models || inspect_textures || inspect_mips {
        source.verify_full_source()?;
    }
    let preview_dir = args.get(4).map(PathBuf::from);
    if preview_dir
        .as_ref()
        .is_some_and(|p| (!inspect_images && !inspect_texture_pixels) || !p.is_dir())
    {
        return Err(
            "Previews require explicit HD/model pixel inspection and an existing private directory"
                .into(),
        );
    }
    if let Some(directory) = &preview_dir {
        source.output_path(&directory.join("preview-admission-check.bmp"))?;
    }
    let mut samples = Vec::new();
    let mut total_bytes = 0usize;
    let mut image_bytes = 0usize;
    let mut texture_rgba_bytes = 0usize;
    let mut texture_image_count = 0usize;
    let (mut animation_slots, mut animation_groups, mut animation_frames) =
        (0usize, 0usize, 0usize);
    for (relative, _) in validation::required_files()
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
            let header = data::read_asset_header(&header, entry.stored_length)?;
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
                data::replacement_payload::read_all_replacements(&mut checked.reader, entry)?
            } else {
                data::replacement_payload::read_first_replacements(&mut checked.reader, entry)?
            };
            total_bytes += replacement_samples
                .iter()
                .map(|s| s.bytes.len())
                .sum::<usize>();
            if total_bytes > 128 * 1024 * 1024 {
                return Err("original probe exceeds 128 MiB cumulative decoded limit".into());
            }
            let mut replacements = Vec::new();
            let replacement_metadata: Vec<_> = if inspect_texture_routing {
                replacement_samples
                    .iter()
                    .map(|s| s.metadata.clone())
                    .collect()
            } else {
                Vec::new()
            };
            for sample in replacement_samples {
                let decoded_image = if inspect_images {
                    let is_chain = inspect_mips
                        && sample.bytes.starts_with(b"DDS ")
                        && sample
                            .bytes
                            .get(28..32)
                            .is_some_and(|v| u32::from_le_bytes(v.try_into().unwrap()) > 1);
                    let (image, mip_observation) = if is_chain {
                        let layout = data::hd_image::read_bc3_mip_layout(
                            &sample.bytes,
                            sample.metadata.declared_decompressed_length as usize,
                        )?;
                        image_bytes += layout.rgba_byte_count;
                        if image_bytes > 128 * 1024 * 1024 {
                            return Err(
                                "HD image probe exceeds 128 MiB cumulative RGBA limit".into()
                            );
                        }
                        let chain = data::hd_image::decode_bc3_mip_chain(
                            &sample.bytes,
                            sample.metadata.declared_decompressed_length as usize,
                        )?;
                        let mut levels = Vec::new();
                        let mip_file = if let Some(directory) = &preview_dir {
                            let name = format!(
                                "sample-{:02}-replacement-{:02}.mips",
                                samples.len(),
                                sample.metadata.ordinal
                            );
                            let path = source.output_path(&directory.join(&name))?;
                            let mut file =
                                OpenOptions::new().write(true).create_new(true).open(path)?;
                            file.write_all(b"MIPS")?;
                            file.write_all(&(chain.layout.levels.len() as u32).to_le_bytes())?;
                            for (level, pixels) in
                                chain.layout.levels.iter().zip(&chain.levels_rgba)
                            {
                                file.write_all(&level.width.to_le_bytes())?;
                                file.write_all(&level.height.to_le_bytes())?;
                                file.write_all(&(level.encoded_offset as u64).to_le_bytes())?;
                                file.write_all(&(level.encoded_length as u64).to_le_bytes())?;
                                file.write_all(pixels)?;
                            }
                            file.sync_all()?;
                            Some(name)
                        } else {
                            None
                        };
                        for (level, pixels) in chain.layout.levels.iter().zip(&chain.levels_rgba) {
                            levels.push(serde_json::json!({"ordinal":level.ordinal,
                                "rgba_byte_count":pixels.len(),"pixels_rgba_sha256":format!("{:x}",Sha256::digest(pixels))}));
                        }
                        let observation = serde_json::json!({"layout":chain.layout,"levels":levels,"mip_file":mip_file,
                            "pixel_decode_profile":chain.pixel_decode_profile,"independent_pixel_check":"NOT RUN","runtime_verified":false});
                        let base = &chain.layout.levels[0];
                        let image = data::hd_image::DecodedHdImage {
                            format: data::hd_image::HdImageFormat::DdsBc3,
                            pixel_decode_profile: chain.pixel_decode_profile,
                            width: base.width,
                            height: base.height,
                            encoded_length: chain.layout.encoded_length,
                            storage_padding: chain.layout.storage_padding,
                            pixels_rgba: chain.levels_rgba.into_iter().next().unwrap(),
                        };
                        (image, Some(observation))
                    } else {
                        let image = data::hd_image::decode_hd_image_with_profile(
                            &sample.bytes,
                            sample.metadata.declared_decompressed_length as usize,
                            dds_profile,
                        )?;
                        image_bytes += image.pixels_rgba.len();
                        if image_bytes > 128 * 1024 * 1024 {
                            return Err(
                                "HD image probe exceeds 128 MiB cumulative RGBA limit".into()
                            );
                        }
                        (image, None)
                    };
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
                    let mut observation = serde_json::json!({"format":image.format,"width":image.width,"height":image.height,
                        "encoded_length":image.encoded_length,"storage_padding":image.storage_padding,
                        "rgba_byte_count":image.pixels_rgba.len(),
                        "pixels_rgba_sha256":format!("{:x}",Sha256::digest(&image.pixels_rgba)),
                        "preview_file":preview,"pixel_decode_profile":image.pixel_decode_profile,
                        "color_profile_transform_applied":false,"independent_pixel_check":"NOT RUN","runtime_verified":false});
                    if let Some(mips) = mip_observation {
                        observation["mip_chain"] = mips;
                    }
                    Some(observation)
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
                Some(data::bar::read_bar(&original.bytes)?)
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
                    let structure = data::model_skeleton::read_model_structure(payload)?;
                    let mut model =
                        serde_json::json!({"bar_ordinal":bar_ordinal,"structure":structure});
                    if inspect_packets {
                        model["packet_groups"] = serde_json::to_value(
                            data::model_packets::read_packet_groups(payload)?,
                        )?;
                    }
                    if inspect_geometry {
                        model["geometry"] =
                            serde_json::to_value(data::model_geometry::read_geometry(payload)?)?;
                    }
                    if let Some(profile) = pose_profile {
                        model["pose"] =
                            serde_json::to_value(data::model_pose::read_pose(payload, profile)?)?;
                    }
                    models.push(model);
                }
                if models.is_empty() {
                    return Err("model inspection requires a type-4 BAR entry".into());
                }
                sample_report["model_skeletons"] = serde_json::to_value(models)?;
            }
            if inspect_textures {
                let inventory = bar
                    .as_ref()
                    .ok_or("model texture inspection requires BAR original")?;
                let mut textures = Vec::new();
                let mut texture_pixels = Vec::new();
                let mut texture_animations = Vec::new();
                for (bar_ordinal, entry) in inventory
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.entry_type == 7)
                {
                    if entry.offset < 0 || entry.size == 0 {
                        return Err("model texture entry has no payload".into());
                    }
                    let start = entry.offset as usize;
                    let layout = data::model_texture::read_layout(
                        &original.bytes[start..start + entry.size as usize],
                    )?;
                    textures.push(serde_json::json!({"bar_ordinal":bar_ordinal,"layout":layout}));
                    if inspect_texture_animations {
                        let records = data::model_texture_animation::read_records(
                            &original.bytes[start..start + entry.size as usize],
                        )?;
                        for animation in &records.animations {
                            animation_slots += animation.slot_table.len();
                            animation_groups += animation.frame_groups.len();
                            animation_frames += animation
                                .frame_groups
                                .iter()
                                .map(|g| g.records.len())
                                .sum::<usize>();
                        }
                        if animation_slots > 262_144
                            || animation_groups > 4096
                            || animation_frames > 65_536
                        {
                            return Err(
                                "probe cumulative TEXA slot/group/frame budget exceeded".into()
                            );
                        }
                        texture_animations
                            .push(serde_json::json!({"bar_ordinal":bar_ordinal,"records":records}));
                    }
                    if inspect_texture_pixels {
                        let decoded = data::model_texture_pixels::decode_images(
                            &original.bytes[start..start + entry.size as usize],
                        )?;
                        for image in decoded {
                            texture_rgba_bytes += image.pixels_rgba.len();
                            texture_image_count += 1;
                            if texture_rgba_bytes > 128 * 1024 * 1024 || texture_image_count > 64 {
                                return Err(
                                    "model texture pixel probe exceeds 128 MiB/64 image budget"
                                        .into(),
                                );
                            }
                            let stem = format!(
                                "model-texture-{:02}-{bar_ordinal:02}-{:03}",
                                samples.len(),
                                image.environment_ordinal
                            );
                            let names = ["bmp", "rgba", "indices", "clut"]
                                .map(|extension| format!("{stem}.{extension}"));
                            let directory = preview_dir.as_ref().unwrap();
                            let paths = names
                                .iter()
                                .map(|name| source.output_path(&directory.join(name)))
                                .collect::<Result<Vec<_>, _>>()?;
                            let create =
                                |path| OpenOptions::new().write(true).create_new(true).open(path);
                            let mut preview = create(&paths[0])?;
                            data::font::write_rgba_preview(
                                &mut preview,
                                image.width as usize,
                                image.height as usize,
                                &image.pixels_rgba,
                            )?;
                            preview.sync_all()?;
                            for (path, content) in [
                                (&paths[1], image.pixels_rgba.as_slice()),
                                (&paths[2], image.packed_indices.as_slice()),
                                (&paths[3], image.clut_rgba.as_slice()),
                            ] {
                                let mut file = create(path)?;
                                if path == &paths[1] {
                                    file.write_all(b"RGBA")?;
                                    file.write_all(&image.width.to_le_bytes())?;
                                    file.write_all(&image.height.to_le_bytes())?;
                                }
                                file.write_all(content)?;
                                file.sync_all()?;
                            }
                            let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
                            texture_pixels.push(serde_json::json!({"bar_ordinal":bar_ordinal,"environment_ordinal":image.environment_ordinal,
                                "pixel_decode_profile":data::model_texture_pixels::PROFILE,"width":image.width,"height":image.height,
                                "indexed_bits_per_pixel":image.indexed_bits_per_pixel,"upload_psm":image.upload_psm,
                                "packed_indices_sha256":hash(&image.packed_indices),"clut_rgba_sha256":hash(&image.clut_rgba),
                                "pixels_rgba_sha256":hash(&image.pixels_rgba),"rgba_byte_count":image.pixels_rgba.len(),
                                "preview_file":names[0],"rgba_file":names[1],"indices_file":names[2],"clut_file":names[3],"runtime_verified":false}));
                        }
                    }
                }
                if textures.is_empty() {
                    return Err("model texture inspection requires type-7 BAR entry".into());
                }
                sample_report["model_texture_layouts"] = serde_json::to_value(textures)?;
                if inspect_texture_animations {
                    sample_report["model_texture_animation_records"] =
                        serde_json::to_value(texture_animations)?;
                }
                if inspect_texture_pixels {
                    sample_report["model_texture_pixels"] = serde_json::to_value(texture_pixels)?;
                }
                if inspect_texture_routing {
                    let named: Vec<_> = inventory
                        .entries
                        .iter()
                        .enumerate()
                        .filter(|(_, entry)| entry.entry_type == 7 && &entry.name_bytes == b"tim_")
                        .collect();
                    if named.len() != 1 {
                        return Err(
                            "texture routing requires exactly one named tim_ type-7 BAR entry"
                                .into(),
                        );
                    }
                    let (texture_bar_ordinal, texture_entry) = named[0];
                    let start = texture_entry.offset as usize;
                    let texture_bytes = &original.bytes[start..start + texture_entry.size as usize];
                    let links = data::model_texture_routing::read_links(
                        texture_bytes,
                        start,
                        &replacement_metadata,
                    )?;
                    let layout = data::model_texture::read_layout(texture_bytes)?;
                    let mut models = Vec::new();
                    for (bar_ordinal, record) in inventory
                        .entries
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.entry_type == 4)
                    {
                        let offset = usize::try_from(record.offset)
                            .map_err(|_| "missing model payload for texture routing")?;
                        let geometry = data::model_geometry::read_geometry(
                            &original.bytes[offset..offset + record.size as usize],
                        )?;
                        let routes = data::model_texture_routing::associate_geometry(
                            &geometry,
                            layout.images.len(),
                        )?;
                        models.push(serde_json::json!({"model_bar_ordinal":bar_ordinal,"geometry_profile":geometry.profile,"part_routes":routes}));
                    }
                    sample_report["model_texture_routing"] = serde_json::json!({"texture_bar_ordinal":texture_bar_ordinal,"links":links,"models":models});
                }
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
    if inspect_mips {
        result["hd_mip_chain_inspection_requested"] = true.into();
        result["source_check"] = "fresh full bound-source hashes before mip inspection; HED content hashes and package size/mtime during reads; final rescan remains required".into();
    }
    if inspect_textures {
        result["model_texture_inspection_requested"] = true.into();
        result["model_texture_profile"] = data::model_texture::PROFILE.into();
        result["source_check"] = "fresh full bound-source hashes before model inspection; HED content hashes and package size/mtime during reads; final rescan remains required".into();
    }
    if inspect_texture_pixels {
        result["model_texture_pixel_inspection_requested"] = true.into();
        result["model_texture_pixel_profile"] = data::model_texture_pixels::PROFILE.into();
        result["model_texture_pixel_files_exported"] = true.into();
    }
    if inspect_texture_routing {
        result["model_texture_routing_requested"] = true.into();
        result["model_texture_routing_profile"] = data::model_texture_routing::PROFILE.into();
    }
    if inspect_texture_animations {
        result["model_texture_animation_records_requested"] = true.into();
        result["model_texture_animation_profile"] = data::model_texture_animation::PROFILE.into();
    }
    if inspect_packets {
        result["model_packet_inspection_requested"] = true.into();
        result["model_packet_profile"] = data::model_packets::PROFILE.into();
        result["vif_cpu_profile"] = data::vif::PROFILE.into();
    }
    if inspect_geometry {
        result["model_geometry_inspection_requested"] = true.into();
        result["model_geometry_profile"] = data::model_geometry::PROFILE.into();
    }
    if let Some(profile) = pose_profile {
        result["model_pose_profile"] = serde_json::to_value(profile)?;
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
