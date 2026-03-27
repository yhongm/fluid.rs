//! # fluid_core_desktop_demo — Reference application driving the fluid_core library
//!
//! This binary owns the window (winit) and the event loop.
//! All fluid physics, GPU setup, and rendering is delegated to the SDK.
//! **No private SDK fields are accessed — pure public API only.**
//!
//! ## SDK call pattern used here
//! ```text
//! FluidEngine::new(window, w, h, &config)      → async init
//! engine.input.pointer_move(id, nx, ny)        → mouse tracking
//! engine.input.pointer_down(id, nx, ny, color) → drag start
//! engine.input.pointer_up(id)                  → drag end
//! engine.input.inject(x, y, dx, dy, color)     → programmatic splat
//! engine.input.burst(n)                        → random explosion
//! engine.update(dt)                            → advance physics
//! engine.render_context()                      → get device/queue refs
//! engine.render_with_ui_split(|enc,view,dev,q| → fluid + UI overlay
//! engine.config_mut().curl = 60.0              → live parameter edit
//! engine.set_bloom(true)                       → convenience setter
//! engine.toggle_paused()                       → pause / resume
//! engine.resize(w, h)                          → on window resize
//! ```

use std::sync::Arc;
use std::time::Instant;

use winit::{event::*, event_loop::EventLoop, window::WindowBuilder};
use egui::Slider;
use egui_wgpu::ScreenDescriptor;

use fluid_core::{FluidEngine, FluidConfig};

#[derive(PartialEq)]
enum Page {
    Main,
    CircleDemo,
    LetterNDemo,
    SpiralDemo,
}

