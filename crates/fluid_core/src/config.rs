// config.rs - Simulation configuration parameters

#[derive(Debug, Clone)]
pub struct FluidConfig {
    pub sim_resolution: u32,
    pub dye_resolution: u32,
    pub density_dissipation: f32,
    pub velocity_dissipation: f32,
    pub pressure: f32,
    pub pressure_iterations: u32,
    pub curl: f32,
    pub splat_radius: f32,
    pub splat_force: f32,
    pub shading: bool,
    pub colorful: bool,
    pub color_update_speed: f32,
    pub brightness_min: f32,
    pub brightness_max: f32,
    pub paused: bool,
    pub back_color: [f32; 3],
    pub back_alpha: f32,
    pub bloom: bool,
    pub bloom_iterations: u32,
    pub bloom_resolution: u32,
    pub bloom_intensity: f32,
    pub bloom_threshold: f32,
    pub bloom_soft_knee: f32,
    pub sunrays: bool,
    pub sunrays_resolution: u32,
    pub sunrays_weight: f32,
    pub capture_resolution: u32,

    // ── Bloom advanced ──────────────────────────────────────
    /// Bloom color tint multiplier. [1,1,1] = white (original).
    pub bloom_color_tint: [f32; 3],
    /// Bloom HDR brightness power.
    /// 1.0 = linear (original), 2.0 = brighter/diffuse, 0.5 = darker/tighter.
    pub bloom_hdr_power: f32,
    /// Global exposure factor before tone mapping.
    /// 1.0 = original, 2.0 = twice as bright (allows "infinite" brightness).
    pub tone_map_exposure: f32,
    /// Sunrays scatter exposure (original hardcoded 0.7).
    pub sunrays_exposure: f32,
    /// Sunrays decay rate (original hardcoded 0.95).
    pub sunrays_decay: f32,
}

impl Default for FluidConfig {
    fn default() -> Self {
        Self::js_defaults()
    }
}

impl FluidConfig {
    /// 与原版 JS PavelDoGreat/WebGL-Fluid-Simulation 完全一致的默认参数。
    /// 新增参数取中性值，使渲染行为与 JS 版本像素级对齐。
    pub fn js_defaults() -> Self {
        Self {
            // ── 模拟参数（直接来自 JS config 对象）────────────────────
            sim_resolution:       128,
            dye_resolution:       1024,
            capture_resolution:   512,
            density_dissipation:  1.0,
            velocity_dissipation: 0.2,
            pressure:             0.8,
            pressure_iterations:  20,
            curl:                 30.0,
            splat_radius:         0.25,   // JS: 0.25（传入前除以100再correctRadius）
            splat_force:          6000.0,

            // ── 颜色参数 ───────────────────────────────────────────────
            shading:              true,
            colorful:             true,
            color_update_speed:   10.0,
            // JS generateColor(): HSV(rand,1,1)*0.15，固定亮度 0.15
            // 设 min=max=0.15 使 Rust 版颜色生成与 JS 完全一致
            brightness_min:       0.15,
            brightness_max:       0.15,
            paused:               false,

            // ── 背景（JS: BACK_COLOR={r:0,g:0,b:0}, TRANSPARENT=false）
            back_color:           [0.0, 0.0, 0.0],
            back_alpha:           1.0,   // TRANSPARENT=false → 不透明背景

            // ── Bloom（直接来自 JS config 对象）──────────────────────
            bloom:                true,
            bloom_iterations:     8,
            bloom_resolution:     256,
            bloom_intensity:      0.8,
            bloom_threshold:      0.6,
            bloom_soft_knee:      0.7,

            // ── Sunrays（直接来自 JS config 对象）────────────────────
            sunrays:              true,
            sunrays_resolution:   196,
            sunrays_weight:       1.0,

            // ── 新增参数：中性值，渲染行为与 JS 原版完全一致 ───────────
            // JS display shader 无此概念，[1,1,1] = 不改变 bloom 颜色
            bloom_color_tint:     [1.0, 1.0, 1.0],
            // JS display shader 无此概念，1.0 = 线性叠加，不做 pow 变换
            bloom_hdr_power:      1.0,
            // JS display shader 无此概念，1.0 = 不做额外曝光
            tone_map_exposure:    1.0,
            // JS sunraysShader 硬编码：float Exposure = 0.7（line 704）
            sunrays_exposure:     0.7,
            // JS sunraysShader 硬编码：float Decay = 0.95（line 703）
            sunrays_decay:        0.95,
        }
    }
}
