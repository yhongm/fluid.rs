// pressure.wgsl - Jacobi iteration for pressure field
//
// Solves the Poisson equation for pressure: Laplacian(p) = divergence(velocity)
// This step ensures the fluid is incompressible (mass conserving).
//
// Algorithm: Jacobi Iteration
// We solve the linear system A*x = b iteratively.
// New pressure at center = (sum of neighbor pressures - divergence) / 4
//
// P_new = (P_left + P_right + P_top + P_bottom - alpha * divergence) * beta
// Here alpha = -1 (so + divergence) and beta = 0.25

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

struct PressureUniforms {
    alpha: f32, // Unused
    beta: f32,  // Unused
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: PressureUniforms;

@group(0) @binding(1)
var u_pressure: texture_2d<f32>;   // Pressure from previous iteration
@group(0) @binding(2)
var u_divergence: texture_2d<f32>; // Divergence of the velocity field
@group(0) @binding(3)
var u_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample pressure from 4 neighbors
    let L = textureSample(u_pressure, u_sampler, in.v_l).x;
    let R = textureSample(u_pressure, u_sampler, in.v_r).x;
    let T = textureSample(u_pressure, u_sampler, in.v_t).x;
    let B = textureSample(u_pressure, u_sampler, in.v_b).x;
    
    // Sample divergence at current cell
    let div = textureSample(u_divergence, u_sampler, in.uv).x;
    
    // Apply Jacobi formula with hardcoded values to fix uniform mismatch:
    // p_new = (L + R + T + B - div) * 0.25
    let pressure = (L + R + T + B - div) * 0.25;
    
    return vec4<f32>(pressure, 0.0, 0.0, 1.0);
}
