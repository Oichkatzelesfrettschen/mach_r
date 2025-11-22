//! Task management for Mach_R
//!
//! Tasks are the basic unit of resource allocation in Mach.
//! A task owns ports, virtual memory, and threads.

use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use spin::{Mutex, Once};
use crate::port::Port;
use crate::types::{TaskId, ThreadId};

use crate::paging::PageTable;
use crate::ipc::PortName;

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



/// Task structure - the basic unit of resource allocation
pub struct Task {
    pub id: TaskId,
    pub state: Mutex<TaskState>,
    pub name: String,
    pub threads: Mutex<Vec<ThreadId>>,
    pub page_table: Box<PageTable>,
    pub ports: Mutex<Vec<PortName>>,
    pub bootstrap_port: PortName,
    pub exception_ports: [PortName; 32],
}

impl Task {
    pub fn new(name: String) -> Arc<Self> {
        let task_id = TaskId::new();
        let _task_port = Port::new(task_id); // This will create a Port and register it globally

        Arc::new(Task {
            id: task_id,
            state: Mutex::new(TaskState::Running),
            name,
            threads: Mutex::new(Vec::new()),
            page_table: Box::new(PageTable::new()), // Initialize with a new page table
            ports: Mutex::new(Vec::new()),
            bootstrap_port: PortName::NULL,
            exception_ports: [PortName::NULL; 32],
        })
    }
    
    pub fn id(&self) -> TaskId {
        self.id
    }
    
    pub fn state(&self) -> TaskState {
        *self.state.lock()
    }
    
    pub fn set_state(&self, new_state: TaskState) {
        *self.state.lock() = new_state;
    }
    
    pub fn suspend(&self) {
        *self.state.lock() = TaskState::Suspended;
        // Suspend all threads
        for &thread_id in self.threads.lock().iter() {
            crate::scheduler::global_scheduler().suspend_thread(thread_id); // Use global scheduler
        }
    }
    
    pub fn resume(&self) {
        *self.state.lock() = TaskState::Running;
        // Resume all threads
        for &thread_id in self.threads.lock().iter() {
            crate::scheduler::global_scheduler().resume_thread(thread_id); // Use global scheduler
        }
    }
    
    pub fn terminate(&self) {
        *self.state.lock() = TaskState::Terminated;
        // Terminate all threads
        for &thread_id in self.threads.lock().iter() {
            crate::scheduler::global_scheduler().terminate_thread(thread_id); // Use global scheduler
        }
    }
    
    pub fn create_thread(&self, entry: usize, stack_size: usize, name: String) -> ThreadId {
        // Allocate stack
        let stack_top = crate::memory::alloc_stack(stack_size);
        let task_thread = TaskThread::new(self.id, entry, stack_top, name);
        let thread_id = task_thread.id;

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
        
        thread_id
    }
    
    pub fn allocate_port(&self) -> PortName { // Return PortName, not Arc<Port>
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
        self.tasks.lock()
            .iter()
            .find(|t| t.id == id)
            .cloned()
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