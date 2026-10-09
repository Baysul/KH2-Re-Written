//! GPU drawing of menu 2D layouts (title, camp menu) in the original's
//! layout space: 512x416 PS2 units widened to 682.67x416 for 16:9 with no X
//! origin (title-01 §3.1), so x runs from -85.33 to 597.33.
//!
//! Colour math follows the exe's sprite shaders: texture x vertex colour
//! (vertex bytes / 128), clamped, blended in gamma space (the camera uses
//! `CompositingSpace::Srgb`). SEQD blend 0 is alpha, 1 additive, 2
//! subtractive (destination - source x alpha), as in the repo's gauge
//! canvas.
use bevy::{
    asset::{AssetPath, RenderAssetUsages, embedded_asset, embedded_path},
    camera::{CompositingSpace, ScalingMode},
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology},
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, Extent3d,
        RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureDimension, TextureFormat,
    },
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin},
};
use package_reader::sequence::Quad;
use std::collections::HashMap;

/// Visible layout width at 16:9 (512 x 4/3).
pub const WIDE_WIDTH: f32 = 512. * 4. / 3.;
pub const HEIGHT: f32 = 416.;
/// Layout x of the visible area's centre.
const CENTRE_X: f32 = 256.;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlendKey(u8);

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(BlendKey)]
pub struct LayoutMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,
    pub blend: u8,
}

impl From<&LayoutMaterial> for BlendKey {
    fn from(m: &LayoutMaterial) -> Self {
        BlendKey(m.blend)
    }
}

impl Material2d for LayoutMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(
            AssetPath::from_path_buf(embedded_path!("ui2d.wgsl")).with_source("embedded"),
        )
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let color = match key.bind_group_data.0 {
            1 => BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
            2 => BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::ReverseSubtract,
            },
            _ => BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
        };
        let alpha = BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState { color, alpha });
            }
        }
        Ok(())
    }
}

pub struct Ui2dPlugin;

impl Plugin for Ui2dPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "ui2d.wgsl");
        app.add_plugins(Material2dPlugin::<LayoutMaterial>::default())
            .init_resource::<Ui2dFrame>()
            .add_systems(Startup, spawn_camera)
            .add_systems(PostUpdate, present);
    }
}

/// A registered sheet: its GPU image, HD size and HD texels per sequence
/// texel.
#[derive(Clone)]
pub struct Sheet {
    pub image: Handle<Image>,
    pub size: [u32; 2],
    pub texel_scale: f32,
}

/// The sheets and this frame's draw list, in draw order.
#[derive(Resource, Default)]
pub struct Ui2dFrame {
    pub sheets: Vec<Sheet>,
    pub draws: Vec<(usize, Vec<Quad>)>,
    materials: HashMap<(usize, u8), Handle<LayoutMaterial>>,
}

impl Ui2dFrame {
    /// Uploads straight RGBA as a gamma-space (non-sRGB) bilinear sheet.
    pub fn add_sheet(
        &mut self,
        images: &mut Assets<Image>,
        rgba: Vec<u8>,
        size: [u32; 2],
        texel_scale: f32,
    ) -> usize {
        let mut image = Image::new(
            Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            ..default()
        });
        self.sheets.push(Sheet {
            image: images.add(image),
            size,
            texel_scale,
        });
        self.sheets.len() - 1
    }

    /// Re-points a sheet slot at another upload (e.g. a device-glyph swap).
    pub fn rebind(&mut self, slot: usize, sheet: Sheet) {
        self.sheets[slot] = sheet;
        self.materials.retain(|(s, _), _| *s != slot);
    }

    /// A full-screen solid rectangle (screen fades); needs a white sheet.
    pub fn fill(&mut self, white: usize, rgba: [f32; 4]) {
        let (l, r) = (CENTRE_X - WIDE_WIDTH, CENTRE_X + WIDE_WIDTH);
        self.draws.push((
            white,
            vec![Quad {
                corners: [
                    [l, -HEIGHT],
                    [r, -HEIGHT],
                    [l, 2. * HEIGHT],
                    [r, 2. * HEIGHT],
                ],
                uv: [0, 0, 1, 1],
                colors: [rgba; 4],
                blend: 0,
            }],
        ));
    }
}

#[derive(Component)]
struct Slot;

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        CompositingSpace::Srgb,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: WIDE_WIDTH,
                min_height: HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

fn quad_mesh(quads: &[&Quad], sheet: &Sheet) -> Mesh {
    let mut positions = Vec::with_capacity(quads.len() * 4);
    let mut uvs = Vec::with_capacity(quads.len() * 4);
    let mut colors = Vec::with_capacity(quads.len() * 4);
    let mut indices = Vec::with_capacity(quads.len() * 6);
    let [w, h] = sheet.size.map(|v| v as f32);
    for q in quads {
        let base = positions.len() as u32;
        let [u0, v0, u1, v1] = q.uv.map(|v| v as f32 * sheet.texel_scale);
        for (k, [x, y]) in q.corners.into_iter().enumerate() {
            positions.push([x - CENTRE_X, HEIGHT / 2. - y, 0.]);
            let (u, v) = (
                if k % 2 == 0 { u0 } else { u1 },
                if k < 2 { v0 } else { v1 },
            );
            uvs.push([u / w, v / h]);
            colors.push(q.colors[k]);
        }
        indices.extend([0, 1, 2, 1, 3, 2].map(|i| base + i));
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// Draws this frame's list: one mesh per run of quads sharing a sheet and a
/// blend, stacked in list order; then clears the list.
fn present(
    mut commands: Commands,
    mut frame: ResMut<Ui2dFrame>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LayoutMaterial>>,
    mut slots: Query<
        (
            &Mesh2d,
            &mut MeshMaterial2d<LayoutMaterial>,
            &mut Transform,
            &mut Visibility,
        ),
        With<Slot>,
    >,
) {
    let frame = &mut *frame;
    let mut runs: Vec<(usize, u8, Vec<&Quad>)> = Vec::new();
    for (sheet, quads) in &frame.draws {
        for q in quads {
            let blend = q.blend.clamp(0, 2) as u8;
            match runs.last_mut() {
                Some((s, b, list)) if *s == *sheet && *b == blend => list.push(q),
                _ => runs.push((*sheet, blend, vec![q])),
            }
        }
    }
    let mut existing = slots.iter_mut();
    for (order, (sheet, blend, quads)) in runs.iter().enumerate() {
        let mesh = quad_mesh(quads, &frame.sheets[*sheet]);
        let material = frame
            .materials
            .entry((*sheet, *blend))
            .or_insert_with(|| {
                materials.add(LayoutMaterial {
                    texture: frame.sheets[*sheet].image.clone(),
                    blend: *blend,
                })
            })
            .clone();
        let z = order as f32 * 0.01;
        match existing.next() {
            Some((handle, mut m, mut t, mut v)) => {
                if let Some(mut slot) = meshes.get_mut(&handle.0) {
                    *slot = mesh;
                }
                if m.0 != material {
                    m.0 = material;
                }
                t.translation.z = z;
                *v = Visibility::Visible;
            }
            None => {
                commands.spawn((
                    Slot,
                    Mesh2d(meshes.add(mesh)),
                    MeshMaterial2d(material),
                    Transform::from_xyz(0., 0., z),
                    Visibility::Visible,
                ));
            }
        }
    }
    for (_, _, _, mut v) in existing {
        *v = Visibility::Hidden;
    }
    frame.draws.clear();
}
