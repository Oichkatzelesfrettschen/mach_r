# Code Style Guide - Mach_R

*In the spirit of Lions' Commentary: A meditation on clarity, consistency, and the art of readable code*

## Philosophy

Code is read far more often than it is written. In a microkernel, where correctness is paramount and bugs can corrupt entire systems, clarity is not a luxury - it is a necessity.

Good style emerges from asking: "Will this be clear to someone reading it six months from now? Will it be clear to me?"

## Fundamental Principles

### 1. Clarity Over Cleverness

```rust
// Clever - Saves two lines
fn check(p: &Port) -> bool { matches!(p.state.lock().deref(), PortState::Active) }

// Clear - Explains what we're checking
fn is_port_active(port: &Port) -> bool {
    let state = port.state.lock();
    matches!(*state, PortState::Active)
}
```

The second version is longer, but its intent is immediately obvious. In kernel code, obvious is valuable.

### 2. Consistency Over Personal Preference

The Mach_R codebase follows Rust standard conventions. Even if you prefer different formatting, consistency across the codebase aids comprehension.

### 3. Type Safety Over Performance (Until Profiling Proves Otherwise)

```rust
// Unsafe - Micro-optimization without measurement
unsafe fn get_port_unchecked(id: PortId) -> &'static Port {
    &*PORT_TABLE[id.as_usize()]
}

// Safe - Use until profiling shows this is a bottleneck
fn get_port(id: PortId) -> Option<Arc<Port>> {
    PORT_TABLE.get(&id).map(Arc::clone)
}
```

Rust's zero-cost abstractions mean safe code is often as fast as unsafe code. Measure before optimizing.

## Naming Conventions

### General Rules

```rust
// Types: CamelCase
struct Port { }
enum PortState { }
trait IpcCapable { }

// Functions and variables: snake_case
fn create_port() { }
let message_count = 0;

// Constants: SCREAMING_SNAKE_CASE
const MAX_MESSAGE_SIZE: usize = 65536;
const PORT_QUEUE_CAPACITY: usize = 256;

// Modules: snake_case
mod port_rights;
mod message_queue;
```

### Descriptive Names

Names should reveal intent:

```rust
// Bad - Meaningless
let x = Port::new(tid);
let y = x.recv();

// Good - Intent is clear
let control_port = Port::new(task_id);
let received_message = control_port.receive();
```

### Avoid Abbreviations (Except Well-Known Ones)

```rust
// Bad - Unclear abbreviations
fn proc_msg(p: &Port, m: &Msg) -> Res<()>

// Good - Full words
fn process_message(port: &Port, message: &Message) -> Result<()>

// Acceptable - Well-known in Mach context
fn ipc_send(port: &Port, msg: &Message) -> Result<()>  // IPC is standard
fn vm_allocate(size: usize) -> Result<*mut u8>         // VM is standard
```

## Formatting

### Use `rustfmt`

All code must be formatted with `cargo fmt` before commit:

```bash
# Format all code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check
```

Rustfmt enforces:
- 4-space indentation (no tabs)
- 100-character line limit (soft)
- Consistent brace placement
- Standardized spacing

### Manual Formatting Guidelines

When rustfmt isn't sufficient:

#### Align Related Items

```rust
// Good - Alignment shows structure
let port_id      = PortId::generate();
let task_id      = TaskId::new(1);
let message_seq  = sequence.fetch_add(1, Ordering::SeqCst);

// Also good if items are unrelated
let port_id = PortId::generate();
let task_id = TaskId::new(1);
let message_seq = sequence.fetch_add(1, Ordering::SeqCst);
```

#### Break Long Chains Logically

```rust
// Bad - Hard to read
let result = port.lock().unwrap().messages.pop_front().ok_or(PortError::Empty)?;

// Good - Each operation on its own line
let result = port.lock()
    .unwrap()
    .messages
    .pop_front()
    .ok_or(PortError::Empty)?;
```

#### Group Related Code with Blank Lines

