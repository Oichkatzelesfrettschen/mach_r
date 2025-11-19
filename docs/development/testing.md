# Testing Guide - Mach_R

*In the spirit of Lions' UNIX v6 Commentary: A pedagogical exploration of testing methodology*

## Introduction

Testing is not merely verification - it is a dialogue between the programmer and the machine, a way to interrogate our assumptions and illuminate hidden behaviors. This guide explains Mach_R's testing philosophy and practices in detail.

## Philosophy of Testing

### Why We Test

In a microkernel, correctness is paramount. Unlike user applications where crashes are tolerated, kernel bugs can:
- Corrupt memory across process boundaries
- Violate security invariants
- Cause unrecoverable system panics
- Create subtle race conditions that appear only under load

Rust prevents *many* classes of bugs, but not all. Tests catch what the type system cannot.

### What We Test

```
┌─────────────────────────────────────┐
│  Rust Type System Guarantees        │
│  - Memory safety                    │
│  - Thread safety (Send/Sync)        │
│  - No null pointer dereferences     │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│  Tests Must Verify                  │
│  - Correctness of algorithms        │
│  - Port semantics and ordering      │
│  - IPC message delivery             │
│  - Resource cleanup                 │
│  - Edge cases and error paths       │
└─────────────────────────────────────┘
```

The type system is our first line of defense; tests are our second.

## Test Organization

### Unit Tests - Testing in Isolation

Unit tests live alongside the code they test, enclosed in `#[cfg(test)]` modules:

```rust
// src/port.rs

pub struct Port {
    id: PortId,
    state: Mutex<PortState>,
    messages: MessageQueue,
}

impl Port {
    pub fn new(receiver: TaskId) -> Arc<Self> {
        Arc::new(Self {
            id: PortId::generate(),
            state: Mutex::new(PortState::Active),
            messages: MessageQueue::new(),
        })
    }

    pub fn send(&self, msg: Message) -> Result<(), PortError> {
        // Implementation...
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_creation_assigns_unique_id() {
        // Why this test: Port IDs must be unique system-wide
        // to prevent IPC confusion. We verify the generator.

        let task = TaskId::new(1);
        let port1 = Port::new(task);
        let port2 = Port::new(task);

        assert_ne!(port1.id, port2.id,
            "Port IDs must be unique across all ports");
    }

    #[test]
    fn new_port_starts_in_active_state() {
        // Why this test: Ports must be immediately usable
        // after creation. No separate "activate" step.

        let port = Port::new(TaskId::new(1));
        let state = port.state.lock();

        assert!(matches!(*state, PortState::Active),
            "New ports must be Active, not {:?}", *state);
    }

    #[test]
    fn send_to_active_port_succeeds() {
        // Why this test: Basic IPC functionality.
        // A message to an active port should always succeed.

        let port = Port::new(TaskId::new(1));
        let msg = Message::new(b"test data");

        let result = port.send(msg);

        assert!(result.is_ok(),
            "Sending to active port should succeed, got {:?}", result);
    }

    #[test]
    fn send_to_dead_port_fails() {
        // Why this test: Error handling. Dead ports must
        // reject messages to prevent resource leaks.

        let port = Port::new(TaskId::new(1));

        // Simulate port death
        *port.state.lock() = PortState::Dead;

        let msg = Message::new(b"test data");
        let result = port.send(msg);

        assert!(matches!(result, Err(PortError::PortDead)),
            "Sending to dead port must fail with PortDead");
    }
}
```

### Integration Tests - Testing Interactions

Integration tests verify that components work together correctly:

