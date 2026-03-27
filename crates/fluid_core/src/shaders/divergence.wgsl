// divergence.wgsl - Computes divergence of velocity field

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
    var L = textureSample(u_velocity, u_sampler, in.v_l).x;
    var R = textureSample(u_velocity, u_sampler, in.v_r).x;
    var T = textureSample(u_velocity, u_sampler, in.v_t).y;
    var B = textureSample(u_velocity, u_sampler, in.v_b).y;
    
    // Neumann boundary: reflect at edges
    // if (vL.x < 0.0) { L = -C.x; }
    // if (vR.x > 1.0) { R = -C.x; }
    // if (vT.y > 1.0) { T = -C.y; }
    // if (vB.y < 0.0) { B = -C.y; }
    
    let C = textureSample(u_velocity, u_sampler, in.uv).xy;
    if (in.v_l.x < 0.0) { L = -C.x; }
    if (in.v_r.x > 1.0) { R = -C.x; }
    if (in.v_t.y > 1.0) { T = -C.y; }
    if (in.v_b.y < 0.0) { B = -C.y; }

    // div = 0.5 * (R - L + T - B)
    let div = 0.5 * (R - L + T - B);
    return vec4<f32>(div, 0.0, 0.0, 1.0);
}
