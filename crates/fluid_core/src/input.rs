//! # input — 输入抽象层
//!
//! 本模块将所有外部输入（鼠标、触摸、程序化注入）与流体物理引擎**完全解耦**。
//! 引擎不监听任何系统事件，所有驱动来源均转化为以下两种数据结构后统一投递：
//!
//! - [`InputPoint`]  ：追踪一个持续运动的输入点（鼠标按键 / 触摸手指）
//! - [`SplatEvent`]  ：一次性速度+颜色注入（程序化动画 / 音频 / 网络等）
//!
//! ## 使用方式
//!
//! ```no_run
//! let mut input = InputManager::new();
//!
//! // 鼠标驱动
//! input.pointer_down(0, 0.5, 0.5, None);
//! input.pointer_move(0, 0.52, 0.51);
//! input.pointer_up(0);
//!
//! // 程序化注入（不需要鼠标）
//! input.inject(0.3, 0.7, 200.0, -400.0, [0.8, 0.1, 0.2]);
//!
//! // 每帧由引擎消费
//! let (points, splats, bursts) = input.drain();
//! ```

use crate::fluid::hsv_to_rgb;
use rand::Rng;

// ──────────────────────────────────────────────────────────────────
// InputPoint
// ──────────────────────────────────────────────────────────────────

/// 一个活跃输入点的完整运动状态。
///
/// 每次调用 [`InputManager::pointer_move`] 时，引擎会用位移差
/// 计算速度冲量，驱动流体速度场。
#[derive(Debug, Clone)]
pub struct InputPoint {
    /// 输入点唯一标识。鼠标固定为 `0`，多点触摸按系统手指 ID。
    pub id: u32,
    /// 当前归一化 X 坐标，范围 `[0.0, 1.0]`，左→右
    pub x: f32,
    /// 当前归一化 Y 坐标，范围 `[0.0, 1.0]`，上→下
    pub y: f32,
    /// 上一帧归一化 X 坐标
    pub prev_x: f32,
    /// 上一帧归一化 Y 坐标
    pub prev_y: f32,
    /// 是否处于按下 / 触摸中状态
    pub active: bool,
    /// 本帧是否有位移（`|dx|>0 || |dy|>0`）
    pub moved: bool,
    /// 注入到染料场的 RGB 颜色，分量范围 `[0.0, 1.0]`
    pub color: [f32; 3],
}

impl InputPoint {
    fn new(id: u32, x: f32, y: f32, color: [f32; 3]) -> Self {
        Self {
            id, x, y,
            prev_x: x, prev_y: y,
            active: true, moved: false, color,
        }
    }
}

// ──────────────────────────────────────────────────────────────────
// SplatEvent
// ──────────────────────────────────────────────────────────────────

/// 一次程序化 splat 注入请求。
///
/// 由 [`InputManager::inject`] 排入队列，在引擎下一帧 `update()` 时执行。
/// 无需任何鼠标或触摸事件，适合音频可视化、AI 驱动、网络协同等场景。
#[derive(Debug, Clone)]
pub struct SplatEvent {
    /// 归一化 X 坐标 `[0.0, 1.0]`
    pub x: f32,
    /// 归一化 Y 坐标 `[0.0, 1.0]`
    pub y: f32,
    /// X 方向速度冲量（推荐范围 `-8000.0..8000.0`）
    pub dx: f32,
    /// Y 方向速度冲量（推荐范围 `-8000.0..8000.0`）
    pub dy: f32,
    /// RGB 染料颜色，分量范围 `[0.0, 1.0]`
    pub color: [f32; 3],
}

// ──────────────────────────────────────────────────────────────────
// InputManager
// ──────────────────────────────────────────────────────────────────

/// 输入管理器：收集所有外部输入，在帧末一次性交给流体引擎消费。
///
/// 设计原则：
/// - **无状态依赖**：不持有 wgpu / winit 句柄，可在任意线程构造
/// - **多来源统一**：鼠标、触摸、程序化注入均通过同一接口入队
/// - **帧消费清零**：[`drain`] 调用后队列清空，避免重复处理
pub struct InputManager {
    pub(crate) points: Vec<InputPoint>,
    pub(crate) splat_queue: Vec<SplatEvent>,
    pub(crate) burst_queue: Vec<u32>,
}

