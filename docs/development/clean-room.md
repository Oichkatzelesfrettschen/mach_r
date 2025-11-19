# Clean-Room Translation Policy

_Contributing: see [AGENTS.md](../../AGENTS.md) for guidelines._

- Use OSFMK/Mach headers and `.defs` files only as references for data layout,
  constants, and message interface shapes. Do not copy source code into the
  kernel or userland crates.
- Store reference material under `archive/osfmk/reference/` (not compiled).
- Implement Rust equivalents in `synthesis/src/` with tests to validate behavior.
- Document mappings and decisions in `reports/OSFMK_TRANSLATION_PLAN.md`.
- Ensure all new Rust code follows repository style and safety guidelines.

MIG Policy
- MIG is a Rust-native, clean-room reimplementation; no legacy MIG components are used.
- `.defs` files under `archive/osfmk/reference` may guide interface shapes only.
- Any future codegen will be written in Rust and kept within this repository.
