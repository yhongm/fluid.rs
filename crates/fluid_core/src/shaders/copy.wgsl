// copy.wgsl - Copy a texture (used to copy sunrays temp back)

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

struct CopyUniforms {
    texel_size: vec2<f32>,
    value: f32,
    _pad: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: CopyUniforms;

@group(0) @binding(1)
var u_texture: texture_2d<f32>;
@group(0) @binding(2)
var u_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(u_texture, u_sampler, in.uv) * uniforms.value;
}
