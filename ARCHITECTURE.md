# Mach_R Architecture Documentation

## Overview

Mach_R is a modern Rust implementation of the Mach microkernel, preserving the architectural elegance of the original while leveraging Rust's memory safety and modern concurrency features.

## Core Design Principles

### 1. Microkernel Architecture
- **Minimal kernel**: Only essential services in kernel space
- **User-mode servers**: File systems, network stacks, device drivers as servers
- **Message-passing IPC**: All communication through ports and messages
- **No shared memory**: Tasks communicate only through IPC

### 2. Rust Safety Guarantees
- **Memory safety**: No buffer overflows, use-after-free, or data races
- **Type safety**: Compile-time verification of operations
- **Zero-cost abstractions**: High-level code with no runtime overhead
- **RAII**: Automatic resource management

## System Architecture

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

## Core Components

### 1. Port System (`src/port.rs`)

The foundation of Mach IPC. Ports are unidirectional communication endpoints.

#### Key Structures
- **Port**: Core IPC primitive with message queue
- **PortId**: Unique identifier for each port
- **PortState**: Dead, Active, InTransit, or Limbo
- **PortRights**: Capability tokens (receive, send, send-once)

#### Implementation Details
```rust
pub struct Port {
    id: PortId,                              // Unique identifier
    state: Mutex<PortState>,                 // Current state
    send_rights: AtomicU32,                  // Reference counted
    send_once_rights: AtomicU32,             // Single-use rights
    messages: MessageQueue,                  // Lock-free queue
    sequence: AtomicU64,                     // Message ordering
}
```

#### Port Operations
- **Creation**: `Port::new(receiver: TaskId) -> Arc<Port>`
- **Send**: Non-blocking message send with rights checking
- **Receive**: Dequeue messages from port's queue
- **Rights Transfer**: Move capabilities between tasks
- **Notifications**: No-senders and port-death events

### 2. Message System (`src/message.rs`)

Messages carry data and capabilities between ports.

#### Message Types
- **Inline**: Small messages (<256 bytes) stored directly
- **Out-of-line**: Large messages with heap allocation
- **Port rights**: Transfer capabilities between tasks

#### Message Structure
```rust
pub struct Message {
    header: MessageHeader {
        size: u32,
        msg_type: MessageType,
        remote_port: Option<PortId>,
        local_port: Option<PortId>,
        sequence: u64,
    },
    body: MessageBody,
}
```

### 3. Task Management (`src/task.rs`)

Tasks are the unit of resource allocation.

#### Task Components
- **TaskId**: Unique identifier
- **VM Map**: Virtual memory address space
- **Threads**: Execution contexts within task
- **Port Namespace**: Ports and rights owned by task

#### Task Operations
- **Creation**: Allocate resources and create task port
- **Thread Management**: Create/destroy threads
- **Port Allocation**: Create ports in task's namespace
- **State Transitions**: Running → Suspended → Terminated

### 4. Memory Management (`src/memory.rs`)

Simplified memory system with plans for full VM.

#### Current Implementation
- **Bump Allocator**: Simple allocation for bootstrap
- **Global Allocator**: Rust allocator trait implementation
- **VM Map**: Placeholder for virtual memory regions

#### Future Enhancements
- External pagers (async trait-based)
- Copy-on-write support
- Memory object abstraction
- Distributed shared memory

### 5. Console System (`src/console.rs`)

Basic I/O for kernel debugging and status.

#### Features
- Platform-agnostic interface
- Formatted output via `println!` macro
- Future: UART, VGA, framebuffer backends

## IPC Flow

### Message Send Operation
1. Task acquires send right to port
2. Task creates message with data/capabilities
3. Message enqueued in port's message queue
4. Sequence number assigned for ordering
5. Receiver task notified (if waiting)

### Message Receive Operation
1. Task checks port's receive right
2. Dequeue message from port's queue
3. Process inline data or out-of-line references
4. Handle any transferred port rights
5. Update task's port namespace

## Safety Guarantees

### Memory Safety
- **No null pointers**: Option<T> for nullable values
- **No dangling references**: Rust's borrow checker
- **No buffer overflows**: Bounds checking
- **No data races**: Send/Sync traits

### Capability Security
- **Unforgeable rights**: Type system prevents capability forging
- **Rights accounting**: Atomic reference counting
- **Controlled transfer**: Rights move through messages only

## Performance Optimizations

### Lock-Free Operations
- Atomic counters for rights management
- Lock-free message queue (planned)
- Wait-free port ID generation

### Zero-Copy Semantics
- Arc<Port> for shared port references
- Message passing without copying (planned)
- Direct memory mapping for large messages

## Testing Strategy

### Unit Tests
- Port creation and destruction
- Message send/receive operations
- Rights management and transfer
- Task state transitions

### Integration Tests (Planned)
- Multi-task message passing
- Port death notifications
- Memory pressure scenarios
- Concurrent operations

## Build and Development

### Building Mach_R
```bash
# Build library
cargo build --lib

# Run tests
cargo test --lib

# Build for release
cargo build --release --lib
```

### Project Structure
```
synthesis/
├── src/
│   ├── lib.rs          # Main library entry
│   ├── types.rs        # Shared type definitions
│   ├── port.rs         # Port implementation
│   ├── message.rs      # Message system
│   ├── task.rs         # Task management
│   ├── memory.rs       # Memory allocation
│   └── console.rs      # Console I/O
├── Cargo.toml          # Project configuration
└── tests/              # Integration tests
```

## Roadmap

### Phase 1: Core IPC ✅
- [x] Port abstraction
- [x] Message structure
- [x] Basic rights management
- [x] Unit tests

### Phase 2: Enhanced IPC (Current)
- [ ] Async message operations
- [ ] Port sets
- [ ] Complex message types
- [ ] Notifications

### Phase 3: Virtual Memory
- [ ] Page management
- [ ] External pagers
- [ ] Copy-on-write
- [ ] Memory objects

### Phase 4: Scheduling
- [ ] Thread scheduler
- [ ] Priority management
- [ ] CPU affinity
- [ ] Real-time support

### Phase 5: Personality Servers
- [ ] BSD server
- [ ] System V server
- [ ] POSIX compliance
- [ ] Linux compatibility

## Comparison with Original Mach

| Feature | Original Mach (C) | Mach_R (Rust) |
|---------|-------------------|---------------|
| Memory Safety | Manual management | Automatic via RAII |
| Type Safety | Weak (void*) | Strong (generics) |
| Concurrency | Locks everywhere | Lock-free where possible |
| Error Handling | Error codes | Result<T, E> |
| Resource Management | Manual refcounting | Arc<T> automatic |
| Build System | Complex Makefiles | Cargo |
| Testing | Limited | Comprehensive |

## Contributing

Mach_R is a research project exploring how classic OS concepts can be modernized with Rust. Contributions focusing on:
- Improving safety guarantees
- Performance optimizations
- Test coverage
- Documentation

## References

- CMU Mach MK83 source code
- "The Mach System" - Rashid et al.
- "Mach: A New Kernel Foundation" - Accetta et al.
- Rust Programming Language Book
- Redox OS for Rust OS patterns