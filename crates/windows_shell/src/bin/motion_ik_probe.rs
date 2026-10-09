//! Private research dump of a model skeleton and the IK-related tables of
//! selected motions (helpers, joint order, constraints, activations,
//! limiters, expressions, root position, inverse curves), decoded with the
//! public OpenKH motion.md layouts. Output stays outside Git.
use package_reader::{model_inspection, motion, probe::ProbeSource};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

#[derive(Deserialize)]
struct Selection {
    schema_version: u32,
    source_content_identity: String,
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
    battle: bool,
    has_weapon: bool,
    motion_ids: Vec<u16>,
}

fn f(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn i16_(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}
fn u32_(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn vec4(b: &[u8], at: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, at + i * 4))
}

fn tables(m: &motion::InterpolatedMotion) -> Value {
    let helpers: Vec<_> = m
        .helpers
        .bytes
        .as_chunks::<64>()
        .0
        .iter()
        .map(|b| {
            json!({"index":i16_(b,0),"sibling":i16_(b,2),"parent":i16_(b,4),"child":i16_(b,6),
            "reserved":u32_(b,8),"flags":u32_(b,12),"scale":vec4(b,16),"rotate":vec4(b,32),"translate":vec4(b,48)})
        })
        .collect();
    let joints: Vec<_> = m
        .joints
        .bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| {
            let flags = b[2];
            json!({"joint":i16_(b,0),"flags":flags,"ik":flags & 3,"translation":(flags>>2)&1,"rotation":(flags>>3)&1,
            "fixed":(flags>>4)&1,"calculated":(flags>>5)&1,"calc_matrix2_rotation":(flags>>6)&1,"external_effector":(flags>>7)&1,"reserved":b[3]})
        })
        .collect();
    let constraints: Vec<_> = m
        .constraints
        .bytes
        .as_chunks::<12>()
        .0
        .iter()
        .map(|b| {
            json!({"type":b[0],"temporary_active":b[1],"constrained":i16_(b,2),"source":i16_(b,4),
            "limiter":i16_(b,6),"activation_count":i16_(b,8),"activation_start":i16_(b,10)})
        })
        .collect();
    let activations: Vec<_> = m
        .activations
        .bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| json!({"time":f(b,0),"active":u32_(b,4)}))
        .collect();
    let limiters: Vec<_> = m
        .limiters
        .bytes
        .as_chunks::<48>()
        .0
        .iter()
        .map(|b| {
            let bits = u32_(b, 0);
            json!({"bits":bits,"type":bits & 7,"global":(bits>>3)&1,"xmin":(bits>>4)&1,"xmax":(bits>>5)&1,
            "ymin":(bits>>6)&1,"ymax":(bits>>7)&1,"zmin":(bits>>8)&1,"zmax":(bits>>9)&1,"padding":u32_(b,4),
            "damping_width":f(b,8),"damping_strength":f(b,12),"min":vec4(b,16),"max":vec4(b,32)})
        })
        .collect();
    let expressions: Vec<_> = m
        .expressions
        .bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| json!({"joint":i16_(b,0),"channel":i16_(b,2),"reserved":i16_(b,4),"node":i16_(b,6)}))
        .collect();
    let nodes: Vec<_> = m
        .expression_nodes
        .bytes
        .as_chunks::<12>()
        .0
        .iter()
        .map(|b| {
            json!({"type":b[0] as i8,"is_global":b[1],"element":i16_(b,2),"value":f(b,4),"car":i16_(b,8),"cdr":i16_(b,10)})
        })
        .collect();
    let root = m.root.bytes.get(..84).map(|b| {
        json!({"scale":vec4(b,0),"rotation":vec4(b,16),"translation":vec4(b,32),
        "fcurves":(0..9).map(|i| u32_(b,48+i*4) as i32).collect::<Vec<_>>()})
    });
    let curve = |c: &motion::Curve| json!({"joint":c.joint_id,"channel":c.channel & 15,"pre":(c.channel>>4)&3,"post":(c.channel>>6)&3,"keys":c.key_count,"start":c.key_start});
    let words = |i: usize| f32::from_bits(m.interpolated_header_words[i]);
    json!({"bone_count":m.bone_count,"total_bone_count":m.total_bone_count,
        "frame_start":words(32),"frame_end":words(33),"fps":words(34),"frame_return":words(35),
        "initial_poses":m.initial_poses.iter().map(|p| json!({"bone":p.bone_id,"channel":p.channel,"value":f32::from_bits(p.value_bits)})).collect::<Vec<_>>(),
        "forward_curves":m.forward_curves.iter().map(curve).collect::<Vec<_>>(),
        "inverse_curves":m.inverse_curves.iter().map(curve).collect::<Vec<_>>(),
        "helpers":helpers,"joints":joints,"constraints":constraints,"activations":activations,"limiters":limiters,
        "expressions":expressions,"expression_nodes":nodes,"root":root,
        "external_effectors":m.external_effectors.bytes.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect::<Vec<_>>()})
}

