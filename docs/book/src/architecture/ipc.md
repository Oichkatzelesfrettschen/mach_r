# The Mach IPC Model

The Inter-Process Communication (IPC) system is the cornerstone of the Mach architecture. It is the sole means by which tasks communicate, enforcing the microkernel's philosophy of isolation and modularity.

## Ports: The Communication Endpoint

A **port** is a unidirectional communication channel, managed by the kernel, into which messages can be queued. Each port has a message queue and space for exactly one receiver.

## Port Rights: The Capability System

Tasks cannot access ports directly. Instead, they hold **port rights**, which are capabilities that grant the ability to interact with a port. The kernel ensures that these rights are unforgeable.

There are three fundamental types of port rights:

- **Receive Right**: A unique right that grants a task the ability to receive messages from a port's queue. Only one task can hold the receive right for a given port at any time.
- **Send Right**: A right that grants a task the ability to send messages to a port. Multiple tasks can hold send rights for the same port.
- **Send-Once Right**: A special, single-use send right that is consumed upon use. It is used for reply messages and notifications, preventing the sender from sending further messages.

```rust
// Conceptual structure of a Port
pub struct Port {
    id: PortId,                              // Unique identifier
    state: Mutex<PortState>,                 // Current state
    receiver: TaskId,                        // The task holding the receive right
    send_rights: AtomicU32,                  // Reference count of send rights
    send_once_rights: AtomicU32,             // Reference count of send-once rights
    messages: MessageQueue,                  // Lock-free message queue
    sequence: AtomicU64,                     // Message ordering
}
```

## Messages: The Unit of Communication

Messages are the data containers that are sent between tasks via ports. A message consists of a header and a body.

- **Header**: Contains metadata about the message, including its size, the destination port (a send right), and an optional port for a reply (a send-once right).
- **Body**: Can contain:
    - **Inline Data**: Small amounts of raw data copied directly into the message.
    - **Out-of-Line Data (Memory Objects)**: For large data transfers, the message can contain a reference to a memory object. The kernel can use the MMU to map this memory into the receiver's address space, avoiding a full data copy.
    - **Port Rights**: The message can carry other port rights, allowing capabilities to be transferred between tasks.

```rust
// Conceptual structure of a Message Header
struct MessageHeader {
    msg_bits: u32,              // Type, complexity, etc.
    msg_size: u32,              // Total size of the message
    msg_remote_port: PortRight, // The destination port (send right)
    msg_local_port: PortRight,  // The reply port (send-once right)
    msg_id: u32,                // A sequence number or identifier
}
```

## The IPC Flow

1.  **Acquisition**: A task must first acquire a send right to another task's port. This is typically done via a naming service or by receiving the right in a message from another task.
2.  **Send Operation**: The sending task creates a message, specifying the destination port right. It places data and/or other capabilities into the message body. The kernel then enqueues the message in the destination port's message queue.
3.  **Receive Operation**: The receiving task performs a blocking or non-blocking receive operation on its port. The kernel dequeues the next message and delivers it to the task. If the message contained out-of-line memory or other port rights, the kernel updates the receiver's address space and port namespace accordingly.

## MIG (Mach Interface Generator)

MIG in Mach_R is implemented in Rust from scratch — no legacy MIG code, runtimes, or generators are used.

- Interfaces use modern Rust types/traits and no_std-friendly marshalling.
- Message shapes and descriptor constants come from our clean-room `mach/abi.rs`.
- Reference `.defs` under `archive/osfmk/reference` inform message layouts only;
  future codegen (if added) will be fresh, Rust-based.

See also: `synthesis/docs/MIG.md` for contributor guidance.
