use data::{bar, motion, motion_trigger, original, probe::ProbeSource};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
};
#[derive(Deserialize)]
struct Selection {
    schema_version: u32,
    source_content_identity: String,
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
    slots: Vec<usize>,
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 5 {
        return Err("usage: motion_probe <Steam-root> <manifest> <motion-selection> <new-report> <private-payload-dir>".into());
    }
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let output = source.output_path(&args[3])?;
    source.output_path(&args[4].join("admission.bin"))?;
    let mut bytes = Vec::new();
    File::open(&args[2])?
        .take(1048577)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1048576 {
        return Err("motion selection too large".into());
    }
    let selected: Selection = serde_json::from_slice(&bytes)?;
    if selected.schema_version != 1
        || selected.source_content_identity != source.content_identity()
        || selected.slots.is_empty()
        || selected.slots.len() > 8
    {
        return Err("motion selection identity/schema/count invalid".into());
    }
    source.verify_full_source()?;
    let mut checked = source.open_index(&selected.index)?;
    let entry = checked
        .inventory
        .entries
        .get(selected.ordinal)
        .filter(|e| e.name_hash == selected.name_hash)
        .ok_or("MSET source identity mismatch")?;
    let original = original::read_original(&mut checked.reader, entry)?;
    let table = bar::read_bar(&original.bytes)?;
    let mut payload = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(source.output_path(&args[4].join("mset-original.bin"))?)?;
    payload.write_all(&original.bytes)?;
    payload.sync_all()?;
    let mut motions = Vec::new();
    for &slot in &selected.slots {
        let entry = table
            .entries
            .get(slot)
            .filter(|e| e.offset > 0 && e.size > 0)
            .ok_or("selected motion slot absent/dummy")?;
        let anb =
            &original.bytes[entry.offset as usize..entry.offset as usize + entry.size as usize];
        let nested = bar::read_bar(anb)?;
        let matches: Vec<_> = nested
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.entry_type == 9 && e.offset > 0 && e.size > 0)
            .collect();
        if matches.len() != 1 {
            return Err("ANB requires one motion record".into());
        }
        let (bar_ordinal, entry) = matches[0];
        let bytes = &anb[entry.offset as usize..entry.offset as usize + entry.size as usize];
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(source.output_path(&args[4].join(format!("motion-slot-{slot:04}.bin")))?)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        let motion = motion::read_interpolated(bytes)?;
        let triggers = motion_trigger::inspect_anb(anb)?;
        motions.push(serde_json::json!({"slot":slot,"bar":nested,"motion_bar_ordinal":bar_ordinal,"sha256":format!("{:x}",Sha256::digest(bytes)),"motion":motion,"triggers":triggers}));
    }
    checked.verify_unchanged()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    serde_json::to_writer_pretty(
        &mut file,
        &serde_json::json!({"schema_version":1,"source_content_identity":source.content_identity(),"mset_bar":table,
        "original_sha256":format!("{:x}",Sha256::digest(&original.bytes)),"motions":motions,"gameplay_verified":false}),
    )?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!(
        "Decoded {} selected skeletal motions; installed playback remains unqualified.",
        motions.len()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Motion probe rejected: {e}");
        std::process::exit(1);
    }
}
