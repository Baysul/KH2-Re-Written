//! One launcher for the rewrite's programs. Started without arguments (a
//! double-click) it opens the launcher window: choose the game folder, then
//! a test. `kh2fm title|room|talk|play|tt [arguments]` runs one directly with
//! the program's own arguments (none = its launch file; `play` is the room
//! with both the Shadow and Axel).
//!
//! A Windows-subsystem program, so no console window opens behind the
//! launcher; output still reaches a pipe or redirect (the launcher reads
//! its tests' output that way).
#![cfg_attr(windows, windows_subsystem = "windows")]
mod area;
mod cutscene;
mod launcher;
mod npcs;
mod skate;
mod skate_rig;
mod test_room;
mod title;

use std::path::PathBuf;
use std::process::ExitCode;

fn run(program: &str, args: Vec<PathBuf>) -> ExitCode {
    match program {
        "title" => title::main(args, false),
        "room" => test_room::main(args, false),
        // The talk room's launch file, unless arguments are given.
        "talk" if args.is_empty() => test_room::main(vec!["talk".into()], false),
        "talk" => test_room::main(args, false),
        "play" if args.is_empty() => match launcher::play_args() {
            Ok(args) => test_room::main(args, false),
            Err(e) => {
                eprintln!("Test room rejected: {e}");
                ExitCode::FAILURE
            }
        },
        "play" => test_room::main(args, false),
        "tt" if args.is_empty() => match launcher::tt_args() {
            Ok(args) => test_room::main(args, false),
            Err(e) => {
                eprintln!("Twilight Town rejected: {e}");
                ExitCode::FAILURE
            }
        },
        "tt" => test_room::main(args, false),
        "event" => cutscene::main(args, false),
        _ => {
            eprintln!("usage: kh2fm [title|room|talk|play|tt|event] [program arguments]");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let mut args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.is_empty() {
        return launcher::main();
    }
    let program = args.remove(0);
    run(&program.to_string_lossy(), args)
}
