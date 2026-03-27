# fluid_core SDK — 完整 API 文档

> **crate**: `fluid_core` (library)  
> **companion**: `fluid_core_desktop_demo` (example binary)  
> **Rust edition**: 2021  
> **GPU 后端**: wgpu 0.20（Vulkan / Metal / DX12 / OpenGL ES）  
> **窗口系统**: **无依赖** — 接受任何实现 `raw-window-handle` 的窗口

---

## 目录

1. [架构与设计原则](#1-架构与设计原则)
2. [快速上手](#2-快速上手)
3. [Cargo.toml 集成](#3-cargotoml-集成)
4. [类型：`FluidConfig`](#4-类型-fluidconfig)
5. [类型：`InputManager`](#5-类型-inputmanager)
6. [类型：`RenderContext`](#6-类型-rendercontext)
7. [类型：`FluidEngine`](#7-类型-fluidengine)
   - [构造](#71-构造)
   - [输入 API](#72-输入-api)
   - [配置 API](#73-配置-api)
   - [参数 Setter 速查表](#74-参数-setter-速查表)
   - [驱动 API](#75-驱动-api)
   - [查询 API](#76-查询-api)
8. [egui 集成模式](#8-egui-集成模式)
9. [使用场景示例](#9-使用场景示例)
10. [物理管线参考](#10-物理管线参考)
11. [FluidConfig 字段完整参考](#11-fluidconfig-字段完整参考)

---

## 1. 架构与设计原则

```
┌──────────────────────────────────────────────────────────┐
│  你的应用（fluid_core_desktop_demo / 自定义 crate）                     │
│  ├─ 窗口系统：winit / SDL2 / GLFW / 裸 HWND              │
│  ├─ 事件循环                                              │
│  ├─ egui-winit / 自定义 UI                               │
│  └─ engine.input.* / engine.update / engine.render_*     │
└───────────────────────┬──────────────────────────────────┘
                        │ 仅通过公开 API 访问
┌───────────────────────▼──────────────────────────────────┐
│  fluid_core (SDK crate)                                   │
│  ├─ FluidEngine        ← 门面，唯一入口                  │
│  ├─ InputManager       ← 输入解耦（无窗口依赖）          │
│  ├─ FluidConfig        ← 所有运行时参数                  │
│  ├─ RenderContext      ← GPU 句柄的借用安全快照          │
│  ├─ Renderer (内部)   ← wgpu 设备/管线/交换链           │
│  └─ FluidSim (内部)   ← Navier-Stokes 物理引擎          │
└──────────────────────────────────────────────────────────┘
```

**设计原则**

| 原则 | 实现方式 |
|------|----------|
| **零窗口依赖** | `fluid_core` Cargo.toml 不含 winit，使用 `raw-window-handle` trait |
| **输入解耦** | 引擎不订阅任何系统事件，全部通过 `engine.input.*` 统一驱动 |
| **借用安全** | `render_context()` 和 `render_with_ui_split()` 消除 Rust 借用冲突 |
| **配置热修改** | 所有参数运行时可改，分辨率变更自动重建 GPU 纹理 |
| **UI 可叠加** | `render_with_ui_split` 在流体画面上叠加任意 wgpu 渲染 |

---

## 2. 快速上手

```rust
use fluid_core::{FluidEngine, FluidConfig};

// 1. 创建引擎（接受任何实现了 raw-window-handle 的窗口）
let mut engine = FluidEngine::new(
    &window,            // Arc<winit::Window> 或任意窗口包装
    width, height,      // 物理像素尺寸
    &FluidConfig::default(),
).await;

// 2. 每帧循环
loop {
    // 传入输入（来源随意）
    engine.input.pointer_move(0, cursor_x / w, cursor_y / h);

    // 或程序化注入（无需鼠标）
    engine.input.inject(0.5, 0.5, 800.0, -600.0, [0.9, 0.3, 0.1]);

    // 推进物理
    engine.update(dt);

    // 渲染
    engine.render()?;
}
```

---

## 3. Cargo.toml 集成

```toml
# 你的 Cargo.toml
[dependencies]
# 方式 1：在当前仓库内引用（推荐用于本项目 demos）
fluid_core = { path = "crates/fluid_core" }

# 方式 2：跨仓库本地路径引用（按你的目录结构调整）
# fluid_core = { path = "../fluid_core" }

# 方式 3：发布后从 crates.io 引用
# fluid_core = "0.1"

# 窗口系统（SDK 本身不需要，应用层按需添加）
winit = "0.29"
```

`fluid_core` 自身的依赖（无 winit）：

```toml
[dependencies]
wgpu             = "0.20"
raw-window-handle = "0.6"
bytemuck         = { version = "1.14", features = ["derive"] }
log              = "0.4"
rand             = "0.8"
egui             = "0.28"   # 仅 egui-wgpu renderer，不含 egui-winit
egui-wgpu        = "0.28"
```

---

## 4. 类型：`FluidConfig`

```rust
use fluid_core::FluidConfig;
```

所有模拟参数的集合，字段均为 `pub`，可直接读写或通过 `engine.config_mut()` 修改。

详细字段见[第 11 节](#11-fluidconfig-字段完整参考)。

```rust
// 使用默认值
let config = FluidConfig::default();

// 自定义初始参数
let config = FluidConfig {
    sim_resolution: 256,
    curl: 50.0,
    bloom: false,
    ..FluidConfig::default()
};
```

---

## 5. 类型：`InputManager`

```rust
// 通过 engine.input 直接访问，无需手动构造
engine.input.pointer_down(id, x, y, color);
```

输入管理器：收集所有外部输入，帧末由引擎一次性消费。
不持有任何 GPU / 窗口句柄，可在任意线程操作。

### `pointer_down(id, x, y, color)`

```rust
pub fn pointer_down(&mut self, id: u32, x: f32, y: f32, color: Option<[f32; 3]>)
```

注册输入点按下（鼠标左键 / 触摸开始）。

| 参数 | 说明 |
|------|------|
| `id` | 唯一标识，鼠标固定传 `0`，多点触摸传系统手指 ID |
| `x` | 归一化 X `[0,1]`，算法：`pixel_x / window_width` |
| `y` | 归一化 Y `[0,1]`，算法：`pixel_y / window_height` |
| `color` | 染料 RGB；`None` = 自动随机颜色 |

### `pointer_move(id, x, y)`

```rust
pub fn pointer_move(&mut self, id: u32, x: f32, y: f32)
```

更新输入点位置。**每帧无条件调用安全**（未激活时不产生流体效果）。

### `pointer_up(id)`

```rust
pub fn pointer_up(&mut self, id: u32)
```

注册输入点抬起（鼠标释放 / 触摸结束）。

### `inject(x, y, dx, dy, color)`

```rust
pub fn inject(&mut self, x: f32, y: f32, dx: f32, dy: f32, color: [f32; 3])
```

**无需鼠标**，直接在指定坐标注入速度+颜色 splat。

| 参数 | 推荐范围 | 说明 |
|------|---------|------|
| `x`, `y` | `[0,1]` | 归一化坐标 |
| `dx`, `dy` | `-8000..8000` | 速度冲量 |
| `color` | `[0,1]` 每分量 | RGB 染料颜色 |

```rust
// 屏幕中心向右上注入橙色
engine.input.inject(0.5, 0.5, 1200.0, -900.0, [1.0, 0.5, 0.0]);

// 沿水平线扫过
for i in 0..20 {
    engine.input.inject(i as f32 / 20.0, 0.5, 300.0, 0.0, [0.2, 0.8, 0.4]);
}
```

### `burst(count)`

```rust
pub fn burst(&mut self, count: u32)
```

在随机位置发射若干随机颜色 splat（等效于按空格键的爆炸效果）。

```rust
engine.input.burst(15);
```

---

## 6. 类型：`RenderContext`

```rust
use fluid_core::RenderContext;
```

```rust
pub struct RenderContext<'a> {
    pub device:         &'a wgpu::Device,
    pub queue:          &'a wgpu::Queue,
    pub surface_format: wgpu::TextureFormat,
    pub width:          u32,
    pub height:         u32,
}
```

通过 `engine.render_context()` 获取，**不持有可变借用**，
专门用于在 `render_with_ui_split` 之前上传 egui 纹理，解决借用冲突。

```rust
// 典型使用模式
let rc = engine.render_context();
for (id, delta) in &textures_delta.set {
    egui_renderer.update_texture(rc.device, rc.queue, *id, delta);
}
// rc 在此释放
engine.render_with_ui_split(|enc, view, dev, q| { ... })?;
```

---

## 7. 类型：`FluidEngine`

```rust
use fluid_core::FluidEngine;
```

SDK 的唯一入口。公开字段：
- **`input`**（`pub`）：[`InputManager`]，直接调用 `engine.input.*`

### 7.1 构造

```rust
pub async fn new<W>(
    window: W,
    width:  u32,
    height: u32,
    config: &FluidConfig,
) -> FluidEngine<'_>
where
    W: HasWindowHandle + HasDisplayHandle + Send + Sync + '_,
```

异步创建引擎实例，初始化 wgpu 设备、交换链、所有 GPU 纹理和 16 个渲染管线。

`window` 可以是任何实现了 `raw-window-handle 0.6` 的类型：

| 窗口系统 | 传入方式 |
|---------|---------|
| winit | `Arc<winit::Window>` 或 `&winit::Window` |
| SDL2 | 实现 `HasWindowHandle` 的包装结构 |
| 裸 Win32 | `raw_window_handle::Win32WindowHandle` 包装 |

```rust
// winit
let engine = FluidEngine::new(window.clone(), sz.width, sz.height, &cfg).await;

// 自定义窗口包装
let engine = FluidEngine::new(&my_window_wrapper, 1920, 1080, &cfg).await;
```

---

### 7.2 输入 API

通过公开字段 `engine.input` 直接访问，见[第 5 节](#5-类型-inputmanager)。

```rust
engine.input.pointer_down(0, cx, cy, None);
engine.input.pointer_move(0, cx, cy);
engine.input.pointer_up(0);
engine.input.inject(x, y, dx, dy, color);
engine.input.burst(n);
```

---

### 7.3 配置 API

#### `config() -> &FluidConfig`

返回当前配置的不可变引用。

```rust
if engine.config().bloom { println!("bloom on"); }
```

#### `config_mut() -> &mut FluidConfig`

返回当前配置的可变引用，直接修改字段下一帧生效。

> ⚠️ 修改分辨率字段请用 [`set_config`](#set_config)，否则 GPU 纹理不会重建。

```rust
engine.config_mut().curl = 60.0;
```

#### `set_config(config: FluidConfig)` {#set_config}

完整替换配置；若以下字段变化，**自动重建 GPU 纹理**：  
`sim_resolution` / `dye_resolution` / `bloom_resolution` /
`bloom_iterations` / `sunrays_resolution`

```rust
let mut cfg = engine.config().clone();
cfg.sim_resolution = 256;
engine.set_config(cfg);
```

---

### 7.4 参数 Setter 速查表

所有 setter 直接修改对应字段，下一帧 `update()` 生效。

| 方法 | 参数范围 | 说明 |
|------|---------|------|
| `set_density_dissipation(f32)` | `0.0–5.0` | 染料消散速率 |
| `set_velocity_dissipation(f32)` | `0.0–5.0` | 速度衰减速率 |
| `set_curl(f32)` | `0.0–100.0` | 涡度约束（漩涡感） |
| `set_splat_radius(f32)` | `0.001–1.0` | splat 高斯核半径 |
| `set_splat_force(f32)` | `> 0` | 速度冲量乘数 |
| `set_pressure(f32)` | `0.0–1.0` | 压力场每帧保留比例 |
| `set_shading(bool)` | — | 法线光照 shading |
| `set_bloom(bool)` | — | Bloom 泛光开关 |
| `set_bloom_intensity(f32)` | `≥ 0` | 泛光亮度乘数 |
| `set_bloom_threshold(f32)` | `0.0–1.0` | 泛光亮度阈值 |
| `set_sunrays(bool)` | — | Sunrays 体积光开关 |
| `set_sunrays_weight(f32)` | `≥ 0` | 射线强度权重 |
| `set_background_color(r,g,b)` | `[0,1]` | 背景 RGB |
| `set_background_alpha(f32)` | `0.0–1.0` | 背景透明度（0=全透明，1=不透明） |
| `set_paused(bool)` | — | 暂停状态 |
| `toggle_paused() -> bool` | — | 切换暂停，返回新状态 |

---

### 7.5 驱动 API

#### `update(dt: f32)`

推进物理模拟一帧，消费 `engine.input` 中所有待处理的输入。

**必须在 `render*` 之前调用。** 暂停时立即返回，不执行任何 GPU 工作。

`dt` — 帧时间（秒），**建议上限夹到 `0.016667`**（约 60fps）防止低帧率数值爆炸。

```rust
let dt = last.elapsed().as_secs_f32().min(0.016667);
engine.update(dt);
```

**帧内执行顺序：**

```
1. 颜色轮转（按 color_update_speed 定期换色）
2. burst splats（随机爆炸）
3. 程序化 splat 队列（inject 的请求）
4. pointer-driven splats（active+moved → 速度冲量）
5. FluidSim::step(dt)：curl → vorticity → divergence
   → pressure×N → gradient_subtract → velocity advection → dye advection
6. Bloom 后处理（若开启）
7. Sunrays 后处理（若开启）
```

---

#### `render() -> Result<(), wgpu::SurfaceError>`

将当前帧渲染到窗口表面（无 UI 叠加）。

```rust
match engine.render() {
    Ok(()) => {}
    Err(wgpu::SurfaceError::Lost) => engine.resize(w, h),
    Err(e) => eprintln!("{e:?}"),
}
```

---

#### `render_with_ui<F>(draw_ui: F) -> Result<...>`

```rust
pub fn render_with_ui<F>(&self, draw_ui: F) -> Result<(), wgpu::SurfaceError>
where F: FnOnce(&mut wgpu::CommandEncoder, &wgpu::TextureView)
```

渲染流体画面，并在闭包中叠加自定义 UI。  
闭包在流体 composite 后、帧提交前执行。

**限制**：闭包内无法获取 `device` / `queue`（借用冲突）。  
需要 `device` / `queue` 时，请使用 [`render_with_ui_split`](#render_with_ui_split)。

---

#### `render_with_ui_split<F>(draw_ui: F) -> Result<...>` {#render_with_ui_split}

```rust
pub fn render_with_ui_split<F>(&self, draw_ui: F) -> Result<(), wgpu::SurfaceError>
where F: FnOnce(
    &mut wgpu::CommandEncoder,
    &wgpu::TextureView,
    &wgpu::Device,
    &wgpu::Queue,
)
```

与 `render_with_ui` 相同，但额外传入 `device` 和 `queue`，
使 egui-wgpu 的 `update_buffers` 可在同一闭包内调用。

**标准 egui 集成模式**（见[第 8 节](#8-egui-集成模式)）：

```rust
engine.render_with_ui_split(|enc, view, device, queue| {
    egui_renderer.update_buffers(device, queue, enc, &prims, &sd);
    let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor { ... });
    egui_renderer.render(&mut rp, &prims, &sd);
})?;
```

---

#### `resize(width: u32, height: u32)`

通知引擎窗口尺寸变化，自动重建交换链和所有流体纹理。

```rust
WindowEvent::Resized(sz) => engine.resize(sz.width, sz.height),
// 也应在 SurfaceError::Lost 时调用
```

---

### 7.6 查询 API

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `config()` | `&FluidConfig` | 当前配置不可变引用 |
| `config_mut()` | `&mut FluidConfig` | 当前配置可变引用 |
| `is_paused()` | `bool` | 是否暂停 |
| `size()` | `(u32, u32)` | 渲染目标尺寸 `(width, height)` |
| `device()` | `&wgpu::Device` | wgpu 逻辑设备 |
| `queue()` | `&wgpu::Queue` | wgpu 命令队列 |
| `surface_format()` | `wgpu::TextureFormat` | 交换链纹理格式 |
| `render_context()` | `RenderContext<'_>` | GPU 句柄借用安全快照 |

---

## 8. egui 集成模式

egui 需要在渲染帧内访问 `device` 和 `queue`，这与 `engine` 的不可变借用冲突。
SDK 提供三种工具解决此问题：

```rust
// ─── 初始化（一次性）───────────────────────────────────────────────
let rc = engine.render_context();
let mut egui_renderer = egui_wgpu::Renderer::new(
    rc.device,          // ← 来自 render_context()
    rc.surface_format,  // ← 来自 render_context()
    None, 1,
);
// rc 的借用在此结束

// ─── 每帧渲染（三步）──────────────────────────────────────────────

// 步骤 1：上传纹理变更（字体图集等）
// render_context() 仅不可变借用 engine，与后续调用不冲突。
{
    let rc = engine.render_context();
    for (id, delta) in &egui_out.textures_delta.set {
        egui_renderer.update_texture(rc.device, rc.queue, *id, delta);
    }
} // rc 借用在此释放

// 步骤 2：流体 + egui 一帧合成
// render_with_ui_split 将 device/queue 传入闭包，消除借用冲突。
engine.render_with_ui_split(|enc, view, device, queue| {
    // update_buffers 在闭包内调用
    egui_renderer.update_buffers(device, queue, enc, &prims, &sd);

    // 叠加 egui render pass
    let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load:  wgpu::LoadOp::Load,   // Load = 保留流体底层
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set:      None,
        timestamp_writes:         None,
        label: Some("egui"),
    });
    egui_renderer.render(&mut rp, &prims, &sd);
})?;

// 步骤 3：释放不再使用的纹理
for id in &egui_out.textures_delta.free {
    egui_renderer.free_texture(id);
}
```

---

## 9. 使用场景示例

### 场景 A：标准鼠标（winit）

```rust
WindowEvent::CursorMoved { position, .. } => {
    let nx = position.x as f32 / window_w as f32;
    let ny = position.y as f32 / window_h as f32;
    engine.input.pointer_move(0, nx, ny); // 每帧无条件调用
}
WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
    match state {
        ElementState::Pressed  => engine.input.pointer_down(0, cx, cy, None),
        ElementState::Released => engine.input.pointer_up(0),
    }
}
```

### 场景 B：多点触摸

```rust
match touch.phase {
    TouchPhase::Started   => engine.input.pointer_down(touch.id as u32, nx, ny, None),
    TouchPhase::Moved     => engine.input.pointer_move(touch.id as u32, nx, ny),
    TouchPhase::Ended |
    TouchPhase::Cancelled => engine.input.pointer_up(touch.id as u32),
}
```

### 场景 C：纯程序化动画（无鼠标）

```rust
fn animate_circle(engine: &mut FluidEngine, t: f32) {
    let angle = t * std::f32::consts::TAU;
    let x  = 0.5 + 0.35 * angle.cos();
    let y  = 0.5 + 0.35 * angle.sin();
    let dx = -angle.sin() * 2000.0;
    let dy =  angle.cos() * 2000.0;
    engine.input.inject(x, y, dx, dy, fluid_core::random_color());
}
```

### 场景 D：音频频谱可视化

```rust
fn on_audio_frame(engine: &mut FluidEngine, spectrum: &[f32]) {
    for (i, &amp) in spectrum.iter().enumerate() {
        if amp > 0.05 {
            let x = i as f32 / spectrum.len() as f32;
            let [r, g, b] = fluid_core::hsv_to_rgb(x, 1.0, 1.0);
            engine.input.inject(
                x, 0.85,
                0.0, -amp * 5000.0,
                [r * 0.15, g * 0.15, b * 0.15],
            );
        }
    }
}
```

### 场景 E：网络/UDP 远程坐标驱动

```rust
while let Ok(pkt) = udp_rx.try_recv() {
    engine.input.inject(pkt.x, pkt.y, pkt.dx, pkt.dy, pkt.color);
}
engine.update(dt);
engine.render()?;
```

### 场景 F：螺旋注入（Spiral Demo）

```rust
fn animate_spiral(engine: &mut FluidEngine, t: f32) {
    // 半径随时间线性扩张，并周期性回到中心
    let phase = (t * 0.5).fract();
    let r = 0.05 + 0.35 * phase;
    let theta = t * 7.0;

    let x = (0.5 + r * theta.cos()).clamp(0.02, 0.98);
    let y = (0.5 + r * theta.sin()).clamp(0.02, 0.98);
    let dx = -r * 7.0 * theta.sin() * 1200.0;
    let dy =  r * 7.0 * theta.cos() * 1200.0;

    let color = [
        (t * 1.1).sin().abs(),
        (t * 1.7).sin().abs(),
        (t * 2.3).sin().abs(),
    ];
    engine.input.inject(x, y, dx, dy, color);
}
```

### 场景 G：运行时动态调参

```rust
// 单参数 setter
engine.set_curl(80.0);
engine.set_bloom_intensity(1.5);
engine.set_background_color(0.02, 0.02, 0.08);
engine.set_background_alpha(0.35);

// 直接修改字段（非分辨率字段）
engine.config_mut().velocity_dissipation = 0.1;

// 分辨率变更（自动重建 GPU 纹理）
let mut cfg = engine.config().clone();
cfg.sim_resolution = 256;
cfg.dye_resolution = 2048;
engine.set_config(cfg);
```

### 场景 H：暂停 + 手动单步

```rust
engine.toggle_paused();         // 暂停

// 暂停状态下手动推进一帧
engine.set_paused(false);
engine.update(0.016667);
engine.set_paused(true);
```

---

## 10. 物理管线参考

`engine.update(dt)` 在 GPU 上依次执行：

| 步骤 | 着色器 | 说明 |
|------|--------|------|
| 1 | `curl.wgsl` | 计算旋度 ∇×v → curl 纹理 |
| 2 | `vorticity.wgsl` | 涡度约束力，防止数值耗散 |
| 3 | `divergence.wgsl` | 计算散度 ∇·v |
| 4 | `clear.wgsl` | 压力场预衰减（× `pressure`） |
| 5×N | `pressure.wgsl` | Jacobi 迭代求解泊松方程（N = `pressure_iterations`） |
| 6 | `gradient_subtract.wgsl` | v -= ∇p，满足不可压条件 |
| 7 | `advection.wgsl` | 速度自平流（半拉格朗日） |
| 8 | `advection.wgsl` | 染料平流 |
| 9* | `bloom.wgsl` | Bloom：prefilter → downsample → upsample |
| 10* | `sunrays.wgsl` + `blur.wgsl` | Sunrays 体积光 + 分离高斯模糊 |
| 11 | `display.wgsl` | 最终合成 → 屏幕 |
| 12 | 调用方提供 | UI 叠加（egui 等） |

*可选，由 `config.bloom` / `config.sunrays` 控制。

纹理格式：速度/染料/Bloom = `Rgba16Float`；旋度/散度/压力/Sunrays = `R16Float`。

---

## 11. FluidConfig 字段完整参考

```rust
pub struct FluidConfig {
    // ── 分辨率（修改时需 set_config() 重建纹理）─────────────────
    pub sim_resolution:      u32,   // 默认 128  — 速度/压力场短边
    pub dye_resolution:      u32,   // 默认 1024 — 染料颜色场短边
    pub bloom_resolution:    u32,   // 默认 256
    pub bloom_iterations:    u32,   // 默认 8    — 下采样层数
    pub sunrays_resolution:  u32,   // 默认 196

    // ── 物理参数（运行时随时修改）───────────────────────────────
    pub density_dissipation:  f32,  // 默认 1.0  — 染料消散
    pub velocity_dissipation: f32,  // 默认 0.2  — 速度衰减
    pub pressure:             f32,  // 默认 0.8  — 压力场保留比例 [0,1]
    pub pressure_iterations:  u32,  // 默认 20   — Jacobi 次数
    pub curl:                 f32,  // 默认 30.0 — 涡度约束强度

    // ── 交互参数 ─────────────────────────────────────────────────
    pub splat_radius:         f32,  // 默认 0.25  — 归一化注入半径
    pub splat_force:          f32,  // 默认 6000.0— 速度冲量乘数
    pub colorful:             bool, // 默认 true  — 启用颜色轮转
    pub color_update_speed:   f32,  // 默认 10.0  — 颜色轮转速度

    // ── 渲染参数 ─────────────────────────────────────────────────
    pub shading:              bool, // 默认 true  — 法线光照
    pub back_color:           [f32;3],// 默认[0,0,0]— 背景 RGB
    pub back_alpha:           f32,  // 默认 1.0  — 背景 Alpha（0 透明，1 不透明）
    pub paused:               bool, // 默认 false

    // ── Bloom ────────────────────────────────────────────────────
    pub bloom:                bool, // 默认 true
    pub bloom_intensity:      f32,  // 默认 0.8
    pub bloom_threshold:      f32,  // 默认 0.6
    pub bloom_soft_knee:      f32,  // 默认 0.7
    pub bloom_color_tint:     [f32; 3], // 默认 [1,1,1] — 光晕偏色
    pub bloom_hdr_power:      f32,  // 默认 1.0 — HDR 指数

    // ── Sunrays ──────────────────────────────────────────────────
    pub sunrays:              bool, // 默认 true
    pub sunrays_weight:       f32,  // 默认 1.0
    pub sunrays_exposure:     f32,  // 默认 0.7
    pub sunrays_decay:        f32,  // 默认 0.95

    // ── Global Tone ──────────────────────────────────────────────
    pub tone_map_exposure:    f32,  // 默认 1.0
}
```

### 字段用途总览（全量）

| 字段 | 类型 | 主要用途 |
|------|------|----------|
| `sim_resolution` | `u32` | 控制速度场/压力场分辨率，影响流体细节与性能。 |
| `dye_resolution` | `u32` | 控制颜色染料分辨率，影响画面清晰度与显存占用。 |
| `density_dissipation` | `f32` | 控制染料随时间衰减速度，越大消散越快。 |
| `velocity_dissipation` | `f32` | 控制速度场衰减速度，越大流动越快停止。 |
| `pressure` | `f32` | 控制压力场每帧保留比例，影响不可压缩稳定性与“回弹感”。 |
| `pressure_iterations` | `u32` | 控制 Jacobi 求解迭代次数，越高散度抑制越好但更耗时。 |
| `curl` | `f32` | 控制涡度约束强度，决定旋涡感和卷曲力度。 |
| `splat_radius` | `f32` | 控制单次注入影响半径，越大涂抹范围越宽。 |
| `splat_force` | `f32` | 控制注入速度冲量倍数，决定拖拽时“推力”强弱。 |
| `shading` | `bool` | 开关法线光照效果，提升体积感。 |
| `colorful` | `bool` | 是否启用输入点自动换色。 |
| `color_update_speed` | `f32` | 控制自动换色频率。 |
| `paused` | `bool` | 暂停/恢复物理推进。 |
| `back_color` | `[f32; 3]` | 控制最终清屏背景 RGB。 |
| `back_alpha` | `f32` | 控制最终清屏背景 Alpha，用于透明窗口合成。 |
| `bloom` | `bool` | 开关 Bloom 泛光后处理。 |
| `bloom_iterations` | `u32` | 控制 Bloom 金字塔层数（下采样/上采样次数）。 |
| `bloom_resolution` | `u32` | 控制 Bloom 缓冲分辨率。 |
| `bloom_intensity` | `f32` | 控制 Bloom 叠加强度。 |
| `bloom_threshold` | `f32` | 控制参与 Bloom 的亮度阈值。 |
| `bloom_soft_knee` | `f32` | 控制 Bloom 阈值过渡柔和度。 |
| `bloom_color_tint` | `[f32; 3]` | 控制 Bloom 颜色乘数，可让光晕偏色。 |
| `bloom_hdr_power` | `f32` | 控制 Bloom HDR 亮度指数，>1 越亮。 |
| `sunrays` | `bool` | 开关 Sunrays 体积光效果。 |
| `sunrays_resolution` | `u32` | 控制 Sunrays 缓冲分辨率。 |
| `sunrays_weight` | `f32` | 控制 Sunrays 强度权重。 |
| `sunrays_exposure` | `f32` | 控制 Sunrays 散射的曝光系数。 |
| `sunrays_decay` | `f32` | 控制 Sunrays 每步衰减率。 |
| `tone_map_exposure` | `f32` | 全局曝光系数，在 Tone Mapping 前应用。 |

> ⚠️ 分辨率字段（前 5 个）修改后，**必须调用 `engine.set_config(cfg)`**，
> 否则 GPU 纹理不会重建，模拟会以旧尺寸继续运行。

### 参数还原

`engine.reset_to_js_defaults()` 方法可将所有参数一键还原为与原版 WebGL 实现完全一致的值。
这在进行大量参数调整后非常有用。

```rust
// 一键回到初始状态
engine.reset_to_js_defaults();
```

