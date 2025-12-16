//! Task management for Mach_R
//!
//! Tasks are the basic unit of resource allocation in Mach.
//! A task owns ports, virtual memory, and threads.
//!
//! Based on Mach4 kern/task.h
//! Key structures:
//! - IPC space (itk_space) for port name translation
//! - Task ports (itk_self, itk_sself) for task control
//! - Reference counting for lifecycle management
//! - Processor set assignment for scheduling

use crate::port::Port;
use crate::types::{TaskId, ThreadId};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use spin::{Mutex, Once};

use crate::ipc::{space::IpcSpace, PortName};
use crate::kern::processor::ProcessorSetId;
use crate::kern::timer::MachTimeValue;
use crate::paging::PageTable;

/// CPU context for thread switching - ARM64 variant
#[cfg(target_arch = "aarch64")]
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Context {
    // ARM64 general-purpose registers
    pub x0: u64,
    pub x1: u64,
    pub x2: u64,
    pub x3: u64,
    pub x4: u64,
    pub x5: u64,
    pub x6: u64,
    pub x7: u64,
    pub x8: u64,
    pub x9: u64,
    pub x10: u64,
    pub x11: u64,
    pub x12: u64,
    pub x13: u64,
    pub x14: u64,
    pub x15: u64,
    pub x16: u64,
    pub x17: u64,
    pub x18: u64,
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub x28: u64,
    pub x29: u64,    // Frame pointer
    pub x30: u64,    // Link register
    pub sp: u64,     // Stack pointer
    pub pc: u64,     // Program counter
    pub pstate: u64, // Processor state
}

/// CPU context for thread switching - x86_64 variant
#[cfg(target_arch = "x86_64")]
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Context {
    // x86_64 callee-saved registers (per System V ABI)
    pub rbx: u64,
    pub rbp: u64,    // Frame pointer
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rsp: u64,    // Stack pointer
    pub rip: u64,    // Instruction pointer (return address)
    pub rflags: u64, // Flags register
}

/// CPU context for thread switching - generic fallback
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Context {
    /// Generic registers (placeholder for unsupported architectures)
    pub regs: [u64; 32],
    pub sp: u64,
    pub pc: u64,
    pub flags: u64,
}

impl Context {
    /// Create a new zeroed context
    #[cfg(target_arch = "aarch64")]
    pub fn new() -> Self {
        Self {
            x0: 0,
            x1: 0,
            x2: 0,
            x3: 0,
            x4: 0,
            x5: 0,
            x6: 0,
            x7: 0,
            x8: 0,
            x9: 0,
            x10: 0,
            x11: 0,
            x12: 0,
            x13: 0,
            x14: 0,
            x15: 0,
            x16: 0,
            x17: 0,
            x18: 0,
            x19: 0,
            x20: 0,
            x21: 0,
            x22: 0,
            x23: 0,
            x24: 0,
            x25: 0,
            x26: 0,
            x27: 0,
            x28: 0,
            x29: 0,
            x30: 0,
            sp: 0,
            pc: 0,
            pstate: 0,
        }
    }

    /// Create a new zeroed context
    #[cfg(target_arch = "x86_64")]
    pub fn new() -> Self {
        Self {
            rbx: 0,
            rbp: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rsp: 0,
            rip: 0,
            rflags: 0,
        }
    }

    /// Create a new zeroed context
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    pub fn new() -> Self {
        Self {
            regs: [0; 32],
            sp: 0,
            pc: 0,
            flags: 0,
        }
    }
}

/// Task state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskState {
    /// Task is running
    Running,
    /// Task is suspended
    Suspended,
    /// Task is terminated
    Terminated,
    /// Task has exited but not yet reaped
    Zombie,
}

/// A thread - the unit of execution, managed by a Task
pub struct TaskThread {
    pub id: ThreadId,
    pub task: TaskId,
    pub state: ThreadState,
    pub priority: Priority,
    pub context: Context,
    pub kernel_stack: usize, // Base address of the kernel stack
    pub user_stack: usize,   // Base address of the user stack (if applicable)
    pub name: String,
}

impl TaskThread {
    pub fn new(task: TaskId, entry: usize, stack: usize, name: String) -> Self {
        let id = ThreadId::new();
        let mut context = Context::new();
        context.pc = entry as u64;
        context.sp = stack as u64;

        Self {
            id,
            task,
            state: ThreadState::Ready,
            priority: Priority::DEFAULT,
            context,
            kernel_stack: stack,
            user_stack: 0, // Placeholder
            name,
        }
    }

