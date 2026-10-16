//! Private research audit of every interpolated motion of the installed
//! objects: which constraint kinds each motion table uses, whether the
//! evaluator supports them, and whether the evaluated pose is continuous
//! over the clip. It writes only a new private JSON report (and, with an
//! object filter, the model/motion payloads a reference evaluator needs).
use data::{
    bar, model_skeleton, motion, motion_eval, objentry, path_lookup::Library,
    probe_source::ProbeSource,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs::OpenOptions, io::Write, path::PathBuf};

type Error = Box<dyn std::error::Error>;

/// The model BAR's skeleton (entry type 4).
fn skeleton(model: &[u8]) -> Result<Vec<model_skeleton::BoneRecord>, Error> {
    let table = bar::read_bar(model)?;
    let entry = table
        .entries
        .iter()
        .find(|e| e.entry_type == 4 && e.offset > 0 && e.size > 0)
        .ok_or("model entry absent")?;
    let bytes = &model[entry.offset as usize..entry.offset as usize + entry.size as usize];
    let structure = model_skeleton::read_model_structure(bytes)?;
    Ok(structure
        .blocks
        .into_iter()
        .next()
        .ok_or("skeleton absent")?
        .bones)
}

fn audit_motion(bones: &[model_skeleton::BoneRecord], m: &motion::InterpolatedMotion) -> Value {
    let mut kinds: BTreeMap<u8, (usize, usize)> = BTreeMap::new();
    for c in m.constraints.bytes.as_chunks::<12>().0 {
        let e = kinds.entry(c[0]).or_default();
        e.0 += 1;
        e.1 += usize::from(c[1] != 0);
    }
    let (start, end, fps, ret) = m.frame_window().unwrap_or((0., 0., 0., 0.));
    // The test room samples one key-time unit per 60 Hz tick from
    // FrameStart to FrameEnd scaled by 60 / fps (clip_for).
    let scale = if fps > 0. { 60. / fps } else { 1. };
    let (start, end, ret) = (start * scale, end * scale, ret * scale);
    let mut stats = motion_eval::EvaluationStats::default();
    let mut prev: Option<Vec<[f32; 3]>> = None;
    let mut worst_step = (0f32, 0f32, 0usize);
    let mut steps: Vec<f32> = Vec::new();
    let mut error = None;
    let mut first_pose: Option<Vec<[f32; 3]>> = None;
    let mut ret_pose: Option<Vec<[f32; 3]>> = None;
    let mut last_pose: Option<Vec<[f32; 3]>> = None;
    let mut t = start;
    let mut ticks = 0;
    while t <= end + 0.001 && ticks < 4000 {
        match motion_eval::evaluate(bones, m, t) {
            Ok(e) => {
                stats.constraints_applied += e.stats.constraints_applied;
                stats.ik_chains_solved += e.stats.ik_chains_solved;
                stats.expressions_applied += e.stats.expressions_applied;
                stats.unsupported_constraints += e.stats.unsupported_constraints;
                let pose: Vec<[f32; 3]> = e.joints[..bones.len()]
                    .iter()
                    .map(|w| w.translation)
                    .collect();
                if let Some(p) = &prev {
                    let mut worst = 0f32;
                    let mut at = 0;
                    for (i, (a, b)) in p.iter().zip(&pose).enumerate() {
                        let d =
                            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2))
                                .sqrt();
                        if d > worst {
                            worst = d;
                            at = i;
                        }
                    }
                    steps.push(worst);
                    if worst > worst_step.0 {
                        worst_step = (worst, t, at);
                    }
                }
                if first_pose.is_none() {
                    first_pose = Some(pose.clone());
                }
                if ret_pose.is_none() && t >= ret {
                    ret_pose = Some(pose.clone());
                }
                last_pose = Some(pose.clone());
                prev = Some(pose);
            }
            Err(e) => {
                error = Some(e.to_string());
                break;
            }
        }
        t += 1.0;
        ticks += 1;
    }
    steps.sort_by(f32::total_cmp);
    let median = steps.get(steps.len() / 2).copied().unwrap_or(0.);
    let seam = |a: &Option<Vec<[f32; 3]>>, b: &Option<Vec<[f32; 3]>>| -> f32 {
        match (a, b) {
            (Some(a), Some(b)) => a
                .iter()
                .zip(b)
                .map(|(a, b)| {
                    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
                })
                .fold(0., f32::max),
            _ => 0.,
        }
    };
    let detail = std::env::var("AUDIT_DETAIL").ok();
    let half = |b: &[u8], at: usize| i16::from_le_bytes([b[at], b[at + 1]]);
    let tables = detail.is_some().then(|| {
        json!({
            "constraints": m.constraints.bytes.as_chunks::<12>().0.iter().map(|b| json!({
                "kind": b[0], "active": b[1], "constrained": half(b, 2), "source": half(b, 4),
                "limiter": half(b, 6), "act_count": half(b, 8), "act_start": half(b, 10)})).collect::<Vec<_>>(),
            "order": m.joints.bytes.as_chunks::<4>().0.iter().map(|b| json!({
                "joint": half(b, 0), "flags": b[2], "reserved": b[3]})).collect::<Vec<_>>(),
            "helpers": m.helpers.bytes.as_chunks::<64>().0.iter().map(|b| json!({
                "index": half(b, 0), "parent": half(b, 4), "flags": u32::from_le_bytes([b[12], b[13], b[14], b[15]])})).collect::<Vec<_>>(),
            "bones": bones.iter().enumerate().map(|(i, b)| json!({
                "i": i, "parent": b.header_i32[1], "len": f32::from_bits(b.transform_bits[3])})).collect::<Vec<_>>(),
            "curves": m.forward_curves.iter().map(|c| ("f", c)).chain(m.inverse_curves.iter().map(|c| ("i", c))).map(|(k, c)| json!({
                "kind": k, "joint": c.joint_id, "channel": c.channel & 15, "pre": (c.channel >> 4) & 3, "post": (c.channel >> 6) & 3,
                "keys": c.key_count, "start": c.key_start,
                "samples": ([0f32, 20., 30., 32., 34., 36., 40., 50., 70.].iter().map(|&f| motion_eval::sample_curve(m, c, f).ok().flatten()).collect::<Vec<_>>())})).collect::<Vec<_>>(),
            "effectors": m.external_effectors.bytes.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect::<Vec<_>>(),
            "nodes": m.expression_nodes.bytes.as_chunks::<12>().0.iter().map(|b| json!({
                "type": b[0] as i8, "global": b[1], "element": half(b, 2),
                "value": f32::from_le_bytes([b[4], b[5], b[6], b[7]]), "car": half(b, 8), "cdr": half(b, 10)})).collect::<Vec<_>>(),
            "activations": m.activations.bytes.as_chunks::<8>().0.iter().map(|b| json!({
                "time": f32::from_le_bytes([b[0], b[1], b[2], b[3]]), "on": u32::from_le_bytes([b[4], b[5], b[6], b[7]])})).collect::<Vec<_>>(),
            "limiters": m.limiters.bytes.as_chunks::<48>().0.iter().map(|b| {
                let f = |at: usize| f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
                json!({"bits": u32::from_le_bytes([b[0], b[1], b[2], b[3]]), "width": f(8), "strength": f(12),
                    "min": [f(16), f(20), f(24), f(28)], "max": [f(32), f(36), f(40), f(44)]})
            }).collect::<Vec<_>>(),
            "expressions": m.expressions.bytes.as_chunks::<8>().0.iter().map(|b| json!({
                "joint": half(b, 0), "channel": half(b, 2), "node": half(b, 6)})).collect::<Vec<_>>(),
        })
    });
    let mut key_types = [0usize; 4];
    for c in m.forward_curves.iter().chain(&m.inverse_curves) {
        let start = usize::try_from(c.key_start).unwrap_or(0);
        for k in start..(start + c.key_count as usize).min(m.keys.len()) {
            key_types[(m.keys[k][0] & 3) as usize] += 1;
        }
    }
    json!({
        "key_types": key_types,
        "tables": tables,
        "frame_start": start, "frame_end": end, "fps": fps, "frame_return": ret,
        "bones": m.bone_count, "total": m.total_bone_count,
        "constraint_kinds": kinds.iter().map(|(k, (n, a))| json!({"kind": k, "count": n, "active": a})).collect::<Vec<_>>(),
        "stats": format!("{stats:?}"),
        "unsupported": stats.unsupported_constraints,
        "median_step": median,
        "worst_step": {"value": worst_step.0, "at": worst_step.1, "bone": worst_step.2},
        "seam_end_to_return": seam(&last_pose, &ret_pose),
        "seam_end_to_start": seam(&last_pose, &first_pose),
        "expressions": m.expressions.bytes.len() / 8,
        "helpers": m.helpers.bytes.len() / 64,
        "error": error,
    })
}