fn main() {
    env_logger::init();

    // ── 1. Create window (demo application's responsibility) ────────
    let event_loop = EventLoop::new().expect("EventLoop::new");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Fluid Simulation — fluid_core SDK")
            .with_inner_size(winit::dpi::LogicalSize::new(1280u32, 720u32))
            .with_transparent(true)
            .build(&event_loop)
            .unwrap(),
    );

    // ── 2. Create SDK engine ─────────────────────────────────────────
    // Arc<winit::Window> implements HasWindowHandle + HasDisplayHandle.
    // The SDK accepts any such type — winit is not a SDK dependency.
    let sz = window.inner_size();
    let mut initial_config = FluidConfig::default();
    initial_config.brightness_min = 0.3;
    initial_config.brightness_max = 1.0;
    initial_config.density_dissipation = 1.0;
    initial_config.velocity_dissipation = 0.2;
    initial_config.curl = 30.0;
    initial_config.splat_force = 6000.0;
    initial_config.bloom_intensity = 0.8;
    initial_config.bloom_threshold = 0.6;
    initial_config.color_update_speed = 10.0;
    initial_config.sim_resolution = 128;
    initial_config.dye_resolution = 1024;
    initial_config.pressure = 0.8;
    initial_config.pressure_iterations = 20;
    initial_config.splat_radius = 0.25;
    initial_config.shading = true;
    initial_config.colorful = true;
    initial_config.bloom = true;
    initial_config.bloom_iterations = 8;
    initial_config.bloom_resolution = 256;
    initial_config.bloom_soft_knee = 0.7;
    initial_config.sunrays = true;
    initial_config.sunrays_resolution = 196;
    initial_config.sunrays_weight = 1.0;
    initial_config.capture_resolution = 512;
    initial_config.back_color = [1.0, 1.0, 1.0];
    initial_config.back_alpha = 0.0;
    let mut engine = pollster::block_on(
        FluidEngine::new(window.clone(), sz.width, sz.height, &initial_config)
    );

    // ── 3. Set up egui using SDK public accessors ────────────────────
    let egui_ctx = egui::Context::default();
    let mut egui_state = egui_winit::State::new(
        egui_ctx.clone(),
        egui_ctx.viewport_id(),
        &*window,
        None, None,
    );
    // engine.render_context() gives device & surface_format with no borrow conflict
    let mut egui_renderer = {
        let rc = engine.render_context();
        egui_wgpu::Renderer::new(rc.device, rc.surface_format, None, 1)
    };

    let mut last = Instant::now();
    let start_time = Instant::now();
    let mut last_inject_time = Instant::now();
    let mut current_page = Page::Main;
    let mut cx   = 0.5f32;
    let mut cy   = 0.5f32;

    let _ = event_loop.run(move |event, target| {
        match event {
            Event::WindowEvent { window_id, event } if window_id == window.id() => {
                // egui sees events first
                if egui_state.on_window_event(&window, &event).consumed { return; }

                match event {
                    // ── Quit ──────────────────────────────────────────
                    WindowEvent::CloseRequested => target.exit(),

                    // ── Keyboard → SDK setters ────────────────────────
                    WindowEvent::KeyboardInput { event: ke, .. }
                        if ke.state == ElementState::Pressed =>
                    {
                        match ke.logical_key.as_ref() {
                            winit::keyboard::Key::Named(
                                winit::keyboard::NamedKey::Escape,
                            ) => target.exit(),

                            winit::keyboard::Key::Character(s) => match &s[..] {
                                "p" | "P" => { engine.toggle_paused(); }
                                " " => {
                                    use rand::Rng;
                                    engine.input.burst(
                                        rand::thread_rng().gen_range(5u32..25),
                                    );
                                }
                                "b" | "B" => {
                                    let v = !engine.config().bloom;
                                    engine.set_bloom(v);
                                }
                                "s" | "S" => {
                                    let v = !engine.config().sunrays;
                                    engine.set_sunrays(v);
                                }
                                "h" | "H" => {
                                    let v = !engine.config().shading;
                                    engine.set_shading(v);
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }

                    // ── Mouse move → engine.input.pointer_move ────────
                    WindowEvent::CursorMoved { position, .. } => {
                        let sz = window.inner_size();
                        cx = position.x as f32 / sz.width  as f32;
                        cy = position.y as f32 / sz.height as f32;
                        // Safe to call every frame regardless of mouse button state.
                        // The SDK only injects fluid when the pointer is active (down).
                        engine.input.pointer_move(0, cx, cy);
                    }

                    // ── Mouse button → pointer_down / pointer_up ──────
                    WindowEvent::MouseInput {
                        state,
                        button: MouseButton::Left,
                        ..
                    } => match state {
                        ElementState::Pressed  => engine.input.pointer_down(0, cx, cy, None),
                        ElementState::Released => engine.input.pointer_up(0),
                    },

                    // ── Window resize → engine.resize ─────────────────
                    WindowEvent::Resized(sz) => engine.resize(sz.width, sz.height),

                    // ── Render ────────────────────────────────────────
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt  = now.duration_since(last).as_secs_f32().min(0.016667);
                        last    = now;

                        if current_page == Page::CircleDemo {
                            let t = start_time.elapsed().as_secs_f32();
                            // Reduced radius to 0.15 (half of 0.3)
                            let radius = 0.15;
                            let speed = 2.0;
                            
                            // Check if 100ms has passed since last injection
                            if last_inject_time.elapsed().as_millis() >= 100 {
                                let cx = 0.5 + radius * (t * speed).cos();
                                let cy = 0.5 + radius * (t * speed).sin();
                                let dx = -radius * speed * (t * speed).sin() * 2000.0;
                                let dy = radius * speed * (t * speed).cos() * 2000.0;
                                
                                // Use time for color
                                let r = (t * 1.0).sin().abs();
                                let g = (t * 1.3).sin().abs();
                                let b = (t * 1.7).sin().abs();
                                
                                engine.input.inject(cx, cy, dx, dy, [r, g, b]);
                                last_inject_time = Instant::now();
                            }
                        } else if current_page == Page::LetterNDemo {
                            let t = start_time.elapsed().as_secs_f32();
                            let speed = 1.0;
                            // Letter N path: Up -> Down-Right -> Up
                            // Define 3 segments for 'N'
                            // 1. (0.3, 0.8) -> (0.3, 0.2) (Up stroke, visually bottom to top)
                            // 2. (0.3, 0.2) -> (0.7, 0.8) (Diagonal down)
                            // 3. (0.7, 0.8) -> (0.7, 0.2) (Up stroke)
                            
                            // Total cycle time for N
                            let cycle_t = t * speed % 3.0;
                            
                            let (tx, ty, vx, vy) = if cycle_t < 1.0 {
                                // Segment 1: Bottom-Left (0.3, 0.8) to Top-Left (0.3, 0.2)
                                let p = cycle_t; // 0 to 1
                                (0.3, 0.8 - 0.6 * p, 0.0, -1000.0)
                            } else if cycle_t < 2.0 {
                                // Segment 2: Top-Left (0.3, 0.2) to Bottom-Right (0.7, 0.8)
                                let p = cycle_t - 1.0;
                                (0.3 + 0.4 * p, 0.2 + 0.6 * p, 1000.0, 1000.0)
                            } else {
                                // Segment 3: Bottom-Right (0.7, 0.8) to Top-Right (0.7, 0.2)
                                let p = cycle_t - 2.0;
                                (0.7, 0.8 - 0.6 * p, 0.0, -1000.0)
                            };
                            
                            // Continuous injection for letter drawing
                            let r = (t * 2.0).sin().abs();
                            let g = 0.2;
                            let b = (t * 2.0).cos().abs();
                            engine.input.inject(tx, ty, vx, vy, [r, g, b]);
                        } else if current_page == Page::SpiralDemo {
                            let t = start_time.elapsed().as_secs_f32();
                            let phase = (t * 0.5).fract();
                            let r = 0.05 + 0.35 * phase;
                            let theta = t * 7.0;
                            let x = 0.5 + r * theta.cos();
                            let y = 0.5 + r * theta.sin();
                            let dx = -r * 7.0 * theta.sin() * 1200.0;
                            let dy = r * 7.0 * theta.cos() * 1200.0;
                            let c1 = (t * 1.1).sin().abs();
                            let c2 = (t * 1.7).sin().abs();
                            let c3 = (t * 2.3).sin().abs();
                            engine.input.inject(x.clamp(0.02, 0.98), y.clamp(0.02, 0.98), dx, dy, [c1, c2, c3]);
                        }

                        // ① Advance physics + consume all pending input
                        engine.update(dt);

                        // ② Build egui frame (UI state only, no GPU work yet)
                        egui_ctx.begin_frame(egui_state.take_egui_input(&window));
                        let previous_config = engine.config().clone();
                        
                        egui::TopBottomPanel::top("nav_panel").show(&egui_ctx, |ui| {
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut current_page, Page::Main, "Main");
                                ui.selectable_value(&mut current_page, Page::CircleDemo, "Circle Demo");
                                ui.selectable_value(&mut current_page, Page::LetterNDemo, "Letter N");
                                ui.selectable_value(&mut current_page, Page::SpiralDemo, "Spiral Demo");
                            });
                        });

                        match current_page {
                            Page::Main => build_ui(&egui_ctx, engine.config_mut()),
                            Page::CircleDemo => {
                                egui::Window::new("Circle Demo").show(&egui_ctx, |ui| {
                                    ui.label("Injecting circular fluid motion...");
                                    ui.label("Radius: 0.15");
                                    ui.label("Interval: 100ms");
                                });
                            }
                            Page::LetterNDemo => {
                                egui::Window::new("Letter N Demo").show(&egui_ctx, |ui| {
                                    ui.label("Drawing Letter 'N' path...");
                                });
                            }
                            Page::SpiralDemo => {
                                egui::Window::new("Spiral Demo").show(&egui_ctx, |ui| {
                                    ui.label("Injecting colorful spiral trajectories...");
                                    ui.label("Path: expanding orbit + phase reset");
                                });
                            }
                        }
                        if config_requires_rebuild(&previous_config, engine.config()) {
                            let next_config = engine.config().clone();
                            engine.set_config(next_config);
                        }
                        let egui_out = egui_ctx.end_frame();
                        let prims    = egui_ctx.tessellate(
                            egui_out.shapes,
                            egui_out.pixels_per_point,
                        );
                        let (w, h) = engine.size();
                        let sd = ScreenDescriptor {
                            size_in_pixels:  [w, h],
                            pixels_per_point: window.scale_factor() as f32,
                        };

                        // ③ Upload changed egui textures (font atlas etc.)
                        //    render_context() provides device & queue as shared refs.
                        //    This borrow is released before render_with_ui_split.
                        {
                            let rc = engine.render_context();
                            for (id, delta) in &egui_out.textures_delta.set {
                                egui_renderer.update_texture(
                                    rc.device, rc.queue, *id, delta,
                                );
                            }
                        }

                        // ④ Render fluid + egui overlay in one frame.
                        //    render_with_ui_split passes (encoder, view, device, queue)
                        //    so egui_renderer.update_buffers can run inside the closure.
                        let result = engine.render_with_ui_split(
                            |enc, view, device, queue| {
                                egui_renderer.update_buffers(
                                    device, queue, enc, &prims, &sd,
                                );
                                let mut rp =
                                    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                                        label: Some("egui"),
                                        color_attachments: &[Some(
                                            wgpu::RenderPassColorAttachment {
                                                view,
                                                resolve_target: None,
                                                ops: wgpu::Operations {
                                                    load:  wgpu::LoadOp::Load,
                                                    store: wgpu::StoreOp::Store,
                                                },
                                            },
                                        )],
                                        depth_stencil_attachment: None,
                                        occlusion_query_set:      None,
                                        timestamp_writes:         None,
                                    });
                                egui_renderer.render(&mut rp, &prims, &sd);
                            },
                        );

                        // ⑤ Free unused egui textures
                        for id in &egui_out.textures_delta.free {
                            egui_renderer.free_texture(id);
                        }

                        // ⑥ Handle surface errors
                        match result {
                            Ok(()) => {}
                            Err(wgpu::SurfaceError::Lost) => {
                                let sz = window.inner_size();
                                engine.resize(sz.width, sz.height);
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                            Err(e) => log::error!("render error: {e:?}"),
                        }

                        window.request_redraw();
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        }
    });
}

