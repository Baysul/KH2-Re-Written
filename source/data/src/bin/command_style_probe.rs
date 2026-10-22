//! Lists the field command layouts the install holds
//! (field2d/<lang>/<world><n>command.2dd, FUN_140153ee0) with each one's
//! BAR entries and SEQD counts, and the groups the command menu draws, so a
//! layout whose numbering differs from the code's tables is visible.
//! Prints counts and sprite geometry only, never image bytes.
use data::{bar, path_lookup::Library, probe_source::ProbeSource, sprite_sequence};
use std::path::PathBuf;

const WORLDS: [&str; 19] = [
    "zz", "es", "tt", "di", "hb", "bb", "he", "al", "mu", "po", "lk", "lm", "dc", "wi", "nm", "wm",
    "ca", "tr", "eh",
];
const IMGD: u16 = 0x18;
const SEQD: u16 = 0x19;
/// Groups the menu code names (command-01 section 8 tables).
const GROUPS: [usize; 30] = [
    2, 6, 7, 9, 10, 11, 16, 17, 18, 19, 20, 21, 33, 34, 35, 36, 37, 45, 53, 54, 55, 56, 70, 71, 72,
    73, 74, 77, 86, 87,
];

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err(
            "usage: command_style_probe <Steam-root> <manifest> <language, e.g. us>".into(),
        );
    }
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let lang = args[2].to_string_lossy().into_owned();
    let mut library = Library::open(&source)?;
    for w in WORLDS {
        for n in 0..10 {
            let name = format!("field2d/{lang}/{w}{n}command.2dd");
            let Some(found) = library.find(&name) else {
                continue;
            };
            let bytes = library.read_found(&found)?;
            let table = bar::read_bar(&bytes)?;
            let entries: Vec<String> = table
                .entries
                .iter()
                .map(|e| {
                    format!(
                        "{:#x}:{}:{}",
                        e.entry_type,
                        String::from_utf8_lossy(&e.name_bytes).trim_end_matches('\0'),
                        e.size
                    )
                })
                .collect();
            println!("{name} [{}] {}", found.index, entries.join(" "));
            let seq = table.entries.iter().find(|e| e.entry_type == SEQD);
            let img = table.entries.iter().find(|e| e.entry_type == IMGD);
            if let (Some(s), Some(_)) = (seq, img) {
                let seq = sprite_sequence::read(
                    &bytes[s.offset as usize..s.offset as usize + s.size as usize],
                )?;
                println!(
                    "  groups {} animations {} sprite groups {} parts {} sprites {}",
                    seq.groups.len(),
                    seq.animations.len(),
                    seq.sprite_groups.len(),
                    seq.parts.len(),
                    seq.sprites.len()
                );
                for g in GROUPS {
                    let Some(group) = seq.groups.get(g) else {
                        println!("  g{g}: absent");
                        continue;
                    };
                    let first = seq.animations.get(group.animations.start);
                    println!(
                        "  g{g}: anims {} play {} loop {:?} light {} first T {:?} frames {:?} colour {:08x?}",
                        group.animations.len(),
                        group.play_count,
                        group.loop_frames,
                        group.light_position_x,
                        first.map(|a| a.translate),
                        first.map(|a| a.frames),
                        first.map(|a| a.color),
                    );
                }
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
