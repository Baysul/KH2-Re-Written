use package_reader::{
    bar::read_bar, message::read_messages, original::read_original, probe::ProbeSource,
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
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: message_probe <source-root> <private-manifest> <private-selection> <new-private-report>".into());
    }
    let source = ProbeSource::bind(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    let output = source.output_path(&PathBuf::from(&args[3]))?;
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
    let indexes: Vec<_> = source_identity::required_files()
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
            for (ordinal, child) in bar.entries.iter().enumerate() {
                if child.offset < 0 {
                    continue;
                }
                let data = &original.bytes
                    [child.offset as usize..child.offset as usize + child.size as usize];
                children.push(serde_json::json!({"bar_ordinal":ordinal,"sha256":format!("{:x}",Sha256::digest(data))}));
                // List + Msg magic is a structural lead only, restricted to selected message containers.
                if item.purpose == "message"
                    && child.entry_type == 2
                    && data.starts_with(&1u32.to_le_bytes())
                {
                    message_tables.push(serde_json::json!({"bar_ordinal":ordinal,
                        "bar_payload_offset":child.offset,"bar_payload_size":child.size,
                        "inventory":read_messages(data)?}));
                }
            }
            if item.purpose == "message" && message_tables.is_empty() {
                return Err("selected message container has no recognized message table".into());
            }
            samples.push(serde_json::json!({"index":relative,"ordinal":item.ordinal,"candidate_name":item.candidate_name,
                "candidate_name_semantics_verified":false,"purpose":item.purpose,"name_hash":entry.name_hash,
                "package_offset":entry.package_offset,"compression_tag":original.header.compression_tag,
                "original_size":original.bytes.len(),"sha256":format!("{:x}",Sha256::digest(&original.bytes)),
                "replacement_count":original.header.remastered_asset_count,"bar":bar,"child_hashes":children,"message_tables":message_tables}));
        }
        checked.verify_unchanged()?;
    }
    let count = samples.len();
    let result = serde_json::json!({"schema_version":1,"source_content_identity":source.content_identity(),
        "qualification":"message_font_structure_inspection_only","source_check":"HED hashes and package size/mtime; full rescan required for mutation assurance",
        "asset_files_exported":false,"text_decoded_to_unicode":false,"font_pixels_rendered":false,"samples":samples});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&serde_json::to_vec_pretty(&result)?)?;
    file.sync_all()?;
    println!("Inspected {count} selected message/font containers; private spans/hashes only.");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Message/font inspection failed: {error}");
        std::process::exit(1);
    }
}
