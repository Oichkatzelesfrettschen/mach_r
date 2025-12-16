# Mach_R Complete Technical Synthesis

> Unified reference consolidating 38 archived documents into a comprehensive technical specification.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Historical Foundation](#2-historical-foundation)
3. [Architecture Specification](#3-architecture-specification)
4. [Core Subsystems](#4-core-subsystems)
5. [Implementation Roadmap](#5-implementation-roadmap)
6. [MIG Code Generator](#6-mig-code-generator)
7. [Advanced Features](#7-advanced-features)
8. [Current Reality](#8-current-reality)
9. [API Reference](#9-api-reference)
10. [Source Mappings](#10-source-mappings)

---

## 1. Executive Summary

### Project Scope

**Mach_R** modernizes the Mach microkernel in pure Rust, preserving architectural elegance while eliminating memory safety issues.

| Metric | Original Mach (C) | Mach_R Target (Rust) |
|--------|-------------------|----------------------|
| Core IPC | 244 KB | ~60 KB (4x smaller) |
| Total System | 500K+ LOC | 50-100K LOC |
| Memory Safety | Manual | Automatic via RAII |
| Concurrency | Locks everywhere | Lock-free where possible |

### Current State (December 2024)

- **Implemented**: 52,595 lines of Rust across 92 files
- **Working**: Boot infrastructure, basic IPC, task structures, kern/ subsystem
- **Partial**: Scheduler, VM fault handling, MIG codegen
- **Missing**: ~28,000 LOC for full functionality

### Timeline to Bootable System

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| Core Microkernel | 8 weeks | IPC, scheduler, basic VM |
| MIG Compiler | 8 weeks | Type-safe RPC generation |
| System Integration | 6 weeks | Bootable kernel with shell |
| **Total MVP** | **22 weeks** | **~80,000 LOC** |

---

## 2. Historical Foundation

### 2.1 Source Archives Analyzed

| Archive | Era | Key Contribution |
|---------|-----|------------------|
| CMU-Mach-MK83 | ~1991 | Core IPC, task/thread, VM |
| CMU-Mach-US | ~1994 | Multi-server patterns, clean interfaces |
| OSF/Mach 6.1 | ~1996 | DIPC, NORMA, optimizations |
| Lites 1.1 | ~1995 | BSD personality server |
| GNU OSF/Mach | Modern | Toolchain integration |

### 2.2 Architectural Evolution

**Mach-US (Simple, Elegant)**:
```
- 4-state port machine (Dead, Active, InTransit, Limbo)
- ~8 kernel object types
- Object-oriented interfaces (C++ virtual → Rust traits)
- 90% syscall emulation in user-space library
```

**Modern Mach (Complex, Feature-Rich)**:
```
- 23+ kernel object types
- DIPC/NORMA distribution (~80KB overhead per port)
- Multiple notification types (ns, pd, dn)
- Conditional compilation throughout
```

**Mach_R Strategy**: Start with Mach-US simplicity, add modern features as optional layers.

### 2.3 Key Design Lessons

**What Original Mach Got Right**:
1. Port rights model (send/receive/send-once/dead-name)
2. Message synchrony (send non-blocking, receive blocking)
3. External pagers (VM as pluggable service)
4. Capability-based security (unforgeable ports)

**What to Avoid**:
1. Premature distribution (DIPC in core structures)
2. Notification proliferation (3 separate request fields)
3. Backward compatibility cruft (old code paths)
4. Conditional compilation hell

---

## 3. Architecture Specification

### 3.1 System Layers

```
┌─────────────────────────────────────────────────────────────┐
│                      User Space                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  BSD Server  │  │  SysV Server │  │  User Apps   │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         └──────────────────┴─────────────────┘              │
│                     IPC (Message Passing)                    │
├─────────────────────────────────────────────────────────────┤
│                     Kernel Space                             │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                  Mach_R Microkernel                    │ │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐            │ │
│  │  │   IPC    │  │   Task   │  │    VM    │            │ │
│  │  │  Ports   │  │  Thread  │  │  Memory  │            │ │
│  │  └──────────┘  └──────────┘  └──────────┘            │ │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐            │ │
│  │  │Scheduler │  │Exception │  │  Device  │            │ │
│  │  └──────────┘  └──────────┘  └──────────┘            │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Core Data Structures

#### Port (IPC Endpoint)

```rust
pub struct Port {
    id: PortId,
    state: Mutex<PortState>,
    send_rights: AtomicU32,
    send_once_rights: AtomicU32,
    messages: Arc<MessageQueue>,
    sequence: AtomicUsize,
    message_limit: usize,
    no_senders_notification: Option<Arc<Port>>,
    port_death_notification: Option<Arc<Port>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortState {
    Dead { timestamp: u64 },
    Active { receiver_task: TaskId },
    InTransit { destination: PortId },
    Limbo,
}
```

#### Message (IPC Payload)

```rust
pub struct Message {
    header: MessageHeader,
    body: Vec<u8>,
    ports: Vec<PortRight>,
    out_of_line: Option<MemoryRegion>,
}

pub struct MessageHeader {
    msg_bits: u32,
    msg_size: u32,
    msg_remote_port: PortId,
    msg_local_port: Option<PortId>,
    msg_seqno: u64,
    msg_id: u32,
}
```

#### Task (Resource Container)

```rust
pub struct Task {
    id: TaskId,
    port_space: Arc<PortSpace>,
    address_space: Arc<AddressSpace>,
    threads: Mutex<Vec<Arc<Thread>>>,
    priority: Priority,
    suspend_count: AtomicU32,
    ledgers: Arc<ResourceLedger>,
    pager: Arc<dyn ExternalPager>,
}
```

#### Thread (Execution Context)

```rust
pub struct Thread {
    id: ThreadId,
    task: Arc<Task>,
    state: Mutex<ThreadState>,
    priority: AtomicU32,
    context: UnsafeCell<ThreadContext>,
    continuation: Option<(Continuation, *mut c_void)>,
    activation: Option<Arc<ThreadActivation>>,
}

pub type Continuation = fn(*mut c_void);
```

### 3.3 Memory Layout (x86_64)

```
0x0000_0000_0000_0000 - 0x0000_7FFF_FFFF_FFFF: User space (128 TB)
0xFFFF_8000_0000_0000 - 0xFFFF_8000_3FFF_FFFF: Physical memory mapping (1 GB)
0xFFFF_8000_4000_0000 - 0xFFFF_8000_7FFF_FFFF: Kernel heap (1 GB)
0xFFFF_8000_8000_0000 - 0xFFFF_FFFF_FFFF_FFFF: Kernel code/data
```

---

## 4. Core Subsystems

### 4.1 IPC Subsystem

#### File Structure
```
src/ipc/
├── mod.rs          # Module root, exports
├── port.rs         # Port structure (294 LOC)
├── port_ops.rs     # Port operations (587 LOC)
├── entry.rs        # Port name → capability translation
├── space.rs        # Per-task port namespace
├── right.rs        # Capability transfer operations
├── kmsg.rs         # Kernel message representation
├── mqueue.rs       # Message queue management
├── mach_msg.rs     # Mach-specific message format
├── notify.rs       # Port death notifications
├── pset.rs         # Port sets for multiplexed receive
├── ipc_hash.rs     # Port hashtable
├── ipc_table.rs    # Entry tables
└── ipc_object.rs   # Base object types
```

#### Port Operations API

```rust
// Creation
pub fn port_alloc() -> Result<Arc<Port>, IpcError>;
pub fn port_destroy(port: &Arc<Port>) -> Result<(), IpcError>;

// Rights Management
pub fn port_add_send_right(port: &Arc<Port>) -> Result<(), IpcError>;
pub fn port_remove_send_right(port: &Arc<Port>) -> Result<(), IpcError>;
pub fn port_make_send_once(port: &Arc<Port>) -> Result<PortRight, IpcError>;

// Message Operations
pub fn port_send(port: &Arc<Port>, msg: KernelMessage) -> Result<(), SendError>;
pub fn port_receive(port: &Arc<Port>) -> Result<KernelMessage, ReceiveError>;

// Notifications
pub fn port_request_notification(
    port: &Arc<Port>,
    event: PortEvent,
    notify_port: Arc<Port>,
) -> Result<(), IpcError>;
```

#### Message Send Algorithm

```rust
impl Port {
    pub fn send(&self, msg: KernelMessage) -> Result<(), SendError> {
        let state = self.state.lock().unwrap();

        match *state {
            PortState::Dead { .. } => Err(SendError::InvalidDest),

            PortState::InTransit { destination } => {
                drop(state);
                let dest_port = self.resolve_port(destination)?;
                dest_port.send(msg)
            }

            PortState::Active { .. } => {
                if self.messages.is_full() {
                    return Err(SendError::NoBuffer);
                }
                self.messages.enqueue(msg)?;
                Ok(())
            }

            PortState::Limbo => Err(SendError::NoReceiver),
        }
    }
}
```

### 4.2 Scheduler Subsystem

#### File Structure
```
src/kern/
├── sched_prim.rs   # Scheduling primitives
├── runq.rs         # Run queue management
├── processor.rs    # Processor abstraction
├── priority.rs     # Priority calculations
├── continuation.rs # Lightweight context frames
├── ast.rs          # Asynchronous system traps
└── thread_swap.rs  # Context switching
```

#### Priority System

- 128 priority levels (0 = idle, 127 = real-time)
- Run queue bitmap for O(1) highest-priority lookup
- Priority aging to prevent starvation

```rust
pub struct RunQueue {
    bitmap: u128,                           // Priority bitmap
    queues: [VecDeque<ThreadId>; 128],     // Per-priority queues
    count: usize,                           // Total threads
}

impl RunQueue {
    pub fn enqueue(&mut self, thread: ThreadId, priority: u8) {
        self.queues[priority as usize].push_back(thread);
        self.bitmap |= 1u128 << priority;
        self.count += 1;
    }

    pub fn dequeue_highest(&mut self) -> Option<ThreadId> {
        let priority = 127 - self.bitmap.leading_zeros() as u8;
        let queue = &mut self.queues[priority as usize];
        let thread = queue.pop_front()?;
        if queue.is_empty() {
            self.bitmap &= !(1u128 << priority);
        }
        self.count -= 1;
        Some(thread)
    }
}
```

#### Continuation-Based Blocking

```rust
impl Thread {
    pub fn block_with_continuation(&mut self, cont: Continuation, arg: *mut c_void) {
        self.continuation = Some((cont, arg));
        self.state = ThreadState::Waiting;
        scheduler::yield_thread();
    }

    pub fn resume(&mut self) {
        if let Some((cont, arg)) = self.continuation.take() {
            cont(arg);
        }
    }
}
```

### 4.3 VM Subsystem

#### File Structure
```
src/mach_vm/
├── mod.rs           # Module initialization
├── vm_page.rs       # Physical page management
├── vm_object.rs     # Memory objects (backing store)
├── vm_map.rs        # Address space management
├── vm_fault.rs      # Page fault handling
├── vm_pageout.rs    # Page reclamation daemon
├── vm_external.rs   # External memory interface
├── memory_object.rs # Pager protocol
├── vm_user.rs       # User VM operations
└── vm_kern.rs       # Kernel VM operations
```

#### Page States

```rust
pub enum PageState {
    Free,           // On free list
    Active,         // Recently accessed
    Inactive,       // Candidate for reclaim
    Wired,          // Cannot be paged out
    Busy,           // I/O in progress
    Absent,         // Not yet paged in
    Fictitious,     // No physical backing
    Error,          // I/O error occurred
}
```

#### External Pager Interface

```rust
#[async_trait]
pub trait ExternalPager: Send + Sync {
    /// Called on page fault
    async fn memory_object_data_request(
        &self,
        memory_object: PortId,
        offset: usize,
        length: usize,
        prot: VmProt,
    ) -> Result<Vec<u8>, PagerError>;

    /// Called on pageout
    async fn memory_object_data_return(
        &self,
        memory_object: PortId,
        offset: usize,
        data: &[u8],
        dirty: bool,
    ) -> Result<(), PagerError>;

    /// Initialize memory object
    async fn memory_object_init(
        &self,
        memory_object: PortId,
        control_port: PortId,
        page_size: usize,
    ) -> Result<(), PagerError>;

    /// Terminate memory object
    async fn memory_object_terminate(
        &self,
        memory_object: PortId,
    ) -> Result<(), PagerError>;
}
```

#### VM Fault Handler

```rust
pub fn vm_fault(
    map: &VmMap,
    vaddr: VirtualAddr,
    fault_type: FaultType,
    prot: VmProt,
) -> Result<(), VmFaultResult> {
    // 1. Lookup VM entry
    let entry = map.lookup(vaddr)?;

    // 2. Check protection
    if !entry.protection.contains(prot) {
        return Err(VmFaultResult::ProtectionFailure);
    }

    // 3. Get backing object
    let object = entry.object.clone();
    let offset = vaddr - entry.start + entry.offset;

    // 4. Handle copy-on-write
    if fault_type == FaultType::Write && entry.needs_copy {
        return vm_fault_copy_on_write(map, entry, vaddr);
    }

    // 5. Request page from pager
    let page = object.pager.memory_object_data_request(
        object.pager_port,
        offset,
        PAGE_SIZE,
        prot,
    ).await?;

    // 6. Install mapping
    map.pmap.enter(vaddr, page.phys_addr, prot)?;

    Ok(())
}
```

---

## 5. Implementation Roadmap

### 5.1 Phase 1: Core Microkernel (Weeks 1-8)

#### Week 1-2: IPC Message Passing (3,500 LOC)
```
- mach_msg send/receive/overwrite
- Message marshalling with OOL descriptors
- Message queue operations
- Fast IPC path (fipc)
```

#### Week 3-4: Scheduling System (2,500 LOC)
```
- thread_block, thread_setrun, thread_dispatch
- compute_priority with aging
- Run queue with bitmap optimization
- Context switching (x86_64, aarch64)
```

#### Week 5-6: VM Fault Handling (2,000 LOC)
```
- vm_fault main handler
- Copy-on-write implementation
- vm_pageout_scan (clock algorithm)
- Pager protocol integration
```

#### Week 7-8: Exception & Syscall (1,900 LOC)
```
- Exception port chain
- exc_server protocol
- System call dispatcher
- User/kernel transition
```

### 5.2 Phase 2: MIG Compiler (Weeks 9-16)

See [Section 6: MIG Code Generator](#6-mig-code-generator)

### 5.3 Phase 3: System Integration (Weeks 17-22)

#### Boot Infrastructure
```
- Multiboot2 header and 32→64-bit transition
- GDT/IDT setup
- Memory detection and mapping
- Jump to Rust entry point
```

#### User Mode Support
```
- Ring 3 transition via IRET
- ELF64 loader
- Initial RAM disk
- Shell and utilities
```

### 5.4 Code Estimates by Subsystem

| Subsystem | Complete | Partial | Missing | Est. LOC |
|-----------|----------|---------|---------|----------|
| kern/ | 45 | 30 | 55 | 4,000 |
| ipc/ | 35 | 20 | 40 | 3,500 |
| mach_vm/ | 25 | 15 | 50 | 3,000 |
| device/ | 10 | 5 | 25 | 1,500 |
| MIG compiler | 0 | 0 | 80 | 5,500 |
| MIG defs | 0 | 0 | 60 | 2,000 |
| arch/x86_64 | 5 | 10 | 35 | 2,500 |
| arch/aarch64 | 2 | 5 | 25 | 2,000 |
| DDB debugger | 0 | 0 | 40 | 4,000 |
| **TOTAL** | **122** | **85** | **410** | **28,000** |

---

## 6. MIG Code Generator

### 6.1 Overview

The Mach Interface Generator (MIG) compiles `.defs` interface definitions into type-safe RPC stubs.

**Current State**: 5,504 LOC foundation
**Needed**: 3,100 LOC additional

### 6.2 Pipeline

```
Input .defs → Preprocessor → Lexer → Parser → Semantic → Codegen → Output .rs/.c
```

### 6.3 .defs File Format

```c
subsystem mach 2000;

#include <mach/std_types.defs>

type task_t = mach_port_t
    ctype: mach_port_t
    intran: task_t convert_port_to_task(mach_port_t)
    outtran: mach_port_t convert_task_to_port(task_t)
    destructor: task_deallocate(task_t);

routine task_create(
        target_task    : task_t;
        ledger_ports   : ledger_port_array_t;
        inherit_memory : boolean_t;
    out child_task     : task_t);

simpleroutine memory_object_data_unavailable(
        memory_control : memory_object_control_t;
        offset         : vm_offset_t;
        size           : vm_size_t);
```

### 6.4 Type Mapping

| MIG Type | Rust Type | Size/Align |
|----------|-----------|------------|
| int | i32 | 4/4 |
| long | i64 | 8/8 |
| boolean_t | bool | 4/4 |
| natural_t | usize | 8/8 |
| mach_port_t | PortId | 4/4 |
| array[N] of T | [T; N] | N*sizeof(T) |
| array[*:N] of T | Vec<T> | Variable |
| struct | #[repr(C)] struct | Calculated |
| polymorphic | enum PortRight | Runtime |

### 6.5 Port Disposition Rules

| Direction | Default | Can Override |
|-----------|---------|--------------|
| in | MOVE_SEND | COPY_SEND, MAKE_SEND_ONCE |
| out | MAKE_SEND_ONCE | MOVE_SEND, COPY_SEND |
| inout | (ambiguous) | Must specify |
| polymorphic | Runtime | Runtime determination |

### 6.6 Generated Code Structure

**Client Stub**:
```rust
pub struct VmClient {
    port: PortId,
}

impl VmClient {
    pub fn vm_allocate(
        &self,
        target_task: TaskId,
        address: &mut VmAddress,
        size: VmSize,
        anywhere: bool,
    ) -> Result<(), KernReturn> {
        let request = VmAllocateRequest {
            header: make_header(VM_ALLOCATE_ID, self.port),
            target_task,
            address: *address,
            size,
            anywhere,
        };

        let reply: VmAllocateReply = mach_msg_rpc(&request)?;
        *address = reply.address;
        Ok(())
    }
}
```

**Server Trait**:
```rust
pub trait VmServer {
    fn vm_allocate(
        &mut self,
        target_task: TaskId,
        address: VmAddress,
        size: VmSize,
        anywhere: bool,
    ) -> Result<VmAddress, KernReturn>;
}

pub fn dispatch(
    msg: &MachMsgHeader,
    handler: &mut impl VmServer,
) -> Result<MachMsgHeader, DispatchError> {
    match msg.msgh_id {
        VM_ALLOCATE_ID => {
            let req: VmAllocateRequest = unmarshal(msg)?;
            let result = handler.vm_allocate(
                req.target_task,
                req.address,
                req.size,
                req.anywhere,
            );
            marshal_reply(result)
        }
        _ => Err(DispatchError::UnknownId),
    }
}
```

### 6.7 Interface Definitions by Subsystem

| Subsystem | Base ID | Key Routines |
|-----------|---------|--------------|
| notify | 64 | port_deleted, no_senders, dead_name |
| mach_host | 200 | host_info, processor_info |
| mach | 2000 | task_create, thread_create, vm_allocate |
| memory_object | 2200 | data_request, data_supply, lock_request |
| exc | 2400 | exception_raise, exception_raise_state |
| device | 2800 | device_open, device_read, device_write |
| mach_port | 3000 | allocate, destroy, insert_right |
| task | 3400 | suspend, resume, info, special_port |
| thread | 3600 | suspend, resume, get_state, set_state |
| vm_map | 3800 | allocate, deallocate, protect, read, write |

---

## 7. Advanced Features

### 7.1 DIPC - Distributed IPC (8,000 LOC)

Enables transparent IPC across cluster nodes with port migration.

```rust
pub struct DipcPort {
    local: IpcPort,
    remote_node: Option<NodeId>,
    forward_to: Option<DipcPortId>,
    state: DipcState,
}

pub enum DipcState {
    Local,      // Port managed locally
    Proxy,      // Proxy to remote port
    Migrating,  // Migration in progress
    Remote,     // Fully remote
}

pub trait DistributedIpc {
    fn dipc_port_migrate(&mut self, to_node: NodeId) -> Result<(), DipcError>;
    fn dipc_send_remote(&self, kmsg: &IpcKmsg, node: NodeId) -> Result<(), DipcError>;
    fn dipc_receive_remote(&mut self, timeout: Duration) -> Result<IpcKmsg, DipcError>;
}
```

### 7.2 FLIPC - Fast Lightweight IPC (1,500 LOC)

User-space IPC acceleration via shared ring buffers (50-100x faster than mach_msg).

```rust
pub struct FlipEndpoint {
    id: EndpointId,
    buffer: Arc<FlipBuffer>,
    gate: GatewayLock,
    semaphore_port: Option<PortId>,
}

pub struct FlipBuffer {
    ring: UnsafeCell<[u8; FLIP_BUFFER_SIZE]>,
    read_idx: AtomicUsize,
    write_idx: AtomicUsize,
    state: AtomicU32,
}

// Zero-copy send/receive
pub fn flipc_send(endpoint: &FlipEndpoint, data: &[u8]) -> Result<(), FlipError>;
pub fn flipc_receive(endpoint: &FlipEndpoint, buf: &mut [u8]) -> Result<usize, FlipError>;
```

### 7.3 Thread Activations (3,000 LOC)

Separates thread (execution context) from activation (IPC state) for RPC migration.

```rust
pub struct ThreadActivation {
    id: ActivationId,
    task: TaskId,
    thread: Option<ThreadId>,
    user_stack: Option<StackContext>,
    return_handlers: Vec<Box<dyn ReturnHandler>>,
    suspend_count: u32,
    state: ActivationState,
}

pub enum ActivationState {
    Idle,
    Running,
    Waiting,
    Returning,
    Migrating,
}
```

### 7.4 XMM - External Memory Manager (5,000 LOC)

Hierarchical memory object system with freeze/thaw for checkpointing.

```rust
#[async_trait]
pub trait XmmMethods: Send + Sync {
    fn m_init(&mut self, memory_object: &MemoryObject, control: PortName) -> KernReturn;
    fn m_terminate(&mut self) -> KernReturn;
    fn m_data_request(&mut self, offset: usize, length: usize, prot: VmProt) -> KernReturn;
    fn m_data_return(&mut self, offset: usize, data: &[u8], dirty: bool) -> KernReturn;
    fn m_freeze(&mut self) -> KernReturn;   // Checkpoint support
    fn m_thaw(&mut self) -> KernReturn;     // Resume from checkpoint
    fn m_share(&mut self, other: &dyn XmmMethods) -> KernReturn;
}
```

### 7.5 ETAP - Event Tracing (3,000 LOC)

Kernel-wide tracing with lock profiling.

```rust
pub enum EtapEvent {
    LockAcquire { lock_id: u64, thread: ThreadId, timestamp: u64 },
    LockRelease { lock_id: u64, hold_time: u64 },
    LockContention { lock_id: u64, wait_time: u64 },
    ContextSwitch { from: ThreadId, to: ThreadId },
    PageFault { address: usize, fault_type: FaultType },
    IpcSend { port: PortId, size: usize },
    IpcReceive { port: PortId, size: usize },
}

pub struct LockProfile {
    pub lock_id: u64,
    pub acquisitions: u64,
    pub contentions: u64,
    pub total_hold_time: u64,
    pub max_hold_time: u64,
}
```

---

## 8. Current Reality

### 8.1 What Actually Works

**Verified Working (1,607 lines of C + 52,595 lines of Rust)**:

1. **Boot Process**
   - Multiboot magic validated (0x2BADB002)
   - Stack initialized, BSS cleared
   - Control transferred to kernel_main

2. **VGA Console**
   - Direct memory writes to 0xB8000
   - Text output functional
   - Clear screen operational

3. **Basic IPC Structures**
   - Port allocation
   - Message queue skeleton
   - Rights tracking (atomic counters)

4. **Kern Subsystem**
   - Thread/Task ID generation
   - Priority types
   - Queue implementations

5. **Interrupt System** (C bootloader)
   - IDT with 256 entries
   - PS/2 keyboard driver
   - PIT timer at 100Hz

### 8.2 What's Missing

**Critical Gaps**:
- Actual scheduler integration (stubs only)
- Real IPC message passing (returns success, doesn't pass data)
- VM fault handling (stub only)
- User mode transition
- System calls

**Infrastructure Gaps**:
- Cross-compiler toolchain for bare-metal
- Full MIG compiler
- Disk image creation pipeline

### 8.3 Honest Metrics

| Metric | Value |
|--------|-------|
| Total LOC (Rust) | 52,595 |
| Functional LOC | ~10,000 (19%) |
| Stub/Placeholder | ~25,000 (48%) |
| Infrastructure | ~17,000 (33%) |
| Integration Rate | ~20% |

### 8.4 Build Status

```
✅ cargo build --lib        # Compiles
✅ cargo test --lib         # Tests pass
✅ cargo xtask check        # Lint clean
⚠️  cargo xtask kernel      # Needs cross-compiler
❌ cargo xtask qemu         # Not bootable yet
```

---

## 9. API Reference

### 9.1 Kernel Entry Points

```rust
// Library initialization
pub fn mach_r::init();

// Subsystem initialization (called by init)
pub fn kern::init();
pub fn mach_vm::init();
pub fn net::init() -> Result<(), Error>;
pub fn fs::init() -> Result<(), Error>;
pub fn shell::init() -> Result<(), Error>;
```

### 9.2 IPC API

```rust
// Port operations
pub fn mach_port_allocate(task: TaskId, right: PortRight) -> Result<PortId, KernReturn>;
pub fn mach_port_destroy(task: TaskId, port: PortId) -> Result<(), KernReturn>;
pub fn mach_port_insert_right(task: TaskId, name: PortName, port: PortId, right: PortRight) -> Result<(), KernReturn>;

// Message operations
pub fn mach_msg(
    msg: &mut MachMsgHeader,
    option: MachMsgOption,
    send_size: usize,
    rcv_size: usize,
    rcv_name: PortId,
    timeout: MachMsgTimeout,
) -> MachMsgReturn;
```

### 9.3 Task/Thread API

```rust
// Task operations
pub fn task_create(parent: TaskId, inherit_memory: bool) -> Result<TaskId, KernReturn>;
pub fn task_terminate(task: TaskId) -> Result<(), KernReturn>;
pub fn task_suspend(task: TaskId) -> Result<(), KernReturn>;
pub fn task_resume(task: TaskId) -> Result<(), KernReturn>;

// Thread operations
pub fn thread_create(task: TaskId) -> Result<ThreadId, KernReturn>;
pub fn thread_terminate(thread: ThreadId) -> Result<(), KernReturn>;
pub fn thread_suspend(thread: ThreadId) -> Result<(), KernReturn>;
pub fn thread_resume(thread: ThreadId) -> Result<(), KernReturn>;
```

### 9.4 VM API

```rust
// Memory operations
pub fn vm_allocate(task: TaskId, address: &mut VmAddress, size: VmSize, anywhere: bool) -> Result<(), KernReturn>;
pub fn vm_deallocate(task: TaskId, address: VmAddress, size: VmSize) -> Result<(), KernReturn>;
pub fn vm_protect(task: TaskId, address: VmAddress, size: VmSize, prot: VmProt) -> Result<(), KernReturn>;
pub fn vm_read(task: TaskId, address: VmAddress, size: VmSize) -> Result<Vec<u8>, KernReturn>;
pub fn vm_write(task: TaskId, address: VmAddress, data: &[u8]) -> Result<(), KernReturn>;
```

---

## 10. Source Mappings

### 10.1 Historical → Mach_R File Mapping

| Original (GNU OSFMK) | Mach_R | Status |
|----------------------|--------|--------|
| ipc/ipc_port.c (54 KB) | src/ipc/port.rs, port_ops.rs | Partial |
| ipc/ipc_kmsg.c (98 KB) | src/ipc/kmsg.rs | Stub |
| ipc/ipc_right.c (52 KB) | src/ipc/right.rs | Stub |
| ipc/ipc_space.c (8.6 KB) | src/ipc/space.rs | Partial |
| ipc/ipc_mqueue.c (32 KB) | src/ipc/mqueue.rs | Stub |
| kern/task.c | src/kern/task.rs, src/task.rs | Partial |
| kern/thread.c | src/kern/thread.rs | Partial |
| kern/sched_prim.c | src/kern/sched_prim.rs | Stub |
| vm/vm_map.c | src/mach_vm/vm_map.rs | Stub |
| vm/vm_object.c | src/mach_vm/vm_object.rs | Stub |
| vm/vm_fault.c | src/mach_vm/vm_fault.rs | Stub |
| vm/vm_pageout.c | src/mach_vm/vm_pageout.rs | Stub |

### 10.2 Module Dependencies

```
lib.rs (root)
├── kern/ (init first: processor → timer → zalloc → ...)
├── ipc/ (depends on kern for thread/task types)
├── mach_vm/ (depends on kern memory allocators)
├── boot/ (early initialization, provides BootInfo)
├── arch/ (platform-specific, used by boot)
├── drivers/ (uses interrupt, timer from kern)
├── servers/ (depends on ipc for port/message)
│   ├── name_server (uses mig::generated)
│   ├── vm_server (depends on mach_vm)
│   └── pager_server (uses external_pager)
├── mig/ (depends on message, port, ipc)
├── init/ (depends on servers, task, ipc)
└── userland/ (uses task, ipc, syscall)
```

### 10.3 Key References

**Primary Sources**:
- CMU Mach MK83: Core architecture reference
- GNU OSFMK: Implementation detail reference
- Mach-US: Clean interface design

**Documentation**:
- "The Mach System" - Rashid et al.
- "Mach: A New Kernel Foundation" - Accetta et al.
- CMU Mach MK83 header comments

**Modern References**:
- Redox OS (Rust microkernel patterns)
- seL4 (formal verification techniques)
- Theseus (Rust OS research)

---

## Appendix A: Quick Command Reference

```bash
# Build
cargo xtask kernel              # Build kernel
cargo xtask kernel --target x86_64  # Specific target
cargo xtask mig                 # Generate MIG stubs

# Test
cargo xtask test                # Run all tests
cargo xtask check               # Full CI check

# Run
cargo xtask qemu                # Run in QEMU
cargo xtask qemu-debug          # Debug in QEMU

# Development
cargo xtask fmt                 # Format code
cargo xtask clippy              # Lint code
cargo xtask docs                # Generate documentation
```

---

## Appendix B: Version History

| Version | Date | Milestone |
|---------|------|-----------|
| 0.1.0 | 2024-12 | Initial Rust port, basic structures |
| 0.2.0 | TBD | Working IPC, scheduler integration |
| 0.3.0 | TBD | VM fault handling, MIG compiler |
| 0.4.0 | TBD | Bootable system with shell |
| 1.0.0 | TBD | Full Mach compatibility |

---

*This document synthesizes 38 archived files into a unified technical reference.*
*Last updated: December 2024*
