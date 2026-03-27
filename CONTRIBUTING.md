# Contributing to fluid_core

Thanks for contributing.

## Development Environment

- Rust stable (recommended latest)
- A GPU backend supported by `wgpu` (Vulkan/Metal/DX12)

## Local Setup

```bash
cargo check
cargo run -p fluid_core_desktop_demo
```

## Code Organization

- `crates/fluid_core`: core library only
- `demos/desktop_demo`: demo and UI integration

Please keep window/event-loop logic in demo crates instead of moving it into the core crate.

## Pull Request Guidelines

- Keep PRs focused and small when possible
- Include a clear problem statement and expected behavior
- Mention performance implications for simulation/rendering changes
- Update `api.md` and `README.md` when API behavior changes

## Recommended Validation

```bash
cargo check
cargo test
```

If tests are not available for your change yet, include manual verification steps in your PR description.

## Style Notes

- Preserve public API stability when possible
- Avoid introducing unnecessary dependencies in `crates/fluid_core`
- Prefer explicit, readable code over clever compact patterns

## Issues

For bug reports, include:

- OS and GPU information
- Reproduction steps
- Expected result vs actual result
- Logs or screenshots when relevant
