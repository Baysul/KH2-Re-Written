use package_reader::original::read_original;
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: original_probe <selected-install-root> <private-source-manifest.json> <new-private-report.json>".into());
    }
    let source = package_reader::probe::ProbeSource::bind(
        &PathBuf::from(&args[0]),
        &PathBuf::from(&args[1]),
    )?;
    let output = source.output_path(&PathBuf::from(&args[2]))?;
    let mut samples = Vec::new();
    for (relative, _) in source_identity::required_files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".hed"))
    {
        let mut checked = source.open_index(&relative)?;
        let mut selected = [0usize; 3];
        for (ordinal, entry) in checked.inventory.entries.iter().enumerate() {
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
            if selected[category] >= 2 {
                continue;
            }
            let original = read_original(&mut checked.reader, entry)?;
            selected[category] += 1;
            let replacements: Vec<_> = package_reader::replacement::read_first_replacements(
                &mut checked.reader,
                entry,
            )?
            .into_iter()
            .map(|sample| {
                serde_json::json!({
                    "metadata":sample.metadata,"decoded_size":sample.bytes.len(),
                    "format_hint":if sample.bytes.starts_with(b"DDS ") {"DDS"} else {"unknown"},
                    "sha256":format!("{:x}",Sha256::digest(&sample.bytes))
                })
            })
            .collect();
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
            samples.push(serde_json::json!({"index":relative,"ordinal":ordinal,
                "package_offset":entry.package_offset,"name_hash":entry.name_hash,
                "compression_tag":original.header.compression_tag,"compressed_bytes_consumed":original.compressed_bytes_consumed,
                "original_size":original.bytes.len(),"replacement_count":original.header.remastered_asset_count,
                "format_hint":format_hint,"bar":bar,"replacement_samples":replacements,
                "sha256":format!("{:x}",Sha256::digest(&original.bytes))}));
        }
        checked.verify_unchanged()?;
    }
    let count = samples.len();
    let result = serde_json::json!({"schema_version":1,"source_content_identity":source.content_identity(),
        "qualification":"bounded_original_sample_inspection_only","source_check":"HED content hashes; package size/mtime; full rescan required for mutation assurance",
        "selection_policy":"first two originals per storage category per index, with their first two replacements; any decoding error aborts",
        "original_asset_files_exported":false,"samples":samples});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&serde_json::to_vec_pretty(&result)?)?;
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
