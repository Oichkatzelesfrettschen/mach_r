# MIG (Rust-Native, Clean-Room)

_Contributing: see [AGENTS.md](../../AGENTS.md) for guidelines._

- MIG here refers to a Rust-native, clean-room reimplementation of the legacy
  Mach Interface Generator. No legacy MIG code is used.
- Goals:
  - Express interfaces with modern Rust types and traits (client/server stubs).
  - Encode Mach-style messages and descriptors via clean-room constants in
    `synthesis/src/mach/abi.rs`.
  - Optionally parse reference-only `.defs` files from `archive/osfmk/reference`
    to generate Rust stubs (codegen will be new, in Rust).
- Non-goals:
  - Reusing legacy MIG C code, tools, or runtime.
  - Compiling legacy headers directly into the kernel.

Status
- Core types and marshalling helpers exist in `synthesis/src/mig/`.
- Descriptor mapping for ports is implemented; OOL/OOL-Ports planned next.
- A small Name Server example will exercise request/reply paths.

Codegen
- Generate stubs with `cargo run -p xtask -- mig`.
- Specs live in `synthesis/mig/specs/*.toml`; outputs go to `synthesis/src/mig/generated/`.
- Modules:
  - `name_server` — client helpers (register/lookup/unregister) + server dispatch.
  - `vm` — allocate/deallocate/protect dispatcher stubs; server implements trait.
  - `pager` — page_request dispatcher stub; server implements trait.

Contributing
- Keep it Rust-first and no_std-friendly.
- Prefer zero-copy where possible; validate sizes and ids.
- Add unit tests for every new descriptor or marshalling rule.