```rust
pub fn send_message(&self, message: Message) -> Result<(), PortError> {
    // Validate message
    if message.size() > MAX_MESSAGE_SIZE {
        return Err(PortError::MessageTooLarge);
    }

    // Check port state
    let state = self.state.lock();
    if !matches!(*state, PortState::Active) {
        return Err(PortError::PortDead);
    }
    drop(state);  // Release lock early

    // Enqueue message
    self.messages.push(message)?;

    // Notify waiting receivers
    self.wake_receivers();

    Ok(())
}
```

## Documentation

### Public API - Always Document

Every public item must have a doc comment:

```rust
/// Represents a Mach port, the fundamental IPC primitive.
///
/// Ports are unidirectional message queues with capability-based
/// access control. Each port has a single receiver and potentially
/// many senders.
///
/// # Port Rights
///
/// - `Receive`: Allows receiving messages (one per port)
/// - `Send`: Allows sending messages (can be duplicated)
/// - `SendOnce`: Allows one message send (consumed on use)
///
/// # Examples
///
/// ```
/// use mach_r::Port;
///
/// let port = Port::new(task_id);
/// port.send(Message::new(b"Hello"))?;
/// let msg = port.receive()?;
/// ```
///
/// # Thread Safety
///
/// Ports are thread-safe and can be shared across tasks via `Arc<Port>`.
pub struct Port {
    // ...
}
```

### Function Documentation

```rust
/// Sends a message to this port.
///
/// # Arguments
///
/// * `message` - The message to send
///
/// # Returns
///
/// - `Ok(())` if the message was queued successfully
/// - `Err(PortError::PortDead)` if the port is no longer active
/// - `Err(PortError::QueueFull)` if the message queue is at capacity
///
/// # Examples
///
/// ```
/// let port = Port::new(task_id);
/// let msg = Message::new(b"data");
///
/// match port.send(msg) {
///     Ok(()) => println!("Message sent"),
///     Err(e) => eprintln!("Send failed: {:?}", e),
/// }
/// ```
///
/// # Thread Safety
///
/// This method is thread-safe and can be called from multiple threads
/// concurrently. Messages are queued in the order received.
pub fn send(&self, message: Message) -> Result<(), PortError> {
    // ...
}
```

### Internal Code - Comment Complex Logic

```rust
// Bad - No explanation for complex logic
fn allocate_port_id(&self) -> PortId {
    let id = self.next_id.fetch_add(1, Ordering::SeqCst);
    PortId::new((id & 0xFFFF_FFFF) | (self.node_id << 32))
}

// Good - Explains the bit manipulation
fn allocate_port_id(&self) -> PortId {
    // Port IDs are 64-bit values:
    // - Bits 0-31: Sequential counter (wraps at 2^32)
    // - Bits 32-63: Node ID for distributed systems
    //
    // This allows 2^32 unique ports per node while
    // maintaining global uniqueness in clusters.

    let counter = self.next_id.fetch_add(1, Ordering::SeqCst);
    let local_id = counter & 0xFFFF_FFFF;  // Bottom 32 bits
    let global_id = local_id | (self.node_id << 32);  // Add node ID

    PortId::new(global_id)
}
```

### When to Comment

✅ **Do comment:**
- Why code exists (rationale)
- Why this approach over alternatives
- Non-obvious invariants
- Unsafe code justification
- Performance-critical sections

❌ **Don't comment:**
- What the code does (if the code is clear)
- Obvious operations

```rust
// Bad - States the obvious
// Increment the counter
counter += 1;

// Good - Explains why
// Skip sequence number 0 to avoid confusion with uninitialized values
if sequence == 0 {
    sequence = 1;
}

