use package_reader::{
    bar::read_bar, original::read_original, probe::ProbeSource, room_collision, room_geometry,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
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
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if args.len() != 5 && !(args.len() == 6 && args[5].as_os_str() == "--geometry") {
        return Err("usage: room_probe <Steam-root> <manifest> <selection> <new-report> <private-payload-directory>".into());
    }
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let output = source.output_path(&args[3])?;
    let mut bytes = Vec::new();
    File::open(&args[2])?
        .take(1048577)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1048576 {
        return Err("selection too large".into());
    }
    let selection: SelectionFile = serde_json::from_slice(&bytes)?;
    if selection.schema_version != 1
        || selection.source_content_identity != source.content_identity()
        || selection.selections.is_empty()
        || selection.selections.len() > 4
    {
        return Err("room selection schema/identity/count invalid".into());
    }
    source.output_path(&args[4].join("room-admission.bin"))?;
    source.verify_full_source()?;
    let mut samples = Vec::new();
    for (sample_index, item) in selection.selections.iter().enumerate() {
        let mut checked = source.open_index(&item.index)?;
        let entry = checked
            .inventory
            .entries
            .get(item.ordinal)
            .ok_or("room ordinal absent")?;
        if entry.name_hash != item.name_hash {
            return Err("room HED identity mismatch".into());
        }
        let original = read_original(&mut checked.reader, entry)?;
        let bar = read_bar(&original.bytes)?;
        let mut visual_models = Vec::new();
        let mut collisions = Vec::new();
        for (ordinal, record) in bar.entries.iter().enumerate() {
            if args.len() == 6 && record.entry_type == 4 && record.offset > 0 {
                let payload = original
                    .bytes
                    .get(record.offset as usize..record.offset as usize + record.size as usize)
                    .ok_or("visual model span outside BAR")?;
                let model_type = payload
                    .get(0x90..0x94)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
                let model = if model_type == Some(2) {
                    Some(room_geometry::read(payload)?)
                } else {
                    None
                };
                visual_models.push(serde_json::json!({"bar_ordinal":ordinal,"model_type":model_type,"model":model}));
            }
            if record.entry_type != 6 || record.offset <= 0 || record.size == 0 {
                continue;
            }
            let begin = record.offset as usize;
            let payload = original
                .bytes
                .get(begin..begin + record.size as usize)
                .ok_or("collision span outside BAR")?;
            let collision = room_collision::read(payload)?;
            let triangle_indices = collision.triangle_indices()?;
            let path = source.output_path(
                &args[4].join(format!("room-{sample_index:02}-coct-{ordinal:02}.bin")),
            )?;
            let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
            file.write_all(payload)?;
            file.sync_all()?;
            collisions.push(serde_json::json!({"bar_ordinal":ordinal,"sha256":format!("{:x}",Sha256::digest(payload)),"collision":collision,"triangle_indices":triangle_indices}));
        }
        if collisions.is_empty() {
            return Err("selected room has no qualified COCT record".into());
        }
        let path =
            source.output_path(&args[4].join(format!("room-{sample_index:02}-original.bin")))?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(&original.bytes)?;
        file.sync_all()?;
        checked.verify_unchanged()?;
        let mut sample = serde_json::json!({"index":item.index,"ordinal":item.ordinal,"name_hash":item.name_hash,
            "original_sha256":format!("{:x}",Sha256::digest(&original.bytes)),"bar":bar,"collisions":collisions});
        if args.len() == 6 {
            sample["visual_models"] = serde_json::to_value(visual_models)?;
        }
        samples.push(sample);
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    serde_json::to_writer_pretty(
        &mut file,
        &serde_json::json!({"schema_version":1,"source_content_identity":source.content_identity(),
        "profile":"coct_v1_raw_room_records","runtime_collision_rules_verified":false,"samples":samples}),
    )?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!(
        "Decoded {} selected room archive(s) and COCT records; gameplay response remains unqualified.",
        selection.selections.len()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
