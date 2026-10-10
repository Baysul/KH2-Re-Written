//! Event effects: the script's SeqEffect records (opcode 0x09) start PAX
//! elements of the effect files ReadEffect loaded, run by `effect_vm`, and
//! this module presents their sprite quads and OMD model draws.
//!
//! Space: effect elements are placed in the script's coordinates (the
//! actors' space; the exe's y-down game space) and the camera pose is the
//! player's. The effect VM runs against a view matrix built from that
//! camera, and the view-space output goes back through the camera into
//! the scene: `effect view (x right, y down, z forward)` ↔ camera local
//! (x right, y up, −z forward), `game (x, y, z)` ↔ scene `(x, −y, z)`.
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use effect_vm::{Instance, ModelDraw, Quad, Rng, math::Mat};
use event_sim::{Timeline, camera};
use package_reader::{
    effect::{self, Dpd, OmdHeader, OmdMesh, Pax},
    event::Command,
};
use std::{collections::HashMap, sync::Arc};

/// Selects a texture image: which DPD's buffers (own textures shadow the
/// common ones by TBP) and the TEX0 fields the mesh names.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct TexKey {
    dpd: usize,
    tbp: u16,
    cbp: u16,
    tw: u8,
    th: u8,
}

impl TexKey {
    fn new(dpd: &Arc<Dpd>, tex0: u64) -> Self {
        TexKey {
            dpd: Arc::as_ptr(dpd) as usize,
            tbp: (tex0 & 0x3fff) as u16,
            cbp: ((tex0 >> 37) & 0x3fff) as u16,
            tw: ((tex0 >> 26) & 0xf) as u8,
            th: ((tex0 >> 30) & 0xf) as u8,
        }
    }
    fn size(&self) -> [u32; 2] {
        [1 << self.tw.min(10), 1 << self.th.min(10)]
    }
}

type ParsedModel = (OmdHeader, Vec<OmdMesh>);

#[derive(Default)]
struct Batch {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BatchKey {
    texture: Option<TexKey>,
    additive: bool,
    repeat: bool,
}

struct Running {
    instance: Instance,
    dpd: Arc<Dpd>,
}

#[derive(Resource)]
pub struct Fx {
    /// ReadEffect id → PAX.
    paxes: HashMap<i16, Arc<Pax>>,
    common: Option<Arc<Dpd>>,
    script: Vec<Command>,
    next: usize,
    /// EffectId → (PaxId, PaxEntryIndex) declared by EndType 4.
    declared: HashMap<i16, (i16, i16)>,
    running: HashMap<i16, Running>,
    rng: Rng,
    models: HashMap<(usize, i16), Option<ParsedModel>>,
    batches: HashMap<BatchKey, Batch>,
    dirty: bool,
    images: HashMap<TexKey, Handle<Image>>,
    materials: HashMap<BatchKey, Handle<StandardMaterial>>,
    spawned: Vec<Entity>,
    reported: Vec<u32>,
    pub log: Vec<serde_json::Value>,
}

/// The camera's transform and field of view at an event frame (the exe's
/// camera output negates y and z; actors keep the script coordinates).
pub fn camera_pose(timeline: &Timeline, frame: f32) -> Option<(Transform, f32)> {
    let cut = timeline.cut_at(frame)?;
    let data = timeline.camera(cut.camera)?;
    let seconds = (frame - cut.start as f32).min((cut.end - cut.start) as f32) / super::EVENT_FPS;
    let v = camera::view(data, seconds);
    let eye = Vec3::new(v.eye[0], v.eye[1], -v.eye[2]);
    let target = Vec3::new(v.target[0], v.target[1], -v.target[2]);
    let mut transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    transform.rotate_local_z(v.roll.to_radians());
    Some((transform, v.fov.to_radians()))
}

impl Fx {
    pub fn new(paxes: Vec<(i16, Pax)>, common: Option<Dpd>, commands: &[Command]) -> Self {
        let mut script: Vec<Command> = commands
            .iter()
            .filter(|c| matches!(c, Command::Effect { .. }))
            .cloned()
            .collect();
        script.sort_by_key(|c| match c {
            Command::Effect { start, .. } => *start,
            _ => 0,
        });
        Fx {
            paxes: paxes.into_iter().map(|(i, p)| (i, Arc::new(p))).collect(),
            common: common.map(Arc::new),
            script,
            next: 0,
            declared: HashMap::new(),
            running: HashMap::new(),
            rng: Rng::default(),
            models: HashMap::new(),
            batches: HashMap::new(),
            dirty: false,
            images: HashMap::new(),
            materials: HashMap::new(),
            spawned: Vec::new(),
            reported: Vec::new(),
            log: Vec::new(),
        }
    }

