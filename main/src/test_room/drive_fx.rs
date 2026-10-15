//! Sora's own object effects in the test room: the PAX of his object
//! archive (`obj/P_EX100.a.us`, and each form's) started by his motions'
//! frame trigger 1 (FUN_14040dd70 case 1 → FUN_1402c6b30), the revert's
//! elements 15..27 (FUN_140405630) and the form level-up's system element
//! 16 (FUN_1403ed390). Bound elements follow Sora's bone (or his object
//! for bones from 0x4000) every tick; an element's own sound plays at its
//! start.
//!
//! The drawing follows the event player's effect presenter
//! (`cutscene/fx.rs`): per-DPD textures by TEX0 (the element's own buffers
//! shadow the resident `effect/texcommon.dpd`), OMD model draws, blend and
//! layer batches.
use super::*;
use ::effect::{Instance, ModelDraw, Quad, Rng, math::Mat};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSamplerDescriptor};
use data::effect::{Dpd, OmdHeader, OmdMesh, Pax};

/// A start request: the PAX, the element id, and whether it binds to Sora.
pub struct FxRequest {
    pub pax: Arc<Pax>,
    pub id: u16,
    /// The kit (form) whose object owns the effect; 0xff for the system
    /// PAX (it stays when the object is swapped).
    pub owner: u8,
}

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
    layer: i8,
}

struct Bind {
    bone: i16,
    local: Mat,
}

struct Running {
    instance: Instance,
    dpd: Arc<Dpd>,
    bind: Option<Bind>,
    owner: u8,
    /// The element's sound (sound > 0) and the ticks until it plays: it
    /// starts with the element, after its start wait.
    sound: Option<u32>,
    sound_wait: f32,
}

/// Odd layers behind everything, even layers in front, 0 by depth
/// (FUN_1402c2720 / FUN_1402c27a0, as the event presenter ranks them).
fn layer_rank(layer: u8) -> i8 {
    match layer {
        0 => 0,
        v if v % 2 == 1 => -((v as i8 + 1) / 2),
        v => (v / 2) as i8,
    }
}

/// Scene matrix into the test room's game space (scene = (x, −y, −z)).
fn to_game(m: Mat4) -> Mat {
    let flip = Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.));
    (flip * m * flip).to_cols_array_2d()
}

#[derive(Resource, Default)]
pub struct DriveFx {
    common: Option<Arc<Dpd>>,
    running: Vec<Running>,
    rng: Rng,
    models: HashMap<(usize, i16), Option<ParsedModel>>,
    batches: HashMap<BatchKey, Batch>,
    dirty: bool,
    images: HashMap<TexKey, Handle<Image>>,
    materials: HashMap<BatchKey, Handle<StandardMaterial>>,
    spawned: Vec<Entity>,
    /// DPDs seen, for the texture lookup by key.
    dpds: Vec<Arc<Dpd>>,
    reported: Vec<u32>,
}

impl DriveFx {
    pub fn new(common: Option<Dpd>) -> Self {
        DriveFx {
            common: common.map(Arc::new),
            ..Default::default()
        }
    }
}

/// Sora's scene-space object matrix and the bone matrices of this frame.
fn sora_pose(game: &Game) -> (Mat4, Option<&HashMap<usize, Mat4>>) {
    let p = game.player.position;
    let root = Mat4::from_rotation_translation(
        Quat::from_rotation_y(game.player.yaw),
        Vec3::from_array(p),
    );
    let clip = game.poses.get(&(game.player.action, game.player.armed));
    let frame = game.player.motion_frame(&game.clips) as usize;
    let bones = clip
        .and_then(|&c| game.clip_poses.get(c))
        .and_then(|c| c.body.get(frame.min(c.body.len().saturating_sub(1))));
    (root, bones)
}

/// FUN_1402c49d0 for a bound element: the bone's matrix (the bone read
/// through FUN_1401a7af0 negates its y and z columns) or the object's.
fn bound_world(root: Mat4, bones: Option<&HashMap<usize, Mat4>>, bind: &Bind) -> Mat {
    let m = if bind.bone < 0 || bind.bone >= 0x4000 {
        root
    } else {
        match bones.and_then(|b| b.get(&(bind.bone as usize))) {
            Some(b) => root * *b * Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.)),
            None => root,
        }
    };
    ::effect::math::mul(&to_game(m), &bind.local)
}

