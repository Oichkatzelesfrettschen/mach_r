# Build, Test, and Run

This section provides detailed instructions on how to build, test, and run the Mach_R kernel.

## Building

### Prerequisites

To build Mach_R, you need the Rust toolchain and QEMU for testing. It is recommended to use `rustup` to manage your Rust installations.

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install QEMU for testing (using Homebrew on macOS, adjust for other OS)
brew install qemu

# Install build dependencies (Rust target, QEMU, tools)
make deps
```

### Build & Test Commands

The primary way to interact with the build system is through `xtask` commands, which provide a more structured and Rust-native approach. A `Makefile` is also provided for convenience.

```bash
# Install deps (Rust target, QEMU, tools)
make deps

# Build kernel (AArch64 default)
make kernel

# Run unit tests (hosted)
make test
```

## Running in QEMU

QEMU is used for emulating the target hardware and running the Mach_R kernel.

```bash
# Run with disk image
make qemu

# Direct kernel boot (no disk)
make qemu-kernel

# Start with GDB stub for debugging
make qemu-debug
```

See also: `docs/GDB.md` for GDB quickstart and `docs/IMAGES.md` for artifact details.

## Preferred Build Runner: `xtask`

For more advanced or specific build tasks, use `cargo run -p xtask -- <cmd>`.

-   Examples: `xtask kernel`, `xtask filesystem`, `xtask disk-image`, `xtask iso-image`, `xtask qemu`, `xtask qemu-kernel`, `xtask qemu-debug`, `xtask fmt`, `xtask clippy`, `xtask env-check`.
-   `Makefile` targets remain available for convenience.

## Developer Hygiene

-   Format: `make fmt` (or check with `make fmt-check`)
-   Lint: `make clippy` (deny warnings)