    fn run(&mut self, command: &Command, frame: f32) {
        let Command::Effect {
            effect,
            pax,
            pax_entry,
            end_type,
            ..
        } = *command
        else {
            return;
        };
        match end_type {
            4 => {
                self.declared.insert(effect, (pax, pax_entry));
            }
            0 => {
                let Some(&(pax_id, entry)) = self.declared.get(&effect) else {
                    return;
                };
                let Some(pax) = self.paxes.get(&pax_id).cloned() else {
                    return;
                };
                let Some(element) = pax.by_id(entry as u16) else {
                    return;
                };
                let Some(dpx) = pax.effect(element.effect_number) else {
                    return;
                };
                let world = effect_vm::instance_matrix(
                    element.translation,
                    element.rotation,
                    element.scale,
                );
                let dpd = pax.dpds[dpx.dpd].clone();
                let Some(instance) =
                    Instance::new(dpd.clone(), dpx.pdata, world, element.start_wait)
                else {
                    return;
                };
                self.log
                    .push(serde_json::json!({"event":"effect_start","frame":frame,
                    "effect":effect,"element":entry,"dpd":dpx.dpd,"pdata":dpx.pdata,
                    "position":element.translation}));
                self.running.insert(effect, Running { instance, dpd });
            }
            1 => {
                if self
                    .running
                    .get(&effect)
                    .is_some_and(|r| r.instance.looping())
                {
                    self.running.remove(&effect);
                }
            }
            // Stop (FadeFrame is ignored by type 6); type 3 fades — here
            // it is a stop at the record's frame.
            6 | 3 => {
                self.running.remove(&effect);
            }
            _ => {}
        }
    }

