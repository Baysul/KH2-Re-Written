//! Private source-bound finite sampling evidence; no gameplay qualification.
use package_reader::{model_inspection, motion, probe::ProbeSource};
use std::{fs::OpenOptions, io::Write, path::PathBuf};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 5 {
        return Err("usage: motion_sample_probe <Steam-root> <manifest> <actor-selection> <motion-selection> <new-report>".into());
    }
    let source = ProbeSource::bind(&args[0], &args[1])?;
    let output = source.output_path(&args[4])?;
    if output.exists() {
        return Err("sample report must be new".into());
    }
    let models = model_inspection::read_selected_models(&source, &args[2])?;
    if models.len() != 1 {
        return Err("one sample actor required".into());
    }
    let skin = &models[0].skinning;
    let motions = motion::read_selected(&source, &args[3])?;
    let mut samples = Vec::new();
    for selected in motions {
        let end = f32::from_bits(selected.motion.interpolated_header_words[33]) * 2.;
        if !end.is_finite() || !(1. ..=65_536.).contains(&end) {
            return Err("sample window outside bounded profile".into());
        }
        let mut frames = vec![0, 1, 7, 15, 30, (end.ceil() as u32).saturating_sub(1)];
        frames.retain(|&f| (f as f32) < end);
        frames.sort_unstable();
        frames.dedup();
        let poses: Vec<_> = frames.into_iter().map(|frame| {
            Ok(serde_json::json!({"frame":frame,"local":selected.motion.sample_fk(&skin.bones, frame)?,"pose":skin.sample_fk(&selected.motion,frame)?}))
        }).collect::<std::io::Result<_>>()?;
        samples.push(serde_json::json!({"slot":selected.slot,"sha256":selected.sha256,"motion":selected.motion,"frames":poses}));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    serde_json::to_writer_pretty(
        &mut file,
        &serde_json::json!({"schema_version":1,"source_content_identity":source.content_identity(),"skinning":skin,
        "bind_poses":models[0].poses,"samples":samples,"gameplay_verified":false}),
    )?;
    writeln!(file)?;
    file.sync_all()?;
    println!("Finite explicit FK samples retained; installed behavior unqualified.");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Motion sample rejected: {e}");
        std::process::exit(1);
    }
}
