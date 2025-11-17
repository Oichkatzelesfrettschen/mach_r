# Phase 1: Core IPC

**Goal**: Implement the Mach Inter-Process Communication (IPC) system. This is the most critical phase, as IPC is the fundamental building block for all other system services.

## Key Deliverables

- [✅] Port and Port Rights Implementation: Create the Rust structures for `Port` and `PortRight` with strong typing to represent receive, send, and send-once rights.

- [✅] Rights Management: Implement the logic for managing the lifecycle of rights: creation, destruction, and reference counting, including notifications.

- [✅] Message Structure: Define the `Message` struct, including the header and support for inline data, out-of-line data, and port rights.

- [✅] Synchronous IPC: Implement the basic blocking `send()` and `receive()` operations on ports.

- [✅] Port Namespace: Implement the per-task data structure that stores the rights a task holds, and a global port registry.

- [✅] Rights Transfer: Support the transfer of port rights within messages, allowing capabilities to be passed between tasks.

- [✅] Unit and Integration Tests: Comprehensive unit tests are in place for port and message handling, including rights transitions and message queuing.

## Success Criteria

- Two threads within the kernel can successfully exchange messages via the IPC system.
- Port rights are correctly created, transferred, and destroyed without leaking resources.
- The system can withstand a stress test of rapid message passing between multiple threads.
- A working IPC demo between two Rust tasks is demonstrable.
