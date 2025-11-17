# Repository Guidelines

## Project Structure & Module Organization
- Core code in `synthesis/` (Rust, `no_std`). Key: `synthesis/src/` with modules `arch/`, `boot/`, `fs/`, `shell/`, `build/`.
- Build entrypoints: `synthesis/Makefile`; QEMU specs/linker: `synthesis/qemu/`; internal docs: [`synthesis/docs/ARCHITECTURE.md`](synthesis/docs/ARCHITECTURE.md), [`STRUCTURE.md`](synthesis/docs/STRUCTURE.md).
- Experiments: [`synthesis/real_kernel/`](synthesis/real_kernel/), [`synthesis/real_os/`](synthesis/real_os/).
- Reference-only: [`archive/c-reference/merged/`](archive/c-reference/merged/) and [`archive/osfmk/reference/`](archive/osfmk/reference/) (do not modify in normal development).
- Repo-level plans/tools: root `*.md` and Python utilities.

## Build, Test, and Development Commands
- Prefer `xtask`:
  - `cargo run -p xtask -- kernel` — build AArch64 kernel.
  - `cargo run -p xtask -- filesystem|disk-image` — create sysroot and QCOW2.
  - `cargo run -p xtask -- qemu|qemu-kernel|qemu-debug` — run/debug under QEMU.
  - `cargo run -p xtask -- fmt|clippy|test|env-check` — hygiene tasks.
  - `cargo run -p xtask -- mig` — generate MIG stubs from `synthesis/mig/specs/*.toml` into `synthesis/src/mig/generated/`.
- Legacy wrappers remain: `make kernel`, `make qemu*`, `make test`.

## Coding Style & Naming Conventions
- Rust 2021; 4‑space indentation. Modules/files `snake_case`, types `CamelCase`, consts `SCREAMING_SNAKE_CASE`.
- Prefer `no_std`; minimize `unsafe`; return `Result` over panics.
- Format/lint before pushing: `cargo fmt` and `cargo clippy -- -D warnings`.
- Keep diffs minimal; update docs when changing build/test flows.

## Testing Guidelines
- Place unit tests in‑module using `#[cfg(test)] mod tests { ... }`.
- Tests must be deterministic and non‑timing sensitive.
- Run with `cargo run -p xtask -- test` or `make test`.

## Commit & Pull Request Guidelines
- Use Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `build:`).
- PRs include summary, rationale, linked issues, testing notes, and run steps (e.g., `make kernel && make qemu`); attach logs/screenshots if useful.
- Keep diffs focused; update [`synthesis/README.md`](synthesis/README.md) and/or `synthesis/Makefile` when adding flows.

## Agent‑Specific Instructions
- Default change scope: `synthesis/` and `synthesis/docs/`; avoid `archive/` unless requested.
- Prefer adding/updating `xtask` subcommands over editing shell scripts.
- MIG is Rust‑native and clean‑room; do not add legacy MIG code/tools.
- If referencing `.defs` from OSFMK, keep them under `archive/osfmk/reference` only.
- Observe clean‑room policy: [`synthesis/docs/CLEAN_ROOM.md`](synthesis/docs/CLEAN_ROOM.md), [`synthesis/docs/MIG.md`](synthesis/docs/MIG.md). User docs live in [`docs/book/`](docs/book/).

## Related Docs & Cross‑Links
- Overview and quickstart: [`synthesis/README.md`](synthesis/README.md), [`docs/book/src/intro.md`](docs/book/src/intro.md), [`docs/book/src/quickstart.md`](docs/book/src/quickstart.md).
- Architecture & structure: [`synthesis/docs/ARCHITECTURE.md`](synthesis/docs/ARCHITECTURE.md), [`synthesis/docs/STRUCTURE.md`](synthesis/docs/STRUCTURE.md).
- Clean‑room & MIG: [`synthesis/docs/CLEAN_ROOM.md`](synthesis/docs/CLEAN_ROOM.md), [`synthesis/docs/MIG.md`](synthesis/docs/MIG.md).
- Tooling: [`synthesis/docs/GDB.md`](synthesis/docs/GDB.md), [`synthesis/docs/IMAGES.md`](synthesis/docs/IMAGES.md).
- Roadmap & tasks: [`ROADMAP.md`](ROADMAP.md), [`TODO.md`](TODO.md), [`docs/book/src/roadmap.md`](docs/book/src/roadmap.md).
- OSFMK audit references: [`reports/OSFMK_AUDIT.md`](reports/OSFMK_AUDIT.md), [`reports/OSFMK_REFERENCE_INDEX.md`](reports/OSFMK_REFERENCE_INDEX.md), [`reports/OSFMK_TRANSLATION_PLAN.md`](reports/OSFMK_TRANSLATION_PLAN.md).
- Experimental tracks: [`synthesis/real_kernel/README.md`](synthesis/real_kernel/README.md), [`synthesis/real_os/README.md`](synthesis/real_os/README.md).
- Additional plans/analyses: [`ENGINEERING_PLAN.md`](ENGINEERING_PLAN.md), [`SYNTHESIS_STRATEGY.md`](SYNTHESIS_STRATEGY.md), [`WORKING_STATUS.md`](WORKING_STATUS.md), [`AUDIT_SUMMARY.md`](AUDIT_SUMMARY.md), [`TOOL_ANALYSIS.md`](TOOL_ANALYSIS.md).
- Full book index: [`docs/book/src/SUMMARY.md`](docs/book/src/SUMMARY.md).
