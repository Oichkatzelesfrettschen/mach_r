# Mach_R Granular Implementation Plan

> Prioritized, actionable tasks to transform Mach_R from a structured codebase into a bootable microkernel.

---

## Quick Reference

| Phase | Focus | Est. Functions | Priority |
|-------|-------|----------------|----------|
| 0 | Build Infrastructure | 5 | CRITICAL |
| 1 | IPC Core Completion | 15 | CRITICAL |
| 2 | Scheduler Integration | 12 | CRITICAL |
| 3 | Syscall Layer | 10 | HIGH |
| 4 | VM Completion | 18 | HIGH |
| 5 | Boot Integration | 8 | HIGH |
| 6 | Device Layer | 10 | MEDIUM |
| 7 | DDB Debugger | 15 | MEDIUM |

---

## Phase 0: Build Infrastructure (CRITICAL)

### 0.1 Fix Test Environment
The no_std test harness crashes due to conflicting libc symbols and race conditions.

- [x] **0.1.1** Disable conflicting `#[no_mangle]` libc functions in test mode
  - **COMPLETED**: Added `#[cfg(not(test))]` to custom malloc/free/abort/realloc
  - Files: `src/libc/stdlib.rs` - malloc, free, abort, realloc_impl
  - Files: `src/libc/string.rs` - all memcpy/memmove/memset/memcmp/str* functions
  - Root cause: Custom libc functions conflicted with system libc used by test harness
  - Tests now pass with `--test-threads=1`

- [ ] **0.1.2** Fix parallel test execution race conditions
  - Issue: Tests hang when run in parallel due to `spin::Once` initializer conflicts
  - Files: Multiple modules use `spin::Once` for lazy initialization
  - Workaround: Use `--test-threads=1` for now

- [ ] **0.1.3** Add integration test harness using QEMU
  - Create: `tests/integration/` directory
  - Use `cargo xtask test-qemu` for kernel-mode tests

### 0.2 CI/CD Pipeline
- [ ] **0.2.1** Add GitHub Actions workflow for `cargo xtask check`
- [ ] **0.2.2** Add QEMU smoke test in CI

---

## Phase 1: IPC Core Completion (CRITICAL)

The IPC subsystem is 70% complete. These gaps block all higher-level functionality.

### 1.1 Complex Message Processing
Currently stubbed at `src/ipc/mach_msg.rs:733-743, 894-901`

- [ ] **1.1.1** Implement `process_complex_send()` - parse message body descriptors
  - Parse `MACH_MSG_PORT_DESCRIPTOR` entries
  - Parse `MACH_MSG_OOL_DESCRIPTOR` entries
  - Parse `MACH_MSG_OOL_PORTS_DESCRIPTOR` entries
  - File: `src/ipc/mach_msg.rs` lines 733-743

- [ ] **1.1.2** Implement `process_complex_receive()` - reconstruct descriptors for receiver
  - Copyout port rights to receiver's space
  - Map OOL memory into receiver's address space
  - File: `src/ipc/mach_msg.rs` lines 894-901

- [ ] **1.1.3** Implement port descriptor copyin
  - File: `src/ipc/kmsg.rs`
  - Extract port rights from sender's space
  - Validate disposition (send, receive, send-once)

- [ ] **1.1.4** Implement port descriptor copyout
  - Insert rights into receiver's space
  - Handle name collision

### 1.2 Out-of-Line (OOL) Data
Critical for large data transfers (VM operations, file I/O)

- [ ] **1.2.1** Implement OOL memory region copyin
  - Allocate kernel buffer or map sender's pages
  - Handle `MACH_MSG_VIRTUAL_COPY` vs `MACH_MSG_PHYSICAL_COPY`
  - File: `src/ipc/kmsg.rs`

- [ ] **1.2.2** Implement OOL memory region copyout
  - Map into receiver's address space
  - Handle deallocation semantics

- [ ] **1.2.3** Implement OOL ports array handling
  - Array of port rights in OOL memory
  - Copyin/copyout for port arrays

### 1.3 Port Operations Completion
- [ ] **1.3.1** Implement `mach_port_insert_right()`
  - File: `src/ipc/port_ops.rs`
  - Insert a right into a space with specific name

- [ ] **1.3.2** Implement `mach_port_extract_right()`
  - Remove right from space, return disposition

- [ ] **1.3.3** Complete port set operations
  - `mach_port_move_member()` - add/remove port from set
  - `mach_port_get_set_status()` - list ports in set
  - File: `src/ipc/pset.rs`