    pub fn block(&mut self) {
        self.state = ThreadState::Blocked;
    }

    pub fn unblock(&mut self) {
        if self.state == ThreadState::Blocked {
            self.state = ThreadState::Ready;
        }
    }

    pub fn suspend(&mut self) {
        self.state = ThreadState::Suspended;
    }

    pub fn resume(&mut self) {
        if self.state == ThreadState::Suspended {
            self.state = ThreadState::Ready;
        }
    }
}

/// Thread state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThreadState {
    /// Thread is running
    Running,
    /// Thread is ready to run
    Ready,
    /// Thread is blocked
    Blocked,
    /// Thread is suspended
    Suspended,
    /// Thread is terminated
    Terminated,
}

/// Thread priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(pub u8);

impl Priority {
    pub const IDLE: Self = Self(0);
    pub const LOW: Self = Self(31);
    pub const DEFAULT: Self = Self(63);
    pub const HIGH: Self = Self(95);
    pub const REALTIME: Self = Self(127);

    pub fn new(val: u8) -> Self {
        Self(val.min(127))
    }
}

/// Maximum registered ports per task (from Mach4 TASK_PORT_REGISTER_MAX)
pub const TASK_PORT_REGISTER_MAX: usize = 4;

/// Task structure - the basic unit of resource allocation
///
/// Based on Mach4 kern/task.h struct task
pub struct Task {
    // ========================================================================
    // Synchronization/destruction information
    // ========================================================================
    /// Task identifier
    pub id: TaskId,

    /// Reference count for lifecycle management
    ref_count: AtomicU32,

    /// Is task active (not terminated)?
    active: Mutex<bool>,

    /// Task state
    pub state: Mutex<TaskState>,

    /// Task name
    pub name: String,

    // ========================================================================
    // Virtual Memory
    // ========================================================================
    /// Address space (page table)
    pub page_table: Box<PageTable>,

    // TODO: vm_map when VM subsystem is complete
    // pub map: Option<Arc<VmMap>>,

    // ========================================================================
    // Thread information
    // ========================================================================
    /// List of threads in this task
    pub threads: Mutex<Vec<ThreadId>>,

    /// Number of threads (cached for fast access)
    thread_count: AtomicU32,

    /// Internal suspend count (for scheduling)
    suspend_count: AtomicI32,

    /// User-visible suspend count
    user_stop_count: AtomicI32,

    /// Default priority for new threads
    priority: AtomicU32,

    // ========================================================================
    // Processor Set
    // ========================================================================
    /// Processor set for new threads
    processor_set: Mutex<Option<ProcessorSetId>>,

    /// Can assigned pset be changed?
    may_assign: Mutex<bool>,

    // ========================================================================
    // Statistics
    // ========================================================================
    /// Total user time for dead threads
    total_user_time: Mutex<MachTimeValue>,

    /// Total system time for dead threads
    total_system_time: Mutex<MachTimeValue>,

    // ========================================================================
    // IPC structures (from Mach4 itk_*)
    // ========================================================================
    /// Task's IPC space for port name translation
    pub itk_space: Arc<IpcSpace>,

    /// Task control port (not a right, doesn't hold ref)
    itk_self: Mutex<Option<PortName>>,

    /// Task control port (a send right)
    itk_sself: Mutex<Option<PortName>>,

    /// Exception port (a send right)
    itk_exception: Mutex<Option<PortName>>,

    /// Bootstrap port (a send right)
    pub itk_bootstrap: Mutex<Option<PortName>>,

    /// Registered ports (all send rights)
    itk_registered: Mutex<[Option<PortName>; TASK_PORT_REGISTER_MAX]>,

    // ========================================================================
    // Legacy fields (for compatibility)
    // ========================================================================
    /// Legacy port list
    pub ports: Mutex<Vec<PortName>>,

    /// Legacy bootstrap port
    pub bootstrap_port: PortName,

    /// Legacy exception ports array
    pub exception_ports: [PortName; 32],
}

/// Default task priority (from Mach4 BASEPRI_USER)
pub const BASEPRI_USER: u32 = 12;

