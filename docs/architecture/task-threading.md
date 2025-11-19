# Task and Thread Architecture

*In the spirit of Lions' Commentary: Understanding the separation of resources from execution*

## The Fundamental Separation

Traditional UNIX conflates two concepts in the "process":
- Resource container (address space, file descriptors)
- Execution context (program counter, registers, stack)

Mach separates these:

```
┌─────────────────────────────────────────┐
│  Task (Resource Container)              │
│  ┌───────────────────────────────────┐  │
│  │  Virtual Address Space            │  │
│  │  Port Rights                      │  │
│  │  Memory Regions                   │  │
│  └───────────────────────────────────┘  │
│                                         │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐│
│  │Thread 1 │  │Thread 2 │  │Thread 3 ││
│  │ PC: ... │  │ PC: ... │  │ PC: ... ││
│  │ SP: ... │  │ SP: ... │  │ SP: ... ││
│  └─────────┘  └─────────┘  └─────────┘│
└─────────────────────────────────────────┘
```

Why separate?
- **Flexibility**: Multiple threads can execute in one address space
- **Efficiency**: Thread creation is cheaper than process creation
- **Simplicity**: Each concept has a single, clear responsibility

## Tasks - The Resource Abstraction

A task *owns resources*. It does not execute code.

### Task Structure

```rust
/// A Mach task.
///
/// Tasks own resources but do not execute. Threads execute within tasks.
///
/// Resources owned by a task:
/// - Virtual memory map (address space)
/// - Port rights (capabilities)
/// - Threads (execution contexts)
///
/// Tasks form a hierarchy: each task has a parent.
/// When a task terminates, all its threads are killed and resources freed.
///
pub struct Task {
    /// Unique task identifier
    id: TaskId,

    /// Parent task (or None for root task)
    parent: Option<TaskId>,

    /// Virtual memory map
    vm_map: Arc<Mutex<VmMap>>,

    /// Port rights owned by this task
    port_space: Arc<Mutex<PortSpace>>,

    /// Threads executing in this task
    threads: Arc<Mutex<Vec<Arc<Thread>>>>,

    /// Task state
    state: AtomicU32,  // Running, Suspended, Terminated
}
```

### Task Creation

```rust
impl Task {
    /// Creates a new task.
    ///
    /// The new task:
    /// - Has an empty address space
    /// - Has no threads (create explicitly)
    /// - Has no port rights except its own task port
    /// - Is a child of the creating task
    ///
    /// # Returns
    ///
    /// Arc-wrapped task, ready for thread creation.
    ///
    pub fn new(parent_id: TaskId) -> Arc<Self> {
        let task_id = TaskId::generate();

        // Create VM map with fresh page tables
        let vm_map = VmMap::new();

        // Create port space
        let mut port_space = PortSpace::new();

        // Every task gets a task port (for control operations)
        let task_port = Port::new(task_id);
        port_space.insert_receive_right(TASK_PORT_NAME, task_port);

        Arc::new(Task {
            id: task_id,
            parent: Some(parent_id),
            vm_map: Arc::new(Mutex::new(vm_map)),
            port_space: Arc::new(Mutex::new(port_space)),
            threads: Arc::new(Mutex::new(Vec::new())),
            state: AtomicU32::new(TaskState::Running as u32),
        })
    }
}
```

### Port Namespace

Each task has its own port namespace:

```
Task A's port space:      Task B's port space:
┌─────────────────────┐   ┌─────────────────────┐
│ Name  │  Port       │   │ Name  │  Port       │
├───────┼─────────────┤   ├───────┼─────────────┤
│  1    │  Port X     │   │  1    │  Port Y     │
│  2    │  Port Y     │   │  2    │  Port Z     │
│  3    │  Port Z     │   │  5    │  Port X     │
└─────────────────────┘   └─────────────────────┘
```

Note: The same port can have different names in different tasks. Names are local to each task.

