# Design Decisions and Rationale

*In the spirit of Lions' Commentary: Understanding not just what we built, but why we built it this way*

## Introduction - On Making Decisions

Every design choice is a trade-off. Speed vs. safety. Simplicity vs. features. Compatibility vs. innovation.

This document explains the "why" behind Mach_R's architecture: why we chose these approaches over alternatives, what we gain, and what we sacrifice.

## Foundational Decisions

### Why a Microkernel?

**Decision:** Build a microkernel, not a monolithic kernel

**Rationale:**

Traditional monolithic kernels (Linux, BSD) put everything in kernel space:
- File systems
- Device drivers
- Network stacks
- Process management

Advantages of monolithic:
- ✅ Fast: No IPC overhead between components
- ✅ Simple: Everything in one address space

Disadvantages:
- ❌ Fragile: Any bug can crash entire system
- ❌ Insecure: All code runs with highest privilege
- ❌ Hard to maintain: Millions of lines of intertwined code

Mach_R's microkernel puts only essential services in kernel:
- IPC (message passing)
- Memory management (virtual memory)
- Thread scheduling
- Hardware abstraction

Everything else runs in user space:
- File systems
- Device drivers
- Network stacks

**What we gain:**
- **Isolation**: Buggy driver can't crash kernel
- **Security**: Drivers run with minimal privilege
- **Modularity**: Replace components without kernel changes
- **Verifiability**: Small kernel is easier to audit

**What we sacrifice:**
- **Performance**: IPC adds overhead vs. function calls
- **Complexity**: More moving parts to coordinate

**Why it's worth it:**
Modern systems prioritize reliability and security over raw performance. A slightly slower system that never crashes is better than a fast system that crashes daily.

### Why Rust?

**Decision:** Implement in Rust, not C or C++

**Rationale:**

The original Mach was written in C. Why not use C for compatibility?

C's weaknesses:
- ❌ Memory unsafety (buffer overflows, use-after-free)
- ❌ Type unsafety (void*, unchecked casts)
- ❌ No thread safety guarantees
- ❌ Manual resource management

Rust's strengths:
- ✅ Memory safety without garbage collection
- ✅ Strong type system catches bugs at compile time
- ✅ Thread safety enforced by compiler
- ✅ Zero-cost abstractions (safety with no overhead)
- ✅ Modern tooling (Cargo, rustfmt, clippy)

**What we gain:**
- **Correctness**: Entire classes of bugs impossible
- **Maintainability**: Type system documents invariants
- **Performance**: Zero-cost abstractions, LLVM optimizer

**What we sacrifice:**
- **Learning curve**: Rust is harder to learn than C
- **Ecosystem**: Fewer no_std libraries than C
- **Compatibility**: Can't directly use C Mach code

**Why it's worth it:**
Memory safety bugs account for ~70% of security vulnerabilities. Eliminating them is transformative. The upfront cost of learning Rust pays dividends in reliability.

### Why Clean-Room Implementation?

**Decision:** Clean-room reimplementation, not porting C code

**Rationale:**

We could have:
1. **Translated C to Rust**: Line-by-line conversion
2. **Wrapped C code**: FFI bindings to original Mach
3. **Clean-room**: Implement from scratch using published specs

We chose clean-room.

**Why not translate?**
- C idioms don't map to Rust idioms
- C's unsafe patterns would permeate codebase
- Technical debt from C carried forward

**Why not wrap?**
- Defeats purpose of memory safety
- Still vulnerable to C bugs
- Awkward FFI boundary

**Clean-room advantages:**
- ✅ Idiomatic Rust from the start
- ✅ Modern patterns (async/await, traits)
- ✅ No legacy baggage

**Clean-room disadvantages:**
- ❌ Can't reuse existing code
- ❌ More work upfront
- ❌ Risk of incompatibility

**Why it's worth it:**
Mach_R is not a production replacement for existing systems (yet). It's a research platform to explore what a memory-safe microkernel looks like. Starting fresh lets us explore the design space.

## IPC Design Decisions

### Port-Based IPC

**Decision:** Use Mach ports, not other IPC mechanisms

**Alternatives considered:**
- **Shared memory**: Fast but unsafe
- **Sockets**: Familiar but heavyweight
- **Message queues**: Close, but less flexible

**Why ports?**
- **Capability-based**: Ports are unforgeable tokens
- **Unidirectional**: Clear dataflow
- **Flexible**: Can transfer rights between tasks
- **Proven**: Mach's design worked in macOS/iOS

