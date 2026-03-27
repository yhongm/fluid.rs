// sunrays.wgsl - Tyndall/god-ray effect: radial light scatter
// Simulates volumetric light rays (God Rays) emanating from a source.
// Algorithm: Radial Blur (Volumetric Scattering approximation)
// 1. fs_mask: Prepares an occlusion mask. Bright areas (dye) block light (or emit it, depending on interpretation).
//    Here, it seems '1.0 - brightness' implies dye OCCLUDES the light background.
// 2. fs_sunrays: Performs a radial blur by sampling the mask along a vector towards the light center.

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

struct SunraysUniforms {
    texel_size: vec2<f32>,
    weight: f32, // Intensity of the rays
    exposure: f32,
    decay: f32,
    _pad: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: SunraysUniforms;

@group(0) @binding(1)
var u_texture: texture_2d<f32>;
@group(0) @binding(2)
var u_sampler: sampler;

// Sunrays mask: derive occlusion from dye luminance
@fragment
fn fs_mask(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(u_texture, u_sampler, in.uv);
    let br = max(c.x, max(c.y, c.z));
    let a = 1.0 - clamp(br * 20.0, 0.0, 0.8);
    return vec4<f32>(a);
}

@fragment
fn fs_sunrays(in: VertexOutput) -> @location(0) vec4<f32> {
    let density = 0.3;
    let decay = uniforms.decay;
    let exposure = uniforms.exposure;
    let weight = uniforms.weight;
    let ITERATIONS = 16;

    let coord = in.uv;
    var d = (coord - 0.5) * density / f32(ITERATIONS);
    var c = textureSample(u_texture, u_sampler, in.uv).x;
    var illuminationDecay = 1.0;
    
    var current_coord = coord;
    for (var i = 0; i < ITERATIONS; i++) {
        current_coord -= d;
        let s = textureSample(u_texture, u_sampler, current_coord).x;
        c += s * illuminationDecay * weight;
        illuminationDecay *= decay;
    }
    
    return vec4<f32>(c * exposure);
}