```rust
/// Port namespace for a task.
///
/// Maps port names (integers) to actual ports.
/// Port names are local to each task.
///
pub struct PortSpace {
    /// Name -> Port mapping
    rights: HashMap<PortName, PortRight>,

    /// Next available name
    next_name: PortName,
}

pub enum PortRight {
    /// Receive right (only one per port)
    Receive(Arc<Port>),

    /// Send right (can be duplicated)
    Send(Arc<Port>),

    /// Send-once right (consumed on use)
    SendOnce(Arc<Port>),
}

impl PortSpace {
    /// Inserts a port right with a specific name.
    pub fn insert_receive_right(&mut self, name: PortName, port: Arc<Port>) {
        self.rights.insert(name, PortRight::Receive(port));
    }

    /// Allocates a new name and inserts a send right.
    pub fn insert_send_right(&mut self, port: Arc<Port>) -> PortName {
        let name = self.next_name;
        self.next_name = self.next_name.next();
        self.rights.insert(name, PortRight::Send(port));
        name
    }

    /// Looks up a port by name.
    pub fn lookup(&self, name: PortName) -> Option<&PortRight> {
        self.rights.get(&name)
    }
}
```

## Threads - The Execution Abstraction

A thread *executes code*. It does not own resources (except its stack).

### Thread Structure

```rust
/// A Mach thread.
///
/// Threads are the unit of CPU scheduling. Each thread:
/// - Executes within exactly one task
/// - Has its own stack and registers
/// - Shares the task's address space and ports
/// - Can be suspended and resumed
///
pub struct Thread {
    /// Unique thread identifier
    id: ThreadId,

    /// Task this thread executes in
    task: TaskId,

    /// CPU context (registers, PC, SP)
    context: Mutex<CpuContext>,

    /// Thread state
    state: AtomicU32,  // Running, Ready, Blocked, Suspended

    /// Scheduling priority (0 = lowest, 31 = highest)
    priority: AtomicU8,

    /// CPU this thread last ran on (for affinity)
    last_cpu: AtomicU8,
}

/// CPU context saved during context switch.
///
/// Architecture-specific. On AArch64:
#[repr(C)]
pub struct CpuContext {
    // General-purpose registers
    x0:  u64,   // also return value
    x1:  u64,
    // ... x2 through x29
    x30: u64,   // link register (LR)

    // Special registers
    sp:  u64,   // stack pointer
    pc:  u64,   // program counter

    // Processor state
    pstate: u64,
}
```

### Thread States

A thread progresses through states:

```
       create()
          │
          ▼
      ┌─────────┐
      │  Ready  │◄───────┐
      └────┬────┘        │
           │ schedule()  │
           ▼             │
      ┌─────────┐        │
      │ Running │────────┘ yield()
      └────┬────┘
           │
           ├──► Blocked (wait for event)
           │      │
           │      └──► Ready (event occurs)
           │
           └──► Terminated (exit)
```

State transitions:

```rust
pub enum ThreadState {
    /// Thread is ready to run (in run queue)
    Ready,

    /// Thread is currently executing on a CPU
    Running,

    /// Thread is waiting for an event (I/O, IPC, etc.)
    Blocked,

    /// Thread has been suspended by debugger/user
    Suspended,

    /// Thread has exited
    Terminated,
}

impl Thread {
    /// Transitions thread from Running to Ready.
    ///
    /// Called when thread's time slice expires or it yields CPU.
    pub fn preempt(&self) {
        let old_state = self.state.swap(
            ThreadState::Ready as u32,
            Ordering::SeqCst
        );

        debug_assert_eq!(old_state, ThreadState::Running as u32,
            "Can only preempt running thread");

        // Add back to run queue
        SCHEDULER.enqueue(self);
    }

    /// Transitions thread from Running to Blocked.
    ///
    /// Called when thread waits for IPC, I/O, etc.
    pub fn block(&self, wait_queue: &WaitQueue) {
        let old_state = self.state.swap(
            ThreadState::Blocked as u32,
            Ordering::SeqCst
        );

        debug_assert_eq!(old_state, ThreadState::Running as u32,
            "Can only block running thread");

        // Add to wait queue
        wait_queue.add(self);

        // Yield CPU
        SCHEDULER.reschedule();
    }

    /// Transitions thread from Blocked to Ready.
    ///
    /// Called when event thread was waiting for occurs.
    pub fn wakeup(&self) {
        let old_state = self.state.swap(
            ThreadState::Ready as u32,
            Ordering::SeqCst
        );

        debug_assert_eq!(old_state, ThreadState::Blocked as u32,
            "Can only wake blocked thread");

        // Add to run queue
        SCHEDULER.enqueue(self);
    }
}
```