```rust
// tests/test_ipc.rs

use mach_r::{Port, Message, Task};

#[test]
fn message_send_receive_roundtrip() {
    // Why this test: Verifies the complete IPC path:
    // 1. Message creation
    // 2. Send to port
    // 3. Receive from port
    // 4. Data integrity

    // Setup: Create two tasks
    let sender = Task::new();
    let receiver = Task::new();

    // Create a port owned by receiver
    let port = Port::new(receiver.id());

    // Sender gets a send right
    let send_right = port.create_send_right();
    sender.add_right(send_right);

    // Create and send a message
    let original_data = b"Hello from sender task";
    let msg = Message::new(original_data);

    sender.send_message(&port, msg)
        .expect("Send should succeed");

    // Receiver gets the message
    let received = receiver.receive_message(&port)
        .expect("Receive should succeed");

    // Verify data integrity
    assert_eq!(received.data(), original_data,
        "Received data must match sent data exactly");
}

#[test]
fn port_rights_transfer_correctly() {
    // Why this test: Port rights are capabilities.
    // Transferring a right must remove it from sender
    // and add it to receiver.

    let task_a = Task::new();
    let task_b = Task::new();

    let port = Port::new(task_a.id());
    let send_right = port.create_send_right();

    // Task A has the right
    assert!(task_a.has_right(&send_right),
        "Task A should have the right initially");

    // Transfer in a message
    let msg = Message::with_right(send_right.clone());
    task_a.send_to(task_b.id(), msg).expect("Send failed");

    // Now task B has it
    let received = task_b.receive().expect("Receive failed");
    assert!(received.contains_right(&send_right),
        "Message should contain the transferred right");

    // Task A no longer has it
    assert!(!task_a.has_right(&send_right),
        "Right should be removed from sender after transfer");
}
```

### Property-Based Testing - Testing Invariants

Property tests verify that invariants hold across many random inputs:

```rust
// tests/test_property_based.rs

use proptest::prelude::*;
use mach_r::{Port, Message};

proptest! {
    #[test]
    fn port_message_count_never_negative(
        num_sends in 0..100usize,
        num_receives in 0..100usize,
    ) {
        // Invariant: message queue count >= 0 always

        let port = Port::new(TaskId::new(1));

        // Send messages
        for _ in 0..num_sends {
            let msg = Message::new(b"data");
            port.send(msg).ok(); // Ignore full queue
        }

        // Receive messages
        for _ in 0..num_receives {
            port.receive().ok(); // Ignore empty queue
        }

        let count = port.message_count();
        prop_assert!(count >= 0,
            "Message count should never be negative");
    }

    #[test]
    fn message_data_roundtrip(data: Vec<u8>) {
        // Property: Any data sent should be received unchanged

        let port = Port::new(TaskId::new(1));
        let msg = Message::new(&data);

        port.send(msg).unwrap();
        let received = port.receive().unwrap();

        prop_assert_eq!(received.data(), data.as_slice(),
            "Data must survive send/receive unchanged");
    }
}
```

## Test Structure - Anatomy of a Good Test

### The AAA Pattern: Arrange, Act, Assert

```rust
#[test]
fn descriptive_test_name_describes_behavior() {
    // ARRANGE: Set up the test environment
    let port = Port::new(TaskId::new(1));
    let message = Message::new(b"test");

    // ACT: Perform the operation being tested
    let result = port.send(message);

    // ASSERT: Verify the outcome
    assert!(result.is_ok(), "Send should succeed");

    // Optional: CLEANUP (usually automatic via Drop)
}
```

### Error Messages - Make Failures Informative

Bad:
```rust
assert!(result.is_ok()); // Fails with: assertion failed
```

Good:
```rust
assert!(result.is_ok(),
    "Port send failed: expected Ok, got {:?}", result);
// Fails with: Port send failed: expected Ok, got Err(PortDead)
```

Best:
```rust
match result {
    Ok(_) => {}, // Test passes
    Err(e) => panic!(
        "Port send failed unexpectedly:\n\
         Port state: {:?}\n\
         Error: {:?}\n\
         Message size: {} bytes",
        port.state(), e, message.size()
    ),
}
```

## Running Tests

### Basic Test Execution

```bash
# Run all tests
cargo test --lib

# Run specific test
cargo test test_port_creation

# Run tests matching pattern
cargo test ipc

# Run tests in specific module
cargo test port::tests
```

### Test Output

