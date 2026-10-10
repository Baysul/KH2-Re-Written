//! One launcher for the rewrite's programs. Started without arguments (a
//! double-click) it lists them and runs the chosen one with its private
//! launch file; `kh2fm title|room|talk [arguments]` runs one directly with
//! the program's own arguments (none = its launch file).
mod test_room;
mod title;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

/// (command, menu label).
const PROGRAMS: [(&str, &str); 3] = [
    ("title", "Title screen"),
    ("room", "Test room: arena (combat, camp menu, music)"),
    ("talk", "Test room: talk (Axel, dialogue)"),
];

fn choose() -> Option<&'static str> {
    let stdin = io::stdin();
    loop {
        println!("KH2FM rewrite");
        for (i, (_, label)) in PROGRAMS.iter().enumerate() {
            println!("  {}  {label}", i + 1);
        }
        print!("Open which (1-{}, Enter quits)? ", PROGRAMS.len());
        let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        match line.parse::<usize>() {
            Ok(n) if (1..=PROGRAMS.len()).contains(&n) => return Some(PROGRAMS[n - 1].0),
            _ => println!("Type a number from the list.\n"),
        }
    }
}

fn run(program: &str, args: Vec<PathBuf>, interactive: bool) -> ExitCode {
    match program {
        "title" => title::main(args, interactive),
        "room" => test_room::main(args, interactive),
        // The talk room's launch file, unless arguments are given.
        "talk" if args.is_empty() => test_room::main(vec!["talk".into()], interactive),
        "talk" => test_room::main(args, interactive),
        _ => {
            eprintln!("usage: kh2fm [title|room|talk] [program arguments]");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let mut args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.is_empty() {
        return match choose() {
            Some(program) => run(program, Vec::new(), true),
            None => ExitCode::SUCCESS,
        };
    }
    let program = args.remove(0);
    run(&program.to_string_lossy(), args, false)
}
