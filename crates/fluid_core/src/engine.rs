//! # FluidEngine — SDK core facade
//!
//! No winit, no window system — accepts any surface handle via `raw-window-handle`.
//!
//! ```no_run
//! // Caller creates window however they like, then:
//! let mut engine = FluidEngine::new(&window, 1280, 720, &FluidConfig::default()).await;
//!
//! // Every frame:
//! engine.input.pointer_move(0, nx, ny);
//! engine.update(dt);
//! engine.render()?;
//! ```

use raw_window_handle::{HasWindowHandle, HasDisplayHandle};
use crate::config::FluidConfig;
use crate::renderer::Renderer;
use crate::fluid::FluidSim;
use crate::input::{InputManager, random_color};

pub use crate::fluid::hsv_to_rgb;

// ── RenderContext ─────────────────────────────────────────────────────────

/// Borrow-safe snapshot of GPU handles needed by external UI renderers (egui etc.).
///
/// Obtained via [`FluidEngine::render_context`]. The caller can pass `device`
/// and `queue` into egui-wgpu *before* calling [`FluidEngine::render_with_ui`],
/// avoiding a double-borrow of `engine`.
///
/// ```no_run
/// let ctx = engine.render_context();
/// egui_renderer.update_texture(ctx.device, ctx.queue, id, delta);
/// engine.render_with_ui(|enc, view| { ... })?;
/// ```
pub struct RenderContext<'a> {
    /// wgpu logical device.
    pub device: &'a wgpu::Device,
    /// wgpu command queue.
    pub queue: &'a wgpu::Queue,
    /// Swap-chain texture format — required when constructing egui-wgpu renderer.
    pub surface_format: wgpu::TextureFormat,
    /// Surface width in physical pixels.
    pub width: u32,
    /// Surface height in physical pixels.
    pub height: u32,
}

pub struct FluidEngine<'w> {
    /// Input manager — call `pointer_*` / `inject` / `burst` here.
    pub input: InputManager,

    pub(crate) renderer: Renderer<'w>,
    pub(crate) fluid:    FluidSim,
    pub(crate) config:   FluidConfig,
    color_timer: f32,
}

impl<'w> FluidEngine<'w> {
    // ── Construction ────────────────────────────────────────────────

    /// Create an engine instance from any window handle.
    ///
    /// # Parameters
    /// - `window`         : Implements `HasWindowHandle + HasDisplayHandle`
    ///                      (e.g. `Arc<winit::Window>`, SDL2 window wrapper…)
    /// - `width`/`height` : Initial surface size in **physical pixels**
    /// - `config`         : Initial simulation / render configuration
    ///
    /// # Example
    /// ```no_run
    /// // winit
    /// let engine = FluidEngine::new(&window, sz.width, sz.height, &FluidConfig::default()).await;
    ///
    /// // SDL2 (via a thin raw-window-handle wrapper)
    /// let engine = FluidEngine::new(&sdl_wrapper, w, h, &config).await;
    /// ```
    pub async fn new<W>(window: W, width: u32, height: u32, config: &FluidConfig) -> Self
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'w,
    {
        let renderer = Renderer::new(window, width, height, config).await;
        let fluid    = FluidSim::new(&renderer, config);
        let mut input = InputManager::new();
        input.burst(10); // initial splash on startup
        Self { input, renderer, fluid, config: config.clone(), color_timer: 0.0 }
    }

    // ── Config API ──────────────────────────────────────────────────

    /// Read current configuration.
    pub fn config(&self) -> &FluidConfig { &self.config }

    /// Mutably access configuration — fields take effect next `update()`.
    ///
    /// > ⚠️ For resolution fields use [`set_config`][Self::set_config] to trigger GPU texture rebuild.
    pub fn config_mut(&mut self) -> &mut FluidConfig { &mut self.config }

    /// Replace config entirely; auto-rebuilds GPU textures if resolutions changed.
    pub fn set_config(&mut self, config: FluidConfig) {
        let rebuild =
            config.sim_resolution     != self.config.sim_resolution  ||
            config.dye_resolution     != self.config.dye_resolution  ||
            config.bloom_resolution   != self.config.bloom_resolution ||
            config.bloom_iterations   != self.config.bloom_iterations ||
            config.sunrays_resolution != self.config.sunrays_resolution;
        self.config = config;
        if rebuild { self.fluid.resize(&self.renderer, &self.config); }
    }

    // ── Setter shortcuts ─────────────────────────────────────────────

