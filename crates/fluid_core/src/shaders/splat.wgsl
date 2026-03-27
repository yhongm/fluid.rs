// splat.wgsl - Gaussian splat to inject velocity/color at a point

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

struct SplatUniforms {
    point: vec2<f32>,
    _pad1: vec2<f32>,
    color: vec4<f32>,
    radius: f32,
    aspect_ratio: f32,
    _pad2: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: SplatUniforms;

@group(0) @binding(1)
var u_target: texture_2d<f32>;
@group(0) @binding(2)
var u_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var p = in.uv - uniforms.point;
    p.x *= uniforms.aspect_ratio;
    
    let splat = exp(-dot(p, p) / uniforms.radius) * uniforms.color.xyz;
    let base = textureSample(u_target, u_sampler, in.uv).xyz;
    
    return vec4<f32>(base + splat, 1.0);
}
