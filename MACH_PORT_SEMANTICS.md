# Mach Port Semantics - Analysis for Mach_R

## Core Concepts from CMU Mach MK83

### Port States
A Mach port can exist in four distinct states:
1. **Dead** - Not active, timestamp contains death time
2. **In a space** - Has receiver (ip_receiver_name != 0)
3. **In transit** - Being transferred (ip_receiver_name == 0, has destination)
4. **In limbo** - No receiver or destination

### Port Structure Components
```c
struct ipc_port {
    // Object header (references, lock bits)
    struct ipc_object ip_object;
    
    // State union - receiver/destination/timestamp
    union {
        struct ipc_space *receiver;
        struct ipc_port *destination;
        ipc_port_timestamp_t timestamp;
    } data;
    
    // Port naming
    mach_port_t ip_receiver_name;
    
    // Kernel object binding
    ipc_kobject_t ip_kobject;
    
    // Rights management
    mach_port_mscount_t ip_mscount;     // make-send count
    mach_port_rights_t ip_srights;      // send rights
    mach_port_rights_t ip_sorights;     // send-once rights
    
    // Notifications
    struct ipc_port *ip_nsrequest;      // no-senders notification
    struct ipc_port *ip_pdrequest;      // port-death notification
    
    // Message queue
    mach_port_seqno_t ip_seqno;
    mach_port_msgcount_t ip_msgcount;
    mach_port_msgcount_t ip_qlimit;
    struct ipc_mqueue ip_messages;
}
```

### Port Rights Model
Mach uses a capability-based security model with four types of rights:
1. **Receive right** - Exactly one exists, allows receiving messages
2. **Send right** - Multiple can exist, allows sending messages
3. **Send-once right** - Single-use send right, self-destructs after use
4. **Port set right** - Groups multiple ports for receive operations

### Key Invariants
- Only one receive right can exist for a port
- Receive right holder controls the port's lifetime
- Send rights are reference-counted
- Send-once rights provide exactly-once delivery semantics
- Ports are the only IPC mechanism (unifies everything)

## Rust Design for Mach_R

### Core Port Structure
```rust
// In synthesis/src/port.rs

use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use alloc::sync::Arc;

/// Port state in Mach_R
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortState {
    Dead { timestamp: u64 },
    Active { receiver_task: TaskId },
    InTransit { destination: PortId },
    Limbo,
}

/// Port rights capabilities
#[derive(Debug, Clone, Copy)]
pub struct PortRights {
    pub receive: bool,
    pub send_count: u32,
    pub send_once_count: u32,
}

/// A Mach port - fundamental IPC primitive
pub struct Port {
    /// Unique port identifier
    id: PortId,
    
    /// Current port state
    state: Mutex<PortState>,
    
    /// Reference count (Arc handles this in Rust)
    // Implicit via Arc<Port>
    
    /// Rights tracking
    send_rights: AtomicU32,
    send_once_rights: AtomicU32,
    
    /// Message queue
    messages: MessageQueue,
    
    /// Sequence number for ordered delivery
    sequence: AtomicUsize,
    
    /// Queue limits
    message_limit: usize,
    
    /// Notification ports
    no_senders_notification: Option<Arc<Port>>,
    port_death_notification: Option<Arc<Port>>,
}
```

### Rust Advantages Over C Implementation

1. **Memory Safety**
   - No manual reference counting bugs
   - Arc<Port> handles lifecycle automatically
   - No use-after-free vulnerabilities

2. **Type Safety**
   - Port states as enum, not union hacks
   - Rights as structured data, not bit fields
   - Compile-time verification of state transitions

3. **Concurrency**
   - Rust's Send/Sync traits ensure thread safety
   - Atomic operations for lock-free counters
   - Async/await for non-blocking message operations

4. **Zero-Cost Abstractions**
   - Enum variants compile to same size as C union
   - Inline functions with no overhead
   - Smart pointers optimize to raw pointers

### Implementation Plan

#### Phase 1: Basic Port (Week 1)
- [ ] Define PortId type (unique identifier)
- [ ] Implement PortState enum
- [ ] Create Port struct with basic fields
- [ ] Add creation/destruction methods

#### Phase 2: Rights Management (Week 2)
- [ ] Implement send right counting
- [ ] Add send-once right support
- [ ] Create rights transfer mechanism
- [ ] Add rights validation

#### Phase 3: Message Queue (Week 3)
- [ ] Design Message type
- [ ] Implement lock-free message queue
- [ ] Add send/receive operations
- [ ] Support message ordering

#### Phase 4: Notifications (Week 4)
- [ ] No-senders notification
- [ ] Port death notification
- [ ] Dead name notification
- [ ] Integration with task management

## Key Design Decisions for Mach_R

1. **Use Arc<Port> instead of raw pointers**
   - Automatic reference counting
   - Safe sharing between tasks
   - No manual memory management

2. **Async message operations**
   - Non-blocking by default
   - Use Rust futures for waiting
   - Better CPU utilization

3. **Type-safe capabilities**
   - Rights as Rust types, not integers
   - Compile-time checking of operations
   - Impossible to forge capabilities

4. **Lock-free where possible**
   - Atomic counters for rights
   - Lock-free message queue
   - Minimize contention

## Next Steps
1. Set up no_std Rust environment
2. Implement basic Port structure
3. Add unit tests for port operations
4. Build simple message passing demo