    pub fn set_density_dissipation(&mut self, v: f32)  { self.config.density_dissipation  = v; }
    pub fn set_velocity_dissipation(&mut self, v: f32) { self.config.velocity_dissipation = v; }
    pub fn set_curl(&mut self, v: f32)                 { self.config.curl   = v; }
    pub fn set_splat_radius(&mut self, v: f32)         { self.config.splat_radius = v.max(0.001); }
    pub fn set_splat_force(&mut self, v: f32)          { self.config.splat_force  = v; }
    pub fn set_pressure(&mut self, v: f32)             { self.config.pressure     = v.clamp(0.0, 1.0); }
    pub fn set_shading(&mut self, v: bool)             { self.config.shading      = v; }
    pub fn set_bloom(&mut self, v: bool)               { self.config.bloom        = v; }
    pub fn set_bloom_intensity(&mut self, v: f32)      { self.config.bloom_intensity   = v.max(0.0); }
    pub fn set_bloom_threshold(&mut self, v: f32)      { self.config.bloom_threshold   = v.clamp(0.0, 1.0); }
    pub fn set_sunrays(&mut self, v: bool)             { self.config.sunrays       = v; }
    pub fn set_sunrays_weight(&mut self, v: f32)       { self.config.sunrays_weight = v.max(0.0); }
    pub fn set_background_color(&mut self, r: f32, g: f32, b: f32) { self.config.back_color = [r, g, b]; }
    pub fn set_background_alpha(&mut self, a: f32)        { self.config.back_alpha = a.clamp(0.0, 1.0); }
    pub fn set_paused(&mut self, v: bool)              { self.config.paused = v; }
    pub fn toggle_paused(&mut self) -> bool            { self.config.paused = !self.config.paused; self.config.paused }
    
    // New setters for bloom/sunrays advanced controls
    pub fn set_bloom_color_tint(&mut self, r: f32, g: f32, b: f32) {
        self.config.bloom_color_tint = [r.max(0.0), g.max(0.0), b.max(0.0)];
    }
    pub fn set_bloom_hdr_power(&mut self, v: f32) {
        self.config.bloom_hdr_power = v.max(0.1);
    }
    pub fn set_tone_map_exposure(&mut self, v: f32) {
        self.config.tone_map_exposure = v.max(0.0);
    }
    pub fn set_sunrays_exposure(&mut self, v: f32) {
        self.config.sunrays_exposure = v.max(0.0);
    }
    pub fn set_sunrays_decay(&mut self, v: f32) {
        self.config.sunrays_decay = v.clamp(0.1, 0.999);
    }

    /// 将所有参数还原为与原版 JS PavelDoGreat/WebGL-Fluid-Simulation 完全一致的值。
    ///
    /// 包括分辨率参数，因此会触发 GPU 纹理重建（与 `set_config` 相同的行为）。
    ///
    /// # 示例
    /// ```no_run
    /// // 修改了一些参数后，一键还原
    /// engine.set_tone_map_exposure(4.0);
    /// engine.set_bloom_hdr_power(2.0);
    /// // ...做了一些实验...
    /// engine.reset_to_js_defaults();  // 所有参数回到 JS 原版状态
    /// ```
    pub fn reset_to_js_defaults(&mut self) {
        self.set_config(FluidConfig::js_defaults());
    }

    // ── Query API ────────────────────────────────────────────────────

    pub fn is_paused(&self) -> bool                    { self.config.paused }
    /// Surface size in physical pixels `(width, height)`.
    pub fn size(&self) -> (u32, u32)                   { (self.renderer.width, self.renderer.height) }
    /// wgpu device — needed by UI renderers (egui-wgpu etc.)
    pub fn device(&self) -> &wgpu::Device              { &self.renderer.device }
    /// wgpu queue — needed by UI renderers.
    pub fn queue(&self) -> &wgpu::Queue                { &self.renderer.queue }
    /// Surface texture format — needed to create egui-wgpu renderer.
    pub fn surface_format(&self) -> wgpu::TextureFormat { self.renderer.surface_config.format }

