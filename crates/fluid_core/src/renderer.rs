//! GPU rendering infrastructure — **no winit dependency**.
//!
//! [`Renderer`] is created from any window handle that implements
//! [`raw_window_handle::HasWindowHandle`] + [`raw_window_handle::HasDisplayHandle`].
//! This covers winit, SDL2, GLFW, raw Win32/X11/Wayland, and more.

use wgpu::util::DeviceExt;
use raw_window_handle::{HasWindowHandle, HasDisplayHandle};
use crate::config::FluidConfig;
use crate::fluid::FluidSim;

// ── Double-buffered simulation texture ─────────────────────────────

pub struct DoubleFbo {
    pub read: wgpu::Texture,
    pub write: wgpu::Texture,
    pub read_view: wgpu::TextureView,
    pub write_view: wgpu::TextureView,
    #[allow(dead_code)]
    pub width: u32,
    #[allow(dead_code)]
    pub height: u32,
    pub texel_size_x: f32,
    pub texel_size_y: f32,
}

impl DoubleFbo {
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.read,       &mut self.write);
        std::mem::swap(&mut self.read_view,  &mut self.write_view);
    }
}

// ── Renderer ────────────────────────────────────────────────────────

pub struct Renderer<'a> {
    pub device: wgpu::Device,
    pub queue:  wgpu::Queue,
    pub surface: wgpu::Surface<'a>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub width:  u32,
    pub height: u32,
    pub sampler_linear:  wgpu::Sampler,
    pub sampler_nearest: wgpu::Sampler,

    // Simulation pipelines
    pub curl_pipeline:               wgpu::RenderPipeline,
    pub vorticity_pipeline:          wgpu::RenderPipeline,
    pub divergence_pipeline:         wgpu::RenderPipeline,
    pub pressure_pipeline:           wgpu::RenderPipeline,
    pub gradient_subtract_pipeline:  wgpu::RenderPipeline,
    pub advection_pipeline:          wgpu::RenderPipeline,
    pub splat_pipeline:              wgpu::RenderPipeline,
    pub clear_pipeline:              wgpu::RenderPipeline,
    pub display_pipeline:            wgpu::RenderPipeline,
    pub bloom_prefilter_pipeline:    wgpu::RenderPipeline,
    pub bloom_blur_pipeline:         wgpu::RenderPipeline,
    pub bloom_blur_additive_pipeline: wgpu::RenderPipeline,
    pub bloom_final_pipeline:        wgpu::RenderPipeline,
    pub sunrays_mask_pipeline:       wgpu::RenderPipeline,
    pub sunrays_pipeline:            wgpu::RenderPipeline,
    pub blur_pipeline:               wgpu::RenderPipeline,
    pub copy_pipeline:               wgpu::RenderPipeline,

    // Bind group layouts
    pub bgl_1tex:      wgpu::BindGroupLayout,
    pub bgl_2tex:      wgpu::BindGroupLayout,
    #[allow(dead_code)]
    pub bgl_3tex:      wgpu::BindGroupLayout,
    pub bgl_display:   wgpu::BindGroupLayout,
    pub bgl_vorticity: wgpu::BindGroupLayout,
}

// ── Helper free functions ────────────────────────────────────────────

pub(crate) fn create_texture(
    device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
             | wgpu::TextureUsages::RENDER_ATTACHMENT
             | wgpu::TextureUsages::COPY_SRC
             | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

pub(crate) fn create_double_fbo(
    device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat,
) -> DoubleFbo {
    let read  = create_texture(device, width, height, format);
    let write = create_texture(device, width, height, format);
    let read_view  = read.create_view(&wgpu::TextureViewDescriptor::default());
    let write_view = write.create_view(&wgpu::TextureViewDescriptor::default());
    DoubleFbo {
        read, write, read_view, write_view, width, height,
        texel_size_x: 1.0 / width  as f32,
        texel_size_y: 1.0 / height as f32,
    }
}

fn bgl_entry_uniform(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn bgl_entry_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}
fn bgl_entry_sampler(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn make_bgl(device: &wgpu::Device, label: &str, n_textures: u32) -> wgpu::BindGroupLayout {
    let mut entries = vec![bgl_entry_uniform(0)];
    for i in 0..n_textures { entries.push(bgl_entry_tex(1 + i)); }
    entries.push(bgl_entry_sampler(1 + n_textures));
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label), entries: &entries,
    })
}

