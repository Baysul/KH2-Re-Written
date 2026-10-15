//! Read-only, source-bound ability/progression table evidence for private research.
use data::{bar, named::Library, probe::ProbeSource};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

fn entry<'a>(bytes: &'a [u8], name: &[u8; 4]) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let table = bar::read_bar(bytes)?;
    let e = table
        .entries
        .iter()
        .find(|e| &e.name_bytes == name)
        .ok_or("missing BAR entry")?;
    let start = usize::try_from(e.offset)?;
    let size = usize::try_from(e.size)?;
    bytes
        .get(start..start.checked_add(size).ok_or("BAR range overflow")?)
        .ok_or_else(|| "BAR range outside file".into())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err("usage: ability_probe <Steam-root> <manifest> <new-private-directory>".into());
    }
    // Use an uncached research binding so the closing verification rehashes too.
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let directory = source.output_path(&args[2])?;
    if directory.exists() {
        return Err("output directory must be new".into());
    }
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let system = library.read("03system.bin")?;
    let battle = library.read("00battle.bin")?;
    let pref = entry(&system, b"pref")?;
    let tables = [
        ("item", entry(&system, b"item")?),
        ("sstm", entry(pref, b"sstm")?),
        ("fmab", entry(pref, b"fmab")?),
        ("ptya", entry(&battle, b"ptya")?),
        ("fmlv", entry(&battle, b"fmlv")?),
        ("lvup", entry(&battle, b"lvup")?),
        ("plrp", entry(&battle, b"plrp")?),
    ];
    let records: Vec<_> = tables
        .iter()
        .map(|(name, bytes)| {
            json!({
                "name":name, "bytes":bytes.len(), "sha256":format!("{:x}", Sha256::digest(bytes))
            })
        })
        .collect();
    let locations: Vec<_> = ["03system.bin", "00battle.bin"].into_iter().map(|name| {
        let all: Vec<_> = library.find_all(name).into_iter().map(|f| json!({"index":f.index,"ordinal":f.ordinal,"name_hash":f.name_hash})).collect();
        json!({"name":name,"candidates":all,"selection":"last package; existing inspection profile, runtime routing unqualified"})
    }).collect();
    source.verify_full_source()?;
    std::fs::create_dir(&directory)?;
    for (name, bytes) in tables {
        let path = source.output_path(&directory.join(format!("{name}.bin")))?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    let report = json!({"schema_version":1,"source_content_identity":source.content_identity(),
        "game":"KH2FM","tables":records,"archives":locations,
        "qualification":"quarantined read-only table evidence; no gameplay equivalence"});
    let path = source.output_path(&directory.join("tables.json"))?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!("Retained seven source-bound ability/progression tables in private storage.");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ability probe refused: {error}");
        std::process::exit(1);
    }
}
