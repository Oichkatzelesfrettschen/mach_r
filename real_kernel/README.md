# real_kernel (experimental)

This directory contains experimental, pared-down kernel sketches used to
validate linker scripts, target configs, and minimal bring-up flows.

- Primary development happens in `../` (the main `mach_r` kernel).
- Do not depend on this code for features or APIs; it changes frequently.
- If you update build or target settings here, mirror any stable changes
  into the main `synthesis/Makefile` and `Cargo.toml` when appropriate.

Refer to the root `AGENTS.md` for contributor guidelines.

