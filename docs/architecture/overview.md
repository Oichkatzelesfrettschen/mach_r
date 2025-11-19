# Mach_R Architecture Overview

High-level overview of Mach_R's system architecture and design principles.

## Introduction

Mach_R is a modern Rust reimplementation of the Mach microkernel, preserving the elegant design principles of the original while leveraging Rust's safety guarantees. This document provides a high-level overview of the system architecture.

## Core Design Principles

### 1. Microkernel Architecture

Mach_R follows the classic microkernel philosophy:

- **Minimal kernel** - Only essential services run in kernel mode
- **User-mode servers** - File systems, device drivers, and network stacks run as user-space servers
- **Message-passing IPC** - All inter-process communication through ports and messages
- **No kernel bloat** - Keep the kernel small, simple, and verifiable

### 2. Capability-Based Security

All resources are accessed through capabilities:

- **Ports as capabilities** - Unforgeable tokens for accessing resources
- **Rights management** - Send, receive, and send-once rights
- **Secure transfer** - Rights can only be transferred through messages
- **Least privilege** - Tasks only have access to explicitly granted capabilities

### 3. Memory Safety

Rust's ownership and type system provide:

- **No buffer overflows** - Array bounds checking at runtime
- **No use-after-free** - Borrow checker prevents dangling references
- **No data races** - Send/Sync traits enforce thread safety
- **Memory leak prevention** - RAII ensures resource cleanup

## System Components

### Kernel Space

```
┌─────────────────────────────────────────────┐
│           Mach_R Microkernel                │
│                                             │
│  ┌──────────────────────────────────────┐  │
│  │  Core Subsystems                     │  │
│  │  ┌────────┐ ┌────────┐ ┌──────────┐│  │
│  │  │  Port  │ │  Task  │ │  Memory  ││  │
│  │  │  IPC   │ │ Thread │ │    VM    ││  │
│  │  └────────┘ └────────┘ └──────────┘│  │
│  │  ┌────────┐ ┌────────┐ ┌──────────┐│  │
│  │  │Schedule│ │ Console│ │   Boot   ││  │
│  │  └────────┘ └────────┘ └──────────┘│  │
│  └──────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

#### Port System
- Unidirectional communication endpoints
- Message queues for asynchronous communication
- Capability-based access control
- Port rights management

#### Task Management
- Resource allocation units
- Isolated virtual address spaces
- Port namespace ownership
- Security boundaries

#### Thread Management
- Execution contexts within tasks
- Priority-based scheduling
- Multi-core support
- Context switching

#### Virtual Memory
- Page-based memory management
- External pager support
- Copy-on-write optimization
- Memory object abstraction

### User Space

```
┌──────────────────────────────────────────┐
│          System Servers                  │
│  ┌──────────┐ ┌──────────┐ ┌─────────┐ │
│  │   File   │ │  Device  │ │ Network │ │
│  │  Server  │ │  Server  │ │  Stack  │ │
│  └──────────┘ └──────────┘ └─────────┘ │
│                                          │
│  ┌──────────┐ ┌──────────┐ ┌─────────┐ │
│  │   Name   │ │  Default │ │  Init   │ │
│  │  Server  │ │  Pager   │ │ Server  │ │
│  └──────────┘ └──────────┘ └─────────┘ │
└──────────────────────────────────────────┘
         ↕ IPC (Message Passing)
┌──────────────────────────────────────────┐
│          Applications                    │
│  ┌──────────┐ ┌──────────┐ ┌─────────┐ │
│  │  Shell   │ │   Apps   │ │  Tools  │ │
│  └──────────┘ └──────────┘ └─────────┘ │
└──────────────────────────────────────────┘
```

#### System Servers
- **File Server** - Filesystem operations via ports
- **Device Server** - Driver coordination
- **Network Server** - TCP/IP stack
- **Name Server** - Port name resolution
- **Default Pager** - Swap space management
- **Init Server** - Bootstrap and service management

## Communication Model

### Port-Based IPC

All communication happens through ports:

```
┌─────────────┐                ┌─────────────┐
│   Task A    │                │   Task B    │
│             │                │             │
│  ┌───────┐  │                │  ┌───────┐  │
│  │ Send  │──┼────Message────▶│  │Receive│  │
│  │ Right │  │                │  │ Right │  │
│  └───────┘  │                │  └───────┘  │
└─────────────┘                └─────────────┘
```

### Message Passing

Messages carry:
- **Inline data** - Small data (<256 bytes) stored in message
- **Out-of-line data** - Large data referenced by pointer
- **Port rights** - Capability transfer between tasks
- **Type information** - Self-describing message format

## Memory Model

### Virtual Memory

Each task has its own virtual address space:

```
Task A Address Space          Task B Address Space
┌─────────────────┐          ┌─────────────────┐
│    Stack        │          │    Stack        │
├─────────────────┤          ├─────────────────┤
│    Heap         │          │    Heap         │
├─────────────────┤          ├─────────────────┤
│    Data         │          │    Data         │
├─────────────────┤          ├─────────────────┤
│    Code         │          │    Code         │
└─────────────────┘          └─────────────────┘
```

### External Pagers

User-space servers manage memory:

```
┌─────────────┐
│    Task     │
│             │
│  Page Fault │
└──────┬──────┘
       │ IPC
       ▼
