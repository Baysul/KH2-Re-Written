//! Private, source-bound inventory of every Twilight Town spawn-set entity.
//! This is a coverage denominator, not a claim that every set is reachable.
use package_reader::{ard, bar, bdscript, named::Library, objentry, probe::ProbeSource};
use serde_json::json;
use std::{collections::BTreeSet, fs::OpenOptions, io::Write, path::PathBuf};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err("usage: encounter_probe <Steam-root> <manifest> <new-private-report>".into());
    }
    let source = ProbeSource::bind_session(&args[0], &args[1])?;
    let output = source.output_path(&args[2])?;
    if output.exists() {
        return Err("report must be new".into());
    }
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let objects = objentry::read(&library.read("00objentry.bin")?)?;
    let mut rows = Vec::new();
    let mut ids = BTreeSet::new();
    let mut areas = Vec::new();
    for area in 0..42 {
        let name = format!("ard/us/tt{area:02}.ard");
        let bytes = library.read(&name)?;
        let table = bar::read_bar(&bytes)?;
        let mut sets = 0;
        for (entry_index, entry) in table.entries.iter().enumerate() {
            if entry.entry_type != 12 || entry.offset < 0 {
                continue;
            }
            sets += 1;
            let start = entry.offset as usize;
            let set = ard::spawn_set(&bytes[start..start + entry.size as usize])?;
            for (point_index, point) in set.points.iter().enumerate() {
                for (entity_index, entity) in point.entities.iter().enumerate() {
                    ids.insert(entity.object);
                    rows.push(json!({
                        "area":area,"bar_entry":entry_index,"set":entry.name_bytes,
                        "point":point_index,"point_id":point.id,"point_kind":point.kind,
                        "entity":entity_index,"object":entity.object,
                        "spawn_type":entity.spawn_type,"spawn_arg":entity.spawn_arg,
                        "serial":entity.serial,"args":[entity.arg1,entity.arg2]
                    }));
                }
            }
        }
        areas.push(json!({"area":area,"sets":sets}));
    }
    let mut scripts = Vec::new();
    let models: BTreeSet<_> = objects
        .iter()
        .filter(|o| ids.contains(&o.object_id) && matches!(o.object_type, 4 | 21))
        .map(|o| o.model_name.clone())
        .collect();
    for model in models {
        let name = format!("obj/{}.mdlx", std::str::from_utf8(&model)?);
        let bytes = library.read(&name)?;
        for (index, entry) in bar::read_bar(&bytes)?.entries.iter().enumerate() {
            if entry.entry_type != 3 || entry.offset < 0 || entry.size == 0 {
                continue;
            }
            let start = entry.offset as usize;
            let program = bdscript::Program::read(&bytes[start..start + entry.size as usize])?;
            let graph = program.direct_graph();
            scripts.push(json!({"model":name,"bar_entry":index,
                "name":program.name,"work_bytes":program.work_bytes,
                "stack_bytes":program.stack_bytes,"temp_bytes":program.temp_bytes,
                "triggers":program.triggers,
                "direct_graph":graph.as_ref().ok(),
                "error":graph.as_ref().err().map(ToString::to_string),
                "coverage":"direct trigger/call graph only; indirect act callbacks not qualified"
            }));
        }
    }
    let resolved: Vec<_> = ids
        .iter()
        .map(|id| {
            json!({
                "id":id,"entry":objects.iter().find(|o| o.object_id == *id)
            })
        })
        .collect();
    let report = json!({"schema_version":1,"source_content_identity":source.content_identity(),
        "scope":"all type-12 records in 42 English Twilight Town ARDs; reachability unqualified",
        "areas":areas,"objects":resolved,"placements":rows,"scripts":scripts});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!(
        "Inventoried {} placements and {} distinct object ids across 42 areas",
        rows.len(),
        ids.len()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("encounter probe refused: {error}");
        std::process::exit(1);
    }
}