impl InputManager {
    /// 创建空输入管理器。
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            splat_queue: Vec::new(),
            burst_queue: Vec::new(),
        }
    }

    // ── 指针事件 ────────────────────────────────────────────────────

    /// 注册输入点**按下**事件（鼠标左键按下 / 触摸开始）。
    ///
    /// 若该 `id` 已存在则更新坐标并重置颜色；否则新建。
    ///
    /// # 参数
    /// | 参数 | 说明 |
    /// |------|------|
    /// | `id` | 输入点唯一 ID，鼠标固定传 `0`，多点触摸传系统手指 ID |
    /// | `x` | 归一化 X 坐标 `[0.0, 1.0]`，`pixel_x / window_width` |
    /// | `y` | 归一化 Y 坐标 `[0.0, 1.0]`，`pixel_y / window_height` |
    /// | `color` | 指定染料颜色；传 `None` 自动生成随机颜色 |
    ///
    /// # 示例
    /// ```no_run
    /// input.pointer_down(0, cursor_x / w, cursor_y / h, None);
    /// input.pointer_down(2, tx / w, ty / h, Some([0.0, 0.5, 1.0]));
    /// ```
    pub fn pointer_down(&mut self, id: u32, x: f32, y: f32, color: Option<[f32; 3]>) {
        let color = color.unwrap_or_else(random_color);
        if let Some(p) = self.points.iter_mut().find(|p| p.id == id) {
            p.active  = true;
            p.x       = x;
            p.y       = y;
            p.prev_x  = x;
            p.prev_y  = y;
            p.moved   = false;
            p.color   = color;
        } else {
            self.points.push(InputPoint::new(id, x, y, color));
        }
    }

    /// 更新输入点**位置**（鼠标移动 / 触摸滑动）。
    ///
    /// 仅当该 `id` 处于 `active` 状态时，位移差才会被引擎转换为速度冲量。
    /// 未激活时调用此方法**不会**产生任何流体效果，因此可以在每帧
    /// 无条件调用以追踪光标位置，而不需要判断按键状态。
    ///
    /// # 参数
    /// | 参数 | 说明 |
    /// |------|------|
    /// | `id` | 输入点 ID |
    /// | `x` | 新的归一化 X `[0.0, 1.0]` |
    /// | `y` | 新的归一化 Y `[0.0, 1.0]` |
    ///
    /// # 示例
    /// ```no_run
    /// // 每帧无条件调用，无需判断左键是否按下
    /// input.pointer_move(0, cursor_x / window_w, cursor_y / window_h);
    /// ```
    pub fn pointer_move(&mut self, id: u32, x: f32, y: f32) {
        if let Some(p) = self.points.iter_mut().find(|p| p.id == id) {
            p.prev_x = p.x;
            p.prev_y = p.y;
            p.x      = x;
            p.y      = y;
            let dx   = x - p.prev_x;
            let dy   = y - p.prev_y;
            p.moved  = dx != 0.0 || dy != 0.0;
        }
    }

    /// 注册输入点**抬起**事件（鼠标释放 / 触摸结束）。
    ///
    /// 调用后对应 `id` 的 `active` 置为 `false`，后续的 `pointer_move`
    /// 不再产生流体效果。
    ///
    /// # 参数
    /// | 参数 | 说明 |
    /// |------|------|
    /// | `id` | 要释放的输入点 ID |
    pub fn pointer_up(&mut self, id: u32) {
        if let Some(p) = self.points.iter_mut().find(|p| p.id == id) {
            p.active = false;
        }
    }

    // ── 程序化注入 ──────────────────────────────────────────────────

    /// 在指定坐标以给定速度和颜色**直接注入**一次流体 splat。
    ///
    /// 请求被缓冲入队列，在引擎下一次 `update()` 时统一执行。
    /// **无需任何鼠标或触摸事件**，适合：
    /// - 音乐 / 音频可视化
    /// - 定时器或物理事件触发的效果
    /// - AI / 机器学习模型输出驱动
    /// - 网络/UDP 远程坐标驱动
    /// - 程序化动画轨迹
    ///
    /// # 参数
    /// | 参数 | 类型 | 说明 |
    /// |------|------|------|
    /// | `x` | `f32` | 归一化 X `[0.0, 1.0]` |
    /// | `y` | `f32` | 归一化 Y `[0.0, 1.0]` |
    /// | `dx` | `f32` | X 方向速度冲量，推荐 `-8000..8000` |
    /// | `dy` | `f32` | Y 方向速度冲量，推荐 `-8000..8000` |
    /// | `color` | `[f32;3]` | RGB 颜色，分量 `[0.0, 1.0]` |
    ///
    /// # 示例
    /// ```no_run
    /// // 屏幕中心向右上方注入橙色流体
    /// input.inject(0.5, 0.5, 1200.0, -900.0, [1.0, 0.5, 0.0]);
    ///
    /// // 沿水平线扫过
    /// for i in 0..20 {
    ///     input.inject(i as f32 / 20.0, 0.5, 300.0, 0.0, [0.2, 0.8, 0.4]);
    /// }
    /// ```
    pub fn inject(&mut self, x: f32, y: f32, dx: f32, dy: f32, color: [f32; 3]) {
        self.splat_queue.push(SplatEvent { x, y, dx, dy, color });
    }

    /// 在**随机位置**发射若干随机颜色 splat（等同于原版按空格键的爆炸效果）。
    ///
    /// 请求同样被缓冲，在下一次 `update()` 时执行。
    ///
    /// # 参数
    /// | 参数 | 说明 |
    /// |------|------|
    /// | `count` | splat 数量，推荐范围 `1..=30` |
    ///
    /// # 示例
    /// ```no_run
    /// input.burst(15);
    /// ```
    pub fn burst(&mut self, count: u32) {
        self.burst_queue.push(count);
    }

    // ── 帧消费 ──────────────────────────────────────────────────────

    /// 取出本帧所有输入数据，调用后队列清空。
    ///
    /// 由 [`FluidEngine::update`](crate::engine::FluidEngine::update) 内部调用，
    /// 外部代码通常不需要直接调用此方法。
    ///
    /// # 返回
    /// `(&[InputPoint], Vec<SplatEvent>, Vec<u32>)`
    /// - `[0]` 当前所有活跃输入点的快照引用
    /// - `[1]` 本帧积累的程序化 splat 事件列表（已清空）
    /// - `[2]` 本帧积累的随机 burst 数量列表（已清空）
    pub fn drain(&mut self) -> (&[InputPoint], Vec<SplatEvent>, Vec<u32>) {
        let splats = std::mem::take(&mut self.splat_queue);
        let bursts = std::mem::take(&mut self.burst_queue);
        (&self.points, splats, bursts)
    }

    /// 返回当前所有输入点的只读切片（不消费队列）。
    pub fn points(&self) -> &[InputPoint] {
        &self.points
    }

    /// 返回当前按下状态的输入点数量。
    pub fn active_count(&self) -> usize {
        self.points.iter().filter(|p| p.active).count()
    }
}

impl Default for InputManager {
    fn default() -> Self { Self::new() }
}

// ──────────────────────────────────────────────────────────────────
// 工具函数
// ──────────────────────────────────────────────────────────────────

/// 生成一个随机低亮度 RGB 颜色，适合注入流体染料。
///
/// 内部以随机色相、满饱和度/明度生成 HSV 色，再乘以 `0.15` 降低亮度，
/// 避免染料颜色过于刺眼。
///
/// # 示例
/// ```no_run
/// let c = random_color(); // e.g. [0.12, 0.03, 0.09]
/// ```
pub fn random_color() -> [f32; 3] {
    let mut rng = rand::thread_rng();
    let [r, g, b] = hsv_to_rgb(rng.gen::<f32>(), 1.0, 1.0);
    [r * 0.15, g * 0.15, b * 0.15]
}
