// vorticity.wgsl - Vorticity confinement to enhance swirling

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

struct VorticityUniforms {
    texel_size: vec2<f32>,
    curl: f32,
    dt: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: VorticityUniforms;

@group(0) @binding(1)
var u_velocity: texture_2d<f32>;
@group(0) @binding(2)
var u_curl: texture_2d<f32>;
@group(0) @binding(3)
var u_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let L = textureSample(u_curl, u_sampler, in.v_l).x;
    let R = textureSample(u_curl, u_sampler, in.v_r).x;
    let T = textureSample(u_curl, u_sampler, in.v_t).x;
    let B = textureSample(u_curl, u_sampler, in.v_b).x;
    let C = textureSample(u_curl, u_sampler, in.uv).x;
    
    var force = 0.5 * vec2<f32>(abs(T) - abs(B), abs(R) - abs(L));
    force /= length(force) + 0.0001;
    force *= uniforms.curl * C;
    force.y *= -1.0;
    
    let vel = textureSample(u_velocity, u_sampler, in.uv).xy;
    let new_vel = clamp(vel + force * uniforms.dt, vec2<f32>(-1000.0), vec2<f32>(1000.0));
    return vec4<f32>(new_vel, 0.0, 1.0);
}