/// One tick: start the queued elements, follow Sora, step every instance
/// and batch what it drew.
pub fn tick(mut game: ResMut<Game>, rig: Res<CameraRig>, mut fx: ResMut<DriveFx>) {
    let Some(d) = game.drive.as_mut() else { return };
    let requests = std::mem::take(&mut d.fx_requests);
    let swapped = d.fx_owner_left.take();
    let (root, bones) = sora_pose(&game);
    let mut sounds = Vec::new();
    let mut log = Vec::new();
    let tick = game.tick;
    // Waiting effect sounds count down one tick at a time, as the instance's
    // start wait does, and play when their element starts.
    for r in fx.running.iter_mut() {
        if let Some(id) = r.sound
            && r.sound_wait > 0.
        {
            r.sound_wait -= 1.;
            if r.sound_wait <= 0. {
                sounds.push(id);
                r.sound = None;
            }
        }
    }
    // An object's effects fade with it when it is replaced (the swap
    // re-points only elements with flag 0x200, FUN_1402c4480).
    if let Some(old) = swapped {
        for r in fx.running.iter_mut().filter(|r| r.owner == old) {
            r.instance.fade_out(15.);
        }
    }
    for req in requests {
        let Some(element) = req.pax.by_id(req.id) else {
            continue;
        };
        let Some(dpx) = req.pax.effect(element.effect_number) else {
            continue;
        };
        let local = ::effect::instance_matrix(element.translation, element.rotation, element.scale);
        let bind = (element.flags & 1 != 0).then_some(Bind {
            bone: element.bone,
            local,
        });
        let world = match &bind {
            Some(b) => bound_world(root, bones, b),
            None => ::effect::math::mul(&to_game(root), &local),
        };
        // A bound element waits one extra tick (DAT_1406239f8, 0.1).
        let wait = element.start_wait + if bind.is_some() { 0.1 } else { 0. };
        let dpd = req.pax.dpds[dpx.dpd].clone();
        // A sound plays only for sound > 0 (the element's SE id), at the
        // element's start: now when it has no wait.
        let sound = (element.sound > 0).then_some(element.sound as u32);
        if let Some(id) = sound
            && wait <= 0.
        {
            sounds.push(id);
        }
        log.push(serde_json::json!({"event":"drive_effect","tick":tick,"element":req.id,
            "owner":req.owner,"bone":element.bone,"flags":element.flags,"dpd":dpx.dpd,"pdata":dpx.pdata}));
        if let Some(instance) = Instance::new(dpd.clone(), dpx.pdata, world, wait) {
            if !fx.dpds.iter().any(|x| Arc::ptr_eq(x, &dpd)) {
                fx.dpds.push(dpd.clone());
            }
            fx.running.push(Running {
                instance,
                dpd,
                bind,
                owner: req.owner,
                sound: sound.filter(|_| wait > 0.),
                sound_wait: wait,
            });
        }
    }
    for s in sounds {
        game.sound_queue.push((s, "drive_effect_se"));
    }
    for v in log {
        game.event(v);
    }
    let view = game_view(rig.camera.eye, rig.camera.look_at);
    let view_inverse = Mat4::from_cols_array_2d(&view).inverse();
    let (root, bones) = sora_pose(&game);
    let fx = &mut *fx;
    fx.batches.clear();
    fx.dirty = true;
    let mut quads: Vec<(Arc<Dpd>, i8, Vec<Quad>)> = Vec::new();
    let mut draws: Vec<(Arc<Dpd>, i8, Vec<ModelDraw>)> = Vec::new();
    let mut triangles: Vec<(i8, Vec<::effect::Triangle>)> = Vec::new();
    let mut unsupported = Vec::new();
    for r in fx.running.iter_mut() {
        if let Some(b) = &r.bind {
            r.instance.world = bound_world(root, bones, b);
        }
        let q = r.instance.frame(&view, &mut fx.rng);
        for &k in &r.instance.unsupported {
            if !fx.reported.contains(&k) {
                fx.reported.push(k);
                unsupported.push(k);
            }
        }
        // A program this translation lacks: stepped, not shown.
        let complete = r.instance.unsupported.is_empty();
        let d = std::mem::take(&mut r.instance.model_draws);
        let layer = layer_rank(r.instance.layer());
        if complete && !q.is_empty() {
            quads.push((r.dpd.clone(), layer, q));
        }
        if complete && !d.is_empty() {
            draws.push((r.dpd.clone(), layer, d));
        }
        let t = std::mem::take(&mut r.instance.triangles);
        if complete && !t.is_empty() {
            triangles.push((layer, t));
        }
    }
    fx.running.retain(|r| !r.instance.finished());
    for k in unsupported {
        game.event(serde_json::json!({"event":"drive_effect_unsupported","tick":tick,"program":k}));
    }
    let scene = |p: [f32; 3]| {
        from_world(
            view_inverse
                .transform_point3(Vec3::from_array(p))
                .to_array(),
        )
        .to_array()
    };
    for (dpd, layer, list) in quads {
        for q in list {
            let key = TexKey::new(&dpd, q.tex0);
            let size = key.size();
            let batch = fx
                .batches
                .entry(BatchKey {
                    texture: Some(key),
                    additive: q.blend == 0x48,
                    repeat: true,
                    layer,
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
    // KeTh lightning: untextured gouraud triangles (GS colour bytes, 0x80 =
    // 1.0); a subtractive blend (0x42) is drawn as alpha [I].
    for (layer, list) in triangles {
        for t in list {
            let batch = fx
                .batches
                .entry(BatchKey {
                    texture: None,
                    additive: t.blend == 0x48,
                    repeat: false,
                    layer,
                })
                .or_default();
            for i in 0..3 {
                let c = t.colours[i].map(|v| v as f32 / 128.);
                batch.positions.push(scene(t.corners[i]));
                batch.uvs.push([0.; 2]);
                batch.colors.push([
                    c[0].clamp(0., 1.99).powf(2.2),
                    c[1].clamp(0., 1.99).powf(2.2),
                    c[2].clamp(0., 1.99).powf(2.2),
                    c[3].clamp(0., 1.),
                ]);
            }
        }
    }
    for (dpd, layer, list) in draws {
        for d in list {
            model(fx, &dpd, &d, layer, &scene);
        }
    }
}

/// An OMD model draw (pppDrawMdl3), as the event presenter batches it.
fn model(
    fx: &mut DriveFx,
    dpd: &Arc<Dpd>,
    d: &ModelDraw,
    layer: i8,
    scene: &impl Fn([f32; 3]) -> [f32; 3],
) {
    let cached = fx
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
        let p = ::effect::math::apply(&d.a, [v[0] as f32, v[1] as f32, v[2] as f32, 1.]);
        Some(scene([p[0], p[1], p[2]]))
    };
    let additive = header.alpha == 0x48;
    let repeat = header.flags & 4 != 0;
    let part = d.rgba.map(|c| c as f32 / 128.);
    let mut out_batches: Vec<(BatchKey, Batch)> = Vec::new();
    for mesh in meshes {
        let texture = mesh.textured.then(|| TexKey::new(dpd, mesh.tex0));
        let shift = texture.map_or(4, |t| t.tw as u32 + 4).min(31);
        let mut batch = Batch::default();
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
        out_batches.push((
            BatchKey {
                texture,
                additive,
                repeat,
                layer,
            },
            batch,
        ));
    }
    for (key, b) in out_batches {
        let e = fx.batches.entry(key).or_default();
        e.positions.extend(b.positions);
        e.uvs.extend(b.uvs);
        e.colors.extend(b.colors);
    }
}

fn image(fx: &mut DriveFx, key: TexKey, repeat: bool, images: &mut Assets<Image>) -> Handle<Image> {
    if let Some(h) = fx.images.get(&key) {
        return h.clone();
    }
    let [w, h] = key.size();
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    let own = fx
        .dpds
        .iter()
        .find(|d| Arc::as_ptr(d) as usize == key.dpd)
        .cloned();
    let mut sources: Vec<&effect::DpdTexture> = Vec::new();
    if let Some(d) = &own {
        sources.extend(d.textures.iter().filter(|t| t.dbp == key.tbp));
    }
    if sources.is_empty()
        && let Some(c) = &fx.common
    {
        sources.extend(c.textures.iter().filter(|t| t.dbp == key.tbp));
    }
    let palette = own
        .iter()
        .map(|d| d.as_ref())
        .chain(fx.common.as_deref())
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
    fx.images.insert(key, handle.clone());
    handle
}

/// Rebuilds the effect meshes from this tick's batches.
pub fn present(
    mut commands: Commands,
    fx: Option<ResMut<DriveFx>>,
    scene: Option<Res<SceneRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut fx) = fx else { return };
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
                let texture = key
                    .texture
                    .map(|t| image(&mut fx, t, key.repeat, &mut images));
                let m = materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: texture,
                    unlit: true,
                    cull_mode: None,
                    depth_bias: key.layer as f32 * 500.,
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
        let mesh = visual::room::mesh(
            batch.positions.clone(),
            batch.uvs.clone(),
            batch.colors.clone(),
        );
        let mut e = commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::default(),
        ));
        if let Some(s) = &scene {
            e.insert(ChildOf(s.0));
        }
        fx.spawned.push(e.id());
    }
}