```bash
# Show println! output even for passing tests
cargo test -- --nocapture

# Show detailed test information
cargo test -- --show-output

# Run tests sequentially (not parallel)
cargo test -- --test-threads=1
```

### Test Filtering

```bash
# Run only tests containing "port"
cargo test port

# Run only tests in port module
cargo test port::

# Exclude tests (run all except integration tests)
cargo test --lib
```

## Writing Effective Tests

### Test One Thing

Bad - Tests multiple behaviors:
```rust
#[test]
fn port_operations() {
    let port = Port::new(TaskId::new(1));
    assert!(port.is_active());

    let msg = Message::new(b"test");
    port.send(msg.clone()).unwrap();

    let received = port.receive().unwrap();
    assert_eq!(received.data(), msg.data());

    port.close();
    assert!(port.is_dead());
}
```

Good - Separate tests:
```rust
#[test]
fn new_port_is_active() {
    let port = Port::new(TaskId::new(1));
    assert!(port.is_active());
}

#[test]
fn sent_message_can_be_received() {
    let port = Port::new(TaskId::new(1));
    let msg = Message::new(b"test");

    port.send(msg.clone()).unwrap();
    let received = port.receive().unwrap();

    assert_eq!(received.data(), msg.data());
}

#[test]
fn closed_port_becomes_dead() {
    let port = Port::new(TaskId::new(1));
    port.close();
    assert!(port.is_dead());
}
```

### Test Edge Cases

```rust
#[test]
fn empty_message_is_valid() {
    // Edge case: zero-length message
    let port = Port::new(TaskId::new(1));
    let msg = Message::new(b"");

    assert!(port.send(msg).is_ok(),
        "Empty messages should be valid");
}

#[test]
fn maximum_message_size() {
    // Edge case: largest allowed message
    let port = Port::new(TaskId::new(1));
    let data = vec![0u8; MAX_MESSAGE_SIZE];
    let msg = Message::new(&data);

    assert!(port.send(msg).is_ok(),
        "Maximum size message should succeed");
}

#[test]
fn oversized_message_rejected() {
    // Edge case: too-large message
    let port = Port::new(TaskId::new(1));
    let data = vec![0u8; MAX_MESSAGE_SIZE + 1];
    let msg = Message::new(&data);

    assert!(matches!(port.send(msg), Err(PortError::MessageTooLarge)),
        "Oversized message must be rejected");
}
```

### Test Error Paths

Don't just test the happy path:

```rust
#[test]
fn receive_from_empty_port_blocks() {
    let port = Port::new(TaskId::new(1));

    // No messages sent
    let result = port.try_receive(); // Non-blocking version

    assert!(matches!(result, Err(PortError::WouldBlock)),
        "Receiving from empty port should indicate WouldBlock");
}

#[test]
fn send_to_full_port_fails_gracefully() {
    let port = Port::new(TaskId::new(1));

    // Fill the port to capacity
    for i in 0..PORT_QUEUE_CAPACITY {
        let msg = Message::new(&[i as u8]);
        port.send(msg).expect(&format!("Send {} failed", i));
    }

    // Next send should fail
    let overflow_msg = Message::new(b"overflow");
    let result = port.send(overflow_msg);

    assert!(matches!(result, Err(PortError::QueueFull)),
        "Sending to full port must fail with QueueFull");
}
```

## Testing Concurrent Code

Mach_R is inherently concurrent. Testing race conditions is crucial:

```rust
#[test]
fn concurrent_sends_all_succeed() {
    use std::thread;

    let port = Arc::new(Port::new(TaskId::new(1)));
    let num_threads = 10;
    let messages_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let port = Arc::clone(&port);
            thread::spawn(move || {
                for msg_id in 0..messages_per_thread {
                    let data = format!("t{}-m{}", thread_id, msg_id);
                    let msg = Message::new(data.as_bytes());
                    port.send(msg).expect("Concurrent send failed");
                }
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Verify all messages received
    let expected_count = num_threads * messages_per_thread;
    let actual_count = port.message_count();

    assert_eq!(actual_count, expected_count,
        "All {} messages should be in queue", expected_count);
}
```