## Context Switching - Moving Between Threads

When the scheduler decides to run a different thread, we perform a *context switch*:

```
CPU was running Thread A         CPU now running Thread B
┌─────────────────────┐         ┌─────────────────────┐
│ Registers:          │         │ Registers:          │
│  PC = 0x1234        │         │  PC = 0x5678        │
│  SP = 0xFFFF0000    │         │  SP = 0xFFFF8000    │
│  x0 = 42            │         │  x0 = 99            │
│  ...                │         │  ...                │
└─────────────────────┘         └─────────────────────┘
```

The switch:

```rust
/// Performs a context switch from old_thread to new_thread.
///
/// # Arguments
///
/// * `old_thread` - Currently running thread (or None if idle)
/// * `new_thread` - Thread to switch to
///
/// # Implementation
///
/// This is one of the most critical operations in the kernel.
/// Must be written in assembly for precise control of registers.
///
/// Steps:
/// 1. Save old thread's registers to old_thread.context
/// 2. Load new thread's registers from new_thread.context
/// 3. Switch stack pointer
/// 4. Switch page tables (if different task)
/// 5. Return (now executing new_thread's code)
///
#[naked]
#[no_mangle]
pub unsafe extern "C" fn switch_context(
    old_context: *mut CpuContext,
    new_context: *const CpuContext,
) {
    asm!(
        // Save old context
        "stp x0,  x1,  [x0, #0]",    // Save x0, x1
        "stp x2,  x3,  [x0, #16]",   // Save x2, x3
        // ... save all registers

        "mov x2, sp",
        "str x2, [x0, #CTX_SP]",     // Save stack pointer

        "adr x2, 1f",
        "str x2, [x0, #CTX_PC]",     // Save return address

        // Load new context
        "ldp x0,  x1,  [x1, #0]",    // Load x0, x1
        "ldp x2,  x3,  [x1, #16]",   // Load x2, x3
        // ... load all registers

        "ldr x2, [x1, #CTX_SP]",
        "mov sp, x2",                // Load stack pointer

        "ldr x2, [x1, #CTX_PC]",
        "br x2",                     // Jump to new PC

        "1:",                        // Return point
        "ret",
        options(noreturn)
    );
}
```

Note: This is simplified. Real implementation must handle:
- Floating point registers
- System registers
- Page table switching
- TLB flushing

## Thread Scheduling

The scheduler decides which thread runs next.

### Run Queue

Threads are organized by priority:

```
Priority Level        Threads
──────────────────────────────────────
31 (highest)          [Thread A]
30                    []
...                   ...
10                    [Thread B, Thread C, Thread D]
9                     [Thread E]
...                   ...
0 (lowest)            [Thread F, Thread G]
```

Scheduling algorithm:

