// fluid.rs - Fluid simulation state and physics steps

use rand::Rng;
use crate::renderer::{Renderer, DoubleFbo, ClearUniformsR};
use crate::config::FluidConfig as Config;

pub struct FluidSim {
    pub velocity: DoubleFbo,
    pub dye: DoubleFbo,
    #[allow(dead_code)]
    pub divergence: wgpu::Texture,
    pub divergence_view: wgpu::TextureView,
    #[allow(dead_code)]
    pub curl: wgpu::Texture,
    pub curl_view: wgpu::TextureView,
    pub pressure: DoubleFbo,
    pub bloom: wgpu::Texture,
    pub bloom_fbos: Vec<wgpu::Texture>,
    pub sunrays: wgpu::Texture,
    pub sunrays_temp: wgpu::Texture,
}

fn get_resolution(res: u32, width: u32, height: u32) -> (u32, u32) {
    let aspect = width as f32 / height as f32;
    let aspect = if aspect < 1.0 { 1.0 / aspect } else { aspect };
    let min = res;
    let max = (res as f32 * aspect).round() as u32;
    if width > height { (max, min) } else { (min, max) }
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match i % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

impl FluidSim {
    pub fn new(renderer: &Renderer<'_>, config: &Config) -> Self {
        let (sw, sh) = get_resolution(config.sim_resolution, renderer.width, renderer.height);
        let (dw, dh) = get_resolution(config.dye_resolution, renderer.width, renderer.height);
        let (bw, bh) = get_resolution(config.bloom_resolution, renderer.width, renderer.height);
        let (srw, srh) = get_resolution(config.sunrays_resolution, renderer.width, renderer.height);

        let sim_fmt = wgpu::TextureFormat::Rgba16Float;
        let r_fmt = wgpu::TextureFormat::R16Float;

        let velocity = renderer.create_double_fbo(sw, sh, sim_fmt);
        let dye = renderer.create_double_fbo(dw, dh, sim_fmt);
        let divergence = renderer.create_texture(sw, sh, r_fmt);
        let divergence_view = divergence.create_view(&wgpu::TextureViewDescriptor::default());
        let curl = renderer.create_texture(sw, sh, r_fmt);
        let curl_view = curl.create_view(&wgpu::TextureViewDescriptor::default());
        let pressure = renderer.create_double_fbo(sw, sh, r_fmt);
        let bloom = renderer.create_texture(bw, bh, sim_fmt);

        let mut bloom_fbos = Vec::new();
        let mut w = bw / 2;
        let mut h = bh / 2;
        for _ in 0..config.bloom_iterations {
            if w < 2 || h < 2 { break; }
            bloom_fbos.push(renderer.create_texture(w, h, sim_fmt));
            w /= 2;
            h /= 2;
        }

        let sunrays = renderer.create_texture(srw, srh, r_fmt);
        let sunrays_temp = renderer.create_texture(srw, srh, r_fmt);

        Self {
            velocity, dye, divergence, divergence_view,
            curl, curl_view, pressure, bloom, bloom_fbos,
            sunrays, sunrays_temp,
        }
    }

    pub fn resize(&mut self, renderer: &Renderer<'_>, config: &Config) {
        *self = FluidSim::new(renderer, config);
    }

    pub fn step(&mut self, renderer: &Renderer<'_>, dt: f32, config: &Config) {
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct BaseUniforms { texel_size: [f32; 2], _pad: [f32; 2] }

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct VorticityUniforms { texel_size: [f32; 2], curl: f32, dt: f32 }

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct AdvectionUniforms {
            texel_size: [f32; 2],
            dye_texel_size: [f32; 2],
            dt: f32,
            dissipation: f32,
            _pad: [f32; 2],
        }

        let mut enc = renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let vel_tsx = self.velocity.texel_size_x;
        let vel_tsy = self.velocity.texel_size_y;

        // 1. Curl
        let buf = renderer.uniform_buffer(&BaseUniforms { texel_size: [vel_tsx, vel_tsy], _pad: [0.0; 2] });
        renderer.blit_1tex(&mut enc, &renderer.curl_pipeline, &self.curl_view, &buf, &self.velocity.read_view, &renderer.sampler_linear);

        // 2. Vorticity
        let buf = renderer.uniform_buffer(&VorticityUniforms { texel_size: [vel_tsx, vel_tsy], curl: config.curl, dt });
        renderer.blit_vorticity(&mut enc, &renderer.vorticity_pipeline, &self.velocity.write_view, &buf, &self.velocity.read_view, &self.curl_view, &renderer.sampler_linear);
        self.velocity.swap();

        // 3. Divergence
        let buf = renderer.uniform_buffer(&BaseUniforms { texel_size: [vel_tsx, vel_tsy], _pad: [0.0; 2] });
        renderer.blit_1tex(&mut enc, &renderer.divergence_pipeline, &self.divergence_view, &buf, &self.velocity.read_view, &renderer.sampler_linear);

        // 4. Pressure decay
        let buf = renderer.uniform_buffer(&ClearUniformsR { texel_size: [self.pressure.texel_size_x, self.pressure.texel_size_y], value: config.pressure, _pad: 0.0 });
        renderer.blit_1tex(&mut enc, &renderer.clear_pipeline, &self.pressure.write_view, &buf, &self.pressure.read_view, &renderer.sampler_nearest);
        self.pressure.swap();

        // 5. Jacobi pressure iterations
        for _ in 0..config.pressure_iterations {
            let buf = renderer.uniform_buffer(&BaseUniforms { texel_size: [self.pressure.texel_size_x, self.pressure.texel_size_y], _pad: [0.0; 2] });
            renderer.blit_2tex(&mut enc, &renderer.pressure_pipeline, &self.pressure.write_view, &buf, &self.pressure.read_view, &self.divergence_view, &renderer.sampler_nearest);
            self.pressure.swap();
        }

        // 6. Gradient subtract
        let buf = renderer.uniform_buffer(&BaseUniforms { texel_size: [vel_tsx, vel_tsy], _pad: [0.0; 2] });
        renderer.blit_2tex(&mut enc, &renderer.gradient_subtract_pipeline, &self.velocity.write_view, &buf, &self.pressure.read_view, &self.velocity.read_view, &renderer.sampler_linear);
        self.velocity.swap();

        // 7. Velocity advection
        let buf = renderer.uniform_buffer(&AdvectionUniforms {
            texel_size: [vel_tsx, vel_tsy], dye_texel_size: [vel_tsx, vel_tsy],
            dt, dissipation: config.velocity_dissipation, _pad: [0.0; 2],
        });
        renderer.blit_2tex(&mut enc, &renderer.advection_pipeline, &self.velocity.write_view, &buf, &self.velocity.read_view, &self.velocity.read_view, &renderer.sampler_linear);
        self.velocity.swap();

        // 8. Dye advection
        let dye_tsx = self.dye.texel_size_x;
        let dye_tsy = self.dye.texel_size_y;
        let buf = renderer.uniform_buffer(&AdvectionUniforms {
            texel_size: [vel_tsx, vel_tsy], dye_texel_size: [dye_tsx, dye_tsy],
            dt, dissipation: config.density_dissipation, _pad: [0.0; 2],
        });
        renderer.blit_2tex(&mut enc, &renderer.advection_pipeline, &self.dye.write_view, &buf, &self.velocity.read_view, &self.dye.read_view, &renderer.sampler_linear);
        self.dye.swap();

        renderer.queue.submit(std::iter::once(enc.finish()));

        if config.bloom  { self.apply_bloom(renderer, config); }
        if config.sunrays { self.apply_sunrays(renderer, config); }
    }

    // Bloom (泛光) Post-processing Effect
    // Algorithm:
    // 1. Prefilter: Extract bright areas (thresholding).
    // 2. Downsample: Create a mipmap chain (pyramid) of the bright areas to get different blur radii.
    // 3. Upsample & Combine: Blend the blurred layers back together to create a soft glow.
    fn apply_bloom(&self, renderer: &Renderer<'_>, config: &Config) {
        if self.bloom_fbos.len() < 2 { return; }

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct BloomPrefilterUniforms {
            texel_size: [f32; 2],
            _pad1: [f32; 2],
            curve: [f32; 4], // x: threshold - knee, y: knee * 2, z: 0.25 / knee
            threshold: f32,
            _pad2: [f32; 3],
        }
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct BlurUniforms { texel_size: [f32; 2], _pad: [f32; 2] }
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct BloomFinalUniforms { texel_size: [f32; 2], intensity: f32, _pad: f32 }

        // "Knee" for soft thresholding
        let knee = config.bloom_threshold * config.bloom_soft_knee + 0.0001;
        let bw = self.bloom.width();
        let bh = self.bloom.height();

        let fbo_views: Vec<wgpu::TextureView> = self.bloom_fbos.iter()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
            .collect();

        let mut enc = renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // 1. Prefilter: Extract high brightness pixels
        let bloom_view = self.bloom.create_view(&wgpu::TextureViewDescriptor::default());
        let buf = renderer.uniform_buffer(&BloomPrefilterUniforms {
            texel_size: [1.0/bw as f32, 1.0/bh as f32],
            _pad1: [0.0; 2],
            curve: [config.bloom_threshold - knee, knee * 2.0, 0.25/knee, 0.0],
            threshold: config.bloom_threshold,
            _pad2: [0.0; 3],
        });
        renderer.blit_1tex(&mut enc, &renderer.bloom_prefilter_pipeline, &bloom_view, &buf, &self.dye.read_view, &renderer.sampler_linear);

        // 2. Downsample: Repeatedly blur and downscale
        let mut prev_w = bw;
        let mut prev_h = bh;
        let mut src_view = self.bloom.create_view(&wgpu::TextureViewDescriptor::default());

        for (i, view) in fbo_views.iter().enumerate() {
            let buf = renderer.uniform_buffer(&BlurUniforms { texel_size: [1.0/prev_w as f32, 1.0/prev_h as f32], _pad: [0.0;2] });
            renderer.blit_1tex(&mut enc, &renderer.bloom_blur_pipeline, view, &buf, &src_view, &renderer.sampler_linear);
            prev_w = self.bloom_fbos[i].width();
            prev_h = self.bloom_fbos[i].height();
            src_view = self.bloom_fbos[i].create_view(&wgpu::TextureViewDescriptor::default());
        }

        // 3. Upsample: Upscale and blend
        let n = fbo_views.len();
        for i in (0..n.saturating_sub(1)).rev() {
            let buf = renderer.uniform_buffer(&BlurUniforms { texel_size: [1.0/prev_w as f32, 1.0/prev_h as f32], _pad: [0.0;2] });
            // Use additive blending pipeline for upsampling
            renderer.blit_1tex(&mut enc, &renderer.bloom_blur_additive_pipeline, &fbo_views[i], &buf, &src_view, &renderer.sampler_linear);
            prev_w = self.bloom_fbos[i].width();
            prev_h = self.bloom_fbos[i].height();
            src_view = self.bloom_fbos[i].create_view(&wgpu::TextureViewDescriptor::default());
        }

        // Final: Composite bloom texture
        let final_view = self.bloom.create_view(&wgpu::TextureViewDescriptor::default());
        let buf = renderer.uniform_buffer(&BloomFinalUniforms { texel_size: [1.0/prev_w as f32, 1.0/prev_h as f32], intensity: config.bloom_intensity, _pad: 0.0 });
        renderer.blit_1tex(&mut enc, &renderer.bloom_final_pipeline, &final_view, &buf, &src_view, &renderer.sampler_linear);

        renderer.queue.submit(std::iter::once(enc.finish()));
    }

    // Sunrays (God Rays) Post-processing Effect
    // Algorithm:
    // 1. Mask: Create a binary/greyscale mask where dye occludes light (or emits light).
    // 2. Radial Blur: Blur the mask radially outwards from a light source center.
    // 3. Composite: Blend the rays onto the final image.
    fn apply_sunrays(&self, renderer: &Renderer<'_>, config: &Config) {
        let srw = self.sunrays.width();
        let srh = self.sunrays.height();

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct SunraysUniforms { texel_size: [f32; 2], weight: f32, exposure: f32, decay: f32, _pad: [f32; 3], _pad_end: [f32; 4] }
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct BlurUniforms { texel_size: [f32; 2], _pad: [f32; 2] }

        let sunrays_view = self.sunrays.create_view(&wgpu::TextureViewDescriptor::default());
        let sunrays_temp_view = self.sunrays_temp.create_view(&wgpu::TextureViewDescriptor::default());

        let mut enc = renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let buf = renderer.uniform_buffer(&SunraysUniforms {
            texel_size: [1.0/srw as f32, 1.0/srh as f32],
            weight: config.sunrays_weight,
            exposure: config.sunrays_exposure,
            decay: config.sunrays_decay,
            _pad: [0.0; 3],
            _pad_end: [0.0; 4]
        });
        renderer.blit_1tex(&mut enc, &renderer.sunrays_mask_pipeline, &sunrays_view, &buf, &self.dye.read_view, &renderer.sampler_linear);

        let buf = renderer.uniform_buffer(&SunraysUniforms {
            texel_size: [1.0/srw as f32, 1.0/srh as f32],
            weight: config.sunrays_weight,
            exposure: config.sunrays_exposure,
            decay: config.sunrays_decay,
            _pad: [0.0; 3],
            _pad_end: [0.0; 4]
        });
        renderer.blit_1tex(&mut enc, &renderer.sunrays_pipeline, &sunrays_temp_view, &buf, &sunrays_view, &renderer.sampler_linear);

        let buf = renderer.uniform_buffer(&BlurUniforms { texel_size: [1.0/srw as f32, 0.0], _pad: [0.0;2] });
        renderer.blit_1tex(&mut enc, &renderer.blur_pipeline, &sunrays_view, &buf, &sunrays_temp_view, &renderer.sampler_linear);

        let buf = renderer.uniform_buffer(&BlurUniforms { texel_size: [0.0, 1.0/srh as f32], _pad: [0.0;2] });
        renderer.blit_1tex(&mut enc, &renderer.blur_pipeline, &sunrays_temp_view, &buf, &sunrays_view, &renderer.sampler_linear);

        let buf = renderer.uniform_buffer(&ClearUniformsR { texel_size: [1.0/srw as f32, 1.0/srh as f32], value: 1.0, _pad: 0.0 });
        renderer.blit_1tex(&mut enc, &renderer.copy_pipeline, &sunrays_view, &buf, &sunrays_temp_view, &renderer.sampler_linear);

        renderer.queue.submit(std::iter::once(enc.finish()));
    }

    pub fn splat(&mut self, renderer: &Renderer<'_>, x: f32, y: f32, dx: f32, dy: f32, color: [f32; 3], config: &Config) {
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct SplatUniforms {
            point: [f32; 2],
            _pad1: [f32; 2],
            color: [f32; 4],
            radius: f32,
            aspect_ratio: f32,
            _pad2: [f32; 2],
        }

        let aspect = renderer.width as f32 / renderer.height as f32;
        let radius = (config.splat_radius / 100.0) * if aspect > 1.0 { aspect } else { 1.0 };

        let mut enc = renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let buf = renderer.uniform_buffer(&SplatUniforms {
            point: [x, y],
            _pad1: [0.0; 2],
            color: [dx, dy, 0.0, 1.0],
            radius,
            aspect_ratio: aspect,
            _pad2: [0.0; 2],
        });
        renderer.blit_1tex(&mut enc, &renderer.splat_pipeline, &self.velocity.write_view, &buf, &self.velocity.read_view, &renderer.sampler_linear);
        self.velocity.swap();

        let buf = renderer.uniform_buffer(&SplatUniforms {
            point: [x, y],
            _pad1: [0.0; 2],
            color: [color[0], color[1], color[2], 1.0],
            radius,
            aspect_ratio: aspect,
            _pad2: [0.0; 2],
        });
        renderer.blit_1tex(&mut enc, &renderer.splat_pipeline, &self.dye.write_view, &buf, &self.dye.read_view, &renderer.sampler_linear);
        self.dye.swap();

        renderer.queue.submit(std::iter::once(enc.finish()));
    }

    pub fn multiple_splats(&mut self, renderer: &Renderer<'_>, count: u32, config: &Config) {
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let x = rng.gen::<f32>();
            let y = rng.gen::<f32>();
            let dx = rng.gen_range(-1.0f32..1.0) * 1000.0;
            let dy = rng.gen_range(-1.0f32..1.0) * 1000.0;
            let c = hsv_to_rgb(rng.gen::<f32>(), 1.0, 1.0);
            let color = [c[0] * 0.15, c[1] * 0.15, c[2] * 0.15];
            self.splat(renderer, x, y, dx, dy, color, config);
        }
    }
}
