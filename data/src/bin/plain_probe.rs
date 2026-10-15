use data::plain::read_plain_original;
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: plain_probe <selected-install-root> <private-source-manifest.json> <new-private-report.json>".into());
    }
    let source =
        data::probe::ProbeSource::bind(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    let output = source.output_path(&PathBuf::from(&args[2]))?;
    let mut samples = Vec::new();
    for (relative, _) in source::required_files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".hed"))
    {
        let mut checked = source.open_index(&relative)?;
        for (ordinal, entry) in checked.inventory.entries.iter().enumerate() {
            use std::io::{Seek, SeekFrom};
            checked.reader.seek(SeekFrom::Start(entry.package_offset))?;
            let mut header = [0; 16];
            checked.reader.read_exact(&mut header)?;
            let header = data::read_asset_header(&header, entry.stored_length)?;
            if header.compression_tag != -2 {
                continue;
            }
            let original = read_plain_original(&mut checked.reader, entry)?;
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
            samples.push(serde_json::json!({"index":relative,"ordinal":ordinal,
                "package_offset":entry.package_offset,"name_hash":entry.name_hash,
                "original_size":original.bytes.len(),"replacement_count":original.header.remastered_asset_count,
                "format_hint":format_hint,"bar":bar,"sha256":format!("{:x}",Sha256::digest(&original.bytes))}));
        }
        checked.verify_unchanged()?;
    }
    let count = samples.len();
    let result = serde_json::json!({"schema_version":1,"source_content_identity":source.content_identity(),
        "qualification":"plain_original_inspection_only","source_check":"HED content hashes; package size/mtime; full rescan required for mutation assurance",
        "original_asset_files_exported":false,"samples":samples});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&serde_json::to_vec_pretty(&result)?)?;
    file.sync_all()?;
    println!(
        "Inspected {count} tag -2 originals; metadata/hashes saved; no original asset files exported."
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Plain inspection failed: {error}");
        std::process::exit(1);
    }
}