impl Task {
    /// Create a new task
    ///
    /// Based on Mach4 task_create()
    pub fn new(name: String) -> Arc<Self> {
        let task_id = TaskId::new();
        let _task_port = Port::new(task_id); // This will create a Port and register it globally

        // Create IPC space for this task
        let itk_space = IpcSpace::new();

        Arc::new(Task {
            id: task_id,
            ref_count: AtomicU32::new(1),
            active: Mutex::new(true),
            state: Mutex::new(TaskState::Running),
            name,

            // Virtual memory
            page_table: Box::new(PageTable::new()),

            // Thread info
            threads: Mutex::new(Vec::new()),
            thread_count: AtomicU32::new(0),
            suspend_count: AtomicI32::new(0),
            user_stop_count: AtomicI32::new(0),
            priority: AtomicU32::new(BASEPRI_USER),

            // Processor set
            processor_set: Mutex::new(None),
            may_assign: Mutex::new(true),

            // Statistics
            total_user_time: Mutex::new(MachTimeValue::default()),
            total_system_time: Mutex::new(MachTimeValue::default()),

            // IPC (Mach4 itk_*)
            itk_space,
            itk_self: Mutex::new(None),
            itk_sself: Mutex::new(None),
            itk_exception: Mutex::new(None),
            itk_bootstrap: Mutex::new(None),
            itk_registered: Mutex::new([None; TASK_PORT_REGISTER_MAX]),

            // Legacy
            ports: Mutex::new(Vec::new()),
            bootstrap_port: PortName::NULL,
            exception_ports: [PortName::NULL; 32],
        })
    }

    // ========================================================================
    // Reference Counting (from Mach4 task_reference/task_deallocate)
    // ========================================================================

    /// Add a reference to this task
    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Remove a reference - returns true if task should be destroyed
    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    /// Get reference count
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::SeqCst)
    }

    // ========================================================================
    // Basic Accessors
    // ========================================================================

    pub fn id(&self) -> TaskId {
        self.id
    }

    pub fn is_active(&self) -> bool {
        *self.active.lock()
    }

    pub fn state(&self) -> TaskState {
        *self.state.lock()
    }

    pub fn set_state(&self, new_state: TaskState) {
        *self.state.lock() = new_state;
    }

    /// Get IPC space
    pub fn ipc_space(&self) -> &Arc<IpcSpace> {
        &self.itk_space
    }

    /// Get thread count
    pub fn thread_count(&self) -> u32 {
        self.thread_count.load(Ordering::SeqCst)
    }

    /// Get default priority for new threads
    pub fn priority(&self) -> u32 {
        self.priority.load(Ordering::SeqCst)
    }

    /// Set default priority for new threads
    pub fn set_priority(&self, priority: u32) {
        self.priority.store(priority, Ordering::SeqCst);
    }

    // ========================================================================
    // Suspend/Resume (from Mach4 task_suspend/task_resume)
    // ========================================================================

    /// Suspend the task (internal)
    ///
    /// Based on Mach4 task_hold()
    pub fn hold(&self) {
        self.suspend_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Release suspend (internal)
    ///
    /// Based on Mach4 task_release()
    pub fn release(&self) {
        let prev = self.suspend_count.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            // Task is no longer suspended, resume threads
            for &thread_id in self.threads.lock().iter() {
                crate::scheduler::wake_thread(thread_id);
            }
        }
    }

    /// User-visible suspend
    pub fn suspend(&self) {
        self.user_stop_count.fetch_add(1, Ordering::SeqCst);
        self.hold();
        *self.state.lock() = TaskState::Suspended;

        // Suspend all threads
        for &thread_id in self.threads.lock().iter() {
            crate::scheduler::global_scheduler().suspend_thread(thread_id);
        }
    }

    /// User-visible resume
    pub fn resume(&self) {
        let prev = self.user_stop_count.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            self.release();
            *self.state.lock() = TaskState::Running;

            // Resume all threads
            for &thread_id in self.threads.lock().iter() {
                crate::scheduler::global_scheduler().resume_thread(thread_id);
            }
        }
    }

    // ========================================================================
    // Termination (from Mach4 task_terminate)
    // ========================================================================

    /// Terminate the task
    pub fn terminate(&self) {
        // Mark as inactive
        *self.active.lock() = false;
        *self.state.lock() = TaskState::Terminated;

        // Terminate all threads
        let threads = self.threads.lock();
        for &thread_id in threads.iter() {
            crate::scheduler::global_scheduler().terminate_thread(thread_id);
            crate::kern::ast::ast_terminate(thread_id);
        }

        // Destroy IPC space
        let _ = self.itk_space.destroy();

        // TODO: Clean up VM map
    }

    /// Create a new thread in this task
    ///
    /// Based on Mach4 thread_create()
    pub fn create_thread(&self, entry: usize, stack_size: usize, name: String) -> ThreadId {
        // Allocate stack
        let stack_top = crate::memory::alloc_stack(stack_size);
        let task_thread = TaskThread::new(self.id, entry, stack_top, name);
        let thread_id = task_thread.id;

        // Initialize AST state for new thread
        crate::kern::ast::ast_init(thread_id);

        // Create SchedThread and add to scheduler
        let sched_thread = Arc::new(crate::scheduler::SchedThread {
            thread_id: task_thread.id,
            task_id: task_thread.task,
            priority: task_thread.priority.0 as usize, // Convert Priority to usize
            quantum: core::sync::atomic::AtomicU64::new(crate::scheduler::TIME_QUANTUM_MS),
            state: spin::Mutex::new(task_thread.state),
            affinity: core::sync::atomic::AtomicUsize::new(usize::MAX),
            context: spin::Mutex::new(task_thread.context),
        });

        crate::scheduler::add_thread(sched_thread);

        // Track in task
        self.threads.lock().push(thread_id);
        self.thread_count.fetch_add(1, Ordering::SeqCst);

        thread_id
    }

    pub fn allocate_port(&self) -> PortName {
        // Return PortName, not Arc<Port>
        // In Mach, allocating a port gives you a new unique port name (PortId equivalent)
        // and transfers the receive right to the task.
        let port_name = PortName::new();
        self.ports.lock().push(port_name); // Add to task's owned port names
        port_name
    }

    pub fn destroy_port(&self, port_name: PortName) {
        let mut ports = self.ports.lock();
        ports.retain(|&name| name != port_name);
        // TODO: Invalidate port globally and release resources associated with it.
    }
}

