// advection.wgsl - Semi-Lagrangian advection shader
//
// Advection is the process of moving quantities (velocity, dye) along the flow.
// This shader implements the Semi-Lagrangian method, which is unconditionally stable
// (it won't blow up even with large time steps), but it is dissipative (blurs the result over time).
//
// Algorithm:
// For each pixel (destination), we look *backwards* along the velocity field to find
// where the fluid came from in the previous time step. We then sample the value at that
// source location.
//
// source_pos = dest_pos - velocity * dt
// new_value = sample(old_value, source_pos)

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

struct AdvectionUniforms {
    texel_size: vec2<f32>,
    dye_texel_size: vec2<f32>,
    dt: f32,
    dissipation: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: AdvectionUniforms;

@group(0) @binding(1)
var u_velocity: texture_2d<f32>; // The velocity field determining flow
@group(0) @binding(2)
var u_source: texture_2d<f32>;   // The field being advected (velocity or dye)
@group(0) @binding(3)
var u_sampler: sampler;

// Manual bilinear interpolation
// Hardware linear filtering is fast but sometimes we need manual control or higher precision.
fn bilerp(tex: texture_2d<f32>, samp: sampler, uv: vec2<f32>, texel_size: vec2<f32>) -> vec4<f32> {
    let st = uv / texel_size - vec2<f32>(0.5);
    let iuv = floor(st);
    let fuv = fract(st);
    
    let a = textureSample(tex, samp, (iuv + vec2<f32>(0.5, 0.5)) * texel_size);
    let b = textureSample(tex, samp, (iuv + vec2<f32>(1.5, 0.5)) * texel_size);
    let c = textureSample(tex, samp, (iuv + vec2<f32>(0.5, 1.5)) * texel_size);
    let d = textureSample(tex, samp, (iuv + vec2<f32>(1.5, 1.5)) * texel_size);
    
    return mix(mix(a, b, fuv.x), mix(c, d, fuv.x), fuv.y);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Sample velocity at current location
    let vel = textureSample(u_velocity, u_sampler, in.uv).xy;
    
    // 2. Back-trace position: where did the particle come from?
    // coord = current_pos - velocity * time_step
    let coord = in.uv - uniforms.dt * vel * uniforms.texel_size;
    
    // 3. Sample the quantity (dye or velocity) at the source location
    let result = bilerp(u_source, u_sampler, coord, uniforms.dye_texel_size);
    
    // 4. Apply dissipation (decay)
    // Helps stability and simulates viscosity/evaporation
    let decay = 1.0 + uniforms.dissipation * uniforms.dt;
    return result / decay;
}