// Bad - Redundant
// Check if the port is active
if port.is_active() {

// Good - Explains the reason
// Only active ports can receive messages; dead ports return errors
if port.is_active() {
```

## Error Handling

### Use `Result<T, E>` for Recoverable Errors

```rust
// Good - Caller can handle the error
pub fn send(&self, msg: Message) -> Result<(), PortError> {
    if !self.is_active() {
        return Err(PortError::PortDead);
    }
    // ...
}

// Bad - Panic on recoverable error
pub fn send(&self, msg: Message) {
    assert!(self.is_active(), "Port is dead!");
    // ...
}
```

### Use `panic!` Only for Unrecoverable Errors

```rust
// Acceptable - Internal invariant violated
fn internal_send(&mut self, msg: Message) {
    // Caller must ensure queue has space
    if self.queue.len() >= CAPACITY {
        panic!("Queue overflow - internal invariant violated");
    }
    self.queue.push(msg);
}

// Better - Document the precondition
/// # Safety
///
/// Caller must ensure queue has available capacity.
/// Use `has_capacity()` to check before calling.
unsafe fn internal_send_unchecked(&mut self, msg: Message) {
    debug_assert!(self.queue.len() < CAPACITY,
        "Queue overflow - precondition violated");
    self.queue.push(msg);
}
```

### Provide Context in Errors

```rust
// Bad - No context
Err(PortError::SendFailed)

// Good - Contextual information
Err(PortError::SendFailed {
    port_id: self.id,
    port_state: self.state(),
    message_size: message.size(),
})
```

## Type Design

### Use Newtype Pattern for Type Safety

```rust
// Bad - Easy to confuse port IDs and task IDs
fn grant_access(port: u64, task: u64) { }

// Good - Type system prevents confusion
pub struct PortId(u64);
pub struct TaskId(u64);

fn grant_access(port: PortId, task: TaskId) { }

// This won't compile:
grant_access(task_id, port_id);  // Error: type mismatch
```

### Use Enums for State Machines

```rust
/// Port lifecycle states.
///
/// State transitions:
/// ```text
/// New ──> Active ──> Dead
///            │
///            └──> Suspended ──> Active
/// ```
pub enum PortState {
    /// Port is active and can send/receive messages
    Active,

    /// Port is temporarily suspended
    Suspended,

    /// Port is dead and cannot be used
    /// (All operations return PortError::PortDead)
    Dead,
}
```

### Leverage the Type System

```rust
// Bad - Boolean flag allows invalid states
struct Port {
    is_dead: bool,
    is_suspended: bool,  // Can both be true - invalid!
}

// Good - Enum prevents invalid states
struct Port {
    state: PortState,  // Can only be one state at a time
}
```

## Module Organization

### File Structure

```rust
// src/port.rs

//! Port management and IPC primitives.
//!
//! This module implements Mach ports, the fundamental IPC mechanism.

// Imports
use crate::task::TaskId;
use crate::message::Message;
use core::sync::atomic::{AtomicU64, Ordering};

// Type definitions
pub struct Port { /* ... */ }
pub struct PortId(u64);

// Public enums
pub enum PortState { /* ... */ }
pub enum PortError { /* ... */ }

// Public implementation
impl Port {
    // Constructors first
    pub fn new(receiver: TaskId) -> Arc<Self> { }

    // Then main operations (alphabetically or logically grouped)
    pub fn receive(&self) -> Result<Message, PortError> { }
    pub fn send(&self, msg: Message) -> Result<(), PortError> { }

    // Then queries
    pub fn is_active(&self) -> bool { }
    pub fn message_count(&self) -> usize { }

    // Then utilities
    pub fn close(&self) { }
}

// Private implementation
impl Port {
    fn internal_enqueue(&mut self, msg: Message) { }
}

// Tests at the end
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_creation() { }
}
```

### Visibility

Be intentional about visibility:

```rust
// Public - Part of the API
pub struct Port { }

// Private - Internal implementation
struct PortQueue { }

// Pub(crate) - Visible within crate only
pub(crate) fn internal_helper() { }

// Pub(super) - Visible to parent module only
pub(super) struct InternalState { }
```

## `unsafe` Code

### Minimize Unsafe

Unsafe code must be:
1. Absolutely necessary
2. Thoroughly documented
3. Isolated in small functions
4. Encapsulated with safe APIs

```rust
/// # Safety
///
/// Caller must ensure:
/// 1. `ptr` is valid and properly aligned
/// 2. `ptr` points to initialized memory
/// 3. No other references to this memory exist
/// 4. The memory will not be accessed after this call
unsafe fn take_ownership(ptr: *mut Port) -> Box<Port> {
    // SAFETY: Caller guarantees `ptr` is valid and we have exclusive access
    Box::from_raw(ptr)
}

// Provide a safe wrapper
pub fn transfer_port_ownership(port_id: PortId) -> Result<Box<Port>, PortError> {
    let ptr = validate_and_get_port_ptr(port_id)?;

    // SAFETY: `validate_and_get_port_ptr` ensures:
    // - ptr is valid and aligned
    // - ptr points to initialized Port
    // - we have exclusive ownership
    unsafe { Ok(take_ownership(ptr)) }
}
```

### Document Safety Invariants

```rust
pub struct Port {
    // SAFETY INVARIANT: `messages` must never exceed QUEUE_CAPACITY
    // Enforced by: All push operations check capacity first
    messages: Vec<Message>,

    // SAFETY INVARIANT: `state` lock must be held when modifying `messages`
    // Enforced by: All message operations acquire lock first
    state: Mutex<PortState>,
}
```

## Concurrency

### Use Type System for Thread Safety

```rust
// Good - Type system enforces thread safety
pub struct Port {
    // Atomic for lock-free access
    sequence: AtomicU64,

    // Mutex for exclusive access
    state: Mutex<PortState>,

    // Arc for shared ownership
    messages: Arc<Mutex<VecDeque<Message>>>,
}

// The compiler proves this is safe:
unsafe impl Send for Port {}
unsafe impl Sync for Port {}
```

### Avoid Locks When Possible

```rust
// Better - Lock-free when possible
pub fn next_sequence(&self) -> u64 {
    self.sequence.fetch_add(1, Ordering::SeqCst)
}

// Than - Lock-based
pub fn next_sequence(&self) -> u64 {
    let mut seq = self.sequence.lock();
    let val = *seq;
    *seq += 1;
    val
}
```

### Hold Locks Briefly

```rust
// Bad - Lock held during expensive operation
fn process_message(&self, msg: Message) -> Result<()> {
    let mut state = self.state.lock();
    state.messages.push(msg);
    self.expensive_processing(msg)?;  // Lock still held!
    Ok(())
}

// Good - Lock released before expensive operation
fn process_message(&self, msg: Message) -> Result<()> {
    {
        let mut state = self.state.lock();
        state.messages.push(msg);
    }  // Lock released here

    self.expensive_processing(msg)?;  // No lock held
    Ok(())
}
```

## Performance Considerations

### Profile Before Optimizing

```rust
// Don't write this without profiling first:
unsafe fn unchecked_optimization() {
    // Unsafe code for micro-optimization
}

// Write this first:
fn safe_version() {
    // Safe, clear implementation
}

// Then profile. If this is a bottleneck, optimize with data.
```

### Use Zero-Cost Abstractions

```rust
// This abstraction compiles to the same code as manual indexing:
for message in port.messages() {
    process(message);
}

// But is clearer than:
for i in 0..port.message_count() {
    let message = port.get_message(i);
    process(message);
}
```

## Linting

### Run Clippy

```bash
cargo clippy -- -D warnings
```

Clippy catches:
- Common mistakes
- Non-idiomatic code
- Performance issues
- Safety problems

### Address Clippy Warnings

```rust
// Clippy warning: needless borrow
let result = process_port(&port);  // ❌

// Fixed
let result = process_port(port);   // ✅

// Clippy warning: match can be replaced with if-let
match port.receive() {  // ❌
    Ok(msg) => process(msg),
    Err(_) => {},
}

// Fixed
if let Ok(msg) = port.receive() {  // ✅
    process(msg);
}
```

## Summary - The Code Style Checklist

Before submitting code:

- [ ] Formatted with `cargo fmt`
- [ ] Passes `cargo clippy -- -D warnings`
- [ ] Public APIs have doc comments
- [ ] Complex logic has explanatory comments
- [ ] Names are descriptive and follow conventions
- [ ] Unsafe code is documented with SAFETY comments
- [ ] Errors use `Result<T, E>` not `panic!`
- [ ] Tests pass with `cargo test`
- [ ] No unnecessary `unsafe` blocks
- [ ] Locks are held briefly

---

**See Also:**
- [Testing Guide](testing.md) - How to test your code
- [Contributing](../../CONTRIBUTING.md) - Contribution process
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
