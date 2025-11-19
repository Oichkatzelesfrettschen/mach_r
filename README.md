# Mach_R - Modern Rust Mach Microkernel

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Status: Active Development](https://img.shields.io/badge/status-active%20development-blue.svg)](https://github.com)

A pure Rust reimplementation of the Mach microkernel, bringing classic microkernel architecture to modern systems with memory safety, type safety, and multi-architecture support.

## Overview

Mach_R is a research and development project that modernizes the foundational Mach microkernel design using Rust. By leveraging Rust's safety guarantees and zero-cost abstractions, Mach_R eliminates entire classes of bugs that plagued the original C implementation while preserving the elegant port-based IPC architecture that made Mach revolutionary.

### Key Features

- **Pure Rust Implementation** - Memory-safe microkernel with no unsafe C dependencies
- **Port-Based IPC** - Message-passing communication preserving original Mach semantics
- **Multi-Architecture** - Support for ARM64 (AArch64) and x86_64 architectures
- **Modern Tooling** - Built with Cargo, comprehensive testing, and clean-room design
- **MIG Code Generation** - Pure Rust implementation of Mach Interface Generator
- **External Pagers** - User-space memory management with async Rust patterns

### Project Status

**Current State:** Active development - core IPC and task management implemented

- ✅ Port abstraction with capability-based security
- ✅ Message passing system with inline/out-of-line data
- ✅ Task and thread management
- ✅ Basic memory allocation and VM foundations
- ✅ MIG-rust code generator for interface definitions
- 🚧 Scheduler implementation (in progress)
- 🚧 External pager framework (in progress)
- 📋 POSIX compatibility layer (planned)
- 📋 Personality servers (BSD, SysV) (planned)

See [ROADMAP.md](ROADMAP.md) for detailed development timeline and [docs/project/status.md](docs/project/status.md) for current implementation status.

## Quick Start

### Prerequisites

- **Rust Toolchain:** 1.70 or later
- **QEMU:** For running the kernel
- **Cross-compilation targets:**
  ```bash
  rustup target add aarch64-unknown-none
  rustup target add x86_64-unknown-none
  ```

### Building

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/Synthesis.git
cd Synthesis

# Build the kernel (AArch64 default)
cargo build --lib

# Run tests
cargo test --lib

# Build for release
cargo build --release --lib
```

### Running in QEMU

```bash
# Build and run kernel with QEMU (requires kernel binary)
make qemu

# Or use the test boot script
./test-boot.sh
```

## Architecture

Mach_R follows the classic microkernel architecture with minimal kernel services and user-space servers:

```
┌─────────────────────────────────────────┐
│          User Space                     │
│  ┌──────────┐  ┌──────────┐            │
│  │   Apps   │  │ Servers  │            │
│  └────┬─────┘  └────┬─────┘            │
│       └─────────────┘                   │
│             IPC (Message Passing)       │
├─────────────────────────────────────────┤
│       Mach_R Microkernel                │
│  ┌──────────┐ ┌──────────┐ ┌────────┐ │
│  │   Port   │ │   Task   │ │   VM   │ │
│  │   IPC    │ │  Thread  │ │ Memory │ │
│  └──────────┘ └──────────┘ └────────┘ │
└─────────────────────────────────────────┘
```

### Core Components

- **Port System:** Unidirectional communication endpoints with capability-based security
- **Message Passing:** Type-safe IPC with inline and out-of-line data transfer
- **Task Management:** Resource allocation units with isolated address spaces
- **Thread Management:** Execution contexts with priority scheduling
- **Virtual Memory:** External pager support for user-space memory managers
- **MIG Interface Generator:** Generate type-safe Rust client/server stubs from .defs files

For detailed architecture documentation, see [ARCHITECTURE.md](ARCHITECTURE.md).

## Project Structure

```
Synthesis/
├── src/                    # Mach_R kernel source
│   ├── lib.rs              # Library entry point
│   ├── port.rs             # Port and IPC implementation
│   ├── message.rs          # Message passing system
│   ├── task.rs             # Task management
│   ├── memory.rs           # Memory allocation and VM
│   ├── scheduler.rs        # Thread scheduler
│   └── ...
├── tools/
│   └── mig-rust/           # Mach Interface Generator (Rust)
├── docs/                   # Documentation
│   ├── INDEX.md            # Documentation index
│   ├── architecture/       # Architectural documentation
│   ├── development/        # Developer guides
│   └── tools/              # Tool documentation
├── real_os/                # Bootable OS implementation
├── examples/               # Example programs
├── tests/                  # Integration tests
└── archive/                # Historical reference code
```

## Documentation

### Essential Reading

- **[ARCHITECTURE.md](ARCHITECTURE.md)** - System architecture and design principles
- **[CONTRIBUTING.md](CONTRIBUTING.md)** - Contribution guidelines and code standards
- **[ROADMAP.md](ROADMAP.md)** - Development roadmap and milestones
- **[docs/INDEX.md](docs/INDEX.md)** - Complete documentation index

### Developer Guides

- [Building and Testing](docs/development/building.md) - Build system and testing
- [Debugging Guide](docs/development/debugging.md) - GDB and kernel debugging
- [Adding Modules](docs/development/adding-modules.md) - Extending the kernel
- [Clean Room Development](docs/development/clean-room.md) - Clean-room methodology

### Architecture Deep Dives

- [IPC System](docs/architecture/ipc-system.md) - Port semantics and message passing
- [Memory Management](docs/architecture/memory-management.md) - VM and external pagers
- [Task & Threading](docs/architecture/task-threading.md) - Task/thread model

### Tools

- [MIG User Guide](docs/tools/mig/usage.md) - Using the Mach Interface Generator
- [MIG Design](docs/tools/mig/design.md) - MIG implementation details
- [Disk Images](docs/tools/disk-images.md) - Creating bootable disk images

## Development

### Building from Source

```bash
# Install dependencies
rustup component add rust-src
rustup target add aarch64-unknown-none x86_64-unknown-none

# Build library
cd Synthesis
cargo build --lib

# Run unit tests
cargo test --lib

# Build kernel binary
make kernel

# Run in QEMU
make qemu
```

### Running Tests

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test '*'

# Specific test
cargo test port_creation
```

### Code Style

Mach_R follows Rust standard style guidelines:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Check without building
cargo check
```

## Contributing

We welcome contributions from the community! Whether you're fixing bugs, adding features, improving documentation, or helping with design discussions, your help is appreciated.

### How to Contribute

1. **Read the guidelines:** See [CONTRIBUTING.md](CONTRIBUTING.md)
2. **Find an issue:** Check open issues or propose new features
3. **Fork and clone:** Fork the repository and create a feature branch
4. **Make changes:** Follow code style and add tests
5. **Submit PR:** Create a pull request with clear description

### Areas We Need Help

- 🐛 **Bug fixes** - Help identify and fix issues
- ✨ **Features** - Implement planned features from the roadmap
- 📚 **Documentation** - Improve docs, add examples, fix typos
- 🧪 **Testing** - Expand test coverage and add integration tests
- 🎨 **Design** - Contribute to architectural discussions

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

## Historical Context

Mach_R draws inspiration from multiple historical Mach implementations:

- **CMU Mach MK83** - Core IPC and task/thread abstractions
- **OSF/1 1.0** - Unix compatibility layers
- **Lites 1.1** - BSD personality server design
- **Mach 4 (i386)** - Real-time extensions
- **GNU Mach** - Modern toolchain integration

This project is a **clean-room reimplementation** using the published Mach papers and documentation as reference, not a direct port of any existing codebase.

## License

**Mach_R Implementation:** MIT License
**Historical CMU Mach Archives:** CMU Mach License

The Mach_R Rust implementation (all code in `src/`, `tools/`, `real_os/`, etc.) is licensed under the MIT License, allowing free use, modification, and distribution.

Historical CMU Mach source code archived in `archive/c-reference/` for research purposes remains under its original CMU Mach license. This archived code is not part of the Mach_R implementation and is included solely for historical reference.

See [LICENSE](LICENSE) for complete licensing details and CMU Mach acknowledgment.

```
MIT License - Mach_R Implementation
Copyright (c) 2024-2025 Mach_R Contributors

CMU Mach License - Historical Archives Only
Copyright (c) 1989-1991 Carnegie Mellon University
```

## Acknowledgments

- **Rick Rashid and the CMU Mach Team** - Original Mach microkernel design
- **The Rust Community** - For creating an excellent systems programming language
- **seL4, Redox OS, and Theseus** - Modern microkernel inspiration
- **GNU Mach and OpenMach** - Reference implementations

## Related Projects

- **[seL4](https://sel4.systems/)** - Formally verified microkernel
- **[Redox OS](https://www.redox-os.org/)** - Unix-like OS written in Rust
- **[Theseus](https://github.com/theseus-os/Theseus)** - Experimental Rust OS
- **[GNU Mach](https://www.gnu.org/software/hurd/gnumach.html)** - GNU Hurd's Mach implementation
- **[XNU](https://github.com/apple/darwin-xnu)** - Apple's hybrid kernel based on Mach

## Getting Help

- **Issues:** [GitHub Issues](https://github.com/YOUR_USERNAME/Synthesis/issues)
- **Discussions:** [GitHub Discussions](https://github.com/YOUR_USERNAME/Synthesis/discussions)
- **Documentation:** [docs/INDEX.md](docs/INDEX.md)

## Roadmap Highlights

### Phase 1: Core Microkernel (Current)
- ✅ Port-based IPC system
- ✅ Task and thread management
- 🚧 Basic scheduler
- 🚧 Memory management

### Phase 2: Enhanced Services (Next)
- 📋 External pager framework
- 📋 Device driver framework
- 📋 Network stack skeleton
- 📋 File system abstractions

### Phase 3: POSIX Compatibility
- 📋 POSIX syscall layer
- 📋 Process model on tasks/threads
- 📋 Signal handling
- 📋 File descriptor abstraction

### Phase 4: Personality Servers
- 📋 BSD personality server
- 📋 System V personality server
- 📋 User-space init system
- 📋 Shell and core utilities

See [ROADMAP.md](ROADMAP.md) for complete timeline.

---

**Status:** Active Development | **Version:** 0.1.0 (Alpha) | **Last Updated:** 2025-01-19

For questions, suggestions, or contributions, please open an issue or discussion on GitHub.
