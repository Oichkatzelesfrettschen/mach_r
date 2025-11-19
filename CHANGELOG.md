# Changelog

All notable changes to Mach_R will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Documentation Reorganization (2025-01-19)

#### Added
- Comprehensive README.md with badges, quick start, and complete project information
- CONTRIBUTING.md with detailed contribution guidelines and coding standards
- LICENSE file with MIT license and CMU Mach acknowledgment
- docs/INDEX.md - Complete documentation index and navigation
- docs/architecture/overview.md - High-level architecture overview
- docs/development/building.md - Comprehensive build guide
- CHANGELOG.md - This file
- DOCUMENTATION_PLAN.md - Documentation reorganization plan

#### Changed
- Reorganized documentation into logical hierarchy (architecture/, development/, tools/, project/)
- Moved MACH_R_ARCHITECTURE.md to top-level ARCHITECTURE.md
- Moved port semantics documentation to docs/architecture/ipc-system.md
- Consolidated development guides under docs/development/
- Organized tool documentation under docs/tools/
- Updated all cross-references and links

#### Repository Structure
- Created proper documentation hierarchy for GitHub publication
- Prepared repository for public release with professional documentation
- Established clear separation between implementation and historical reference code

## [0.1.0] - 2024-2025 (In Development)

### Added
- Core port-based IPC system with capability security
- Message passing infrastructure (inline and out-of-line data)
- Task management with isolated address spaces
- Thread management and basic scheduling
- Memory allocation with bump allocator
- Console output system for debugging
- MIG-rust: Pure Rust implementation of Mach Interface Generator
  - Lexer and parser for .defs files
  - Type system with layout-driven resolution
  - Rust code generation (client and server stubs)
  - Cross-platform support (macOS, Linux, *BSD)
  - Modern error handling and testing
- Architecture support:
  - AArch64 (ARM64) - primary platform
  - x86_64 (AMD64) - secondary platform
- Build system:
  - Cargo-based build system
  - xtask build automation
  - QEMU integration for testing
  - Cross-compilation support
- Testing infrastructure:
  - Unit tests for core components
  - Integration test framework
  - Property-based testing (planned)

### Core Components

#### Port System (src/port.rs)
- Port creation and destruction
- Port rights management (send, receive, send-once)
- Message queue implementation
- Port state tracking
- Capability-based security

#### Message System (src/message.rs)
- Message structure with headers
- Inline data support (<256 bytes)
- Out-of-line data support (planned)
- Port rights transfer
- Message sequencing

#### Task Management (src/task.rs)
- Task creation and termination
- Virtual address space isolation
- Port namespace per task
- Resource tracking

#### Memory Management (src/memory.rs)
- Bump allocator for early boot
- Global allocator trait implementation
- VM map placeholder
- External pager framework (planned)

### MIG Tool (tools/mig-rust)
- Complete .defs file parser
- Type system with built-in and custom types
- Layout-driven typedef resolution
- Rust client stub generation
- Rust server stub generation
- C code compilation validation
- Pure Rust implementation (no legacy MIG dependencies)

### Changed
- Adopted clean-room development methodology
- Focused on pure Rust implementation over C integration
- Established MIG-rust as primary interface definition tool

### Deprecated
- Legacy C code moved to archive/ (reference only)
- Old Makefiles (replaced by xtask)
- Historical status documents (archived)

## Historical Context

### CMU Mach Lineage
Mach_R is inspired by:
- **CMU Mach MK83** (1989-1991) - Original Mach microkernel
- **OSF/1 1.0** (1990) - Commercial Mach-based Unix
- **Lites 1.1** (1994) - BSD personality server
- **Mach 4** (1994) - Enhanced Mach with real-time support
- **GNU Mach** (1997-present) - GNU Hurd's microkernel

### Implementation Approach
Mach_R is a **clean-room reimplementation** in Rust:
- Based on published Mach papers and documentation
- No direct translation or copying of existing C code
- Modern Rust patterns and safety guarantees
- Preserves Mach architectural concepts and semantics

## Version History

### Version 0.1.0 (Current)
Focus: Core microkernel infrastructure
- Port-based IPC
- Task/thread management
- Basic memory management
- MIG tool implementation

### Version 0.2.0 (Planned)
Focus: Enhanced services
- External pager framework
- Device driver abstraction
- Network stack foundation
- Filesystem abstractions

### Version 0.3.0 (Planned)
Focus: POSIX compatibility
- Syscall emulation layer
- Process model on Mach primitives
- Signal handling
- File descriptor mapping

### Version 1.0.0 (Future)
Focus: Production-ready microkernel
- Complete Mach semantics
- POSIX compatibility
- Personality servers (BSD, SysV)
- Multi-architecture support
- Comprehensive documentation
- Full test coverage

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to contribute to Mach_R.

## License

MIT License - See [LICENSE](LICENSE) for details.

Historical CMU Mach code in archive/ remains under its original CMU license.

---

[Unreleased]: https://github.com/YOUR_USERNAME/Synthesis/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/YOUR_USERNAME/Synthesis/releases/tag/v0.1.0
