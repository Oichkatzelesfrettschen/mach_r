# Build Container Guide - ARM Mac → x86_64 Cross-Compilation

*Complete guide to building Mach_R on Apple Silicon Mac for x86_64 targets*

## Overview

Mach_R uses Docker containers to provide a complete x86_64 build environment on ARM Macs. This allows you to develop and build x86_64 kernel code on Apple Silicon without maintaining a complex native toolchain.

### Why a Build Container?

**Problem:** Building x86_64 bare-metal code on ARM Mac requires:
- x86_64 cross-compilation toolchain
- NASM assembler
- GRUB bootloader tools
- QEMU for testing
- Complex linker configuration

**Solution:** Pre-configured Docker container with all tools installed and configured.

## Architecture

```
┌─────────────────────────────────────────┐
│  ARM Mac (Apple M1/M2/M3)               │
│  ┌───────────────────────────────────┐  │
│  │  Docker Desktop (Rosetta 2)       │  │
│  │  ┌─────────────────────────────┐  │  │
│  │  │  Container (linux/amd64)    │  │  │
│  │  │  ┌───────────────────────┐  │  │  │
│  │  │  │  Rust + NASM + GRUB   │  │  │  │
│  │  │  │  GCC x86_64 Toolchain │  │  │  │
│  │  │  │  QEMU x86_64          │  │  │  │
│  │  │  └───────────────────────┘  │  │  │
│  │  │           ↕                 │  │  │
│  │  │   /workspace (mounted)      │  │  │
│  │  └─────────────────────────────┘  │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
         ↓
    mach_r.qcow2 (bootable x86_64 kernel)
```

## Quick Start

### 1. Prerequisites

Install Docker Desktop for Mac:
```bash
# Download from:
https://www.docker.com/products/docker-desktop

# Or via Homebrew:
brew install --cask docker
```

Start Docker Desktop and wait for it to be running.

### 2. Setup Build Environment

Run the setup script (one-time setup):
```bash
./scripts/setup-build-env.sh
```

This will:
- Create directory structure
- Check Docker installation
- Enable Rosetta 2 (for x86_64 emulation)
- Build the Docker image (~5-10 minutes)
- Test the environment

### 3. Build the Kernel

```bash
# Build for x86_64 (debug)
./scripts/build-in-container.sh build

# Build for x86_64 (release)
./scripts/build-in-container.sh build x86_64-unknown-none release

# Build for ARM64
./scripts/build-in-container.sh build aarch64-unknown-none
```

### 4. Run Tests

```bash
./scripts/build-in-container.sh test
```

### 5. Interactive Development

```bash
# Start a shell inside the container
./scripts/build-in-container.sh shell

# Inside container:
cargo build --lib --target x86_64-unknown-none
cargo test
nasm --version
qemu-system-x86_64 --version
```

## Build Container Components

### Dockerfile.build

Multi-stage Dockerfile with layers:

1. **Base Layer** (Ubuntu 22.04)
   - Build essentials
   - Cross-compilation toolchains (GCC for x86_64 and ARM64)

2. **Assemblers Layer**
   - NASM (x86 assembler)
   - YASM (alternative assembler)

3. **Bootloader Layer**
   - GRUB (bootloader)
   - xorriso (ISO creation)
   - mtools (FAT filesystem)

4. **QEMU Layer**
   - qemu-system-x86_64
   - qemu-system-aarch64
   - qemu-utils

5. **Rust Layer**
   - Latest stable Rust
   - Bare-metal targets (x86_64-unknown-none, aarch64-unknown-none)
   - rust-src, rustfmt, clippy
   - cargo-binutils

**Total Size:** ~2-3 GB (with caching)

### docker-compose.build.yml

Services:
- **builder** - Main build service (interactive)
- **quick-build** - Quick non-interactive build
- **test** - Test runner
- **qemu** - QEMU test environment
- **dev** - Development shell with SSH/git

Volumes:
- Source code (live mount)
- Cargo registry (persistent cache)
- Build artifacts (persistent)

## Directory Structure

```
Mach_R/
├── Dockerfile.build                 # Container definition
├── docker-compose.build.yml         # Compose configuration
├── scripts/
│   ├── setup-build-env.sh           # One-time setup
│   └── build-in-container.sh        # Build wrapper
├── build-scripts/
│   ├── build-kernel.sh              # Kernel build (in-container)
│   └── create-bootable.sh           # Disk image creation
├── build/
│   └── dist/                        # Build outputs
└── ~/.docker/mach_r/                # Persistent cache (on host)
    ├── cargo/registry/
    ├── cargo/git/
    └── target/
```

## Common Commands

### Building

```bash
# Debug build (fast, for development)
./scripts/build-in-container.sh build

# Release build (optimized, for testing)
./scripts/build-in-container.sh build x86_64-unknown-none release

# Clean build
./scripts/build-in-container.sh clean
./scripts/build-in-container.sh build
```

### Testing

```bash
# Run unit tests
./scripts/build-in-container.sh test

# Run in QEMU (when kernel binary exists)
./scripts/build-in-container.sh qemu
```

### Development

```bash
# Interactive shell
./scripts/build-in-container.sh shell

# Inside shell, you have full access:
$ cargo build --target x86_64-unknown-none
$ nasm boot.asm -f elf64
$ ld -T linker.ld -o kernel.bin
$ qemu-system-x86_64 -kernel kernel.bin
```

