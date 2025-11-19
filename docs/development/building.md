# Building Mach_R

Complete guide to building, running, and testing Mach_R.

## Prerequisites

### Required Software

- **Rust Toolchain** - Version 1.70 or later
- **Rust Source** - For cross-compilation
- **Cross-compilation Targets** - For target architectures
- **QEMU** - For running and testing
- **Make** - Build automation (optional)

### Installation

#### Install Rust

```bash
# Install rustup (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Update to latest stable
rustup update stable

# Verify installation
rustc --version
cargo --version
```

#### Install Rust Components

```bash
# Add Rust source for cross-compilation
rustup component add rust-src

# Add formatting and linting tools
rustup component add rustfmt clippy

# Add cross-compilation targets
rustup target add aarch64-unknown-none
rustup target add x86_64-unknown-none
```

#### Install QEMU

**macOS:**
```bash
brew install qemu
```

**Ubuntu/Debian:**
```bash
sudo apt-get update
sudo apt-get install qemu-system-aarch64 qemu-system-x86
```

**Fedora:**
```bash
sudo dnf install qemu-system-aarch64 qemu-system-x86
```

#### Verify Installation

```bash
# Check QEMU
qemu-system-aarch64 --version
qemu-system-x86_64 --version

# Check Rust targets
rustup target list --installed
```

## Quick Start

### Clone and Build

```bash
# Clone repository
git clone https://github.com/YOUR_USERNAME/Synthesis.git
cd Synthesis

# Build kernel library
cargo build --lib

# Run tests
cargo test --lib

# Build for release
cargo build --release --lib
```

## Build System Overview

Mach_R uses two build systems:

1. **Cargo** - Rust's package manager (primary)
2. **xtask** - Custom build automation
3. **Make** - Legacy wrapper (convenience)

### Cargo Commands

```bash
# Build library
cargo build --lib

# Build with optimizations
cargo build --release --lib

# Run tests
cargo test --lib

# Check without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

### xtask Commands

xtask provides high-level build automation:

```bash
# Build kernel for AArch64
cargo run -p xtask -- kernel

# Build kernel for x86_64
cargo run -p xtask -- kernel --arch x86_64

# Run in QEMU
cargo run -p xtask -- qemu

# Debug in QEMU
cargo run -p xtask -- qemu-debug

# Generate MIG stubs
cargo run -p xtask -- mig

# Format code
cargo run -p xtask -- fmt

# Run linter
cargo run -p xtask -- clippy

# Run tests
cargo run -p xtask -- test

# Check environment
cargo run -p xtask -- env-check
```

### Makefile Commands

Legacy convenience wrappers:

```bash
# Build kernel
make kernel

# Run in QEMU
make qemu

# Debug in QEMU
make qemu-debug

# Run tests
make test

# Clean build artifacts
make clean
```

## Building the Kernel

### Library Build (Default)

Build the kernel as a library for testing:

```bash
# Debug build
cargo build --lib

# Release build
cargo build --release --lib

# Check build without artifacts
cargo check
```

### Kernel Binary Build

Build a bootable kernel binary:

```bash
# AArch64 kernel
cargo run -p xtask -- kernel

# x86_64 kernel
cargo run -p xtask -- kernel --arch x86_64

# With custom linker script
cargo run -p xtask -- kernel --linker qemu/aarch64-virt.ld
```

### Build Outputs

```
target/
├── aarch64-unknown-none/
│   ├── debug/
│   │   └── libmach_r.a
│   └── release/
│       └── libmach_r.a
├── x86_64-unknown-none/
│   └── ...
└── debug/
    └── mach_r              # Binary (if built)
```

## Running the Kernel

### QEMU Emulation

Run the kernel in QEMU:

```bash
# Run kernel
cargo run -p xtask -- qemu

# Run with debug output
cargo run -p xtask -- qemu --verbose

