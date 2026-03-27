# fluid_core

Real-time GPU fluid simulation in Rust, powered by `wgpu`.

`fluid_core` focuses on simulation and rendering infrastructure, while app/demo code is kept in separate crates.



## 中文简介

`fluid_core` 是一个面向 Rust 生态的实时 GPU 流体仿真项目。

- 核心库不强绑定窗口框架（不依赖 `winit`）
- 统一输入接口支持鼠标、触摸和程序化注入
- 物理与后处理参数支持运行时热修改
- 提供桌面 demo，便于展示与二次开发

## English Summary

`fluid_core` is an open-source Rust project for interactive GPU fluid simulation.

- Core crate is window-system-agnostic
- Unified input API for mouse, touch, and programmatic splats
- Runtime-tunable simulation and post-processing parameters
- Desktop demo for showcase and experimentation

## Upstream Credit and Origin

This project is a Rust/`wgpu` re-implementation and engineering adaptation inspired by:

- PavelDoGreat/WebGL-Fluid-Simulation  
  https://github.com/PavelDoGreat/WebGL-Fluid-Simulation

The original implementation is based on JavaScript + WebGL.  
This repository ports and restructures core ideas into a Rust architecture (library + demo split, typed API surface, and reusable engine integration).

If you reuse or redistribute this project, please keep upstream attribution and preserve all required license notices.

## What Is Different From the Original WebGL Version

- Language/stack: Rust + `wgpu` instead of JavaScript + WebGL
- Architecture: reusable SDK-like core crate + separate desktop demo crate
- Integration model: accepts any window type implementing `raw-window-handle`
- API design: explicit engine/config/input types (`FluidEngine`, `FluidConfig`, `InputManager`)
- Runtime control: parameter updates and input injection via public Rust API

## Repository Layout

- `crates/fluid_core`: simulation core library
- `demos/desktop_demo`: desktop showcase app (`winit` + `egui`)
- [`api.md`](api.md): complete API reference
- `CONTRIBUTING.md`: contribution workflow and coding conventions
- `LICENSE`: project license text

## API Documentation

- Full SDK API reference: [`api.md`](api.md)
- Recommended entry points: `FluidEngine`, `FluidConfig`, `InputManager`, `RenderContext`
- For integration patterns (`egui`, runtime config updates, input injection), see the corresponding sections in [`api.md`](api.md)

## Features

- GPU fluid pipeline based on `wgpu`
- Bloom and sunrays post-processing
- Public engine API (`FluidEngine`, `FluidConfig`, `InputManager`)
- Demo scenes:
  - Main control panel
  - Circle Demo
  - Letter N Demo
  - Spiral Demo

## Quick Start

### 1) Build and run desktop demo

Run in this workspace root (`fluid_core/`):

```bash
cargo run -p fluid_core_desktop_demo
```

### 2) Use as a local dependency

```toml
[dependencies]
fluid_core = { path = "crates/fluid_core" }
```

### 3) Minimal usage example

```rust
use fluid_core::{FluidConfig, FluidEngine};

// `window` can be any type implementing HasWindowHandle + HasDisplayHandle
let mut engine = FluidEngine::new(&window, width, height, &FluidConfig::default()).await;

loop {
    engine.update(dt);
    engine.render()?;
}
```

## Controls (desktop demo)

- `Mouse Drag`: inject fluid force and dye
- `Space`: random burst
- `P`: pause/resume simulation
- `B`: toggle bloom
- `S`: toggle sunrays
- `H`: toggle shading
- `Esc`: quit

## Documentation

- API reference: [`api.md`](api.md)
- Contribution guide: `CONTRIBUTING.md`

## License

This repository is released under the MIT License. See `LICENSE`.

Upstream reference project (`PavelDoGreat/WebGL-Fluid-Simulation`) is also MIT-licensed.  
Please ensure attribution and license notice retention when redistributing derivative works.