fn run() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: motion_audit <Steam-root> <manifest> <new-report.json> [object-model-prefix] [dump-dir]".into());
    }
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_session()?;
    let output = source.output_path(&PathBuf::from(&args[2]))?;
    let filter = args.get(3).cloned().unwrap_or_default();
    let dump = args.get(4).map(PathBuf::from);
    let mut library = Library::open(&source)?;
    let objects = objentry::read(&library.read("00objentry.bin")?)?;
    let mut report = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut seen_sets = std::collections::HashSet::new();
    // `pair:<model>:<mset>` audits one model with a motion set by name
    // (weapon sets are chosen through 03system `wmst`, not 00objentry).
    let pair = filter.strip_prefix("pair:").and_then(|p| p.split_once(':'));
    let listed: Vec<(String, String)> = match pair {
        Some((m, a)) => vec![(m.to_owned(), a.to_owned())],
        None => objects
            .iter()
            .map(|o| {
                (
                    String::from_utf8_lossy(&o.model_name).into_owned(),
                    String::from_utf8_lossy(&o.animation_name).into_owned(),
                )
            })
            .collect(),
    };
    for (model, anim) in listed {
        if model.is_empty() || anim.is_empty() || (pair.is_none() && !model.starts_with(&filter)) {
            continue;
        }
        if !seen.insert((model.clone(), anim.clone())) {
            continue;
        }
        // UNIQUE_SETS=1: audit each motion set once, with its first model.
        if std::env::var_os("UNIQUE_SETS").is_some() && !seen_sets.insert(anim.clone()) {
            continue;
        }
        let model_bytes = match library.read(&format!("obj/{model}.mdlx")) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let set_bytes = match library.read(&format!("obj/{anim}")) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let bones = match skeleton(&model_bytes) {
            Ok(b) => b,
            Err(e) => {
                report.push(json!({"object": model, "set": anim, "error": e.to_string()}));
                continue;
            }
        };
        let table = match bar::read_bar(&set_bytes) {
            Ok(t) => t,
            Err(e) => {
                report.push(json!({"object": model, "set": anim, "error": e.to_string()}));
                continue;
            }
        };
        if let Some(dir) = &dump {
            std::fs::create_dir_all(dir)?;
            std::fs::write(dir.join(format!("{model}.model.bin")), &model_bytes)?;
        }
        let mut motions = Vec::new();
        for (slot, e) in table.entries.iter().enumerate() {
            if e.offset == 0 || e.size == 0 {
                continue;
            }
            let Some(anb) = set_bytes.get(e.offset as usize..e.offset as usize + e.size as usize)
            else {
                continue;
            };
            let Ok(sel) = motion::read_anb(anb, slot) else {
                continue;
            };
            if sel.motion.raw.is_some() {
                motions.push(json!({"slot": slot, "raw": true}));
                continue;
            }
            if sel.motion.bone_count != bones.len() {
                motions.push(json!({"slot": slot, "skeleton_mismatch": [sel.motion.bone_count, bones.len()]}));
                continue;
            }
            let mut v = audit_motion(&bones, &sel.motion);
            v["slot"] = json!(slot);
            if let Some(dir) = &dump {
                let nested = bar::read_bar(anb)?;
                if let Some(m) = nested
                    .entries
                    .iter()
                    .find(|e| e.entry_type == 9 && e.offset > 0 && e.size > 0)
                {
                    std::fs::write(
                        dir.join(format!("{model}.slot{slot:04}.bin")),
                        &anb[m.offset as usize..m.offset as usize + m.size as usize],
                    )?;
                }
            }
            motions.push(v);
        }
        eprintln!("{model} / {anim}: {} slots", motions.len());
        report
            .push(json!({"object": model, "set": anim, "bones": bones.len(), "motions": motions}));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    serde_json::to_writer(&mut file, &report)?;
    file.flush()?;
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Motion audit rejected: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
