// The Skate 3 score HUD: an APT colour transform on a texel,
// clamp(texel * multiply + add).
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct Transform { multiply: vec4<f32>, add: vec4<f32> }
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> colour: Transform;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var sheet: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var sheet_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(sheet, sheet_sampler, mesh.uv);
    return clamp(texel * colour.multiply + colour.add, vec4(0.0), vec4(1.0));
}