# Run specific architecture
cargo run -p xtask -- qemu --arch x86_64
```

### QEMU Configuration

QEMU settings are in `qemu/*.json`:

```json
{
  "machine": "virt",
  "cpu": "cortex-a72",
  "memory": "256M",
  "serial": "stdio",
  "display": "none"
}
```

### Boot Testing

Test kernel boot:

```bash
# Quick boot test
./test-boot.sh

# With timeout
timeout 10 make qemu
```

## Debugging

### GDB Debugging

Debug the kernel with GDB:

```bash
# Start QEMU with GDB server
cargo run -p xtask -- qemu-debug

# In another terminal, connect GDB
gdb-multiarch target/aarch64-unknown-none/debug/mach_r
(gdb) target remote :1234
(gdb) continue
```

### QEMU Monitor

Access QEMU monitor for debugging:

```bash
# Run with monitor
qemu-system-aarch64 \
  -machine virt \
  -cpu cortex-a72 \
  -kernel target/aarch64-unknown-none/debug/mach_r \
  -serial stdio \
  -monitor telnet::45454,server,nowait
```

Connect to monitor:
```bash
telnet localhost 45454
```

### Logging

Enable debug logging:

```rust
// In kernel code
#[cfg(debug_assertions)]
debug!("Port created: {:?}", port_id);
```

View logs in QEMU output.

## Testing

### Unit Tests

Run unit tests:

```bash
# All tests
cargo test --lib

# Specific test
cargo test test_port_creation

# With output
cargo test -- --nocapture

# Specific module
cargo test port::tests
```

### Integration Tests

Run integration tests:

```bash
# All integration tests
cargo test --test '*'

# Specific test
cargo test --test test_ipc
```

### Test Coverage

Generate test coverage (requires tarpaulin):

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage
cargo tarpaulin --lib --out Html
```

## Build Configurations

### Debug Build

Default build with debug info:

```bash
cargo build --lib
```

Features:
- Debug symbols included
- Assertions enabled
- No optimizations
- Faster compilation

### Release Build

Optimized build for production:

```bash
cargo build --release --lib
```

Features:
- Optimizations enabled (`-C opt-level=z`)
- Link-time optimization (LTO)
- Debug assertions disabled
- Smaller binary size

### Custom Build

Customize build in `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"         # Optimize for size
lto = true              # Link-time optimization
codegen-units = 1       # Single codegen unit
panic = "abort"         # Abort on panic
strip = true            # Strip symbols
```

## Cross-Compilation

### Target Platforms

Mach_R supports multiple architectures:

- **aarch64-unknown-none** - ARM64 (primary)
- **x86_64-unknown-none** - x86-64 (secondary)
- **riscv64gc-unknown-none** - RISC-V (planned)

### Build for Specific Target

```bash
# AArch64
cargo build --lib --target aarch64-unknown-none

# x86_64
cargo build --lib --target x86_64-unknown-none

# Custom target JSON
cargo build --lib --target path/to/target.json
```

### Custom Target Specification

Create custom target in `targets/custom.json`:

```json
{
  "llvm-target": "aarch64-unknown-none",
  "data-layout": "e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128",
  "arch": "aarch64",
  "target-endian": "little",
  "target-pointer-width": "64",
  "target-c-int-width": "32",
  "os": "none",
  "linker-flavor": "ld.lld",
  "panic-strategy": "abort"
}
```

## Troubleshooting

### Common Issues

#### "Cannot find crate for std"

You're building for a no_std target but using std. Fix:
- Remove std dependencies
- Use `#![no_std]` attribute
- Use `core` instead of `std`

#### "Linker not found"

Install appropriate linker:
```bash
# LLD linker
rustup component add llvm-tools-preview
```

#### "QEMU not found"

Install QEMU:
```bash
brew install qemu          # macOS
apt-get install qemu       # Linux
```

#### Build is slow

Try:
- Use release build: `cargo build --release`
- Increase codegen units: `codegen-units = 4` in `Cargo.toml`
- Use `sccache`: `cargo install sccache`

### Clean Build

Force clean rebuild:

```bash
# Remove build artifacts
cargo clean

# Remove target directory
rm -rf target/

# Rebuild
cargo build --lib
```

## Advanced Topics

### Custom Linker Scripts

Edit linker scripts in `qemu/*.ld`:

```ld
/* aarch64-virt.ld */
ENTRY(_start)

SECTIONS {
    . = 0x40000000;

    .text : {
        *(.text.boot)
        *(.text*)
    }

    .data : {
        *(.data*)
    }

    .bss : {
        *(.bss*)
    }
}
```

### Build Scripts

Add pre-build steps in `build.rs`:

```rust
// build.rs
fn main() {
    println!("cargo:rerun-if-changed=qemu/aarch64-virt.ld");
    println!("cargo:rustc-link-arg=-Tqemu/aarch64-virt.ld");
}
```

### Conditional Compilation

Use features for conditional compilation:

```rust
#[cfg(target_arch = "aarch64")]
fn arch_specific() {
    // AArch64 implementation
}

#[cfg(target_arch = "x86_64")]
fn arch_specific() {
    // x86_64 implementation
}
```

## Build Performance

### Optimization Tips

- **Use release builds** for performance testing
- **Enable LTO** for smaller binaries
- **Use sccache** for incremental builds
- **Parallel compilation** via codegen-units

### Incremental Compilation

Enable incremental builds:

```toml
[profile.dev]
incremental = true
```

### Build Times

Typical build times (M1 Mac):
- Clean debug build: ~30 seconds
- Incremental build: ~5 seconds
- Clean release build: ~60 seconds

## Next Steps

- **[Testing Guide](testing.md)** - Writing and running tests
- **[Debugging Guide](debugging.md)** - Debugging the kernel
- **[Code Style](code-style.md)** - Coding standards
- **[Contributing](../../CONTRIBUTING.md)** - How to contribute

## See Also

- [Rust Cross-Compilation](https://rust-lang.github.io/rustup/cross-compilation.html)
- [Cargo Build Reference](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [QEMU Documentation](https://www.qemu.org/docs/master/)

---

**Last Updated:** 2025-01-19

For build issues, see [troubleshooting](#troubleshooting) or open an issue on GitHub.
