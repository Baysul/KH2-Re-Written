//! Authored BGRA bytes and actual-library RGBA outputs, outside source/Git.
use data::hd_image::{HdImageFormat, decode_hd_image};
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 2 || !args[1].is_dir() {
        return Err(
            "usage: bgra_boundaries <install-root-for-output-guard> <existing-private-directory>"
                .into(),
        );
    }
    let write_new = |name: &str, bytes: &[u8]| -> Result<(), Box<dyn std::error::Error>> {
        let path = validation::safe_output(&args[0], &args[1].join(name))?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    };
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let mut observations = Vec::new();
    for (ordinal, (width, height)) in [
        (1u32, 1u32),
        (1, 257),
        (3, 2),
        (257, 3),
        (256, 256),
        (4096, 1),
    ]
    .into_iter()
    .enumerate()
    {
        let mut input = vec![0; 128];
        input[..4].copy_from_slice(b"DDS ");
        for (offset, word) in [
            (4, 124u32),
            (8, 0x2100f),
            (12, height),
            (16, width),
            (20, width * 4),
            (24, 1),
            (28, 1),
            (76, 32),
            (80, 0x41),
            (88, 32),
            (92, 0x00ff0000),
            (96, 0xff00),
            (100, 0xff),
            (104, 0xff000000),
            (108, 0x1000),
        ] {
            input[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
        }
        for pixel in 0..width as usize * height as usize {
            let v = pixel as u8;
            input.extend([
                v,
                v.wrapping_mul(3),
                v.wrapping_mul(7).wrapping_add(5),
                v.wrapping_mul(11),
            ]);
        }
        let decoded = decode_hd_image(&input, input.len())?;
        if decoded.format != HdImageFormat::DdsBgra8 {
            return Err("authored case format differs".into());
        }
        let mut output = b"RGBA".to_vec();
        output.extend(width.to_le_bytes());
        output.extend(height.to_le_bytes());
        output.extend(&decoded.pixels_rgba);
        let input_file = format!("case-{ordinal:02}.dds");
        let output_file = format!("case-{ordinal:02}.rgba");
        write_new(&input_file, &input)?;
        write_new(&output_file, &output)?;
        observations.push(serde_json::json!({"ordinal":ordinal,"width":width,"height":height,
            "input_file":input_file,"output_file":output_file,"input_sha256":hash(&input),"output_sha256":hash(&output),
            "profile":decoded.pixel_decode_profile,"pixels":width*height}));
    }
    write_new(
        "cases.json",
        &serde_json::to_vec_pretty(&serde_json::json!({"schema_version":1,
        "input_provenance":"authored byte permutations, no source bytes", "cases":observations}))?,
    )?;
    println!(
        "Generated {} authored BGRA cases with actual-library outputs.",
        observations.len()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Authored BGRA cases failed: {error}");
        std::process::exit(1)
    }
}
