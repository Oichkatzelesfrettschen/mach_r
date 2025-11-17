# Tasks and Threads

Mach provides a two-level abstraction for execution: tasks and threads.

## Tasks

A **task** is the unit of resource ownership. It is an environment or container that holds the resources an application needs to run. A task itself does not execute any code; it is a passive entity.

Key resources owned by a task include:

- **A Virtual Address Space**: A complete, private view of memory.
- **A Port Namespace**: The set of port rights (capabilities) the task holds.
- **Threads**: One or more threads that execute within the task's context.

```rust
// Conceptual structure of a Task
struct Task {
    task_id: TaskId,
    address_space: AddressSpace,
    threads: Vec<Arc<Thread>>,
    ports: PortNameSpace, // The task's collection of port rights
    stats: TaskStats,
}
```

## Threads

A **thread** is the basic unit of execution. It is an active entity that runs within the context of a single task. Each thread has its own:

- **Processor State**: CPU registers, program counter, stack pointer.
- **Scheduling State**: Priority, run state (e.g., running, ready, blocked).
- **Exception Handler Ports**: A set of ports to which exception notifications can be sent.

Multiple threads can exist within a single task, sharing the task's address space and port namespace. This allows for concurrent execution within a single application.

## The Scheduler

The scheduler is responsible for deciding which thread gets to run on a CPU at any given time. The Mach_R implementation will start with a simple, preemptive, priority-based scheduler.

- **Run Queues**: The scheduler will maintain a set of run queues, one for each priority level.
- **Preemption**: When a higher-priority thread becomes ready to run, it will preempt any lower-priority thread currently running.
- **Round-Robin**: Within a single priority level, threads will be scheduled in a round-robin fashion to ensure fairness.
- **Blocking**: When a thread needs to wait for an event (e.g., receiving a message), it is put into a blocked state and removed from the run queue until the event occurs.