    /// One 60 Hz tick: due script records, then every instance one game
    /// frame, converted to scene-space triangles.
    pub fn step(&mut self, frame: f32, timeline: &Timeline) {
        while let Some(c) = self.script.get(self.next).cloned() {
            let Command::Effect { start, .. } = c else {
                break;
            };
            if start as f32 > frame {
                break;
            }
            self.next += 1;
            self.run(&c, frame);
        }
        self.batches.clear();
        self.dirty = true;
        let Some((cam, _)) = camera_pose(timeline, frame) else {
            return;
        };
        let to_scene =
            Mat4::from_scale_rotation_translation(Vec3::ONE, cam.rotation, cam.translation)
                * Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.));
        let view_m = Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.))
            * Mat4::from_scale_rotation_translation(Vec3::ONE, cam.rotation, cam.translation)
                .inverse()
            * Mat4::from_diagonal(Vec4::new(1., -1., 1., 1.));
        let view: Mat = view_m.to_cols_array_2d();
        let mut finished = Vec::new();
        let mut quads: Vec<(Arc<Dpd>, Vec<Quad>)> = Vec::new();
        let mut draws: Vec<(Arc<Dpd>, Vec<ModelDraw>)> = Vec::new();
        let ids: Vec<i16> = self.running.keys().copied().collect();
        for id in ids {
            let Some(r) = self.running.get_mut(&id) else {
                continue;
            };
            let q = r.instance.frame(&view, &mut self.rng);
            for &kind in &r.instance.unsupported {
                if !self.reported.contains(&kind) {
                    self.reported.push(kind);
                    self.log
                        .push(serde_json::json!({"event":"effect_unsupported",
                        "program":kind,"effect":id}));
                }
            }
            // An instance that runs a program this translation lacks is
            // stepped (so its timing holds) but not shown: a partial
            // effect would misrepresent the original.
            let complete = r.instance.unsupported.is_empty();
            let d = std::mem::take(&mut r.instance.model_draws);
            if complete && !q.is_empty() {
                quads.push((r.dpd.clone(), q));
            }
            if complete && !d.is_empty() {
                draws.push((r.dpd.clone(), d));
            }
            if r.instance.finished() {
                finished.push(id);
            }
        }
        for id in finished {
            self.running.remove(&id);
        }
        let scene = |p: [f32; 3]| to_scene.transform_point3(Vec3::from_array(p)).to_array();
        for (dpd, list) in quads {
            for q in list {
                let key = TexKey::new(&dpd, q.tex0);
                let size = key.size();
                let batch = self
                    .batches
                    .entry(BatchKey {
                        texture: Some(key),
                        additive: q.blend == 0x48,
                        repeat: true,
                    })
                    .or_default();
                let colour = [
                    q.colour[0].clamp(0., 1.99).powf(2.2),
                    q.colour[1].clamp(0., 1.99).powf(2.2),
                    q.colour[2].clamp(0., 1.99).powf(2.2),
                    q.colour[3].clamp(0., 1.),
                ];
                for i in [0, 1, 2, 0, 2, 3] {
                    batch.positions.push(scene(q.corners[i]));
                    batch
                        .uvs
                        .push([q.uv[i][0] / size[0] as f32, q.uv[i][1] / size[1] as f32]);
                    batch.colors.push(colour);
                }
            }
        }
        for (dpd, list) in draws {
            for d in list {
                self.model(&dpd, &d, &scene);
            }
        }
    }

    fn model(&mut self, dpd: &Arc<Dpd>, d: &ModelDraw, scene: &impl Fn([f32; 3]) -> [f32; 3]) {
        let cached = self
            .models
            .entry((Arc::as_ptr(dpd) as usize, d.model))
            .or_insert_with(|| {
                let m = dpd.models.get(&d.model)?;
                let header = effect::omd_header(&dpd.bytes, m.hdr).ok()?;
                let meshes = effect::omd_meshes(&dpd.bytes, m.hdr).ok()?;
                Some((header, meshes))
            });
        let Some((header, meshes)) = cached.as_ref() else {
            return;
        };
        let vertex = |i: u16| -> Option<[f32; 3]> {
            let v = match &d.vertices {
                Some(o) => o.get(i as usize).copied(),
                None => dpd.vertex(d.model, i),
            }?;
            let p = effect_vm::math::apply(&d.a, [v[0] as f32, v[1] as f32, v[2] as f32, 1.]);
            Some(scene([p[0], p[1], p[2]]))
        };
        let additive = header.alpha == 0x48;
        let repeat = header.flags & 4 != 0;
        let part = d.rgba.map(|c| c as f32 / 128.);
        for mesh in meshes {
            let texture = mesh.textured.then(|| TexKey::new(dpd, mesh.tex0));
            let shift = texture.map_or(4, |t| t.tw as u32 + 4).min(31);
            let batch = self
                .batches
                .entry(BatchKey {
                    texture,
                    additive,
                    repeat,
                })
                .or_default();
            for t in &mesh.triangles {
                let Some(p) = t.index.map(vertex).into_iter().collect::<Option<Vec<_>>>() else {
                    continue;
                };
                for (k, point) in p.iter().enumerate() {
                    batch.positions.push(*point);
                    let (u, v) = (t.uv[k][0] as i32, t.uv[k][1] as i32);
                    let su = d.scroll[0] >> shift;
                    let sv = d.scroll[1] >> (texture.map_or(4, |t| t.th as u32 + 4).min(31));
                    batch.uvs.push([
                        u.wrapping_add(su) as f32 / 4096.,
                        v.wrapping_add(sv) as f32 / 4096.,
                    ]);
                    // VS 005: sat(vertex/255 · part/128) · 255/128.
                    let c = t.colour[k];
                    let out: [f32; 4] = std::array::from_fn(|i| {
                        ((c[i] as f32 / 255.) * part[i]).min(1.) * (255. / 128.)
                    });
                    batch.colors.push([
                        out[0].powf(2.2),
                        out[1].powf(2.2),
                        out[2].powf(2.2),
                        out[3].min(1.),
                    ]);
                }
            }
        }
    }

    fn image(&mut self, key: TexKey, repeat: bool, images: &mut Assets<Image>) -> Handle<Image> {
        if let Some(h) = self.images.get(&key) {
            return h.clone();
        }
        let [w, h] = key.size();
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        // The DPD whose pointer the key holds is a running instance's.
        let own = self
            .running
            .values()
            .map(|r| &r.dpd)
            .chain(self.paxes.values().flat_map(|p| p.dpds.iter()))
            .find(|d| Arc::as_ptr(d) as usize == key.dpd)
            .cloned();
        let mut sources: Vec<&effect::DpdTexture> = Vec::new();
        if let Some(d) = &own {
            sources.extend(d.textures.iter().filter(|t| t.dbp == key.tbp));
        }
        if sources.is_empty()
            && let Some(c) = &self.common
        {
            sources.extend(c.textures.iter().filter(|t| t.dbp == key.tbp));
        }
        // The CLUT is the mesh's own CBP: the buffer's images are all
        // read through it, wherever their data was uploaded from.
        let palette = own
            .iter()
            .map(|d| d.as_ref())
            .chain(self.common.as_deref())
            .flat_map(|d| d.textures.iter())
            .find(|t| t.cbp == key.cbp)
            .map(|t| t.palette.clone());
        for t in sources {
            let rgba = effect::decode_psmt8(&t.indices, palette.as_deref().unwrap_or(&t.palette));
            for row in 0..t.height {
                let y = t.y as u32 + row;
                if y >= h {
                    break;
                }
                for col in 0..t.width {
                    let x = t.x as u32 + col;
                    if x >= w {
                        break;
                    }
                    let s = ((row * t.width + col) * 4) as usize;
                    let d = ((y * w + x) * 4) as usize;
                    pixels[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
                }
            }
        }
        let mut image = Image::new(
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        let mode = if repeat {
            ImageAddressMode::Repeat
        } else {
            ImageAddressMode::ClampToEdge
        };
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: mode,
            address_mode_v: mode,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            ..default()
        });
        let handle = images.add(image);
        self.images.insert(key, handle.clone());
        handle
    }
}

/// Rebuilds the effect meshes whenever a tick produced new geometry.
pub fn present(
    mut commands: Commands,
    mut fx: ResMut<Fx>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !fx.dirty {
        return;
    }
    fx.dirty = false;
    for e in std::mem::take(&mut fx.spawned) {
        commands.entity(e).despawn();
    }
    let batches = std::mem::take(&mut fx.batches);
    for (key, batch) in &batches {
        if batch.positions.is_empty() {
            continue;
        }
        let material = match fx.materials.get(key) {
            Some(m) => m.clone(),
            None => {
                let texture = key.texture.map(|t| fx.image(t, key.repeat, &mut images));
                let m = materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: texture,
                    unlit: true,
                    cull_mode: None,
                    alpha_mode: if key.additive {
                        AlphaMode::Add
                    } else {
                        AlphaMode::Blend
                    },
                    ..default()
                });
                fx.materials.insert(*key, m.clone());
                m
            }
        };
        let mesh = windows_shell::room::mesh(
            batch.positions.clone(),
            batch.uvs.clone(),
            batch.colors.clone(),
        );
        let entity = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material),
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        fx.spawned.push(entity);
    }
}
