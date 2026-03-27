// bloom.wgsl - Bloom prefilter, blur, and final shaders
// Bloom creates a "glow" effect around bright areas.
// It works by:
// 1. Extracting pixels brighter than a threshold (Prefilter).
// 2. Blurring these pixels (often multiple times at different scales).
// 3. Adding the blurred result back to the original image.

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

// ===== PREFILTER =====
// Extracts bright pixels to be blurred. Uses a "soft knee" threshold curve
// to avoid harsh transitions where the glow starts.

struct BloomPrefilterUniforms {
    texel_size: vec2<f32>,
    curve: vec4<f32>, // x: threshold - knee, y: knee * 2, z: 0.25 / knee
    threshold: f32,
    _pad: f32,
};

@group(0) @binding(0)
var<uniform> prefilter_uniforms: BloomPrefilterUniforms;

@group(0) @binding(1)
var u_texture: texture_2d<f32>;
@group(0) @binding(2)
var u_sampler: sampler;

@fragment
fn fs_prefilter(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Downsample: Sample 4 neighbors to get average brightness of the area
    let dt = prefilter_uniforms.texel_size * 2.0;
    var s = vec3<f32>(0.0);
    s += textureSample(u_texture, u_sampler, in.uv - dt).xyz;
    s += textureSample(u_texture, u_sampler, in.uv + vec2<f32>(dt.x, -dt.y)).xyz;
    s += textureSample(u_texture, u_sampler, in.uv + dt).xyz;
    s += textureSample(u_texture, u_sampler, in.uv + vec2<f32>(-dt.x, dt.y)).xyz;
    s *= 0.25;
    
    // 2. Thresholding with Soft Knee
    // Standard threshold: max(brightness - threshold, 0)
    // Soft knee: smooths the transition around the threshold
    let brightness = max(s.x, max(s.y, s.z));
    
    // Calculate contribution curve
    var rq = clamp(brightness - prefilter_uniforms.curve.x, 0.0, prefilter_uniforms.curve.y);
    rq = prefilter_uniforms.curve.z * rq * rq;
    
    // Apply threshold
    s *= max(rq, brightness - prefilter_uniforms.threshold) / max(brightness, 0.0001);
    
    return vec4<f32>(s, 1.0);
}

// ===== BLUR (used for both downsample and upsample) =====
// Simple 4-tap box filter/tent filter.
// When applied repeatedly in a mipmap chain, it approximates a Gaussian blur efficiently.

struct BloomBlurUniforms {
    texel_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> blur_uniforms: BloomBlurUniforms;

@fragment
fn fs_blur(in: VertexOutput) -> @location(0) vec4<f32> {
    let ts = blur_uniforms.texel_size;
    var s = vec4<f32>(0.0);
    s += textureSample(u_texture, u_sampler, in.uv - ts) * 0.25;
    s += textureSample(u_texture, u_sampler, in.uv + vec2<f32>(ts.x, -ts.y)) * 0.25;
    s += textureSample(u_texture, u_sampler, in.uv + ts) * 0.25;
    s += textureSample(u_texture, u_sampler, in.uv + vec2<f32>(-ts.x, ts.y)) * 0.25;
    return s;
}

// ===== FINAL =====
// Upsamples the bloom texture and blends it.

struct BloomFinalUniforms {
    texel_size: vec2<f32>,
    intensity: f32,
    _pad: f32,
};

@group(0) @binding(0)
var<uniform> final_uniforms: BloomFinalUniforms;

@fragment
fn fs_final(in: VertexOutput) -> @location(0) vec4<f32> {
    let ts = final_uniforms.texel_size;
    var s = vec4<f32>(0.0);
    s += textureSample(u_texture, u_sampler, in.uv - ts) * 0.25;
    s += textureSample(u_texture, u_sampler, in.uv + vec2<f32>(ts.x, -ts.y)) * 0.25;
    s += textureSample(u_texture, u_sampler, in.uv + ts) * 0.25;
    s += textureSample(u_texture, u_sampler, in.uv + vec2<f32>(-ts.x, ts.y)) * 0.25;
    return s * final_uniforms.intensity;
}
