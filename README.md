# Mach_R - Modern Rust Mach Microkernel

A pure Rust reimplementation of the Mach microkernel, bringing classic microkernel concepts to modern systems with memory safety and multi-architecture support.

## Documentation

For comprehensive information on Mach_R's architecture, features, roadmap, build instructions, and development status, see the official mdBook:

[**Mach_R Documentation (mdBook)**](../docs/book/)

## Quick Start (Build & Run)

```bash
# Install dependencies (Rust target, QEMU, tools)
make deps

# Build kernel (AArch64 default)
make kernel

# Run with disk image in QEMU
make qemu
```

## Development

-   **Preferred Build Runner**: Use `cargo run -p xtask -- <cmd>` for builds and dev tasks (e.g., `xtask kernel`, `xtask fmt`, `xtask clippy`).
-   **MIG Codegen**: Generate stubs with `cargo run -p xtask -- mig` (outputs to `src/mig/generated/`).
-   **Contributor Guidelines**: See [AGENTS.md](../AGENTS.md) for structure, style, testing, and PR conventions.

## License

MIT License - See LICENSE file for details

## Acknowledgments

Based on the original CMU Mach microkernel design by Rick Rashid and team.