### Synchronous vs. Asynchronous IPC

**Decision:** Support both sync and async message passing

**Why not just synchronous?**
- Blocking waits waste CPU time
- Can't overlap communication with computation

**Why not just asynchronous?**
- More complex to program
- Harder to reason about ordering

**Solution:** Provide both:

```rust
// Synchronous: Wait for reply
let reply = port.send_and_receive(request)?;

// Asynchronous: Send and continue
port.send(message)?;
// ... do other work ...
let reply = port.receive()?;
```

Programmer chooses based on needs.

### Message Size Limits

**Decision:** Inline data up to 256 bytes, out-of-line for larger

**Why not unlimited inline?**
- Large messages would bloat message queues
- Memory waste for small messages

**Why not all out-of-line?**
- Extra complexity for common case
- Pointer overhead for tiny messages

**256 bytes chosen because:**
- Covers most small messages (ports, integers, small strings)
- Fits in cache line (64 bytes) × 4
- Powers-of-two friendly

## Memory Management Decisions

### External Pagers

**Decision:** User-space pagers, not in-kernel file systems

**Why not kernel file systems (like Linux)?**
- Kernel bloat (ext4 is 50,000+ lines)
- Security risk (bug in FS crashes kernel)
- Flexibility (hard to add new FS types)

**External pagers advantages:**
- ✅ User-space implementation
- ✅ Crashes don't affect kernel
- ✅ Easy to add new pagers (file, swap, device, network)

**Challenge:** Performance
- IPC overhead on every page fault
- Solution: Cache pages in kernel, async pagers

### Page Size

**Decision:** 4 KB pages (matches hardware)

**Why not larger pages?**
- **Larger (2 MB "huge pages"):**
  - Advantage: Fewer TLB misses
  - Disadvantage: Waste (internal fragmentation)
  - Use case: Databases, HPC

- **Smaller (1 KB):**
  - Advantage: Less waste
  - Disadvantage: More page table entries, slower TLB

**4 KB is a sweet spot:**
- Hardware supports it efficiently
- Good balance of overhead vs. waste
- Standard across architectures

**Note:** We may support huge pages later for specific use cases.

### Copy-on-Write

**Decision:** Implement COW for fork()

**Why?**
Traditional fork() copies entire address space:
- Wasteful: Child often execs immediately
- Slow: Copying gigabytes takes time

COW defers copying until necessary:
- Parent and child share pages (read-only)
- On write, copy that page only

**Implementation:**
- Mark pages read-only in both processes
- On write fault, copy page and mark writable
- Reference count tracks sharing

**Trade-off:**
- More complex page fault handling
- But massive performance gain for fork()

## Scheduler Decisions

### Priority-Based Scheduling

**Decision:** 32 priority levels, FIFO within priority

**Why not round-robin (all equal priority)?**
- Can't prioritize interactive tasks
- Can't deprioritize background work

**Why not completely fair scheduler (like Linux CFS)?**
- More complex
- Harder to predict behavior
- Microkernel doesn't need it (few kernel threads)

**32 priorities chosen:**
- Enough granularity for most needs
- Fits in 5 bits (0-31)
- Matches traditional UNIX nice levels (-20 to +19)

### Preemptive Scheduling

**Decision:** Preempt threads after time slice

**Why not cooperative (threads yield explicitly)?**
- Buggy/malicious thread could hog CPU
- Unpredictable latency

**Preemptive guarantees:**
- Every thread gets CPU time
- Bounded latency for high-priority tasks

**Cost:**
- Context switch overhead
- Timer interrupts consume CPU

**Trade-off worth it:** Fairness and responsiveness matter more than raw throughput.

## Type System Decisions

### Newtype Pattern

**Decision:** Wrap primitive types (u64) in structs

**Example:**
```rust
struct PortId(u64);
struct TaskId(u64);
```

**Why not just use u64?**
- Type confusion: `fn send(port: u64, task: u64)` — easy to swap arguments
- No semantics: u64 doesn't convey meaning

**Benefits:**
- Type safety: Can't pass TaskId where PortId expected
- Documentation: Type names convey meaning
- Encapsulation: Can change internal representation

**Cost:**
- More types to manage
- Slight API verbosity

**Worth it:** Type errors are caught at compile time, not runtime.

### State Machines as Enums

**Decision:** Use enums for state, not booleans

