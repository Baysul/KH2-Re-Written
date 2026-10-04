use package_reader::{plain::read_plain_original, read_hed};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: plain_probe <selected-install-root> <private-source-manifest.json> <new-private-report.json>".into());
    }
    let root = fs::canonicalize(PathBuf::from(&args[0]))?;
    let output = source_identity::safe_output(&root, &PathBuf::from(&args[2]))?;
    let mut manifest_bytes = Vec::new();
    File::open(&args[1])?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut manifest_bytes)?;
    if manifest_bytes.len() > 1024 * 1024 {
        return Err("source manifest exceeds inspection limit".into());
    }
    let manifest: source_identity::SourceManifest = serde_json::from_slice(&manifest_bytes)?;
    if manifest.schema_version != 1
        || manifest.canonical_source_path != root
        || source_identity::content_identity(&manifest.files) != manifest.content_identity
    {
        return Err("source manifest identity/path/schema is inconsistent".into());
    }
    let mut samples = Vec::new();
    for (relative, _) in source_identity::required_files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".hed"))
    {
        let expected = manifest
            .files
            .iter()
            .find(|f| f.relative_path == relative)
            .ok_or("index not in source manifest")?;
        let observed = source_identity::inspect_file(&root, &relative, "index_check")?;
        if observed.sha256 != expected.sha256
            || observed.size != expected.size
            || observed.modified_unix_nanos != expected.modified_unix_nanos
        {
            return Err("index differs from bound source manifest".into());
        }
        let hed = fs::canonicalize(root.join(&relative))?;
        let pkg = fs::canonicalize(hed.with_extension("pkg"))?;
        if !pkg.starts_with(&root) {
            return Err("package escapes source".into());
        }
        let pkg_relative = relative.replace(".hed", ".pkg");
        let expected = manifest
            .files
            .iter()
            .find(|f| f.relative_path == pkg_relative)
            .ok_or("package not in source manifest")?;
        let before = fs::metadata(&pkg)?;
        let timestamp = |metadata: &fs::Metadata| -> Result<u128, Box<dyn std::error::Error>> {
            Ok(metadata
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos())
        };
        if before.len() != expected.size || timestamp(&before)? != expected.modified_unix_nanos {
            return Err("package metadata differs from source manifest; rescan fully".into());
        }
        let mut index_bytes = Vec::new();
        File::open(hed)?
            .take((package_reader::MAX_INDEX_BYTES + 1) as u64)
            .read_to_end(&mut index_bytes)?;
        let inventory = read_hed(&index_bytes, before.len())?;
        let mut reader = File::open(&pkg)?;
        for (ordinal, entry) in inventory.entries.iter().enumerate() {
            use std::io::{Seek, SeekFrom};
            reader.seek(SeekFrom::Start(entry.package_offset))?;
            let mut header = [0; 16];
            reader.read_exact(&mut header)?;
            let header = package_reader::read_asset_header(&header, entry.stored_length)?;
            if header.compression_tag != -2 {
                continue;
            }
            let original = read_plain_original(&mut reader, entry)?;
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
                "original_size":original.bytes.len(),"replacement_count":original.header.remastered_asset_count,
                "format_hint":format_hint,"bar":bar,"sha256":format!("{:x}",Sha256::digest(&original.bytes))}));
        }
        let after = fs::metadata(pkg)?;
        if after.len() != before.len() || after.modified()? != before.modified()? {
            return Err("package changed during plain inspection".into());
        }
    }
    let count = samples.len();
    let result = serde_json::json!({"schema_version":1,"source_content_identity":manifest.content_identity,
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
