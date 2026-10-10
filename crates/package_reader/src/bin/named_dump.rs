//! Research probe: copies installed files, looked up by path name, into a
//! private directory outside the installation. Names that are absent are
//! reported and skipped.
use package_reader::{named::Library, probe::ProbeSource};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() < 4 {
        return Err("usage: named_dump <Steam-root> <manifest> <private-out-dir> <name>...".into());
    }
    let source = ProbeSource::bind(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    let out = PathBuf::from(&args[2]);
    let out = source.output_path(&out)?;
    std::fs::create_dir_all(&out)?;
    let mut library = Library::open(&source)?;
    for name in &args[3..] {
        let name = name.to_str().ok_or("name not UTF-8")?;
        let Some(found) = library.find(name) else {
            println!("absent {name}");
            continue;
        };
        let bytes = library.read_found(&found)?;
        let file = out.join(name.replace('/', "__"));
        std::fs::write(&file, &bytes)?;
        println!("{} {} {name}", found.index, bytes.len());
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