### 1.4 Notification System
- [ ] **1.4.1** Implement dead-name notification delivery
  - When port dies, notify registered watchers
  - File: `src/ipc/notify.rs`

- [ ] **1.4.2** Implement no-senders notification
  - When last send right released, notify receiver

- [ ] **1.4.3** Implement port-destroyed notification
  - When receive right deallocated

---

## Phase 2: Scheduler Integration (CRITICAL)

The scheduler works in isolation but isn't connected to real execution.

### 2.1 Context Switch Assembly
- [ ] **2.1.1** Complete ARM64 context switch
  - File: `src/scheduler.rs:330` (currently stub)
  - Save/restore x0-x30, sp, pc, PSTATE
  - Handle floating-point state (v0-v31)

- [ ] **2.1.2** Verify x86_64 context switch
  - File: `src/scheduler.rs:263-310`
  - Test actual register save/restore
  - Handle FPU/SSE state

### 2.2 Timer Integration
- [ ] **2.2.1** Connect timer interrupt to scheduler
  - File: `src/drivers/interrupt.rs`
  - Call `scheduler.tick()` from timer handler
  - Implement quantum expiration

- [ ] **2.2.2** Implement preemption
  - Set `need_resched` flag on quantum expire
  - Check flag on return from interrupt

### 2.3 Thread State Management
- [ ] **2.3.1** Implement `thread_block()` with continuation
  - File: `src/kern/sched_prim.rs`
  - Save continuation function pointer
  - Switch to next thread without saving full context

- [ ] **2.3.2** Implement `thread_wakeup_prim()`
  - Wake threads waiting on event
  - Handle priority boost for woken thread

- [ ] **2.3.3** Integrate wait queues with IPC blocking
  - When `mach_msg_receive()` blocks, add to wait queue
  - When message arrives, wake receiver

### 2.4 Idle Thread
- [ ] **2.4.1** Create per-CPU idle thread
  - File: `src/kern/processor.rs`
  - Idle loop with `wfi` (ARM) or `hlt` (x86)

- [ ] **2.4.2** Handle idle → running transition
  - Wake idle thread when work available

---

## Phase 3: Syscall Layer (HIGH)

Connect user-space to kernel via trap interface.

### 3.1 Trap Entry/Exit
- [ ] **3.1.1** Implement x86_64 syscall entry
  - File: `src/arch/x86_64/` (new: syscall.rs)
  - SYSCALL instruction handler
  - Save user registers, switch to kernel stack

- [ ] **3.1.2** Implement ARM64 svc entry
  - File: `src/arch/aarch64/` (new: syscall.rs)
  - SVC #0 handler
  - Exception level transition

### 3.2 Mach Trap Table
- [ ] **3.2.1** Implement trap dispatcher
  - File: `src/kern/syscall_sw.rs:243-346` (currently stubs)
  - Dispatch based on trap number
  - Call appropriate kernel function

- [ ] **3.2.2** Implement core traps:
  - `mach_msg_trap` (-25) → `mach_msg()`
  - `mach_reply_port` (-26) → allocate reply port
  - `thread_self_trap` (-27) → return thread port
  - `task_self_trap` (-28) → return task port
  - `mach_port_allocate` (3000+) → port allocation

### 3.3 Copyin/Copyout
- [ ] **3.3.1** Implement `copyin()` - user → kernel
  - Validate user pointer
  - Copy with page fault handling

- [ ] **3.3.2** Implement `copyout()` - kernel → user
  - Validate user buffer
  - Copy with COW handling

---

## Phase 4: VM Completion (HIGH)

### 4.1 Page Fault Handling
- [ ] **4.1.1** Complete `vm_fault()` implementation
  - File: `src/mach_vm/vm_fault.rs:212-272`
  - Handle read faults (demand page-in)
  - Handle write faults (COW, zero-fill)

- [ ] **4.1.2** Implement fault page lookup
  - Search shadow chain for page
  - Request from memory object if not found

- [ ] **4.1.3** Implement COW page copy
  - Allocate new page
  - Copy contents
  - Update page table

### 4.2 External Pager Interface
- [ ] **4.2.1** Implement `memory_object_data_request()`
  - File: `src/mach_vm/memory_object.rs`
  - Send request to pager via IPC
  - Block thread until data arrives

- [ ] **4.2.2** Implement `memory_object_data_supply()`
  - Receive page data from pager
  - Insert into VM object
  - Wake waiting threads

- [ ] **4.2.3** Implement `memory_object_data_return()`
  - Send dirty pages back to pager
  - Clean page after successful return

