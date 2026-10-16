//! Admits a Skate 3 disc and lists one directory: `disc_list <iso> [dir]`.
use skate_source::{Disc, admit};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().map(PathBuf::from);
    let dir = args.next().map(|d| d.to_string_lossy().into_owned());
    match admit(path.as_deref()) {
        Ok(Disc::Image(mut image)) => {
            println!("admitted (image)");
            let entry = match &dir {
                Some(d) => image.find(d).ok().flatten(),
                None => Some(image.root().clone()),
            };
            match entry.map(|e| image.list(&e)) {
                Some(Ok(list)) => {
                    for e in list {
                        println!(
                            "{}{} {}",
                            e.name,
                            if e.directory { "/" } else { "" },
                            e.size
                        );
                    }
                }
                other => println!("cannot list: {:?}", other.map(|r| r.err())),
            }
        }
        Ok(Disc::Folder(root)) => println!("admitted (folder {})", root.display()),
        Err(e) => println!("refused: {e}"),
    }
}
