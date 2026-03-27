// curl.wgsl - Computes curl (vorticity) of velocity field

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

@group(0) @binding(1)
var u_velocity: texture_2d<f32>;
@group(0) @binding(2)
var u_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let L = textureSample(u_velocity, u_sampler, in.v_l).xy;
    let R = textureSample(u_velocity, u_sampler, in.v_r).xy;
    let T = textureSample(u_velocity, u_sampler, in.v_t).xy;
    let B = textureSample(u_velocity, u_sampler, in.v_b).xy;
    
    // curl = dv/dx - du/dy
    // Matches JS: vorticity = R.y - L.y - T.x + B.x
    let vorticity = (R.y - L.y) - (T.x - B.x);
    return vec4<f32>(vorticity, 0.0, 0.0, 1.0);
}
