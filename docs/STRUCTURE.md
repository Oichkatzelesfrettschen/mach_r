# Source Structure Overview

_Contributing: see [AGENTS.md](../../AGENTS.md) for guidelines._

Top-level modules in `synthesis/src/`:
- `arch/` — architecture-specific (aarch64, x86_64) code paths.
- `boot/` — boot support and early hardware probing.
- `console.rs`, `drivers/` — basic I/O paths (UART/serial, etc.).
- `external_pager.rs`, `paging.rs`, `memory.rs` — VM and pager interfaces.
- `port.rs`, `message.rs`, `syscall.rs` — Mach IPC and trap surfaces.
- `task.rs`, `scheduler.rs`, `interrupt.rs` — core kernel scheduling and interrupts.
- `fs/`, `net/`, `vm/` — pure-Rust stack layers.
- `servers/`, `init/`, `shell/`, `coreutils/` — system components and user interaction.

Conventions
- Modules use `snake_case` filenames; types use `CamelCase`.
- `no_std` environment; minimize `unsafe`; prefer `Result<T, E>`.
- Architecture abstractions live under `arch/` and are selected by target.

Build & Docs
- Rust-native tasks via `xtask` (see AGENTS.md).
- API docs: `cargo run -p xtask -- docs`.
- Book: `cargo run -p xtask -- book` (mdBook, optional).
- MIG codegen: specs in `mig/specs/*.toml`, generated stubs in `src/mig/generated/` via `cargo run -p xtask -- mig`.