### Container Management

```bash
# Rebuild container (after Dockerfile changes)
./scripts/build-in-container.sh rebuild

# Clean everything and start fresh
docker-compose -f docker-compose.build.yml down -v
rm -rf ~/.docker/mach_r
./scripts/setup-build-env.sh
```

## Performance Optimization

### Rosetta 2 Translation

Docker on ARM Mac uses Rosetta 2 to translate x86_64 instructions to ARM64. This adds ~20-30% overhead but is much faster than full emulation.

### Build Caching

Three levels of caching:

1. **Docker layer caching** - Dockerfile layers cached
2. **Cargo registry** - Dependencies cached in volume
3. **Build artifacts** - Compiled code cached in volume

**First build:** 10-15 minutes
**Incremental build:** 30 seconds - 2 minutes

### Parallel Builds

```bash
# Set build parallelism (in container)
export CARGO_BUILD_JOBS=4

# Or in docker-compose.build.yml:
environment:
  - CARGO_BUILD_JOBS=4
```

## Troubleshooting

### Docker Not Starting

```bash
# Check Docker status
docker info

# If not running:
# 1. Open Docker Desktop
# 2. Wait for "Docker Desktop is running" in menu bar
# 3. Retry

# If still failing:
# Preferences → Reset → Reset to factory defaults
```

### Slow Builds

```bash
# Check Docker resource allocation:
# Docker Desktop → Preferences → Resources
# Recommended:
#   CPUs: 4 (or half of available)
#   Memory: 8 GB
#   Swap: 2 GB
#   Disk: 64 GB
```

### Permission Errors

```bash
# If build artifacts have wrong ownership:
sudo chown -R $(whoami) build/ target/

# If cache volumes have issues:
docker-compose -f docker-compose.build.yml down -v
rm -rf ~/.docker/mach_r
./scripts/setup-build-env.sh
```

### Platform Issues

```bash
# Force x86_64 platform:
docker-compose -f docker-compose.build.yml build --platform linux/amd64

# Check current platform:
docker-compose -f docker-compose.build.yml run --rm builder uname -m
# Should output: x86_64
```

### Out of Disk Space

```bash
# Clean Docker cache
docker system prune -a

# Remove unused volumes
docker volume prune

# Check disk usage
docker system df
```

## Advanced Usage

### Custom Linker Scripts

Place linker scripts in `qemu/`:
```bash
# In container:
ld -T /workspace/qemu/x86_64-pc.ld -o kernel.bin
```

### QEMU with GDB

```bash
# Start QEMU with GDB server (in container)
qemu-system-x86_64 \
  -kernel target/x86_64-unknown-none/debug/mach_r \
  -s \
  -S

# In another terminal, connect GDB:
docker-compose -f docker-compose.build.yml exec builder \
  gdb-multiarch target/x86_64-unknown-none/debug/mach_r

(gdb) target remote :1234
(gdb) continue
```

### Building Without Container (Not Recommended)

If you really want to build natively on ARM Mac:
```bash
# Install toolchains
brew install nasm
brew install qemu
rustup target add x86_64-unknown-none

# Build (may have linker issues)
cargo build --target x86_64-unknown-none
```

**Note:** Container is highly recommended for consistency.

## CI/CD Integration

### GitHub Actions

```yaml
# .github/workflows/build.yml
name: Build
on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build Container
        run: docker-compose -f docker-compose.build.yml build
      - name: Build Kernel
        run: docker-compose -f docker-compose.build.yml run --rm builder \
          cargo build --target x86_64-unknown-none
      - name: Run Tests
        run: docker-compose -f docker-compose.build.yml run --rm test
```

## Best Practices

### Development Workflow

1. **Edit code** on your Mac (use your favorite editor)
2. **Build in container** (`./scripts/build-in-container.sh build`)
3. **Test in container** (QEMU or unit tests)
4. **Iterate** quickly with incremental builds

### Version Control

**Do commit:**
- Source code
- Dockerfile.build
- docker-compose.build.yml
- Build scripts

**Don't commit:**
- build/ directory
- target/ directory
- .docker/ cache
- *.img, *.qcow2 files

### Security

**Container is isolated:**
- Cannot access host filesystem (except mounted /workspace)
- Cannot access network (unless explicitly enabled)
- Runs as root inside container (for GRUB installation)

**Host safety:**
- All build artifacts owned by your user
- No system-level changes
- Can delete container anytime

## Summary

**Build container provides:**
- ✅ Complete x86_64 toolchain on ARM Mac
- ✅ Consistent environment (same on all machines)
- ✅ Fast incremental builds with caching
- ✅ Easy to use (one command to build)
- ✅ Isolated and secure

**Getting started:**
```bash
# One-time setup
./scripts/setup-build-env.sh

# Build kernel
./scripts/build-in-container.sh build

# Start developing!
./scripts/build-in-container.sh shell
```

---

**See Also:**
- [BOOTABLE_ROADMAP.md](../BOOTABLE_ROADMAP.md) - Implementation plan
- [Building Guide](development/building.md) - Build details
- [Contributing](../CONTRIBUTING.md) - Contribution guidelines
