# System Overview

The Mach_R system is divided into two distinct privilege levels: kernel space and user space. The microkernel resides in kernel space, while all applications and system services execute in the less-privileged user space.

```
┌─────────────────────────────────────────────────────────┐
│                    User Space                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐             │
│  │   BSD    │  │  SysV    │  │   User   │             │
│  │  Server  │  │  Server  │  │   Apps   │             │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘             │
│       │             │              │                    │
│  ─────┴─────────────┴──────────────┴─────── IPC        │
│                                                         │
├─────────────────────────────────────────────────────────┤
│                    Kernel Space                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │              Mach_R Microkernel                  │  │
│  │                                                  │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐     │  │
│  │  │   Port   │  │   Task   │  │  Memory  │     │  │
│  │  │   IPC    │  │  Thread  │  │    VM    │     │  │
│  │  └──────────┘  └──────────┘  └──────────┘     │  │
│  │                                                  │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐     │  │
│  │  │Scheduler │  │ Console  │  │ Bootstrap│     │  │
│  │  └──────────┘  └──────────┘  └──────────┘     │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Core Kernel Components

- **IPC System (`port.rs`, `message.rs`)**: The heart of the kernel. Manages the creation, transfer, and destruction of port rights and the queuing/dequeuing of messages.

- **Task & Thread Management (`task.rs`, `thread.rs`)**: Manages the fundamental units of resource ownership (tasks) and execution (threads).

- **Memory Management (`memory.rs`, `vm.rs`)**: Controls virtual address spaces, memory objects, and the physical page frames that back them. It interfaces with user-space pagers to handle page faults.

- **Scheduler (`scheduler.rs`)**: Determines which thread should run on which CPU at any given time. Implements scheduling policies (e.g., fixed-priority, round-robin).

- **Bootstrap & Hardware Abstraction (`bootstrap/`, `arch/`)**: Contains the code necessary to initialize the kernel on a specific hardware architecture (e.g., x86_64, AArch64), including setting up memory management units and interrupt controllers.