┌─────────────┐
│   Pager     │
│   Server    │
│             │
│  Provide    │
│  Page Data  │
└─────────────┘
```

## Boot Sequence

### AArch64 Boot Process

1. **QEMU loads kernel** - ELF loaded at configured address
2. **Early boot** - Set up exception levels, MMU off
3. **UART initialization** - Enable early logging
4. **MMU setup** - Identity mapping, enable paging
5. **Kernel init** - Initialize subsystems
6. **Init task** - Start first user-space task
7. **System servers** - Bootstrap essential servers

### x86_64 Boot Process

1. **Bootloader** - Multiboot or UEFI
2. **Protected mode** - 32-bit to 64-bit transition
3. **Paging setup** - 4-level page tables
4. **Kernel entry** - Jump to Rust code
5. **Subsystem init** - Port, task, VM initialization
6. **User space** - Launch init server

## Multi-Architecture Support

### Supported Architectures

- **AArch64 (ARM64)** - Primary development platform
- **x86_64 (AMD64)** - Secondary platform
- **RISC-V** - Planned future support

### Architecture Abstraction

```rust
// src/arch/mod.rs
pub trait Architecture {
    fn initialize();
    fn setup_mmu();
    fn context_switch(old: &mut Context, new: &Context);
    fn enable_interrupts();
    fn disable_interrupts();
}
```

## Safety Guarantees

### Type Safety

- Strong type system prevents type confusion
- No void* pointers - use generics instead
- Enums for state machines
- Pattern matching for exhaustive handling

### Memory Safety

- Ownership prevents use-after-free
- Borrow checker prevents data races
- No null pointers - use Option<T>
- Automatic resource cleanup with Drop

### Concurrency Safety

- Send/Sync traits for thread safety
- Lock-free data structures where possible
- Atomic operations for counters
- No data races by construction

## Performance Characteristics

### IPC Performance

- **Lock-free message queues** - Minimize contention
- **Zero-copy transfers** - Direct memory mapping for large messages
- **Fast path optimization** - Common case optimized

### Memory Performance

- **External pagers** - Flexible memory management
- **Copy-on-write** - Efficient fork and memory sharing
- **Demand paging** - Load memory only when needed

### Scheduler Performance

- **O(1) scheduling** - Constant time task selection
- **Multi-core aware** - CPU affinity and load balancing
- **Priority inheritance** - Avoid priority inversion

## Comparison with Other Systems

| Feature | Mach_R | seL4 | Redox | Linux |
|---------|--------|------|-------|-------|
| Kernel Type | Microkernel | Microkernel | Microkernel | Monolithic |
| Language | Rust | C | Rust | C |
| Formal Verification | No | Yes | No | No |
| IPC Mechanism | Ports | Endpoints | Schemes | Various |
| Memory Safety | Yes | No | Yes | No |
| User-space Drivers | Yes | Yes | Yes | Hybrid |

## Future Directions

### Planned Features

- **POSIX compatibility** - Syscall emulation layer
- **Personality servers** - BSD and SysV personalities
- **Real-time support** - Deterministic scheduling
- **Formal verification** - Prove safety properties
- **Distributed systems** - Cluster Mach_R support

### Research Opportunities

- **Rust-native IPC** - Leverage Rust type system
- **Async/await integration** - Modern async patterns
- **WASM support** - WebAssembly for safe untrusted code
- **Capability-based security** - Enhanced security models

## References

- [Port System Details](ipc-system.md)
- [Memory Management](memory-management.md)
- [Task and Threading](task-threading.md)
- [Design Decisions](design-decisions.md)
- [ARCHITECTURE.md](../../ARCHITECTURE.md) - Complete architecture document

## See Also

- **[IPC System](ipc-system.md)** - Port semantics and message passing
- **[Memory Management](memory-management.md)** - Virtual memory and external pagers
- **[Task & Threading](task-threading.md)** - Task and thread model
- **[Building](../development/building.md)** - How to build Mach_R
- **[Contributing](../../CONTRIBUTING.md)** - How to contribute

---

**Last Updated:** 2025-01-19

This document provides a high-level overview. For implementation details, see the module-specific documentation and source code.
