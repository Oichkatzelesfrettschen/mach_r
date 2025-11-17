# Mach_R Architecture - Modern Mach Microkernel

_Contributing: see [AGENTS.md](../../AGENTS.md) for guidelines._

## Overview

Mach_R is a pure Rust reimplementation of the Mach microkernel, maintaining its elegant port-based IPC design while adding POSIX compatibility through modern Rust implementations.

## Core Design Principles (Pure Mach Philosophy)

1. **Everything is a port** - All resources and services accessed via ports
2. **Message-based IPC** - Typed messages with inline and out-of-line data
3. **MIG-style interfaces** - Interface generator for client/server stubs (in Rust)
4. **External pagers** - User-space memory management servers
5. **Task/thread separation** - Tasks own resources, threads execute

## System Architecture

```
┌─────────────────────────────────────────────────┐
│                   Userland                      │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │   Shell  │ │   Init   │ │   Apps   │       │
│  │   (zsh)  │ │  Server  │ │  (cargo) │       │
│  └──────────┘ └──────────┘ └──────────┘       │
│                                                 │
│  ┌───────────────────────────────────┐         │
│  │   POSIX libc (from xv6-rust)      │         │
│  └───────────────────────────────────┘         │
└─────────────────────────────────────────────────┘
                    ↕ Mach Traps
┌─────────────────────────────────────────────────┐
│              System Servers                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │   File   │ │  Network │ │  Device  │       │
│  │  Server  │ │  Server  │ │  Server  │       │
│  └──────────┘ └──────────┘ └──────────┘       │
│         ↕ Port IPC    ↕ Port IPC    ↕          │
└─────────────────────────────────────────────────┘
                    ↕ Mach Messages
┌─────────────────────────────────────────────────┐
│                 Mach_R Kernel                   │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │   Ports  │ │  Virtual │ │   Tasks  │       │
│  │    IPC   │ │  Memory  │ │  Threads │       │
│  └──────────┘ └──────────┘ └──────────┘       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │Scheduler │ │ External │ │   Mach   │       │
│  │          │ │  Pagers  │ │   Traps  │       │
│  └──────────┘ └──────────┘ └──────────┘       │
└─────────────────────────────────────────────────┘
```

## Implementation Plan

### Phase 1: Core Kernel Services
- [x] Mach ports and IPC
- [x] Task/thread management  
- [x] Memory management with external pager
- [ ] Serial console driver
- [ ] Bootstrap and early init
- [ ] MIG-style interface generator (Rust version)

### Phase 2: Mach System Servers
- [ ] Name Server (port registry)
- [ ] File Server (filesystem via ports)
- [ ] Network Server (TCP/IP stack)
- [ ] Device Server (driver coordinator)
- [ ] Default Pager (swap management)

### Phase 3: POSIX Compatibility Layer
- [ ] Mach trap interface (syscall emulation)
- [ ] Port xv6-rust libc or adapt relibc
- [ ] POSIX process model on top of tasks/threads
- [ ] Signal handling via exception ports
- [ ] File descriptors mapped to ports

### Phase 4: Userland
- [ ] Init server (bootstrap tasks)
- [ ] Shell with port-aware job control
- [ ] Package management via cargo
- [ ] Core utilities (ls, cat, echo, etc.)
- [ ] Port zsh or implement Rust shell

### Phase 5: Device Drivers as User Tasks
- [ ] Serial driver task
- [ ] Disk driver task (AHCI/VirtIO)
- [ ] Network driver task
- [ ] Framebuffer driver task

## POSIX Implementation Strategy

Based on research of xv6-rust, relibc, and other Rust UNIX implementations:

1. **xv6-rust approach**: Simple, educational POSIX subset
   - Minimal syscall set (~40 calls)
   - Basic file operations
   - Simple process model
   - Good starting point for Mach_R

2. **relibc approach**: Full POSIX compliance
   - Complete C standard library in Rust
   - Can run on top of any syscall interface
   - We'll adapt it to use Mach traps

3. **Our synthesis**: 
   - Start with xv6-rust's simple libc
   - Map POSIX calls to Mach messages
   - File operations → File Server port
   - Process operations → Task/Thread ports
   - Gradually expand to relibc compatibility

## Mach Trap Interface

Traditional Mach traps we'll implement:
- `mach_msg_trap` - Send/receive messages
- `mach_reply_port` - Create reply port
- `thread_switch` - Yield to another thread
- `task_self_trap` - Get task port
- `host_self_trap` - Get host port
- `mach_thread_self` - Get thread port
- `semaphore_signal_trap` - Semaphore operations
- `semaphore_wait_trap` - Semaphore wait

POSIX emulation traps:
- `unix_syscall` - POSIX syscall dispatcher
- Maps Linux/BSD syscall numbers to Mach operations

## Build System

- Kernel: no_std Rust with custom targets
- Drivers: std Rust running in userspace
- Userland: Full std Rust with cargo
- Cross-compilation for ARM64 and x86_64

## Boot Sequence (AArch64)
1. QEMU loads kernel ELF at configured entry point.
2. Early boot sets up exception levels and MMU off.
3. Initialize UART for early logging.
4. Establish identity mappings and enable MMU.
5. Initialize scheduler, IPC ports, and memory subsystems.
6. Transition to init task and start system servers.

## Memory Map (early)
- 0x0000_0000..: Device memory (QEMU virt platform)
- Higher half: kernel text/data/bss
- Linear map for physical memory (temporary identity maps during bring‑up)
- Per-CPU stacks in a guarded region

## AArch64 Exception Levels
- EL3: Not used (QEMU virt boots at EL1 by default).
- EL2: Optional (hypervisor); currently bypassed.
- EL1: Kernel executes here.
- EL0: User tasks/threads.

## Page Tables (4-level)
- 4 KB pages; 4 levels: L0..L3.
- Typical mappings:
  - Kernel text: RX, global, higher-half virtual addresses.
  - Kernel data/BSS: RW, no-exec.
  - Device MMIO: RW, device memory attributes.
  - Temporary identity map during early boot for MMU enable.
