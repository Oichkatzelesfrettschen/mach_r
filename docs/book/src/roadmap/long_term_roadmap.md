# Mach_R Long-Term Roadmap (30 items)

This document outlines the comprehensive, long-term development plan for Mach_R, broken down into 30 key items across various kernel subsystems. This roadmap serves as a guiding vision for the project's evolution.

## Kernel IPC
1) Implement full rights transitions (move/make/copy/send-once) with table-driven logic and tests.
2) Add dead-name and no-senders notifications with delivery guarantees.
3) Support complex messages (descriptors) and header bits; size validation.
4) Implement port sets and receiving on sets; unit tests.
5) Extend MIG marshalling for Port, OOL, and OOL-Ports descriptors.

## VM & Pager
6) Complete map/unmap/protect with per-page attributes and alignment checks.
7) Wire page-fault path; add stub pager-backed page-in/out.
8) Minimal default pager (RAM-backed) and object lifecycle.
9) Copy-on-write and shared memory regions; tests for fork semantics.
10) VM stats and debug support (fault counters, object/region introspection).

## Scheduler & Timing
11) Priority queues with aging; starvation protection; preemption boundaries.
12) High-res timer integration for sleeps/timeouts; calibrate tick frequency.
13) Lightweight tracing for scheduling latency and IPC round-trip times.

## Boot & Architecture
14) AArch64 boot: finalize MMU/TTBR setup, per-CPU stacks, exception vectors.
15) x86_64 build path parity (kernel ELF) and early-boot stub.
16) Unified UART/panic paths across arch; consistent early logging.

## System Servers
17) Name server (registry) with leases and restart behavior.
18) Default pager server with pager objects and async replies.
19) File server MVP (in-memory FS) with port-based API (open/read/write/stat).
20) Device server façade (virtio-style stubs) and capability model.
21) Network server skeleton (loopback, basic IPC plumbing) and ping test.

## POSIX Layer & Libc
22) POSIX syscall translation (minimal set) to server messages.
23) libc subset: open/read/write/close/exit/fork/exec (exec stub initially).
24) FD table, poll/select mapped to ports and notifications.
25) Signals via exception ports and basic handlers.

## Tooling & Release
26) MIG .defs parser (reference-only) that generates Rust stubs; integrate with xtask.
27) disk-image --with-sysroot loopback population (Linux CI step), signed checksums.
28) Release pipeline: versioned artifacts, MANIFEST.json, change log generation.

## Testing & CI
29) Property tests for IPC rights and message marshalling.
30) QEMU boot smoke tests on CI (headless) with captured serial logs.
