# Build System Decision (2025)

Summary
- Adopt Rust-native `xtask` for build orchestration in `synthesis/`.
- Keep Makefile as a thin convenience layer; prefer `cargo run -p xtask`.

Why move beyond Makefiles
- Single source of truth in Rust: typed logic, easy refactors, testable.
- Cross‑platform parity without shell quirks; fewer env dependencies.
- Simpler CI/CD: `cargo run -p xtask -- kernel` is explicit and portable.
- Extensible: add new subcommands instead of accumulating shell scripts.

Alternatives considered
- `cargo-make`: feature‑rich, but YAML/TOML scripting and plugins add indirection.
- `just`: great for local tasks; still shell‑first and not type‑checked.
- `bazel`/`ninja`: heavy for this repo; overkill for a two‑crate workspace.

Decision
- Use the well‑established `xtask` pattern (widely used across Rust projects) as the primary build/run/test interface. CI calls `xtask` for kernel builds.

Migration notes
- New commands live in `synthesis/xtask/src/main.rs`.
- README and AGENTS updated; Make targets remain for continuity.
- Future work: fold disk image creation and QEMU variants into `xtask`.
