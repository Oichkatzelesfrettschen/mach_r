//! Task and thread management

use crate::ipc::PortName;
use crate::memory::virt::PageTable;
use crate::sync::SpinLock;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicU32, Ordering};

/// Global task ID counter
static NEXT_TASK_ID: AtomicU32 = AtomicU32::new(1);
static NEXT_THREAD_ID: AtomicU32 = AtomicU32::new(1000);

/// Task ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(pub u32);

/// Thread ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadId(u32);

impl TaskId {
    pub fn new() -> Self {
        Self(NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst))
    }
}

impl ThreadId {
    pub fn new() -> Self {
        Self(NEXT_THREAD_ID.fetch_add(1, Ordering::SeqCst))
    }
}

/// Task state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Suspended,
    Terminated,
}

/// Thread state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Running,
    Ready,
    Blocked,
    Suspended,
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

/// CPU context for thread switching
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Context {
    // ARM64 registers
    pub x0: u64, pub x1: u64, pub x2: u64, pub x3: u64,
    pub x4: u64, pub x5: u64, pub x6: u64, pub x7: u64,
    pub x8: u64, pub x9: u64, pub x10: u64, pub x11: u64,
    pub x12: u64, pub x13: u64, pub x14: u64, pub x15: u64,
    pub x16: u64, pub x17: u64, pub x18: u64, pub x19: u64,
    pub x20: u64, pub x21: u64, pub x22: u64, pub x23: u64,
    pub x24: u64, pub x25: u64, pub x26: u64, pub x27: u64,
    pub x28: u64, pub x29: u64, pub x30: u64,  // x30 is link register
    pub sp: u64,   // Stack pointer
    pub pc: u64,   // Program counter
    pub pstate: u64,  // Processor state
}

impl Context {
    pub fn new() -> Self {
        Self {
            x0: 0, x1: 0, x2: 0, x3: 0,
            x4: 0, x5: 0, x6: 0, x7: 0,
            x8: 0, x9: 0, x10: 0, x11: 0,
            x12: 0, x13: 0, x14: 0, x15: 0,
            x16: 0, x17: 0, x18: 0, x19: 0,
            x20: 0, x21: 0, x22: 0, x23: 0,
            x24: 0, x25: 0, x26: 0, x27: 0,
            x28: 0, x29: 0, x30: 0,
            sp: 0,
            pc: 0,
            pstate: 0,
        }
    }
}

/// A thread - the unit of execution
pub struct Thread {
    pub id: ThreadId,
    pub task: TaskId,
    pub state: ThreadState,
    pub priority: Priority,
    pub context: Context,
    pub kernel_stack: usize,
    pub user_stack: usize,
    pub name: String,
}

impl Thread {
    pub fn new(task: TaskId, entry: usize, stack: usize) -> Self {
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
            user_stack: 0,
            name: String::from("thread"),
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

/// A task - container for threads and resources
pub struct Task {
    pub id: TaskId,
    pub state: TaskState,
    pub name: String,
    pub threads: SpinLock<Vec<ThreadId>>,
    pub page_table: Box<PageTable>,
    pub ports: SpinLock<Vec<PortName>>,
    pub bootstrap_port: PortName,
    pub exception_ports: [PortName; 32],
}

impl Task {
    pub fn new(name: String) -> Self {
        Self {
            id: TaskId::new(),
            state: TaskState::Running,
            name,
            threads: SpinLock::new(Vec::new()),
            page_table: Box::new(PageTable::new()),
            ports: SpinLock::new(Vec::new()),
            bootstrap_port: PortName::NULL,
            exception_ports: [PortName::NULL; 32],
        }
    }
    
    pub fn create_thread(&self, entry: usize, stack_size: usize) -> ThreadId {
        // Allocate stack
        let stack = crate::memory::alloc_stack(stack_size);
        let thread = Thread::new(self.id, entry, stack);
        let thread_id = thread.id;
        
        // Add to scheduler
        crate::scheduler::add_thread(thread);
        
        // Track in task
        self.threads.lock().push(thread_id);
        
        thread_id
    }
    
    pub fn suspend(&mut self) {
        self.state = TaskState::Suspended;
        // Suspend all threads
        for &thread_id in self.threads.lock().iter() {
            crate::scheduler::suspend_thread(thread_id);
        }
    }
    
    pub fn resume(&mut self) {
        self.state = TaskState::Running;
        // Resume all threads
        for &thread_id in self.threads.lock().iter() {
            crate::scheduler::resume_thread(thread_id);
        }
    }
    
    pub fn terminate(&mut self) {
        self.state = TaskState::Terminated;
        // Terminate all threads
        for &thread_id in self.threads.lock().iter() {
            crate::scheduler::terminate_thread(thread_id);
        }
    }
}

/// Global task table
static mut TASK_TABLE: Option<SpinLock<Vec<Option<Box<Task>>>>> = None;

/// Initialize task subsystem
pub fn init() {
    unsafe {
        TASK_TABLE = Some(SpinLock::new(Vec::new()));
    }
}

/// Create a new task
pub fn create_task(name: String) -> TaskId {
    let task = Box::new(Task::new(name));
    let id = task.id;
    
    unsafe {
        if let Some(table) = &TASK_TABLE {
            let mut table = table.lock();
            table.push(Some(task));
        }
    }
    
    id
}

/// Get current thread ID (placeholder)
pub fn current_thread() -> ThreadId {
    // Will be implemented with scheduler
    ThreadId(0)
}

/// Wake a thread
pub fn wake_thread(id: ThreadId) {
    crate::scheduler::wake_thread(id);
}

/// Block current thread
pub fn block_current() {
    crate::scheduler::block_current();
}