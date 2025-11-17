# REAL MACH_R: Pure Rust Microkernel OS Design

## Project Scope & Commitment

This document outlines the design and implementation of a **REAL, FUNCTIONAL** Mach-derived microkernel OS written in pure Rust. No more stubs, no more fake claims - actual working code.

## Core Architecture

### 1. Microkernel Core (Ring 0)
**Size Target: ~100-200KB**

#### Memory Management
- Physical page allocator (buddy system)
- Virtual memory with 4-level page tables (ARM64/x86_64)
- Kernel heap allocator
- Memory object abstraction (Mach-style)

#### Inter-Process Communication (IPC)
- Port-based message passing (Mach ports)
- Synchronous and asynchronous messaging
- Port rights and capabilities
- Message queues with backpressure

#### Task & Thread Management
- Task = resource container (address space, ports)
- Threads = execution units within tasks
- Preemptive round-robin scheduling
- Priority levels (0-31)

#### Exception & Interrupt Handling
- Hardware interrupt routing
- Exception ports (Mach-style)
- Signal delivery mechanism

### 2. System Servers (User Space)

#### Memory Server
- Paging and swapping
- Memory object management
- Copy-on-write implementation

#### Process Server
- Process creation/destruction
- POSIX process semantics
- Fork/exec emulation

#### File Server
- VFS layer
- Basic ext2-like filesystem
- Device file abstraction

#### Network Server
- TCP/IP stack (later phase)
- Socket abstraction

### 3. POSIX Compatibility Layer

#### System Call Translation
- POSIX calls → IPC messages
- File descriptors → port rights
- Signal handling

#### Standard C Library (mini-libc)
- Basic stdio
- Memory functions
- String operations
- Math functions

### 4. Userland

#### Init System
- Service management
- Dependency resolution
- Process supervision

#### Shell (msh - Mach Shell)
- Command execution
- Pipes and redirection
- Job control

#### Core Utilities
- ls, cat, echo, mkdir, rm
- ps, kill, mount
- Simple text editor

## Implementation Phases

### Phase 1: Boot & Basic Kernel (Week 1)
- [x] Boot sequence for ARM64
- [ ] Physical memory management
- [ ] Virtual memory initialization
- [ ] Basic console output

### Phase 2: Core Microkernel (Week 2)
- [ ] Port/IPC implementation
- [ ] Task/thread creation
- [ ] Basic scheduler
- [ ] Exception handling

### Phase 3: System Servers (Week 3)
- [ ] Memory server
- [ ] Process server
- [ ] Simple filesystem

### Phase 4: POSIX Layer (Week 4)
- [ ] System call interface
- [ ] Basic libc
- [ ] Fork/exec support

### Phase 5: Userland (Week 5)
- [ ] Init system
- [ ] Shell
- [ ] Basic utilities

## Technical Specifications

### Memory Layout (ARM64)

```
0x0000_0000_0000_0000 - 0x0000_0000_3FFF_FFFF : Low memory (1GB)
0x0000_0000_4000_0000 - 0x0000_0000_7FFF_FFFF : Kernel (1GB)
0x0000_0000_8000_0000 - 0x0000_7FFF_FFFF_FFFF : User space
0xFFFF_8000_0000_0000 - 0xFFFF_FFFF_FFFF_FFFF : Kernel space (higher half)
```

### IPC Message Format

```rust
struct Message {
    header: MessageHeader,
    body: [u8; 8192],
    ports: [PortRight; 16],
    out_of_line: Option<MemoryObject>,
}

struct MessageHeader {
    msg_bits: u32,
    msg_size: u32,
    msg_remote_port: PortRight,
    msg_local_port: PortRight,
    msg_id: u32,
}
```

### Task Structure

```rust
struct Task {
    task_id: TaskId,
    address_space: AddressSpace,
    threads: Vec<Thread>,
    ports: PortNameSpace,
    memory_objects: Vec<MemoryObject>,
    stats: TaskStats,
}
```

## Success Criteria

1. **Boots successfully** on QEMU ARM64/x86_64
2. **Creates and schedules** multiple processes
3. **IPC works** between processes
4. **Can run** a shell with basic commands
5. **Memory protection** prevents process interference
6. **POSIX compatibility** for simple C programs

## Development Principles

1. **Incremental functionality** - Each commit adds working features
2. **Test everything** - Automated tests for each component
3. **No stubs** - Only commit working code
4. **Document reality** - Accurate status reporting
5. **Pure Rust** - No C dependencies except where absolutely required

## Current Status: DESIGN PHASE

Next Step: Implement Phase 1 - Boot & Basic Kernel