//! Pre-tessellated APT shape geometry, .geo (tools/vendor/skate3_ui/geo.py).
use super::binary::{Reader, bad};
use crate::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub render_type: u32,
    pub color: [f64; 4],
    pub texture_id: u32,
    pub uv_matrix: [f64; 6],
    pub triangles: Vec<[[f64; 2]; 3]>,
}

impl Unit {
    pub fn render_type_name(&self) -> String {
        match self.render_type {
            0 => "line".into(),
            1 => "solid".into(),
            2 => "texture_clamped".into(),
            3 => "texture_wrapped".into(),
            n => format!("unknown_{n}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub id: u32,
    pub units: Vec<Unit>,
}

pub fn parse_geo(data: &[u8]) -> Result<Vec<Shape>, Error> {
    let r = Reader::new(data);
    // Retail bundles use an eight-byte header as the canonical empty GEO.
    if data.len() == 8 && r.u32be(4)? == 0 {
        return Ok(Vec::new());
    }
    r.require(0, 12)?;
    let count = r.at(4)?;
    if count > 4096 {
        return Err(bad("unreasonable GEO shape count"));
    }
    r.require(8, count * 4)?;
    let mut shapes = Vec::new();
    for index in 0..count {
        let shape_offset = r.at(8 + index * 4)?;
        if shape_offset == 0 {
            continue;
        }
        r.require(shape_offset, 8)?;
        let id = r.u32be(shape_offset)?;
        let unit_count = r.at(shape_offset + 4)?;
        if unit_count > 65536 {
            return Err(bad("unreasonable GEO unit count"));
        }
        r.require(shape_offset + 8, unit_count * 4)?;
        let mut units = Vec::with_capacity(unit_count);
        for unit_index in 0..unit_count {
            let u = r.at(shape_offset + 8 + unit_index * 4)?;
            r.require(u, 0x34)?;
            let render_type = r.u32be(u)?;
            let mut color = [0.0; 4];
            for (c, slot) in color.iter_mut().enumerate() {
                *slot = r.f32be(u + 4 + c * 4)?;
            }
            let texture_id = r.u32be(u + 0x14)?;
            let mut uv_matrix = [0.0; 6];
            for (c, slot) in uv_matrix.iter_mut().enumerate() {
                *slot = r.f32be(u + 0x18 + c * 4)?;
            }
            let primitive_count = r.at(u + 0x30)?;
            if primitive_count > 1_000_000 {
                return Err(bad("unreasonable GEO primitive count"));
            }
            r.require(u + 0x34, primitive_count * 0x18)?;
            let mut triangles = Vec::with_capacity(primitive_count);
            for primitive in 0..primitive_count {
                let base = u + 0x34 + primitive * 0x18;
                let mut triangle = [[0.0; 2]; 3];
                for (v, point) in triangle.iter_mut().enumerate() {
                    *point = [r.f32be(base + v * 8)?, r.f32be(base + v * 8 + 4)?];
                }
                triangles.push(triangle);
            }
            units.push(Unit {
                render_type,
                color,
                texture_id,
                uv_matrix,
                triangles,
            });
        }
        shapes.push(Shape { id, units });
    }
    Ok(shapes)
}