/// Evaluated root-curve translation and pelvis (bone 0) position every
/// fourth key-time unit, for root-motion analysis.
fn samples(
    bones: &[package_reader::model_skeleton::BoneRecord],
    m: &motion::InterpolatedMotion,
) -> Value {
    let end = (f32::from_bits(m.interpolated_header_words[33]) * 2.).ceil() as u32;
    (0..end)
        .step_by(4)
        .filter_map(|frame| {
            let e = package_reader::motion_eval::evaluate(bones, m, frame as f32).ok()?;
            Some(json!({"frame":frame,"root":e.root_translation,"bone0":e.joints[0].translation,"stats":format!("{:?}", e.stats)}))
        })
        .collect()
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 5 && args.len() != 6 {
        return Err("usage: motion_ik_probe <Steam-root> <manifest> <model-selection> <motion-id-selection> <new-report> [<new-private-payload-dir>]".into());
    }
    let source = ProbeSource::bind_session(&args[0], &args[1])?;
    let output = source.output_path(&args[4])?;
    let selection: Selection = serde_json::from_slice(&std::fs::read(&args[3])?)?;
    if selection.schema_version != 1
        || selection.source_content_identity != source.content_identity()
    {
        return Err("motion selection identity/schema invalid".into());
    }
    let models = model_inspection::read_selected_models(&source, &args[2])?;
    let model = models.first().ok_or("model absent")?;
    let bones: Vec<_> = model
        .skinning
        .bones
        .iter()
        .map(|b| {
            let t = b.transform_bits.map(f32::from_bits);
            json!({"header_i32":b.header_i32,"header_i16":b.header_i16,"scale":&t[0..4],"rotate":&t[4..8],"translate":&t[8..12]})
        })
        .collect();
    let set = motion::MotionSetSelection {
        index: selection.index,
        ordinal: selection.ordinal,
        name_hash: selection.name_hash,
        battle: selection.battle,
        has_weapon: selection.has_weapon,
    };
    let motions = motion::read_motion_ids(&source, &set, &selection.motion_ids)?;
    // Optional private payloads (original model BAR and each selected
    // type-9 motion record) for comparison against a reference evaluator.
    if let Some(dir) = args.get(5) {
        std::fs::create_dir(dir)?;
        source.output_path(&dir.join("model-original.bin"))?;
        let write = |name: String, bytes: &[u8]| -> std::io::Result<()> {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dir.join(name))?;
            f.write_all(bytes)?;
            f.sync_all()
        };
        let mut checked = source.open_index(&model.index)?;
        let entry = checked
            .inventory
            .entries
            .get(model.ordinal)
            .ok_or("model entry absent")?;
        let original = package_reader::original::read_original(&mut checked.reader, entry)?;
        write("model-original.bin".into(), &original.bytes)?;
        let mut mset = source.open_index(&set.index)?;
        let entry = mset
            .inventory
            .entries
            .get(set.ordinal)
            .filter(|e| e.name_hash == set.name_hash)
            .ok_or("motion set entry absent")?;
        let original = package_reader::original::read_original(&mut mset.reader, entry)?;
        let table = package_reader::bar::read_bar(&original.bytes)?;
        for (selected, id) in motions.iter().zip(&selection.motion_ids) {
            let e = &table.entries[selected.slot];
            let anb = &original.bytes[e.offset as usize..e.offset as usize + e.size as usize];
            let nested = package_reader::bar::read_bar(anb)?;
            let m = nested
                .entries
                .iter()
                .find(|e| e.entry_type == 9 && e.offset > 0 && e.size > 0)
                .ok_or("motion record absent")?;
            write(
                format!("motion-{id:03}-slot-{:04}.bin", selected.slot),
                &anb[m.offset as usize..m.offset as usize + m.size as usize],
            )?;
        }
        checked.verify_unchanged()?;
        mset.verify_unchanged()?;
    }
    let report = json!({"source_qualification":"quarantined_unqualified","skeleton":bones,
        "motions":motions.iter().zip(&selection.motion_ids).map(|(m,id)| json!({"motion_id":id,"slot":m.slot,"sha256":m.sha256,
            "triggers":m.triggers.iter().map(|t| &t.records).collect::<Vec<_>>(),"tables":tables(&m.motion),
            "samples":samples(&model.skinning.bones, &m.motion)})).collect::<Vec<_>>(),
        "gameplay_verified":false});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.flush()?;
    eprintln!(
        "Wrote skeleton and {} motion IK table dumps.",
        motions.len()
    );
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Motion IK probe rejected: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
