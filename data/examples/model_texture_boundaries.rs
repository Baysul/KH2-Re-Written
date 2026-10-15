//! Authored CPU byte cases only. Outputs are new private files, never fixtures
//! copied from installed models; the install argument guards output isolation.
use data::model_texture_pixels::{PROFILE, decode_stored_indices};
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 2 || !args[1].is_dir() {
        return Err("usage: model_texture_boundaries <install-root-for-output-guard> <existing-private-directory>".into());
    }
    let cases = [
        (128u32, 64u32, 8u32, 0u32),
        (128, 128, 8, 0),
        (256, 64, 8, 0),
        (256, 128, 8, 0),
        (384, 192, 8, 0),
        (512, 256, 8, 0),
        (128, 128, 4, 0),
        (128, 256, 4, 0),
        (256, 128, 4, 0),
        (256, 256, 4, 0),
        (384, 384, 4, 0),
        (512, 512, 4, 0),
        (2, 2, 8, 19),
        (128, 64, 8, 63),
        (2, 2, 4, 20),
        (16, 16, 4, 1),
    ];
    let write_new = |name: &str, bytes: &[u8]| -> Result<(), Box<dyn std::error::Error>> {
        let path = source::safe_output(&args[0], &args[1].join(name))?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    };
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let mut observations = Vec::new();
    for (ordinal, (width, height, depth, upload)) in cases.into_iter().enumerate() {
        let mut state = 0x9e3779b9u32 ^ ordinal as u32;
        let input: Vec<_> = (0..width as usize * height as usize / (8 / depth) as usize)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        let output = decode_stored_indices(&input, width, height, depth, upload)?;
        let input_file = format!("case-{ordinal:02}.stored");
        let output_file = format!("case-{ordinal:02}.indices");
        write_new(&input_file, &input)?;
        write_new(&output_file, &output)?;
        observations.push(
            serde_json::json!({"ordinal":ordinal,"width":width,"height":height,
            "depth":depth,"upload_psm":upload,"input_file":input_file,"output_file":output_file,
            "input_sha256":hash(&input),"output_sha256":hash(&output),"bytes":output.len()}),
        );
    }
    write_new(
        "cases.json",
        &serde_json::to_vec_pretty(&serde_json::json!({"schema_version":1,
        "profile":PROFILE,"input_provenance":"authored xorshift32 bytes","cases":observations}))?,
    )?;
    println!(
        "Generated {} authored model texture CPU cases outside source/Git.",
        cases.len()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Authored model texture cases failed: {error}");
        std::process::exit(1);
    }
}
