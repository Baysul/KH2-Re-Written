//! Private, source-bound survey of every object-model script (BAR type 3) and the
//! native traps it can reach. A coverage denominator, not a behavior claim.
use data::{
    bar, bdscript, enemy_actions, objentry, path_lookup::Library, probe_source::ProbeSource,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 4 {
        return Err("usage: enemy_script_survey <Steam-root> <manifest> <new-private-report> <new-private-script-dir>".into());
    }
    let source = ProbeSource::bind_session(&args[0], &args[1])?;
    let output = source.output_path(&args[2])?;
    if output.exists() {
        return Err("report must be new".into());
    }
    fs::create_dir_all(&args[3])?;
    source.verify_full_source()?;
    let mut library = Library::open(&source)?;
    let objects = objentry::read(&library.read("00objentry.bin")?)?;
    let mut models: BTreeMap<Vec<u8>, Vec<(u32, u8)>> = BTreeMap::new();
    for o in &objects {
        models
            .entry(o.model_name.clone())
            .or_default()
            .push((o.object_id, o.object_type));
    }
    let mut rows = Vec::new();
    let mut trap_users: BTreeMap<(u16, u16), BTreeSet<String>> = BTreeMap::new();
    let mut failures = Vec::new();
    for (model, users) in &models {
        let model_name = String::from_utf8_lossy(model).to_string();
        if model_name.is_empty() {
            continue;
        }
        let path = format!("obj/{model_name}.mdlx");
        let bytes = match library.read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let Ok(table) = bar::read_bar(&bytes) else {
            continue;
        };
        for (index, entry) in table.entries.iter().enumerate() {
            if entry.entry_type != 3 || entry.offset < 0 || entry.size == 0 {
                continue;
            }
            let start = entry.offset as usize;
            let body = &bytes[start..start + entry.size as usize];
            let dump = args[3].join(format!("{model_name}.{index}.bdx"));
            if !dump.exists() {
                fs::write(&dump, body)?;
            }
            let program = match bdscript::Program::read(body) {
                Ok(p) => p,
                Err(e) => {
                    failures.push(json!({"model":model_name,"error":e.to_string()}));
                    continue;
                }
            };
            // Linear decode from the code start until the first undecodable word.
            let mut at = program.code_start;
            let mut traps = BTreeSet::new();
            let mut count = 0u32;
            while let Ok(i) = program.instruction(at) {
                if let Some(t) = i.trap() {
                    traps.insert(t);
                }
                at += i.words;
                count += 1;
                if count > 400_000 {
                    break;
                }
            }
            let actions = enemy_actions::inspect(body).ok();
            for t in &traps {
                trap_users.entry(*t).or_default().insert(model_name.clone());
            }
            rows.push(json!({
                "model":model_name,"objects":users,"bar_entry":index,
                "program":String::from_utf8_lossy(program.name),
                "size":entry.size,"work":program.work_bytes,"stack":program.stack_bytes,
                "temp":program.temp_bytes,"triggers":program.triggers,
                "linear_instructions":count,"linear_end_word":at,
                "traps":traps.iter().map(|t| format!("{}:{}",t.0,t.1)).collect::<Vec<_>>(),
                "actions":actions.map(|a| a.actions.iter().map(|x| String::from_utf8_lossy(&x.name).to_string()).collect::<Vec<_>>()),
            }));
        }
    }
    let histogram: Vec<_> = trap_users
        .iter()
        .map(|(t, m)| json!({"trap":format!("{}:{}",t.0,t.1),"models":m.len()}))
        .collect();
    let report = json!({"schema_version":1,"source_content_identity":source.content_identity(),
        "scripts":rows,"trap_histogram":histogram,"failures":failures,
        "coverage":"linear decode from the code start; the trap set may include a few words of string pool"});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!(
        "{} scripts, {} distinct traps, {} failures",
        rows.len(),
        trap_users.len(),
        failures.len()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("enemy script survey refused: {error}");
        std::process::exit(1);
    }
}
