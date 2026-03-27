//! # fluid_core — GPU Fluid Simulation SDK
//!
//! Zero winit dependency. Accepts any window via `raw-window-handle`.
//!
//! ## Minimal usage
//! ```no_run
//! use fluid_core::{FluidEngine, FluidConfig};
//!
//! // window = any type implementing HasWindowHandle + HasDisplayHandle
//! let mut engine = FluidEngine::new(&window, width, height, &FluidConfig::default()).await;
//!
//! loop {
//!     engine.input.pointer_move(0, nx, ny);
//!     engine.update(dt);
//!     engine.render()?;
//! }
//! ```

// Internal modules (not part of public API)
mod renderer;
mod fluid;

// Public modules
pub mod config;
pub mod input;
pub mod engine;

// Convenience re-exports
pub use config::FluidConfig;
pub use engine::{FluidEngine, RenderContext};
pub use input::{InputManager, SplatEvent, random_color};
pub use engine::hsv_to_rgb;
