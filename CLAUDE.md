# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview: Mach_R

**Mach_R** is a planned modernized Rust implementation of the Mach microkernel. 

**CURRENT REALITY**: This project currently contains:
- 872 lines of C code that boots and shows status (a bootloader, not an OS)
- 803,504 lines of unintegrated historical Mach source code
- An empty Rust project (`synthesis/`) with no implementation
- No actual synthesis or integration has occurred (1.4% integration rate)

**GOAL**: Study the historical Mach sources and implement core concepts in modern Rust, creating a memory-safe microkernel for the 21st century.

## Core Architecture

Mach_R modernizes and refactors the classic Mach microkernel architecture, drawing from multiple historical implementations to create a unified, modern system:

### Historical Sources Being Modernized
- **CMU Mach MK83**: Refactoring core IPC, task/thread abstractions, and external pager concepts into Rust
- **OSF/1 1.0**: Modernizing Unix compatibility layers using Rust's type system
- **Lites 1.1**: Reimplementing BSD personality server with async Rust patterns
- **Mach 4 i386**: Adapting real-time extensions for modern multi-core systems
- **GNU OSF/Mach**: Integrating modern toolchain support natively

### Available Source Archives
Located in `~/OSFMK/`:
- `CMU-Mach-MK83.tar.bz2`
- `OSF-Mach-6.1.tar.gz`
- `gnu-osfmach.tar.gz`
- `mach4-i386-UK22.tar.gz`
- `mach_us.tar.gz`
- `CMU-Mach-US53.tar.bz2`
- `CMU-Mach-US52.tar.bz2`
- `bsd_mach_bundle.tar.gz`
- `mach25-i386.tar.gz`

## Development Commands

### Rust Development
```bash
# Source Rust environment (required each session)
source $HOME/.cargo/env

# Build the synthesis library
cd synthesis && cargo build

# Run tests
cargo test

# Build documentation
cargo doc --open

# Check for issues without building
cargo check

# Format code
cargo fmt

# Run clippy lints
cargo clippy
```

### Analysis Tools Installation
Required tools for reverse engineering and analysis:

**Homebrew tools:**
```bash
brew install radare2              # Reverse engineering framework
brew install hopper-disassembler  # Commercial disassembler
brew install binutils             # Binary utilities
brew install x86_64-elf-binutils  # Cross-platform binutils
```

**MacPorts tools:**
```bash
port install ghidra              # NSA's reverse engineering suite
port install rizin               # UNIX-like reverse engineering framework
port install binwalk             # Firmware analysis tool
port install cutter-rizin        # GUI for rizin
```

## Project Structure

```
synthesis/                    # Mach_R core library
├── src/
│   ├── lib.rs               # Mach_R library entry point
│   ├── kernel/              # Modernized microkernel
│   │   ├── ipc/            # Refactored Mach IPC in Rust
│   │   ├── vm/             # Modernized VM subsystem
│   │   ├── task/           # Rust task abstractions
│   │   └── thread/         # Modern thread management
│   ├── servers/            # Refactored personality servers
│   │   ├── bsd/           # Modernized BSD server
│   │   ├── sysv/          # Modernized SysV server
│   │   └── unified/       # New unified server design
│   ├── port/              # Rust port abstractions
│   ├── message/           # Type-safe message passing
│   └── driver/            # Modern driver framework
├── Cargo.toml             # Rust package configuration
└── tests/                 # Integration tests

[Legacy Code Being Refactored:]
- Historical C code → Modern Rust implementations
- Complex macros → Rust generics and traits
- Manual memory management → RAII and ownership
- Undefined behavior → Type-safe abstractions
```

## Development Workflow

### Phase 1: Analysis and Extraction
1. Analyze historical Mach source code from archives
2. Extract core algorithms and architectural patterns
3. Document original behavior for compatibility testing

### Phase 2: Rust Refactoring
1. Refactor Mach IPC into async Rust with zero-copy semantics
2. Modernize VM subsystem with Rust's ownership model
3. Reimplement task/thread management using modern concurrency primitives
4. Transform C macros and preprocessor logic into Rust traits and generics

### Phase 3: Mach_R Enhancements
1. Add modern features while preserving Mach semantics
2. Implement LISP-based scripting layer for runtime extensions
3. Support modern hardware (ARM64, x86_64, RISC-V)
4. Enable kernel-mode switching for specialized workloads

## Key Design Principles

### Mach_R Modernization Goals
- **Preserve Mach semantics** while leveraging Rust's safety guarantees
- **Refactor, don't rewrite**: Maintain architectural compatibility where sensible
- **Zero-cost abstractions**: Modern features without performance penalty
- **Memory safety by default**: Eliminate entire classes of bugs from original C code

### Rust Refactoring Patterns
- Transform Mach ports → Rust channels with capability tokens
- Convert external pagers → Async trait implementations
- Refactor message passing → Type-safe serialization with serde
- Modernize VM operations → RAII and ownership-based management

### Modern Enhancements to Classic Mach
- **Async everywhere**: Non-blocking operations throughout Mach_R
- **Type-safe IPC**: Compile-time message validation
- **Hardware abstraction**: Single codebase for multiple architectures
- **LISP integration**: Runtime extensibility while maintaining Rust core

## Critical Refactoring Points

### IPC Modernization
- Original Mach ports → Rust async channels with backpressure
- C message formats → Type-safe Rust enums with serde
- Manual buffer management → Zero-copy using bytes crate

### VM Subsystem Refactoring
- Original external pagers → Async trait objects
- C memory management → Rust ownership and Arc/Mutex patterns
- Page fault handlers → Async fault resolution

### Task/Thread Modernization
- Mach threads → Rust async tasks with work-stealing scheduler
- C scheduling → Modern multi-core aware algorithms
- Manual synchronization → Rust's sync primitives

## Success Criteria for Mach_R

- [ ] Successfully refactor core Mach IPC into pure Rust
- [ ] Boot Mach_R on modern hardware (ARM64/x86_64)
- [ ] Run legacy Mach binaries through compatibility layer
- [ ] Maintain microkernel architecture while modernizing implementation
- [ ] Achieve better performance than original C implementation
- [ ] Zero memory safety issues (no segfaults, data races, or buffer overflows)
- [ ] Support both BSD and SysV personalities through refactored servers
- [ ] Enable LISP scripting for runtime kernel extensions

## Mach_R Vision

Mach_R represents the modernization of a foundational piece of operating systems history. By refactoring the original Mach microkernel into Rust, we preserve its architectural elegance while eliminating decades of accumulated technical debt. This isn't just a port - it's a thoughtful refactoring that maintains Mach's revolutionary design while leveraging Rust's safety, performance, and modern concurrency features.

The goal is to create a microkernel that the original Mach designers would recognize architecturally, but implemented with 21st-century programming language technology that eliminates entire categories of bugs that plagued the original C implementation.