/// Global task management
pub struct TaskManager {
    tasks: Mutex<Vec<Arc<Task>>>,
    kernel_task: Option<Arc<Task>>,
}

impl TaskManager {
    pub const fn new() -> Self {
        TaskManager {
            tasks: Mutex::new(Vec::new()),
            kernel_task: None,
        }
    }

    pub fn create_kernel_task(&mut self, name: String) -> Arc<Task> {
        let kernel_task = Task::new(name);
        self.kernel_task = Some(kernel_task.clone());
        self.tasks.lock().push(kernel_task.clone());
        kernel_task
    }

    pub fn create_task(&self, name: String) -> Arc<Task> {
        let task = Task::new(name);
        self.tasks.lock().push(task.clone());
        task
    }

    pub fn get_task(&self, id: TaskId) -> Option<Arc<Task>> {
        self.tasks.lock().iter().find(|t| t.id == id).cloned()
    }

    pub fn destroy_task(&self, id: TaskId) {
        let mut tasks = self.tasks.lock();
        if let Some(pos) = tasks.iter().position(|t| t.id == id) {
            let task = tasks.remove(pos);
            task.terminate(); // Terminate the task and its threads
                              // TODO: Clean up VM map, ports, etc.
        }
    }
}

static TASK_MANAGER: Once<TaskManager> = Once::new();

pub fn init(kernel_task_name: String) {
    TASK_MANAGER.call_once(|| {
        let mut manager = TaskManager::new();
        manager.create_kernel_task(kernel_task_name);
        manager
    });
}

pub fn manager() -> &'static TaskManager {
    TASK_MANAGER.get().expect("Task Manager not initialized")
}

// Global functions for creating tasks/threads from outside task module
pub fn create_task(name: String) -> Arc<Task> {
    manager().create_task(name)
}

pub fn create_thread(task: &Arc<Task>, entry: usize, stack_size: usize, name: String) -> ThreadId {
    task.create_thread(entry, stack_size, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new(String::from("test_task")); // Update
        assert_eq!(task.state(), TaskState::Running);
    }

    #[test]
    fn test_thread_creation() {
        let task = Task::new(String::from("test_task")); // Update
        let thread_id = task.create_thread(0, 0, String::from("test_thread")); // Update

        let threads = task.threads.lock();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0], thread_id); // threads now stores ThreadId
    }

    #[test]
    fn test_task_state_transitions() {
        let task = Task::new(String::from("test_task")); // Update

        task.suspend();
        assert_eq!(task.state(), TaskState::Suspended);

        task.resume();
        assert_eq!(task.state(), TaskState::Running);

        task.terminate();
        assert_eq!(task.state(), TaskState::Terminated);
    }
}
