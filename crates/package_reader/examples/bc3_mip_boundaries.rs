//! Authored BC3 chains and actual-library outputs; no source bytes.
use package_reader::hd_image::decode_bc3_mip_chain;
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 2 || !args[1].is_dir() {
        return Err("usage: bc3_mip_boundaries <install-root-for-output-guard> <existing-private-directory>".into());
    }
    let write_new = |name: &str, bytes: &[u8]| -> Result<(), Box<dyn std::error::Error>> {
        let path = source_identity::safe_output(&args[0], &args[1].join(name))?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    };
    let hash = |v: &[u8]| format!("{:x}", Sha256::digest(v));
    let mut observations = Vec::new();
    for (ordinal, (width, height, count)) in [
        (1u32, 1u32, 0u32),
        (2, 2, 2),
        (4, 4, 3),
        (8, 4, 4),
        (5, 7, 3),
        (1, 257, 9),
        (257, 1, 9),
        (257, 259, 9),
        (4096, 1, 13),
        (64, 32, 3),
    ]
    .into_iter()
    .enumerate()
    {
        let mut input = vec![0; 128];
        input[..4].copy_from_slice(b"DDS ");
        for (offset, word) in [
            (4, 124u32),
            (8, if count > 1 { 0xa1007 } else { 0x81007 }),
            (12, height),
            (16, width),
            (20, width.div_ceil(4) * height.div_ceil(4) * 16),
            (24, 1),
            (28, count),
            (76, 32),
            (80, 4),
            (108, if count > 1 { 0x401008 } else { 0x1000 }),
        ] {
            input[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
        }
        input[84..88].copy_from_slice(b"DXT5");
        let (mut w, mut h) = (width, height);
        for mip in 0..count.max(1) {
            for block in 0..w.div_ceil(4) * h.div_ceil(4) {
                let seed = block
                    .wrapping_mul(313)
                    .wrapping_add(mip * 997)
                    .wrapping_add(ordinal as u32 * 127);
                let mut bytes = [0u8; 16];
                bytes[0] = seed as u8;
                bytes[1] = (seed >> 3) as u8;
                let mut alpha = 0u64;
                let mut color = 0u32;
                for p in 0..16 {
                    alpha |= (((seed + p) % 8) as u64) << (p * 3);
                    color |= ((seed + p / 4 + p) % 4) << (p * 2);
                }
                bytes[2..8].copy_from_slice(&alpha.to_le_bytes()[..6]);
                bytes[8..10].copy_from_slice(&(seed as u16).to_le_bytes());
                bytes[10..12].copy_from_slice(&(seed.wrapping_mul(17) as u16).to_le_bytes());
                bytes[12..16].copy_from_slice(&color.to_le_bytes());
                input.extend(bytes);
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        let chain = decode_bc3_mip_chain(&input, input.len())?;
        // Owned inspection envelope: count, then width/height/u64 source span/RGBA.
        let mut output = b"MIPS".to_vec();
        output.extend((chain.layout.levels.len() as u32).to_le_bytes());
        for (level, pixels) in chain.layout.levels.iter().zip(&chain.levels_rgba) {
            output.extend(level.width.to_le_bytes());
            output.extend(level.height.to_le_bytes());
            output.extend((level.encoded_offset as u64).to_le_bytes());
            output.extend((level.encoded_length as u64).to_le_bytes());
            output.extend(pixels);
        }
        let input_file = format!("case-{ordinal:02}.dds");
        let output_file = format!("case-{ordinal:02}.mips");
        write_new(&input_file, &input)?;
        write_new(&output_file, &output)?;
        observations.push(serde_json::json!({"ordinal":ordinal,"input_file":input_file,"output_file":output_file,
            "input_sha256":hash(&input),"output_sha256":hash(&output),"layout":chain.layout,"profile":chain.pixel_decode_profile}));
    }
    write_new(
        "cases.json",
        &serde_json::to_vec_pretty(&serde_json::json!({"schema_version":1,
        "input_provenance":"authored byte permutations, no source bytes","cases":observations}))?,
    )?;
    println!(
        "Generated {} authored BC3 chains and complete actual-library outputs.",
        observations.len()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Authored BC3 chains failed: {error}");
        std::process::exit(1)
    }
}