**Bad:**
```rust
struct Port {
    is_active: bool,
    is_suspended: bool,
    is_dead: bool,  // Can all three be true?
}
```

**Good:**
```rust
enum PortState {
    Active,
    Suspended,
    Dead,
}
```

**Why enums?**
- ✅ Only one state at a time (by construction)
- ✅ Exhaustive match forces handling all cases
- ✅ Easy to add new states

## Error Handling Decisions

### Result<T, E> Everywhere

**Decision:** Errors return Result, not error codes

**Why not int error codes (C style)?**
```c
int send_message(port_t port, message_t* msg);  // Returns 0 or -errno
```

Problems:
- Easy to ignore: `send_message(port, msg);  // Forgot to check!`
- No type safety: All errors are int
- Unclear which errors possible

**Rust's Result:**
```rust
fn send_message(&self, msg: Message) -> Result<(), PortError>;
```

Benefits:
- ✅ Must handle error (or explicitly ignore with `?`)
- ✅ Type-safe: PortError is specific
- ✅ Documents possible errors

### When to Panic

**Decision:** Panic only for unrecoverable errors or broken invariants

**Recoverable errors → Result:**
- Port is dead
- Queue is full
- Permission denied

**Unrecoverable errors → panic:**
- Internal data structure corrupted
- Precondition violated in unsafe code
- Out of memory (can't allocate page table)

**Guideline:**
If the caller can handle it, use Result.
If it indicates a bug, panic.

## Architecture Support Decisions

### AArch64 Primary, x86_64 Secondary

**Decision:** Develop on AArch64, port to x86_64

**Why not x86_64 first?**
- More complex (legacy baggage)
- Worse ISA (variable-length instructions)
- AArch64 is cleaner, more modern

**Why not AArch64 only?**
- x86_64 still dominant in desktops/servers
- Want to prove portability

**Strategy:**
- Implement features on AArch64 first
- Port to x86_64 to validate architecture abstraction
- Most code should be architecture-independent

## Build System Decisions

### Cargo + xtask

**Decision:** Use Cargo as primary build system, xtask for automation

**Why not Make?**
- Makefiles grow unwieldy
- No cross-platform (Windows support harder)
- Cargo already builds Rust code

**Why not just Cargo?**
- Some tasks don't fit cargo (disk image creation, QEMU launch)
- xtask fills the gap (Rust-based make)

**xtask advantages:**
- Written in Rust (type-safe build scripts)
- Cross-platform
- Integrates with Cargo

## Testing Decisions

### Unit Tests In-Module

**Decision:** Place unit tests next to code they test

**Why not separate test directories?**
- Distance from implementation
- Harder to keep in sync
- Can't test private functions

**Co-located tests:**
```rust
// src/port.rs
impl Port {
    fn internal_helper(&self) { }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_internal_helper() {
        // Can test private functions
    }
}
```

### Integration Tests Separate

**Decision:** Integration tests in tests/ directory

**Why separate?**
- Test public API only (as external users would)
- Can test multiple modules together
- Clear distinction from unit tests

## Documentation Decisions

### Mandatory Public API Docs

**Decision:** All public items must have /// comments

**Why enforce?**
- Documentation rots when optional
- New contributors need context
- API docs are for users, not just devs

**What to document:**
- What the function does
- Arguments and return value
- Possible errors
- Examples
- Thread safety considerations

### Lions-Style Commentary

**Decision:** Write pedagogical documentation explaining "why"

**Inspiration:** John Lions' UNIX v6 Commentary

**Goals:**
- Explain concepts, not just APIs
- Show reasoning behind decisions
- Make kernel approachable for learners

## Summary - Design Philosophy

Mach_R's design follows these principles:

1. **Safety First**: Correctness over performance
2. **Simplicity**: Simple parts compose into complex systems
3. **Separation**: Tasks separate from threads, mechanism from policy
4. **Modularity**: User-space servers, not kernel monolith
5. **Type Safety**: Use Rust's type system to prevent bugs
6. **Clarity**: Code should explain itself

Every decision balances trade-offs. We chose reliability, security, and maintainability over raw performance and backward compatibility.

The result: A microkernel that's safer, clearer, and more maintainable than its C predecessors, at the cost of some performance and compatibility.

---

**See Also:**
- [Overview](overview.md) - High-level architecture
- [Memory Management](memory-management.md) - VM design details
- [Task & Threading](task-threading.md) - Execution model
- [IPC System](ipc-system.md) - Message passing design
