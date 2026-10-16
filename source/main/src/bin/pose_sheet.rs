//! Private research renderer: evaluates a model's motion frames with the
//! motion evaluator and draws them as a contact sheet (front, side and back
//! views, flat-shaded, no textures) into a new PNG outside the install. It
//! renders no installed-game output and claims nothing about the installed
//! look. The `blend` form compares a joint-space blend of two poses against
//! a plain mix of their skinned vertices.
use data::{
    bar, model_inspection, motion, motion_eval, objentry, path_lookup::Library,
    probe_source::ProbeSource,
};
use std::path::{Path, PathBuf};

type Error = Box<dyn std::error::Error>;

struct Scene {
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
    colour: [f32; 3],
    /// Drawn in the same cell as the scene before it.
    weapon: bool,
}

/// Row-vector matrix words applied to a point.
fn transform(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    [
        p[0] * m[0] + p[1] * m[4] + p[2] * m[8] + m[12],
        p[0] * m[1] + p[1] * m[5] + p[2] * m[9] + m[13],
        p[0] * m[2] + p[1] * m[6] + p[2] * m[10] + m[14],
    ]
}

fn load(
    library: &mut Library,
    source: &ProbeSource,
    model: &str,
    set: &str,
    slot: usize,
) -> Result<
    (
        model_inspection::InspectionModel,
        motion::InterpolatedMotion,
    ),
    Error,
> {
    let found = library
        .find(&format!("obj/{model}.mdlx"))
        .ok_or("model is not installed")?;
    let mut models =
        model_inspection::read_model_at(source, found.index, found.ordinal, found.name_hash)?;
    if models.len() != 1 {
        return Err("model selection is not one model".into());
    }
    let bytes = library.read(&format!("obj/{set}"))?;
    let table = bar::read_bar(&bytes)?;
    let e = table
        .entries
        .get(slot)
        .filter(|e| e.offset > 0 && e.size > 0)
        .ok_or("slot absent")?;
    let anb = &bytes[e.offset as usize..e.offset as usize + e.size as usize];
    let selected = motion::read_anb(anb, slot)?;
    Ok((models.remove(0), selected.motion))
}

fn draw(
    image: &mut [u8],
    width: usize,
    cell: (usize, usize, usize, usize),
    scene: &Scene,
    view: usize,
    bounds: ([f32; 3], f32),
    depth: &mut [f32],
) {
    let (x0, y0, w, h) = cell;
    let (centre, radius) = bounds;
    let scale = (w.min(h) as f32 * 0.5) / radius;
    // View 0: camera on +Z, 1: camera on +X, 2: camera on -Z.
    let project = |p: [f32; 3]| -> (f32, f32, f32) {
        let q = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
        let (sx, sz) = match view {
            0 => (-q[0], -q[2]),
            1 => (q[2], -q[0]),
            _ => (q[0], q[2]),
        };
        (
            w as f32 * 0.5 + sx * scale,
            h as f32 * 0.5 - q[1] * scale,
            sz,
        )
    };
    let light = [0.3f32, 0.5, 0.8];
    for tri in scene.indices.as_chunks::<3>().0 {
        let pts: Vec<[f32; 3]> = tri.iter().map(|&i| scene.positions[i as usize]).collect();
        let a = project(pts[0]);
        let b = project(pts[1]);
        let c = project(pts[2]);
        let u = [
            pts[1][0] - pts[0][0],
            pts[1][1] - pts[0][1],
            pts[1][2] - pts[0][2],
        ];
        let v = [
            pts[2][0] - pts[0][0],
            pts[2][1] - pts[0][1],
            pts[2][2] - pts[0][2],
        ];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
        let shade = ((n[0] * light[0] + n[1] * light[1] + n[2] * light[2]) / nl).abs() * 0.7 + 0.3;
        let min_x = a.0.min(b.0).min(c.0).floor().max(0.) as usize;
        let max_x = (a.0.max(b.0).max(c.0).ceil() as usize).min(w.saturating_sub(1));
        let min_y = a.1.min(b.1).min(c.1).floor().max(0.) as usize;
        let max_y = (a.1.max(b.1).max(c.1).ceil() as usize).min(h.saturating_sub(1));
        let area = (b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1);
        if area.abs() < 1e-6 {
            continue;
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((b.0 - px) * (c.1 - py) - (c.0 - px) * (b.1 - py)) / area;
                let w1 = ((c.0 - px) * (a.1 - py) - (a.0 - px) * (c.1 - py)) / area;
                let w2 = 1. - w0 - w1;
                if w0 < 0. || w1 < 0. || w2 < 0. {
                    continue;
                }
                let z = w0 * a.2 + w1 * b.2 + w2 * c.2;
                if z < depth[y * w + x] {
                    depth[y * w + x] = z;
                    let at = ((y0 + y) * width + x0 + x) * 3;
                    for k in 0..3 {
                        image[at + k] = (scene.colour[k] * shade * 255.) as u8;
                    }
                }
            }
        }
    }
}

