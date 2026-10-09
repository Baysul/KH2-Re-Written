//! Source-bound room loading shared by the room preview and test room.
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
};
use package_reader::{
    bar,
    model_texture_pixels::{self, DecodedTexture},
    original,
    probe::ProbeSource,
    room_collision,
    room_geometry::{self, RoomModel},
};
use std::{fs::File, io::Read, path::Path};

pub struct Room {
    pub model: RoomModel,
    pub textures: Vec<DecodedTexture>,
}
pub struct LoadedRoom {
    pub rooms: Vec<Room>,
    pub frame_points: Vec<Vec3>,
    pub inspection_ground: Option<f32>,
    /// Collision display triangles in scene coordinates (pinned MapStudio
    /// GenerateVertex conversion).
    pub collision_triangles: Vec<CollisionTriangle>,
    /// Camera collision (BAR type 11, COCT layout) triangles in scene
    /// coordinates.
    pub camera_triangles: Vec<[Vec3; 3]>,
}
pub struct CollisionTriangle {
    pub polygon: usize,
    /// Raw COCT surface flags word (public labels: bit 0 partyStand,
    /// bit 4 partyCollide).
    pub surface_flags: u32,
    pub corners: [Vec3; 3],
}

/// Pinned MapStudio CollisionModel.GenerateVertex display conversion.
fn scene_point(bits: [u32; 4]) -> Vec3 {
    let [x, y, z, _] = bits.map(f32::from_bits);
    Vec3::new(x, -y, -z)
}

pub fn load_rooms(
    source: &ProbeSource,
    selection: &Path,
) -> Result<LoadedRoom, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(selection)?
        .take(1048577)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1048576 {
        return Err("room selection too large".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let items = value["selections"]
        .as_array()
        .ok_or("room selection absent")?;
    if value["schema_version"] != 1
        || value["source_content_identity"] != source.content_identity()
        || items.len() != 1
    {
        return Err("one source-bound room selection required".into());
    }
    let item = &items[0];
    let mut index = source.open_index(item["index"].as_str().ok_or("room index absent")?)?;
    let ordinal = usize::try_from(item["ordinal"].as_u64().ok_or("room ordinal absent")?)?;
    let entry = index
        .inventory
        .entries
        .get(ordinal)
        .ok_or("room entry absent")?;
    if serde_json::to_value(entry.name_hash)? != item["name_hash"] {
        return Err("room identity mismatch".into());
    }
    let archive = original::read_original(&mut index.reader, entry)?;
    let table = bar::read_bar(&archive.bytes)?;
    let mut rooms = Vec::new();
    let mut frame_points = Vec::new();
    let mut inspection_ground = None;
    let mut collision_triangles = Vec::new();
    for entry in table
        .entries
        .iter()
        .filter(|e| e.entry_type == 6 && e.offset > 0 && e.size > 0)
    {
        let collision = room_collision::read(
            &archive.bytes[entry.offset as usize..entry.offset as usize + entry.size as usize],
        )?;
        frame_points.extend(collision.vertex_bits.iter().map(|&p| scene_point(p)));
        for (polygon, [a, b, c]) in collision.triangle_indices()? {
            let attribute = collision.polygons[polygon].attribute;
            let surface_flags = usize::try_from(attribute)
                .ok()
                .and_then(|i| collision.attributes.get(i))
                .copied()
                .ok_or("collision surface flags index outside table")?;
            collision_triangles.push(CollisionTriangle {
                polygon,
                surface_flags,
                corners: [a, b, c].map(|i| scene_point(collision.vertex_bits[i as usize])),
            });
        }
        if let Some(polygon) = item.get("inspection_ground_polygon") {
            if inspection_ground.is_some() {
                return Err("inspection ground polygon ambiguous across collision blocks".into());
            }
            let ordinal = usize::try_from(
                polygon
                    .as_u64()
                    .ok_or("inspection polygon ordinal invalid")?,
            )?;
            let polygon = collision
                .polygons
                .get(ordinal)
                .ok_or("inspection ground polygon absent")?;
            let heights: Vec<_> = polygon
                .vertices
                .iter()
                .filter(|&&i| i >= 0)
                .map(|&i| -f32::from_bits(collision.vertex_bits[i as usize][1]))
                .collect();
            if heights.len() < 3 || heights.iter().any(|&y| !y.is_finite() || y != heights[0]) {
                return Err(
                    "inspection grounding requires an explicitly selected horizontal polygon"
                        .into(),
                );
            }
            inspection_ground = Some(heights[0]);
        }
    }
    let mut camera_triangles = Vec::new();
    for entry in table
        .entries
        .iter()
        .filter(|e| e.entry_type == 11 && e.offset > 0 && e.size > 0)
    {
        let collision = room_collision::read(
            &archive.bytes[entry.offset as usize..entry.offset as usize + entry.size as usize],
        )?;
        for (_, corners) in collision.triangle_indices()? {
            camera_triangles.push(corners.map(|i| scene_point(collision.vertex_bits[i as usize])));
        }
    }
    let mut rgba_bytes = 0;
    for entry in table
        .entries
        .iter()
        .filter(|e| e.entry_type == 4 && e.offset > 0 && e.size > 0)
    {
        let payload =
            &archive.bytes[entry.offset as usize..entry.offset as usize + entry.size as usize];
        if payload.get(0x90..0x94) != Some(2u32.to_le_bytes().as_slice()) {
            continue;
        }
        let model = room_geometry::read(payload)?;
        let textures: Vec<_> = table
            .entries
            .iter()
            .filter(|e| {
                e.entry_type == 7 && e.name_bytes == entry.name_bytes && e.offset > 0 && e.size > 0
            })
            .collect();
        if textures.len() != 1 {
            return Err("room requires unique same-name texture block".into());
        }
        let entry = textures[0];
        let textures = model_texture_pixels::decode_images(
            &archive.bytes[entry.offset as usize..entry.offset as usize + entry.size as usize],
        )?;
        rgba_bytes += textures.iter().map(|t| t.pixels_rgba.len()).sum::<usize>();
        if rgba_bytes > 128 * 1024 * 1024 {
            return Err("room textures exceed cumulative limit".into());
        }
        rooms.push(Room { model, textures });
    }
    index.verify_unchanged()?;
    if rooms.is_empty() {
        return Err("no decoded room model".into());
    }
    if frame_points.is_empty() || frame_points.iter().any(|p| !p.is_finite()) {
        return Err("finite collision area required for scene framing".into());
    }
    Ok(LoadedRoom {
        rooms,
        frame_points,
        inspection_ground,
        collision_triangles,
        camera_triangles,
    })
}

pub fn upload(texture: &DecodedTexture, images: &mut Assets<Image>) -> Handle<Image> {
    rgba_image(
        texture.width,
        texture.height,
        texture.pixels_rgba.clone(),
        images,
    )
}

pub fn rgba_image(
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    images: &mut Assets<Image>,
) -> Handle<Image> {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    images.add(image)
}

pub fn mesh(positions: Vec<[f32; 3]>, uvs: Vec<[f32; 2]>, colors: Vec<[f32; 4]>) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
}
