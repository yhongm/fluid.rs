// gradient_subtract.wgsl - Subtracts pressure gradient from velocity to enforce incompressibility

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

@group(0) @binding(1)
var u_pressure: texture_2d<f32>;
@group(0) @binding(2)
var u_velocity: texture_2d<f32>;
@group(0) @binding(3)
var u_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pL = textureSample(u_pressure, u_sampler, in.v_l).x;
    let pR = textureSample(u_pressure, u_sampler, in.v_r).x;
    let pT = textureSample(u_pressure, u_sampler, in.v_t).x;
    let pB = textureSample(u_pressure, u_sampler, in.v_b).x;
    
    var vel = textureSample(u_velocity, u_sampler, in.uv).xy;
    // v -= grad(p)
    // Matches JS: velocity -= vec2(R - L, T - B)
    vel -= vec2<f32>(pR - pL, pT - pB);
    return vec4<f32>(vel, 0.0, 1.0);
}
