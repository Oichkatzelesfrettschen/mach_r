# Mach_R - Modern Rust Mach Microkernel

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Status: Active Development](https://img.shields.io/badge/status-active%20development-blue.svg)](https://github.com)

A pure Rust reimplementation of the Mach microkernel, bringing classic microkernel architecture to modern systems with memory safety, type safety, and multi-architecture support.

## Overview

Mach_R modernizes the foundational Mach microkernel design using Rust. By leveraging Rust's safety guarantees and zero-cost abstractions, Mach_R eliminates entire classes of bugs that plagued the original C implementation while preserving the elegant port-based IPC architecture that made Mach revolutionary.

### Key Features

- **Pure Rust Implementation** - Memory-safe microkernel with no unsafe C dependencies
- **Port-Based IPC** - Message-passing communication preserving original Mach semantics
- **Multi-Architecture** - Support for ARM64 (AArch64) and x86_64 architectures
- **Modern Tooling** - Built with Cargo xtask, comprehensive testing, and clean-room design
- **MIG Code Generation** - Pure Rust implementation of Mach Interface Generator

### Project Status

**Current State:** Active development - core IPC and task management implemented

- Port abstraction with capability-based security
- Message passing system with inline/out-of-line data
- Task and thread management (`kern/` subsystem)
- Mach VM subsystem (`mach_vm/`)
- MIG-rust code generator for interface definitions
- Boot infrastructure (Multiboot2, UEFI, Device Tree)

See [TODO.md](TODO.md) for implementation checklist and [docs/book/](docs/book/) for documentation.

## Quick Start

### Prerequisites

- Rust 1.70+ with `cargo`
- Docker (optional, for dev container)
- QEMU (for running the kernel)

### Building

```bash
# Build the kernel (AArch64)
cargo xtask kernel

# Build for x86_64
cargo xtask kernel --target x86_64

# Run in QEMU
cargo xtask qemu

# Run tests
cargo xtask test

# Full check (fmt, clippy, test)
cargo xtask check

# See all commands
cargo xtask help
```

## Project Structure

```
mach_r/
├── src/                    # Kernel source (52K lines)
│   ├── kern/              # Kernel primitives (threads, tasks, scheduling)
│   ├── ipc/               # Mach IPC subsystem
│   ├── mach_vm/           # Mach virtual memory
│   ├── boot/              # Multi-architecture boot
│   ├── arch/              # Architecture support (x86_64, aarch64)
│   ├── drivers/           # Device drivers
│   ├── servers/           # System servers
│   └── mig/               # MIG generated stubs
├── tools/
│   └── mig-rust/          # Mach Interface Generator (Rust)
├── xtask/                  # Build automation
├── docs/
│   ├── book/              # mdBook documentation
│   └── specs/             # TLA+ specifications
├── linkers/               # Linker scripts
├── mig/specs/             # MIG interface definitions
└── archive/               # Historical docs & research
```

## Documentation

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | System architecture |
| [TODO.md](TODO.md) | Implementation checklist |
| [AGENTS.md](AGENTS.md) | Development guidelines |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guide |
| [docs/book/](docs/book/) | Full documentation |

## Architecture

```
+---------------------------------------+
|          User Space                   |
|  +----------+  +----------+           |
|  |   Apps   |  | Servers  |           |
|  +----+-----+  +----+-----+           |
|       +-------------+                 |
|         IPC (Message Passing)         |
+---------------------------------------+
|       Mach_R Microkernel              |
|  +--------+ +--------+ +--------+     |
|  |  Port  | |  Task  | |   VM   |     |
|  |  IPC   | | Thread | | Memory |     |
|  +--------+ +--------+ +--------+     |
+---------------------------------------+
```

### Core Subsystems

- **kern/** - Kernel primitives: threads, tasks, scheduling, timers, locks
- **ipc/** - Port-based IPC: messages, port sets, notifications, rights
- **mach_vm/** - Virtual memory: pages, objects, maps, faults, external pagers
- **boot/** - Multi-arch boot: Multiboot2, UEFI, Device Tree
- **mig/** - Interface generator stubs

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

```bash
# Before submitting
cargo xtask check
```

## Historical Context

Mach_R draws inspiration from multiple historical Mach implementations:

- **CMU Mach MK83** - Core IPC and task/thread abstractions
- **OSF/1 1.0** - Unix compatibility layers
- **Lites 1.1** - BSD personality server design
- **Mach 4 (i386)** - Real-time extensions

This is a **clean-room reimplementation** using published Mach papers and documentation.

## License

- **Mach_R Implementation:** MIT License
- **Historical Archives:** CMU Mach License (reference only)

See [LICENSE](LICENSE) for details.

---

**Status:** Active Development | **Version:** 0.1.0