### 4.3 Page Replacement
- [ ] **4.3.1** Implement page scanning
  - File: `src/mach_vm/vm_pageout.rs`
  - LRU approximation using reference bits
  - Identify candidates for eviction

- [ ] **4.3.2** Implement pageout
  - Write dirty pages to pager
  - Free clean pages

### 4.4 Physical Map (pmap)
- [ ] **4.4.1** Implement x86_64 pmap
  - File: `src/arch/x86_64/pmap.rs` (new)
  - 4-level page tables
  - `pmap_enter()`, `pmap_remove()`, `pmap_protect()`

- [ ] **4.4.2** Implement ARM64 pmap
  - File: `src/arch/aarch64/pmap.rs` (new)
  - 4-level page tables with granule support
  - TLB invalidation

---

## Phase 5: Boot Integration (HIGH)

### 5.1 Kernel Initialization Sequence
- [ ] **5.1.1** Define init order
  - File: `src/main.rs` or `src/boot/mod.rs`
  - 1. Console init
  - 2. Memory init (physical allocator, heap)
  - 3. VM init (kernel pmap, zones)
  - 4. IPC init (kernel space, ports)
  - 5. Scheduler init (idle thread, timer)
  - 6. First user task

- [ ] **5.1.2** Create kernel bootstrap task
  - Kernel task with IPC space
  - Special ports (host, host_priv)

### 5.2 First User Process
- [ ] **5.2.1** Load init binary
  - ELF loader for user-space init
  - Map code/data segments

- [ ] **5.2.2** Create init task and thread
  - User address space
  - Initial thread state
  - Entry point setup

- [ ] **5.2.3** Transfer to user mode
  - Switch to user page tables
  - ERET (ARM) or IRETQ (x86)

---

## Phase 6: Device Layer (MEDIUM)

### 6.1 Console Device
- [ ] **6.1.1** Implement console read/write
  - File: `src/device/` (new: cons.rs)
  - UART for serial output
  - Keyboard input

### 6.2 Device Framework
- [ ] **6.2.1** Complete `device_open()`
  - File: `src/device/ds_routines.rs`
  - Device lookup and reference

- [ ] **6.2.2** Implement `device_read()` / `device_write()`
  - Async I/O with completion

- [ ] **6.2.3** Implement `device_map()`
  - Map device memory into task

---

## Phase 7: DDB Debugger (MEDIUM)

### 7.1 Debugger Core
- [ ] **7.1.1** Implement breakpoint trap handler
  - Enter debugger on INT3 (x86) or BRK (ARM)

- [ ] **7.1.2** Implement command parser
  - Basic commands: examine, print, continue, step

### 7.2 Inspection Commands
- [ ] **7.2.1** Memory examination (`x/` command)
- [ ] **7.2.2** Register display
- [ ] **7.2.3** Thread/task listing
- [ ] **7.2.4** Port/IPC inspection

---

## Implementation Order Summary

```
Week 1-2:   Phase 0 (Build) + Phase 1.1-1.2 (IPC Complex Msgs)
Week 3-4:   Phase 1.3-1.4 (IPC Ports/Notify) + Phase 2.1 (Context Switch)
Week 5-6:   Phase 2.2-2.4 (Scheduler) + Phase 3.1-3.2 (Syscalls)
Week 7-8:   Phase 3.3 (Copyin) + Phase 4.1-4.2 (VM Faults)
Week 9-10:  Phase 4.3-4.4 (Paging) + Phase 5 (Boot)
Week 11-12: Phase 6 (Devices) + Phase 7 (DDB)
```

---

## Files to Create

| File | Purpose |
|------|---------|
| `src/arch/x86_64/syscall.rs` | Syscall entry/exit |
| `src/arch/x86_64/pmap.rs` | x86_64 page tables |
| `src/arch/aarch64/syscall.rs` | SVC handling |
| `src/arch/aarch64/pmap.rs` | ARM64 page tables |
| `src/device/cons.rs` | Console device |
| `src/ddb/mod.rs` | Kernel debugger |
| `tests/integration/boot.rs` | QEMU boot test |

---

## Verification Milestones

- [ ] **M1**: `cargo test --lib` passes (Phase 0)
- [ ] **M2**: IPC message with port descriptor works (Phase 1)
- [ ] **M3**: Two threads context switch correctly (Phase 2)
- [ ] **M4**: User-space syscall reaches kernel (Phase 3)
- [ ] **M5**: Page fault triggers and resolves (Phase 4)
- [ ] **M6**: Kernel boots to shell prompt (Phase 5+6)