```rust
/// Simple priority-based scheduler.
///
/// Algorithm:
/// 1. Find highest priority non-empty queue
/// 2. Take first thread from that queue (FIFO within priority)
/// 3. Run thread for its time slice
/// 4. When time slice expires, re-queue thread
///
pub struct Scheduler {
    /// Run queues, one per priority level
    run_queues: [Mutex<VecDeque<Arc<Thread>>>; 32],

    /// Currently running thread on each CPU
    current_threads: [AtomicPtr<Thread>; MAX_CPUS],
}

impl Scheduler {
    /// Selects next thread to run.
    ///
    /// Returns: Thread to run, or None if all idle.
    pub fn schedule(&self) -> Option<Arc<Thread>> {
        // Search from highest to lowest priority
        for priority in (0..32).rev() {
            let mut queue = self.run_queues[priority].lock();
            if let Some(thread) = queue.pop_front() {
                return Some(thread);
            }
        }

        None  // All queues empty
    }

    /// Adds thread to run queue.
    pub fn enqueue(&self, thread: &Arc<Thread>) {
        let priority = thread.priority.load(Ordering::Relaxed);
        let mut queue = self.run_queues[priority as usize].lock();
        queue.push_back(Arc::clone(thread));
    }

    /// Yields CPU to next thread.
    pub fn reschedule(&self) {
        let cpu_id = current_cpu_id();
        let current = self.current_threads[cpu_id]
            .swap(core::ptr::null_mut(), Ordering::SeqCst);

        if !current.is_null() {
            // Save current thread
            let current_thread = unsafe { &*current };
            if current_thread.state() == ThreadState::Running {
                current_thread.preempt();  // Move to Ready
            }
        }

        // Get next thread
        if let Some(next_thread) = self.schedule() {
            next_thread.state.store(
                ThreadState::Running as u32,
                Ordering::SeqCst
            );

            self.current_threads[cpu_id].store(
                Arc::into_raw(next_thread) as *mut _,
                Ordering::SeqCst
            );

            // Perform context switch
            unsafe {
                if !current.is_null() {
                    let old_ctx = &mut (*current).context.lock();
                    let new_ctx = &next_thread.context.lock();
                    switch_context(old_ctx, new_ctx);
                } else {
                    // First thread on this CPU
                    let new_ctx = &next_thread.context.lock();
                    load_context(new_ctx);
                }
            }
        } else {
            // No threads to run - idle
            wait_for_interrupt();
        }
    }
}
```

## Example: Creating and Running a Thread

```rust
// Create a task
let task = Task::new(parent_task_id);

// Set up initial memory (stack, code)
let mut vm_map = task.vm_map.lock();
vm_map.allocate_stack(STACK_SIZE)?;
vm_map.map_code(code_addr, code_size)?;
drop(vm_map);

// Create a thread
let thread = Thread::new(task.id, thread_func, arg);

// Set up thread's initial state
{
    let mut context = thread.context.lock();

    // Entry point
    context.pc = thread_func as u64;

    // Stack (grows down from top)
    context.sp = stack_top;

    // First argument
    context.x0 = arg as u64;

    // Processor state: user mode, interrupts enabled
    context.pstate = PSTATE_USER | PSTATE_IRQ_ENABLED;
}

// Add thread to task
task.add_thread(thread.clone());

// Make thread runnable
thread.state.store(ThreadState::Ready as u32, Ordering::SeqCst);
SCHEDULER.enqueue(&thread);

// Thread will run when scheduler picks it
```

## The Task-Thread Duality

Consider these operations and what they affect:

| Operation | Affects Task | Affects Thread |
|-----------|-------------|---------------|
| `vm_allocate()` | ✓ Memory map | ✗ |
| `port_allocate()` | ✓ Port space | ✗ |
| `thread_create()` | ✓ Thread list | ✓ New thread |
| `thread_suspend()` | ✗ | ✓ Thread state |
| `task_terminate()` | ✓ All resources | ✓ All threads |

The separation is clean: tasks own, threads execute.

## Inter-Thread Synchronization

Threads in the same task share memory. How do they synchronize?

### Via Ports (Preferred)

```rust
// Thread A: Send message
let msg = Message::new(b"data");
port.send(msg)?;

// Thread B: Receive message
let msg = port.receive()?;
```

Advantage: Same mechanism as inter-task communication.

### Via Shared Memory (When Necessary)

```rust
// Shared counter protected by mutex
static COUNTER: Mutex<u64> = Mutex::new(0);

// Thread A
{
    let mut count = COUNTER.lock();
    *count += 1;
}  // Lock released

// Thread B
{
    let count = COUNTER.lock();
    println!("Count: {}", *count);
}
```

## Summary

The task/thread separation provides:

- **Clarity**: Resources and execution are distinct concepts
- **Efficiency**: Threads are lightweight (no separate address space)
- **Flexibility**: Multiple threads can cooperate in one address space
- **Simplicity**: Each abstraction does one thing well

This separation is one of Mach's key innovations, now standard in modern operating systems (POSIX threads, Win32 threads, etc.).

---

**See Also:**
- [Memory Management](memory-management.md) - How tasks use memory
- [IPC System](ipc-system.md) - How threads communicate
- [Overview](overview.md) - High-level architecture
