//! Read-only, source-bound evidence for every Keyblade, armor and accessory:
//! the item record, its stat row and granted ability, its picture file, and
//! for a Keyblade its WENT-resolved weapon object, model, motion set and
//! attack collision. Writes one JSON report to a new private directory.
use data::{
    bar, item_table, model_inspection, motion, object_collision, objentry, path_lookup::Library,
    probe_source::ProbeSource, weapon_entry,
};
use serde_json::json;
use std::{fs::OpenOptions, io::Write, path::PathBuf};

type Fail = Box<dyn std::error::Error>;

fn entry<'a>(bytes: &'a [u8], name: &[u8; 4]) -> Result<&'a [u8], Fail> {
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

fn typed(bytes: &[u8], kind: u16) -> Option<&[u8]> {
    let table = bar::read_bar(bytes).ok()?;
    let e = table
        .entries
        .iter()
        .find(|e| e.entry_type == kind && e.offset > 0 && e.size > 0)?;
    bytes.get(e.offset as usize..e.offset as usize + e.size as usize)
}

fn run() -> Result<(), Fail> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err(
            "usage: equipment_probe <Steam-root> <manifest> <new-private-directory>".into(),
        );
    }
    // A session binding: the executable and indexes are rehashed once, then
    // every package byte read is verified against the full-scan record.
    let source = ProbeSource::bind_session(&args[0], &args[1])?;
    let directory = source.output_path(&args[2])?;
    if directory.exists() {
        return Err("output directory must be new".into());
    }
    source.verify_session()?;
    let mut library = Library::open(&source)?;
    let system = library.read("03system.bin")?;
    let item_bytes = entry(&system, b"item")?;
    let items = item_table::read(item_bytes)?;
    let stats = item_table::read_stats(item_bytes)?;
    let went = entry(&system, b"went")?.to_vec();
    let objects = objentry::read(&library.read("00objentry.bin")?)?;
    let player = objects
        .iter()
        .find(|o| o.model_name == b"P_EX100")
        .ok_or("P_EX100 object entry absent")?;
    let set = weapon_entry::set(&went, player.neo_moveset as usize)?;
    let ability_items: std::collections::HashMap<u16, (u8, u16, u8, u8)> = items
        .iter()
        .filter(|i| (0x12..=0x14).contains(&i.kind))
        .map(|i| {
            (
                i.id,
                (
                    i.kind,
                    i.amount,
                    (i.weapon_stats & 0xff) as u8,
                    (i.weapon_stats >> 8) as u8,
                ),
            )
        })
        .collect();
    let idle = {
        let found = library
            .find("obj/W_EX010.mset")
            .ok_or("obj/W_EX010.mset is not installed")?;
        motion::read_motion_ids(
            &source,
            &motion::MotionSetSelection {
                index: found.index.to_owned(),
                ordinal: found.ordinal,
                name_hash: found.name_hash,
                battle: true,
                has_weapon: true,
            },
            &[0],
        )
        .ok()
        .and_then(|mut m| m.pop())
    };
    let mut rows = Vec::new();
    for item in items.iter().filter(|i| matches!(i.kind, 2 | 14 | 15)) {
        let stat = item
            .equipment_stats_id()
            .and_then(|k| stats.iter().find(|s| s.id == k));
        let picture = format!("itempic/item-{:03}.imd", item.picture);
        let mut row = json!({
            "id": item.id, "kind": item.kind, "amount": item.amount,
            "weapon_stats": item.weapon_stats, "name": item.name,
            "description": item.description, "picture": item.picture,
            "picture_installed": library.find(&picture).is_some(),
            "stat": stat.map(|s| json!({
                "id": s.id, "ability": s.ability, "strength": s.strength,
                "magic": s.magic, "defense": s.defense, "ap": s.ap,
                "resist": s.resist,
            })),
            "ability": stat.and_then(|s| ability_items.get(&s.ability)).map(|a| json!({
                "item_kind": a.0, "bit": a.1, "class": a.2, "ap": a.3,
            })),
        });
        if item.kind == 2 {
            let object_id = (item.amount as usize)
                .checked_sub(1)
                .and_then(|i| set.get(i))
                .copied();
            let object = object_id.and_then(|id| objects.iter().find(|o| o.object_id == id));
            let mut weapon = json!({"object_id": object_id});
            if let Some(o) = object {
                let model = String::from_utf8_lossy(&o.model_name).into_owned();
                let mset = String::from_utf8_lossy(&o.animation_name).into_owned();
                weapon["model"] = json!(model);
                weapon["mset"] = json!(mset);
                weapon["weapon_joint"] = json!(o.weapon_joint);
                let path = format!("obj/{model}.mdlx");
                weapon["mdlx_installed"] = json!(library.find(&path).is_some());
                weapon["mset_installed"] = json!(library.find(&format!("obj/{mset}")).is_some());
                if let Some(found) = library.find(&path) {
                    let archive = library.read_found(&found)?;
                    let collision = typed(&archive, 23)
                        .and_then(|b| object_collision::read(b).ok())
                        .unwrap_or_default();
                    let attack: Vec<_> = collision
                        .iter()
                        .filter(|e| e.kind == object_collision::ATTACK)
                        .map(|e| {
                            json!({"group": e.group, "bone": e.bone, "position": e.position,
                                "radius": e.radius})
                        })
                        .collect();
                    weapon["attack_collision"] = json!(attack);
                    match model_inspection::read_model_at(
                        &source,
                        found.index,
                        found.ordinal,
                        found.name_hash,
                    ) {
                        Ok(mut models) if !models.is_empty() => {
                            let m = models.remove(0);
                            weapon["parts"] = json!(m.parts.len());
                            weapon["bones"] = json!(m.skinning.bones.len());
                            weapon["idle_motion_samples"] = json!(
                                idle.as_ref()
                                    .map(|i| m.skinning.sample_evaluated(&i.motion, 1.0).is_ok())
                            );
                        }
                        Ok(_) => weapon["model_error"] = json!("no model in the archive"),
                        Err(e) => weapon["model_error"] = json!(e.to_string()),
                    }
                }
            }
            row["weapon"] = weapon;
        }
        rows.push(row);
    }
    source.verify_session()?;
    std::fs::create_dir(&directory)?;
    let report = json!({"schema_version": 1, "game": "KH2FM",
        "source_content_identity": source.content_identity(),
        "player_object": {"id": player.object_id, "neo_moveset": player.neo_moveset,
            "went_set_len": set.len()},
        "player_objects": objects.iter()
            .filter(|o| o.model_name.starts_with(b"P_EX100"))
            .map(|o| json!({"id": o.object_id, "model": String::from_utf8_lossy(&o.model_name),
                "form": o.form, "neo_moveset": o.neo_moveset, "character": o.character,
                "weapon_joint": o.weapon_joint}))
            .collect::<Vec<_>>(),
        "items": rows,
        "qualification": "quarantined read-only table evidence; no gameplay equivalence"});
    let path = source.output_path(&directory.join("equipment.json"))?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!("Retained the equipment table evidence in private storage.");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("equipment probe refused: {error}");
        std::process::exit(1);
    }
}
