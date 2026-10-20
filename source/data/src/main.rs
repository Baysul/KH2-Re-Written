use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(
            "usage: data <selected-Steam-install-root> <new-private-inventory.json>".into(),
        );
    }
    let root = fs::canonicalize(PathBuf::from(&args[0]))?;
    let output = validation::safe_output(&root, &PathBuf::from(&args[1]))?;
    let mut inventories = serde_json::Map::new();
    for (relative, _) in validation::required_files_in(validation::detect_package_folder(&root)?)
        .into_iter()
        .filter(|(name, _)| name.ends_with(".hed"))
    {
        let path = fs::canonicalize(root.join(&relative))?;
        let pkg = fs::canonicalize(path.with_extension("pkg"))?;
        if !path.starts_with(&root) || !pkg.starts_with(&root) {
            return Err("package link escapes source".into());
        }
        let mut bytes = Vec::new();
        File::open(&path)?
            .take((data::MAX_INDEX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let mut package = File::open(pkg)?;
        let inventory = data::read_hed(&bytes, package.metadata()?.len())?;
        let mut headers = Vec::new();
        let mut remastered = 0u64;
        let mut compressed = 0u64;
        let mut encrypted_tag = 0u64;
        for entry in &inventory.entries {
            if entry.stored_length < 16 {
                return Err("indexed asset too short for package header".into());
            }
            package.seek(SeekFrom::Start(entry.package_offset))?;
            let mut bytes = [0; 16];
            package.read_exact(&mut bytes)?;
            let header = data::read_asset_header(&bytes, entry.stored_length)?;
            remastered += header.remastered_asset_count as u64;
            compressed += u64::from(header.compression_tag >= 0);
            encrypted_tag += u64::from(header.compression_tag > -2);
            headers.push(header);
        }
        println!(
            "{}: records={}, duplicate hashes={}, unaligned offsets={}, overlaps={}",
            relative,
            inventory.record_count,
            inventory.duplicate_name_hashes,
            inventory.unaligned_offsets,
            inventory.overlapping_ranges
        );
        println!(
            "header metadata: remastered entries={remastered}, compressed tags={compressed}, encrypted tags={encrypted_tag}; payloads not decoded"
        );
        inventories.insert(
            relative,
            serde_json::json!({"index":inventory,"headers":headers}),
        );
    }
    let result = serde_json::json!({"schema_version":1,"qualification":"inspection_only_unqualified","openkh_layout_commit":"7a3b945c538d32c6a285128c98aefba093f52ceb","indexes":inventories});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&serde_json::to_vec_pretty(&result)?)?;
    file.sync_all()?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Package inspection failed: {error}");
        std::process::exit(1);
    }
}
