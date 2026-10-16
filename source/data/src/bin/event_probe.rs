//! Research probe: decodes one event script from an installed ARD and prints
//! its commands.
use data::{event_script, path_lookup::Library, probe_source::ProbeSource};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        return Err("usage: event_probe <Steam-root> <manifest> <ard name> <event name>".into());
    }
    let source = ProbeSource::bind(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    let mut library = Library::open(&source)?;
    let ard = library.read(args[2].to_str().ok_or("name not UTF-8")?)?;
    let script = event_script::find_in_ard(&ard, args[3].to_str().ok_or("name not UTF-8")?)?;
    for command in event_script::read(script)? {
        println!("{command:?}");
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
