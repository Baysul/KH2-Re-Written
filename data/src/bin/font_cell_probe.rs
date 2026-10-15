use data::{
    bar::read_bar,
    font::{FontSlot, PaletteLane, decode_font},
    font_cell::{CellProfile, project_font_cell},
    original_payload::read_original,
    probe_source::ProbeSource,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs::File, fs::OpenOptions, io::Read, io::Write, path::PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionFile {
    schema_version: u32,
    source_content_identity: String,
    pairs: Vec<Pair>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PayloadSelection {
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
    bar_ordinal: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    id: String,
    profile: CellProfile,
    image: PayloadSelection,
    spacing: PayloadSelection,
}

fn payload(
    source: &ProbeSource,
    selected: &PayloadSelection,
    slot: FontSlot,
    entry_type: u16,
    total_bytes: &mut usize,
) -> Result<(Vec<u8>, serde_json::Value)> {
    let mut checked = source.open_index(&selected.index)?;
    let entry = checked
        .inventory
        .entries
        .get(selected.ordinal)
        .ok_or("selected ordinal exceeds bound index")?;
    if entry.name_hash != selected.name_hash {
        return Err("selected hash differs from bound HED entry".into());
    }
    let original = read_original(&mut checked.reader, entry)?;
    *total_bytes += original.bytes.len();
    if *total_bytes > 128 * 1024 * 1024 {
        return Err("font cell probe exceeds 128 MiB original budget".into());
    }
    let bar = read_bar(&original.bytes)?;
    if bar.entries.len() > 64 {
        return Err("font cell probe BAR exceeds 64-entry metadata limit".into());
    }
    let child = bar
        .entries
        .get(selected.bar_ordinal)
        .ok_or("selected child exceeds BAR table")?;
    let name = match slot {
        FontSlot::System => *b"sys\0",
        FontSlot::Event => *b"evt\0",
        FontSlot::Icon => *b"icon",
    };
    if child.offset < 0 || child.entry_type != entry_type || child.name_bytes != name {
        return Err("selected BAR entry does not match the explicit font slot/type".into());
    }
    let data =
        original.bytes[child.offset as usize..child.offset as usize + child.size as usize].to_vec();
    let metadata = serde_json::json!({
        "selection":selected,"package_offset":entry.package_offset,
        "original_size":original.bytes.len(),"replacement_count":original.header.remastered_asset_count,
        "compression_tag":original.header.compression_tag,
        "original_sha256":format!("{:x}",Sha256::digest(&original.bytes)),
        "bar":bar,"payload_sha256":format!("{:x}",Sha256::digest(&data)),
    });
    checked.verify_unchanged()?;
    Ok((data, metadata))
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: font_cell_probe <selected-source-root> <private-manifest> <private-pairs> <new-private-report>".into());
    }
    let source = ProbeSource::bind(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    let output = source.output_path(&PathBuf::from(&args[3]))?;
    let mut bytes = Vec::new();
    File::open(&args[2])?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("font pair selection exceeds 1 MiB limit".into());
    }
    let selections: SelectionFile = serde_json::from_slice(&bytes)?;
    if selections.schema_version != 1
        || selections.source_content_identity != source.content_identity()
        || selections.pairs.is_empty()
        || selections.pairs.len() > 32
    {
        return Err("font pair schema/source identity/count is invalid".into());
    }
    let mut ids = HashSet::new();
    for pair in &selections.pairs {
        if pair.id.is_empty() || pair.id.len() > 128 || !ids.insert(&pair.id) {
            return Err("font pair ID is empty, oversized or duplicate".into());
        }
    }
    let mut total_bytes = 0;
    let mut pairs = Vec::new();
    for pair in &selections.pairs {
        let (image_data, image_source) = payload(
            &source,
            &pair.image,
            pair.profile.slot(),
            36,
            &mut total_bytes,
        )?;
        let (spacing, spacing_source) = payload(
            &source,
            &pair.spacing,
            pair.profile.slot(),
            2,
            &mut total_bytes,
        )?;
        if spacing.is_empty() || spacing.len() > 4096 {
            return Err("spacing payload is empty or exceeds 4096-byte probe limit".into());
        }
        let first = decode_font(&image_data, pair.profile.slot(), PaletteLane::First)?;
        let second = if pair.profile != CellProfile::Icon {
            Some(decode_font(
                &image_data,
                pair.profile.slot(),
                PaletteLane::Second,
            )?)
        } else {
            None
        };
        let mut cells = Vec::new();
        let mut failed = 0;
        for index in 0..spacing.len() {
            let cell = match project_font_cell(
                pair.profile,
                first.width,
                first.height,
                &spacing,
                index,
            ) {
                Ok(cell) => cell,
                Err(error) => {
                    failed += 1;
                    cells.push(serde_json::json!({"index":index,"status":"FAIL","reason":error.to_string()}));
                    continue;
                }
            };
            let image = if cell.palette_lane == 0 {
                &first
            } else {
                second.as_ref().unwrap()
            };
            let pixels = cell.crop_rgba(image)?;
            cells.push(
                serde_json::json!({"index":index,"status":"PASS","cell":cell,
                "pixels_rgba_sha256":format!("{:x}",Sha256::digest(&pixels))}),
            );
        }
        pairs.push(
            serde_json::json!({"request":pair,"image_source":image_source,
            "spacing_source":spacing_source,"atlas_width":first.width,"atlas_height":first.height,
            "spacing_count":spacing.len(),"failed_cells":failed,"cells":cells,
            "runtime_pairing_verified":false,"spacing_units_verified":false}),
        );
    }
    let report = serde_json::json!({"schema_version":1,"source_content_identity":source.content_identity(),
        "qualification":"explicit_font_cell_candidate_inspection_only",
        "source_check":"HED hash and package metadata per handle; full before/after scans required",
        "runtime_rendering_verified":false,"asset_files_exported":false,"pairs":pairs});
    let bytes = serde_json::to_vec_pretty(&report)?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("font cell report exceeds 32 MiB limit".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    println!(
        "Inspected {} explicitly paired font cell candidates; no runtime rendering claim.",
        selections.pairs.len()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Font cell inspection failed: {error}");
        std::process::exit(1);
    }
}
