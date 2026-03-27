// blur.wgsl - Separable Gaussian blur (for smoothing sunrays)

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

struct BlurUniforms {
    texel_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: BlurUniforms;

@group(0) @binding(1)
var u_texture: texture_2d<f32>;
@group(0) @binding(2)
var u_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 3-tap Gaussian approximation, used twice (H then V)
    let uv = in.uv;
    let off = uniforms.texel_size;
    
    var color = textureSample(u_texture, u_sampler, uv) * 0.29411764;
    color += textureSample(u_texture, u_sampler, uv + off) * 0.35294117;
    color += textureSample(u_texture, u_sampler, uv - off) * 0.35294117;
    return color;
}