    /// Return a [`RenderContext`] holding shared GPU references.
    ///
    /// Use this to upload egui textures / buffers *before* calling
    /// [`render_with_ui`][Self::render_with_ui], so the two calls don't
    /// conflict over `&engine` vs `&mut engine`.
    ///
    /// ```no_run
    /// let ctx = engine.render_context();
    /// for (id, delta) in &textures_delta.set {
    ///     egui_renderer.update_texture(ctx.device, ctx.queue, *id, delta);
    /// }
    /// engine.render_with_ui(|enc, view| { ... })?;
    /// ```
    pub fn render_context(&self) -> RenderContext<'_> {
        RenderContext {
            device: &self.renderer.device,
            queue:  &self.renderer.queue,
            surface_format: self.renderer.surface_config.format,
            width:  self.renderer.width,
            height: self.renderer.height,
        }
    }

    // ── Drive API ────────────────────────────────────────────────────

    /// Advance physics by `dt` seconds and consume all pending input.
    ///
    /// Call **before** `render()`. No-op when paused.
    ///
    /// `dt` — frame delta in seconds. Clamp to `0.016667` to avoid instability.
    pub fn update(&mut self, dt: f32) {
        if self.config.paused { return; }

        // Colour cycling
        self.color_timer += dt * self.config.color_update_speed;
        if self.color_timer >= 1.0 {
            self.color_timer = 0.0;
            for p in self.input.points.iter_mut() {
                if p.active { p.color = random_color(); }
            }
        }

        // Drain input queues
        let splats = std::mem::take(&mut self.input.splat_queue);
        let bursts = std::mem::take(&mut self.input.burst_queue);

        for count in bursts {
            self.fluid.multiple_splats(&self.renderer, count, &self.config);
        }
        for s in &splats {
            self.fluid.splat(&self.renderer, s.x, s.y, s.dx, s.dy, s.color, &self.config);
        }

        // Pointer-driven splats
        let aspect = self.renderer.width as f32 / self.renderer.height as f32;
        let force  = self.config.splat_force;
        for p in self.input.points.iter_mut() {
            if p.active && p.moved {
                let dx = (p.x - p.prev_x) * aspect * force;
                let dy = (p.y - p.prev_y)           * force;
                self.fluid.splat(&self.renderer, p.x, p.y, dx, dy, p.color, &self.config);
                p.moved = false;
            }
        }

        self.fluid.step(&self.renderer, dt, &self.config);
    }

    /// Render the current frame to the surface (no UI overlay).
    pub fn render(&self) -> Result<(), wgpu::SurfaceError> {
        self.renderer.render(
            &self.fluid,
            self.config.shading,
            self.config.bloom,
            self.config.sunrays,
            [
                self.config.back_color[0],
                self.config.back_color[1],
                self.config.back_color[2],
                self.config.back_alpha,
            ],
            self.config.bloom_color_tint,
            self.config.bloom_hdr_power,
            self.config.tone_map_exposure,
            |_, _| {},
        )
    }

    /// Render the current frame and overlay UI via a callback.
    ///
    /// `draw_ui(encoder, target_view)` is called after the fluid composite,
    /// before the frame is presented — ideal for egui overlays.
    pub fn render_with_ui<F>(&self, draw_ui: F) -> Result<(), wgpu::SurfaceError>
    where F: FnOnce(&mut wgpu::CommandEncoder, &wgpu::TextureView) {
        self.renderer.render(
            &self.fluid,
            self.config.shading,
            self.config.bloom,
            self.config.sunrays,
            [
                self.config.back_color[0],
                self.config.back_color[1],
                self.config.back_color[2],
                self.config.back_alpha,
            ],
            self.config.bloom_color_tint,
            self.config.bloom_hdr_power,
            self.config.tone_map_exposure,
            draw_ui,
        )
    }

    /// Like [`render_with_ui`][Self::render_with_ui] but also passes `device` and `queue`
    /// into the closure, so egui-wgpu `update_buffers` can be called inside the same
    /// closure without a separate `render_context()` call.
    ///
    /// Signature: `draw_ui(encoder, view, device, queue)`
    ///
    /// ```no_run
    /// engine.render_with_ui_split(|enc, view, device, queue| {
    ///     egui_renderer.update_buffers(device, queue, enc, &prims, &sd);
    ///     // begin render pass and call egui_renderer.render(...)
    /// })?;
    /// ```
    pub fn render_with_ui_split<F>(&self, draw_ui: F) -> Result<(), wgpu::SurfaceError>
    where F: FnOnce(&mut wgpu::CommandEncoder, &wgpu::TextureView, &wgpu::Device, &wgpu::Queue) {
        let device = &self.renderer.device;
        let queue  = &self.renderer.queue;
        self.renderer.render(
            &self.fluid,
            self.config.shading,
            self.config.bloom,
            self.config.sunrays,
            [
                self.config.back_color[0],
                self.config.back_color[1],
                self.config.back_color[2],
                self.config.back_alpha,
            ],
            self.config.bloom_color_tint,
            self.config.bloom_hdr_power,
            self.config.tone_map_exposure,
            move |enc, view| draw_ui(enc, view, device, queue),
        )
    }

    /// Notify the engine that the surface has been resized.
    ///
    /// Automatically rebuilds the swap chain and all simulation textures.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
        self.fluid.resize(&self.renderer, &self.config);
    }
}