fn make_pipeline(
    device: &wgpu::Device, bgl: &wgpu::BindGroupLayout,
    vs: &wgpu::ShaderModule, fs: &wgpu::ShaderModule, fs_entry: &str,
    format: wgpu::TextureFormat, blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None, bind_group_layouts: &[bgl], push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: vs, entry_point: "vs_main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: fs, entry_point: fs_entry,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format, blend, write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

/// Uniform buffer for pressure-decay / clear pass
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ClearUniformsR {
    pub texel_size: [f32; 2],
    pub value:      f32,
    pub _pad:       f32,
}

// ── Renderer impl ────────────────────────────────────────────────────

impl<'a> Renderer<'a> {
    /// Create a renderer from any window handle — no winit required.
    ///
    /// # Parameters
    /// - `window`  : Any type implementing `HasWindowHandle + HasDisplayHandle`
    ///               (e.g. `Arc<winit::Window>`, SDL2 window, raw HWND wrapper…)
    /// - `width` / `height` : Initial surface size in physical pixels
    /// - `_config` : Reserved for future config-driven renderer options
    ///
    /// # Safety
    /// The window handle must remain valid for the lifetime `'a` of this renderer.
    pub async fn new<W>(window: W, width: u32, height: u32, _config: &FluidConfig) -> Self
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'a,
    {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // SAFETY: caller guarantees window outlives renderer
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits:   wgpu::Limits::default(),
                label: None,
            },
            None,
        ).await.unwrap();

        let caps   = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width, height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes
                .iter()
                .copied()
                .find(|m| matches!(m, wgpu::CompositeAlphaMode::PreMultiplied | wgpu::CompositeAlphaMode::PostMultiplied))
                .unwrap_or(caps.alpha_modes[0]),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let sampler_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Shaders (all embedded at compile time)
        macro_rules! shader {
            ($label:literal, $path:literal) => {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some($label),
                    source: wgpu::ShaderSource::Wgsl(include_str!($path).into()),
                })
            };
        }
        let vs           = shader!("base_vs",            "shaders/base.wgsl");
        let curl_fs      = shader!("curl_fs",            "shaders/curl.wgsl");
        let vort_fs      = shader!("vorticity_fs",       "shaders/vorticity.wgsl");
        let div_fs       = shader!("divergence_fs",      "shaders/divergence.wgsl");
        let pres_fs      = shader!("pressure_fs",        "shaders/pressure.wgsl");
        let grad_fs      = shader!("gradient_sub_fs",    "shaders/gradient_subtract.wgsl");
        let adv_fs       = shader!("advection_fs",       "shaders/advection.wgsl");
        let splat_fs     = shader!("splat_fs",           "shaders/splat.wgsl");
        let clear_fs     = shader!("clear_fs",           "shaders/clear.wgsl");
        let display_fs   = shader!("display_fs",         "shaders/display.wgsl");
        let bloom_fs     = shader!("bloom_fs",           "shaders/bloom.wgsl");
        let sunrays_fs   = shader!("sunrays_fs",         "shaders/sunrays.wgsl");
        let blur_fs      = shader!("blur_fs",            "shaders/blur.wgsl");
        let copy_fs      = shader!("copy_fs",            "shaders/copy.wgsl");

        let sim_fmt = wgpu::TextureFormat::Rgba16Float;
        let r_fmt   = wgpu::TextureFormat::R16Float;

        let bgl_1tex      = make_bgl(&device, "bgl_1tex",      1);
        let bgl_2tex      = make_bgl(&device, "bgl_2tex",      2);
        let bgl_3tex      = make_bgl(&device, "bgl_3tex",      3);
        let bgl_display   = make_bgl(&device, "bgl_display",   3);
        let bgl_vorticity = make_bgl(&device, "bgl_vorticity", 2);

        let p  = |bgl: &_, fs: &_, e, fmt, blend| make_pipeline(&device, bgl, &vs, fs, e, fmt, blend);
        let no = None;
        let ad = Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation:  wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        });

        let curl_pipeline              = p(&bgl_1tex,      &curl_fs,    "fs_main",       r_fmt,   no);
        let vorticity_pipeline         = p(&bgl_vorticity, &vort_fs,    "fs_main",       sim_fmt, no);
        let divergence_pipeline        = p(&bgl_1tex,      &div_fs,     "fs_main",       r_fmt,   no);
        let pressure_pipeline          = p(&bgl_2tex,      &pres_fs,    "fs_main",       r_fmt,   no);
        let gradient_subtract_pipeline = p(&bgl_2tex,      &grad_fs,    "fs_main",       sim_fmt, no);
        let advection_pipeline         = p(&bgl_2tex,      &adv_fs,     "fs_main",       sim_fmt, no);
        let splat_pipeline             = p(&bgl_1tex,      &splat_fs,   "fs_main",       sim_fmt, no);
        let clear_pipeline             = p(&bgl_1tex,      &clear_fs,   "fs_main",       r_fmt,   no);
        let display_pipeline           = p(&bgl_display,   &display_fs, "fs_main",       format,  no);
        let bloom_prefilter_pipeline   = p(&bgl_1tex,      &bloom_fs,   "fs_prefilter",  sim_fmt, no);
        let bloom_blur_pipeline        = p(&bgl_1tex,      &bloom_fs,   "fs_blur",       sim_fmt, no);
        let bloom_blur_additive_pipeline = p(&bgl_1tex,    &bloom_fs,   "fs_blur",       sim_fmt, ad);
        let bloom_final_pipeline       = p(&bgl_1tex,      &bloom_fs,   "fs_final",      sim_fmt, ad);
        let sunrays_mask_pipeline      = p(&bgl_1tex,      &sunrays_fs, "fs_mask",       r_fmt,   no);
        let sunrays_pipeline           = p(&bgl_1tex,      &sunrays_fs, "fs_sunrays",    r_fmt,   no);
        let blur_pipeline              = p(&bgl_1tex,      &blur_fs,    "fs_main",       r_fmt,   no);
        let copy_pipeline              = p(&bgl_1tex,      &copy_fs,    "fs_main",       r_fmt,   no);

        Self {
            device, queue, surface, surface_config,
            width, height,
            sampler_linear, sampler_nearest,
            curl_pipeline, vorticity_pipeline, divergence_pipeline,
            pressure_pipeline, gradient_subtract_pipeline, advection_pipeline,
            splat_pipeline, clear_pipeline, display_pipeline,
            bloom_prefilter_pipeline, bloom_blur_pipeline, bloom_blur_additive_pipeline, bloom_final_pipeline,
            sunrays_mask_pipeline, sunrays_pipeline, blur_pipeline, copy_pipeline,
            bgl_1tex, bgl_2tex, bgl_3tex, bgl_display, bgl_vorticity,
        }
    }

    /// Resize the surface. Call this when the window reports a size change.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.width  = width;
            self.height = height;
            self.surface_config.width  = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    pub fn create_texture(&self, w: u32, h: u32, fmt: wgpu::TextureFormat) -> wgpu::Texture {
        create_texture(&self.device, w, h, fmt)
    }
    pub fn create_double_fbo(&self, w: u32, h: u32, fmt: wgpu::TextureFormat) -> DoubleFbo {
        create_double_fbo(&self.device, w, h, fmt)
    }
    pub fn uniform_buffer<T: bytemuck::Pod>(&self, data: &T) -> wgpu::Buffer {
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    // ── Internal blit helpers ──────────────────────────────────────

    pub fn blit_1tex(
        &self, enc: &mut wgpu::CommandEncoder, pipeline: &wgpu::RenderPipeline,
        target: &wgpu::TextureView, ub: &wgpu::Buffer,
        tex: &wgpu::TextureView, sampler: &wgpu::Sampler,
    ) {
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bgl_1tex, label: None,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ub.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(tex) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        self.draw_pass(enc, target, pipeline, &bg, wgpu::LoadOp::Load);
    }

    pub fn blit_2tex(
        &self, enc: &mut wgpu::CommandEncoder, pipeline: &wgpu::RenderPipeline,
        target: &wgpu::TextureView, ub: &wgpu::Buffer,
        tex1: &wgpu::TextureView, tex2: &wgpu::TextureView, sampler: &wgpu::Sampler,
    ) {
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bgl_2tex, label: None,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ub.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(tex1) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(tex2) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        self.draw_pass(enc, target, pipeline, &bg, wgpu::LoadOp::Load);
    }

    pub fn blit_vorticity(
        &self, enc: &mut wgpu::CommandEncoder, pipeline: &wgpu::RenderPipeline,
        target: &wgpu::TextureView, ub: &wgpu::Buffer,
        tex1: &wgpu::TextureView, tex2: &wgpu::TextureView, sampler: &wgpu::Sampler,
    ) {
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bgl_vorticity, label: None,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ub.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(tex1) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(tex2) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        self.draw_pass(enc, target, pipeline, &bg, wgpu::LoadOp::Load);
    }

    pub fn blit_display(
        &self, enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView, ub: &wgpu::Buffer,
        dye: &wgpu::TextureView, bloom: &wgpu::TextureView, sunrays: &wgpu::TextureView,
        sampler: &wgpu::Sampler, clear_color: [f32; 4],
    ) {
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bgl_display, label: None,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ub.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(dye) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(bloom) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(sunrays) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        self.draw_pass(
            enc,
            target,
            &self.display_pipeline,
            &bg,
            wgpu::LoadOp::Clear(wgpu::Color {
                r: clear_color[0] as f64,
                g: clear_color[1] as f64,
                b: clear_color[2] as f64,
                a: clear_color[3] as f64,
            }),
        );
    }

    fn draw_pass(
        &self, enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView, pipeline: &wgpu::RenderPipeline,
        bg: &wgpu::BindGroup, load: wgpu::LoadOp<wgpu::Color>,
    ) {
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target, resolve_target: None,
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        rp.set_pipeline(pipeline);
        rp.set_bind_group(0, bg, &[]);
        rp.draw(0..3, 0..1);
    }

    /// Render one frame to the surface.
    ///
    /// `draw_ui` is called after the fluid composite — use it to overlay egui or any custom UI.
    pub fn render<F>(
        &self, fluid: &FluidSim,
        shading: bool, bloom: bool, sunrays: bool, clear_color: [f32; 4],
        bloom_color_tint: [f32; 3], bloom_hdr_power: f32, tone_map_exposure: f32,
        draw_ui: F,
    ) -> Result<(), wgpu::SurfaceError>
    where
        F: FnOnce(&mut wgpu::CommandEncoder, &wgpu::TextureView),
    {
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct DisplayUniforms {
            texel_size:        [f32; 2],
            shading:           u32,
            use_bloom:         u32,
            use_sunrays:       u32,
            _pad0:             u32,
            _pad_align:        [f32; 2], // Pad 24->32 for bloom_color_tint (vec3 align 16)
            bloom_color_tint:  [f32; 3],
            bloom_hdr_power:   f32,
            tone_map_exposure: f32,
            _pad1:             [f32; 7], // Pad 52->80 (vec3 align 16 + struct align 16)
        }
        let output = self.surface.get_current_texture()?;
        let view   = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let ub = self.uniform_buffer(&DisplayUniforms {
            texel_size:        [1.0 / self.width as f32, 1.0 / self.height as f32],
            shading:           shading  as u32,
            use_bloom:         bloom    as u32,
            use_sunrays:       sunrays  as u32,
            _pad0:             0,
            _pad_align:        [0.0; 2],
            bloom_color_tint,
            bloom_hdr_power,
            tone_map_exposure,
            _pad1:             [0.0; 7],
        });
        self.blit_display(
            &mut enc, &view, &ub,
            &fluid.dye.read_view,
            &fluid.bloom.create_view(&wgpu::TextureViewDescriptor::default()),
            &fluid.sunrays.create_view(&wgpu::TextureViewDescriptor::default()),
            &self.sampler_linear,
            clear_color,
        );

        draw_ui(&mut enc, &view);

        self.queue.submit(std::iter::once(enc.finish()));
        output.present();
        Ok(())
    }
}