## Test Coverage

### Measuring Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --lib --out Html

# View in browser
open tarpaulin-report.html
```

### Coverage Goals

```
Module              Target    Current
------------------  -------   -------
port.rs            95%       87%   ← needs work
message.rs         95%       92%   ← almost there
task.rs            90%       78%   ← needs work
memory.rs          85%       45%   ← critical gap
scheduler.rs       90%       0%    ← not started
```

## Testing Best Practices

### 1. Tests Should Be Fast

```rust
// Good - Fast test
#[test]
fn port_state_transition() {
    let port = Port::new(TaskId::new(1));
    port.close();
    assert!(port.is_dead());
} // Completes in microseconds

// Bad - Slow test
#[test]
fn stress_test_million_messages() {
    let port = Port::new(TaskId::new(1));
    for i in 0..1_000_000 {
        port.send(Message::new(&i.to_le_bytes())).unwrap();
    }
} // Takes seconds - mark with #[ignore]
```

### 2. Tests Should Be Deterministic

```rust
// Bad - Non-deterministic (timing dependent)
#[test]
fn message_arrives_quickly() {
    let port = Port::new(TaskId::new(1));
    let start = Instant::now();

    port.send(Message::new(b"test")).unwrap();
    let received = port.receive().unwrap();

    let duration = start.elapsed();
    assert!(duration < Duration::from_millis(1)); // Flaky!
}

// Good - Deterministic (tests behavior, not timing)
#[test]
fn sent_message_is_receivable() {
    let port = Port::new(TaskId::new(1));

    port.send(Message::new(b"test")).unwrap();
    let received = port.receive().unwrap();

    assert_eq!(received.data(), b"test");
}
```

### 3. Tests Should Be Independent

```rust
// Bad - Tests depend on order
static GLOBAL_PORT: OnceCell<Port> = OnceCell::new();

#[test]
fn test_a_creates_port() {
    let port = Port::new(TaskId::new(1));
    GLOBAL_PORT.set(port).unwrap();
}

#[test]
fn test_b_uses_port() {
    let port = GLOBAL_PORT.get().unwrap(); // Fails if test_a didn't run!
    // ...
}

// Good - Each test is self-contained
#[test]
fn test_a() {
    let port = Port::new(TaskId::new(1));
    // Use port...
}

#[test]
fn test_b() {
    let port = Port::new(TaskId::new(1));
    // Use port...
}
```

## Debugging Failed Tests

### Use `--nocapture` for Debug Output

```rust
#[test]
fn debug_example() {
    let port = Port::new(TaskId::new(1));
    println!("Port created: {:?}", port);

    let msg = Message::new(b"test");
    println!("Sending message: {:?}", msg);

    port.send(msg).unwrap();
    println!("Message sent successfully");
}
```

Run with: `cargo test debug_example -- --nocapture`

### Use `dbg!` Macro

```rust
#[test]
fn investigate_failure() {
    let port = Port::new(TaskId::new(1));
    dbg!(&port); // Prints: [src/port.rs:123] &port = Port { ... }

    let result = port.send(Message::new(b"test"));
    dbg!(&result); // Prints: [src/port.rs:126] &result = Ok(())
}
```

## Continuous Integration

Tests run automatically on every commit via GitHub Actions:

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test --lib
      - run: cargo test --test '*'
```

## Summary

Good tests:
- ✅ Test one behavior per test
- ✅ Have descriptive names
- ✅ Include helpful error messages
- ✅ Test edge cases and error paths
- ✅ Are fast, deterministic, and independent
- ✅ Cover both happy paths and failure modes

Testing is not overhead - it is insurance against regression and documentation of expected behavior.

---

**See Also:**
- [Building Guide](building.md) - How to build and run tests
- [Code Style](code-style.md) - Coding standards
- [Contributing](../../CONTRIBUTING.md) - Contribution guidelines
