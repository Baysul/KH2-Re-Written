//! Guarded original BAR inspection for a single explicit source-bound selection.
use package_reader::{bar, original, preferences, probe::ProbeSource};
use serde::Deserialize;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
};
#[derive(Deserialize)]
struct Selection {
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
}
#[derive(Deserialize)]
struct SelectionFile {
    schema_version: u32,
    source_content_identity: String,
    selections: Vec<Selection>,
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 5 {
        return Err("usage: archive_probe <Steam-root> <manifest> <selection> <new-report> <new-private-original>".into());
    }
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let report = source.output_path(&args[3])?;
    let payload = source.output_path(&args[4])?;
    if report.exists() || payload.exists() {
        return Err("archive outputs must be new".into());
    }
    let mut bytes = Vec::new();
    File::open(&args[2])?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        return Err("archive selection exceeds limit".into());
    }
    let selected: SelectionFile = serde_json::from_slice(&bytes)?;
    if selected.schema_version != 1
        || selected.source_content_identity != source.content_identity()
        || selected.selections.len() != 1
    {
        return Err("one source-bound archive selection required".into());
    }
    source.verify_full_source()?;
    let item = &selected.selections[0];
    let mut checked = source.open_index(&item.index)?;
    let entry = checked
        .inventory
        .entries
        .get(item.ordinal)
        .filter(|e| e.name_hash == item.name_hash)
        .ok_or("archive source entry mismatch")?;
    let original = original::read_original(&mut checked.reader, entry)?;
    if original.bytes.len() > 32 * 1024 * 1024 {
        return Err("archive exceeds bounded output profile".into());
    }
    let table = bar::read_bar(&original.bytes)?;
    let mut pref_tables = Vec::new();
    for entry in &table.entries {
        if entry.entry_type != 1 || entry.name_bytes != *b"pref" || entry.offset < 0 {
            continue;
        }
        if !pref_tables.is_empty() || entry.size > 1_048_576 {
            return Err("preferences entry ambiguous or exceeds bounded profile".into());
        }
        let start = entry.offset as usize;
        let data = &original.bytes[start..start + entry.size as usize];
        let inventory = bar::read_bar(data)?;
        if inventory.entries.len() > 64 {
            return Err("preferences inventory exceeds bounded profile".into());
        }
        let mut prty_tables = Vec::new();
        for item in &inventory.entries {
            if item.entry_type == 2 && item.name_bytes == *b"prty" && item.offset >= 0 {
                if !prty_tables.is_empty() {
                    return Err("PRTY entry ambiguous".into());
                }
                let start = item.offset as usize;
                prty_tables.push(preferences::read_prty(
                    &data[start..start + item.size as usize],
                )?);
            }
        }
        pref_tables.push(serde_json::json!({"inventory":inventory,"prty_tables":prty_tables}));
    }
    checked.verify_unchanged()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(payload)?;
    file.write_all(&original.bytes)?;
    file.sync_all()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(report)?;
    serde_json::to_writer_pretty(
        &mut file,
        &serde_json::json!({"schema_version":1,"source_content_identity":source.content_identity(),"index":item.index,"ordinal":item.ordinal,"bar":table,"preferences":pref_tables,"gameplay_verified":false}),
    )?;
    writeln!(file)?;
    file.sync_all()?;
    println!("Selected BAR retained privately; gameplay semantics unqualified.");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Archive probe rejected: {e}");
        std::process::exit(1);
    }
}
