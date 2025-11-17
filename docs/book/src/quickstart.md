# Quickstart

- Check environment: `cargo run -p xtask -- env-check`
- Build kernel: `cargo run -p xtask -- kernel`
- Create images:
  - `cargo run -p xtask -- filesystem`
  - `cargo run -p xtask -- disk-image`
  - `cargo run -p xtask -- iso-image`
- Run QEMU:
  - Headless: `cargo run -p xtask -- qemu`
  - With GUI: `cargo run -p xtask -- qemu --gui --display default`
- Debug: `make qemu-debug` or attach GDB per `synthesis/docs/GDB.md`.