// ── egui panel ───────────────────────────────────────────────────────

/// Control panel UI. Receives a mutable ref to `FluidConfig` via
/// `engine.config_mut()` — entirely through the public SDK API.
fn build_ui(ctx: &egui::Context, cfg: &mut FluidConfig) {
    let mut frame = egui::Frame::window(&ctx.style());
    frame.fill = egui::Color32::from_black_alpha(150); // 半透明黑色背景
    frame.shadow = egui::epaint::Shadow::NONE; // 可选：移除阴影使模糊更干净
    frame.rounding = egui::Rounding::same(8.0); // 圆角

    // 在 wgpu 渲染器中直接实现模糊比较复杂，这里我们通过设置窗口的透明背景
    // 并结合 egui 的 Window frame 来达到类似毛玻璃的视觉效果（依赖底层窗口透明）。
    // 若要真正的背景模糊，需要在 fluid 渲染管线后加一趟模糊 pass 给 UI 用。
    
    egui::Window::new("Controls")
        .default_width(270.0)
        .frame(frame)
        .show(ctx, |ui| {
            // 给滚动区加一点内边距
            egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                // Add reset button at the very top
                if ui.button("Reset to Default").clicked() {
                    *cfg = FluidConfig::js_defaults();
                }
            ui.separator();

            ui.heading("Physics");
            ui.add(Slider::new(&mut cfg.density_dissipation, 0.0..=5.0).text("Density dissipation"));
            ui.add(Slider::new(&mut cfg.velocity_dissipation, 0.0..=5.0).text("Velocity dissipation"));
            ui.add(Slider::new(&mut cfg.pressure, 0.0..=1.0).text("Pressure"));
            ui.add(Slider::new(&mut cfg.pressure_iterations, 1..=80).text("Pressure iterations"));
            ui.add(Slider::new(&mut cfg.curl, 0.0..=100.0).text("Curl"));
            ui.add(Slider::new(&mut cfg.color_update_speed, 0.0..=30.0).text("Color update speed"));
            ui.add(Slider::new(&mut cfg.brightness_min, 0.0..=1.0).text("Brightness min"));
            ui.add(Slider::new(&mut cfg.brightness_max, 0.0..=2.0).text("Brightness max"));

            ui.separator();
            ui.heading("Interaction");
            ui.add(Slider::new(&mut cfg.splat_radius, 0.01..=1.0).text("Splat radius"));
            ui.add(Slider::new(&mut cfg.splat_force, 100.0..=10000.0).text("Splat force"));

            ui.separator();
            ui.heading("Resolution");
            ui.add(Slider::new(&mut cfg.sim_resolution, 32..=1024).text("Sim resolution"));
            ui.add(Slider::new(&mut cfg.dye_resolution, 128..=4096).text("Dye resolution"));
            ui.add(Slider::new(&mut cfg.bloom_resolution, 32..=1024).text("Bloom resolution"));
            ui.add(Slider::new(&mut cfg.bloom_iterations, 1..=12).text("Bloom iterations"));
            ui.add(Slider::new(&mut cfg.sunrays_resolution, 32..=1024).text("Sunrays resolution"));
            ui.add(Slider::new(&mut cfg.capture_resolution, 128..=4096).text("Capture resolution"));

            ui.separator();
            ui.heading("Rendering");
            ui.checkbox(&mut cfg.shading, "Shading");
            ui.checkbox(&mut cfg.colorful, "Colorful");
            ui.checkbox(&mut cfg.paused, "Paused  [P]");
            ui.label("Background colour");
            ui.color_edit_button_rgb(&mut cfg.back_color);
            ui.add(Slider::new(&mut cfg.back_alpha, 0.0..=1.0).text("Background alpha"));

            ui.separator();
            ui.heading("Bloom  [B]");
            ui.checkbox(&mut cfg.bloom, "Enabled");
            ui.add(Slider::new(&mut cfg.bloom_intensity, 0.0..=2.0).text("Intensity"));
            ui.add(Slider::new(&mut cfg.bloom_threshold, 0.0..=1.0).text("Threshold"));
            ui.add(Slider::new(&mut cfg.bloom_soft_knee, 0.0..=1.0).text("Soft knee"));

            ui.separator();
            ui.label("Bloom advanced");
            ui.horizontal(|ui| {
                ui.label("Color tint  R G B");
                ui.add(Slider::new(&mut cfg.bloom_color_tint[0], 0.0..=3.0).text("R"));
                ui.add(Slider::new(&mut cfg.bloom_color_tint[1], 0.0..=3.0).text("G"));
                ui.add(Slider::new(&mut cfg.bloom_color_tint[2], 0.0..=3.0).text("B"));
            });
            ui.add(
                Slider::new(&mut cfg.bloom_hdr_power, 0.5..=4.0)
                    .text("HDR power")
                    .logarithmic(true),
            );

            ui.separator();
            ui.heading("Sunrays  [S]");
            ui.checkbox(&mut cfg.sunrays, "Enabled");
            ui.add(Slider::new(&mut cfg.sunrays_weight, 0.0..=5.0).text("Weight"));
            ui.add(
                Slider::new(&mut cfg.sunrays_exposure, 0.1..=5.0)
                    .text("Exposure")
                    .logarithmic(true),
            );
            ui.add(
                Slider::new(&mut cfg.sunrays_decay, 0.5..=0.99)
                    .text("Decay"),
            );

            ui.separator();
            ui.heading("Exposure / Tone");
            ui.add(
                Slider::new(&mut cfg.tone_map_exposure, 0.1..=8.0)
                    .text("Global exposure")
                    .logarithmic(true),
            );

            ui.separator();
            ui.label("Space: random burst   P: pause   B/S/H: toggles");
        });
    });
}

fn config_requires_rebuild(prev: &FluidConfig, next: &FluidConfig) -> bool {
    prev.sim_resolution != next.sim_resolution
        || prev.dye_resolution != next.dye_resolution
        || prev.bloom_resolution != next.bloom_resolution
        || prev.bloom_iterations != next.bloom_iterations
        || prev.sunrays_resolution != next.sunrays_resolution
}

