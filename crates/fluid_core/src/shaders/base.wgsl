// base.wgsl - Base vertex shader for fullscreen quad rendering

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

struct Uniforms {
    texel_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle trick
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );

    var out: VertexOutput;
    let pos = positions[vertex_index];
    let uv = uvs[vertex_index];
    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    out.v_l = uv - vec2<f32>(uniforms.texel_size.x, 0.0);
    out.v_r = uv + vec2<f32>(uniforms.texel_size.x, 0.0);
    out.v_t = uv + vec2<f32>(0.0, uniforms.texel_size.y);
    out.v_b = uv - vec2<f32>(0.0, uniforms.texel_size.y);
    return out;
}
