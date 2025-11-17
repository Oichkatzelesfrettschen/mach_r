# Architecture

The design of Mach_R is deeply rooted in the principles of the original Mach microkernel, but reimagined with the safety, concurrency, and expressive power of modern Rust.

This section details the core architectural components of the system.

## Key Pillars

1.  **Microkernel Design**: A minimal kernel that provides only the most fundamental services: IPC, scheduling, and memory management primitives. All other services (filesystems, device drivers, network stacks) are implemented as user-space servers.
2.  **Capability-Based Security**: Resources (like memory objects and communication channels) are accessed via unforgeable capabilities known as "port rights." A task can only access resources for which it holds a valid right.
3.  **Message-Passing IPC**: All interactions between components, including between user-space servers and the kernel itself, are mediated through a single, powerful Inter-Process Communication (IPC) mechanism.
4.  **Rust's Safety Guarantees**: The entire kernel is being developed in Rust, leveraging its strong type system and ownership model to eliminate entire classes of bugs (e.g., null pointer dereferences, buffer overflows, data races) at compile time.

Explore the subsections for a deeper dive into each part of the Mach_R architecture.