/// Columns of scenes drawn in the given (view, scene index) rows.
fn sheet(columns: &[Vec<Scene>], rows: &[(usize, usize)], output: &Path) -> Result<(), Error> {
    let (cell_w, cell_h) = (240usize, 320usize);
    let width = cell_w * columns.len();
    let height = cell_h * rows.len();
    let mut image = vec![24u8; width * height * 3];
    let (mut low, mut high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for scene in columns.iter().flatten() {
        for p in &scene.positions {
            for k in 0..3 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
    }
    let centre = [
        (low[0] + high[0]) * 0.5,
        (low[1] + high[1]) * 0.5,
        (low[2] + high[2]) * 0.5,
    ];
    let radius = (0..3)
        .map(|k| (high[k] - low[k]) * 0.5)
        .fold(1f32, f32::max)
        * 1.05;
    let mut depth = vec![f32::INFINITY; cell_w * cell_h];
    for (col, scenes) in columns.iter().enumerate() {
        for (row, &(view, which)) in rows.iter().enumerate() {
            let cell = (col * cell_w, row * cell_h, cell_w, cell_h);
            depth.fill(f32::INFINITY);
            for scene in scenes.iter().skip(which).take(1) {
                draw(
                    &mut image,
                    width,
                    cell,
                    scene,
                    view,
                    (centre, radius),
                    &mut depth,
                );
            }
            for scene in scenes.iter().skip(which + 1).filter(|s| s.weapon) {
                draw(
                    &mut image,
                    width,
                    cell,
                    scene,
                    view,
                    (centre, radius),
                    &mut depth,
                );
            }
        }
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let encoder = image::codecs::png::PngEncoder::new(file);
    image::ImageEncoder::write_image(
        encoder,
        &image,
        width as u32,
        height as u32,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(())
}

fn run() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 7 {
        return Err("usage: pose_sheet <Steam-root> <manifest> <new-output.png> <model> <mset> <slot> <frame,...> [weapon-model weapon-mset]\n       pose_sheet <Steam-root> <manifest> <new-output.png> <model> <mset> blend <slot>:<frame> <slot>:<frame> <weight,...>".into());
    }
    let source = ProbeSource::bind_session(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]))?;
    source.verify_session()?;
    let output = source.output_path(&PathBuf::from(&args[2]))?;
    let mut library = Library::open(&source)?;
    if args[5] == "blend" {
        let pair = |text: &str| -> Result<(usize, f32), Error> {
            let (slot, frame) = text.split_once(':').ok_or("slot:frame expected")?;
            Ok((slot.parse()?, frame.parse()?))
        };
        let ((slot_a, frame_a), (slot_b, frame_b)) = (pair(&args[6])?, pair(&args[7])?);
        let weights: Vec<f32> = args[8]
            .split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        let (actor, from_motion) = load(&mut library, &source, &args[3], &args[4], slot_a)?;
        let (_, to_motion) = load(&mut library, &source, &args[3], &args[4], slot_b)?;
        let parents: Vec<i32> = actor
            .skinning
            .bones
            .iter()
            .map(|b| b.header_i32[1])
            .collect();
        let indices: Vec<u32> = actor
            .parts
            .iter()
            .flat_map(|p| p.indices.iter().copied())
            .collect();
        let evaluate =
            |m: &motion::InterpolatedMotion, f: f32| -> Result<Vec<motion_eval::World>, Error> {
                let e = motion_eval::evaluate(&actor.skinning.bones, m, f)?;
                let mut joints = e.joints[..parents.len()].to_vec();
                for w in &mut joints {
                    for k in 0..3 {
                        w.translation[k] -= e.root_translation[k];
                    }
                }
                Ok(joints)
            };
        let from = evaluate(&from_motion, frame_a)?;
        let to = evaluate(&to_motion, frame_b)?;
        let positions_of = |worlds: &[motion_eval::World]| -> Result<Vec<[f32; 3]>, Error> {
            Ok(actor
                .skinning
                .pose_from_worlds(worlds)?
                .position_bits
                .iter()
                .map(|p| p.map(f32::from_bits))
                .collect())
        };
        let (pa, pb) = (positions_of(&from)?, positions_of(&to)?);
        let mut columns = Vec::new();
        for &w in &weights {
            let blended = motion_eval::blend_poses(&parents, &from, &to, w);
            let joint = positions_of(&blended)?;
            let mix: Vec<[f32; 3]> = pa
                .iter()
                .zip(&pb)
                .map(|(a, b)| std::array::from_fn(|k| a[k] + (b[k] - a[k]) * w))
                .collect();
            columns.push(vec![
                Scene {
                    positions: joint,
                    indices: indices.clone(),
                    colour: [0.55, 0.85, 0.65],
                    weapon: false,
                },
                Scene {
                    positions: mix,
                    indices: indices.clone(),
                    colour: [0.85, 0.6, 0.6],
                    weapon: false,
                },
            ]);
        }
        return sheet(&columns, &[(0, 0), (0, 1), (1, 0), (1, 1)], &output);
    }
    let slot: usize = args[5].parse()?;
    let frames: Vec<f32> = args[6]
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let (actor, actor_motion) = load(&mut library, &source, &args[3], &args[4], slot)?;
    let weapon = match (args.get(7), args.get(8)) {
        (Some(model), Some(set)) => {
            let w = load(&mut library, &source, model, set, slot)?;
            let objects = objentry::read(&library.read("00objentry.bin")?)?;
            let joint = objects
                .iter()
                .find(|o| o.model_name == model.as_bytes())
                .map(|o| o.weapon_joint as usize)
                .ok_or("weapon joint unknown")?;
            Some((w.0, w.1, joint))
        }
        _ => None,
    };
    let indices: Vec<u32> = actor
        .parts
        .iter()
        .flat_map(|p| p.indices.iter().copied())
        .collect();
    let mut columns = Vec::new();
    for &frame in &frames {
        let (pose, evaluated) = actor.skinning.sample_evaluated(&actor_motion, frame)?;
        let r = evaluated.root_translation;
        let positions: Vec<[f32; 3]> = pose
            .position_bits
            .iter()
            .map(|p| {
                let q = p.map(f32::from_bits);
                [q[0] - r[0], q[1] - r[1], q[2] - r[2]]
            })
            .collect();
        let mut list = vec![Scene {
            positions,
            indices: indices.clone(),
            colour: [0.55, 0.65, 0.85],
            weapon: false,
        }];
        if let Some((model, mot, joint)) = &weapon {
            let (wp, _) = model.skinning.sample_evaluated(mot, frame)?;
            let mut hand = pose.matrix_bits[*joint].map(f32::from_bits);
            for k in 0..3 {
                hand[12 + k] -= r[k];
            }
            let wi: Vec<u32> = model
                .parts
                .iter()
                .flat_map(|p| p.indices.iter().copied())
                .collect();
            list.push(Scene {
                positions: wp
                    .position_bits
                    .iter()
                    .map(|p| transform(&hand, p.map(f32::from_bits)))
                    .collect(),
                indices: wi,
                colour: [0.95, 0.8, 0.2],
                weapon: true,
            });
        }
        columns.push(list);
    }
    sheet(&columns, &[(0, 0), (1, 0), (2, 0)], &output)
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Pose sheet rejected: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
