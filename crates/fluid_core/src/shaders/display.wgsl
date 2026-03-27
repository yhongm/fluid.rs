// display.wgsl - Final display with shading, bloom, and sunrays

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) v_l: vec2<f32>,
    @location(2) v_r: vec2<f32>,
    @location(3) v_t: vec2<f32>,
    @location(4) v_b: vec2<f32>,
};

struct DisplayUniforms {
    texel_size: vec2<f32>,
    shading: u32,
    use_bloom: u32,
    use_sunrays: u32,
    _pad0: u32,
    // 16-byte aligned block
    bloom_color_tint: vec3<f32>,
    bloom_hdr_power: f32,
    tone_map_exposure: f32,
    _pad1: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: DisplayUniforms;

@group(0) @binding(1)
var u_texture: texture_2d<f32>;
@group(0) @binding(2)
var u_bloom: texture_2d<f32>;
@group(0) @binding(3)
var u_sunrays: texture_2d<f32>;
@group(0) @binding(4)
var u_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var c = textureSample(u_texture, u_sampler, in.uv).xyz;
    
    // Shading: fake normal-mapped lighting from density gradient
    if (uniforms.shading != 0u) {
        let L = textureSample(u_texture, u_sampler, in.v_l);
        let R = textureSample(u_texture, u_sampler, in.v_r);
        let T = textureSample(u_texture, u_sampler, in.v_t);
        let B = textureSample(u_texture, u_sampler, in.v_b);
        
        let lL = length(L.xyz);
        let lR = length(R.xyz);
        let lT = length(T.xyz);
        let lB = length(B.xyz);
        
        var normal = vec3<f32>(lL - lR, lB - lT, length(uniforms.texel_size));
        normal = normalize(normal);
        
        let light_dir = normalize(vec3<f32>(0.0, 0.0, 1.0));
        let diffuse = clamp(dot(normal, light_dir) + 0.7, 0.7, 1.0);
        
        c = c * diffuse;
    }

    // Sunrays composite (darken base fluid)
    var sunrays = 1.0;
    if (uniforms.use_sunrays != 0u) {
        sunrays = textureSample(u_sunrays, u_sampler, in.uv).x;
        c *= sunrays;
    }

    // Bloom composite
    if (uniforms.use_bloom != 0u) {
        var bloom = textureSample(u_bloom, u_sampler, in.uv).xyz;
        
        // Also darken bloom with sunrays - apply BEFORE gamma correction
        bloom *= sunrays;

        // LinearToGamma correction
        bloom = max(bloom, vec3<f32>(0.0));
        bloom = max(1.055 * pow(bloom, vec3<f32>(0.416666667)) - 0.055, vec3<f32>(0.0));

        // Color tint
        bloom *= uniforms.bloom_color_tint;

        // HDR brightness power
        if (uniforms.bloom_hdr_power != 1.0) {
            bloom = pow(max(bloom, vec3<f32>(0.0)), vec3<f32>(1.0 / uniforms.bloom_hdr_power));
        }

        // Dithering
        let dither = fract(sin(dot(in.uv, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        let dither_strength = 1.0 / 255.0;
        let noise = vec3<f32>(dither, dither, dither);
        bloom += noise * dither_strength;

        c += bloom;
    }

    // Global exposure before tone mapping
    c *= uniforms.tone_map_exposure;
    
    // Tone mapping (simple)
    let a = max(c.x, max(c.y, c.z));
    return vec4<f32>(c.xyz / max(a, 1.0), 1.0);
}
