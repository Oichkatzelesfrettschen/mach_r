//! Task management for Mach_R
//!
//! Tasks are the basic unit of resource allocation in Mach.
//! A task owns ports, virtual memory, and threads.

use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use crate::port::Port;
use crate::types::{TaskId, ThreadId, PortId};
use crate::memory::vm::VmMap;

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


/// Thread structure (simplified)
pub struct Thread {
    /// Thread ID
    pub id: ThreadId,
    /// Owning task
    pub task: TaskId,
    /// Thread state
    pub state: ThreadState,
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
    /// Thread is terminated
    Terminated,
}

/// Scheduler module
pub mod scheduler {
    pub use crate::scheduler::*;
}

/// Port namespace for a task
pub struct PortNamespace {
    /// Ports owned by this task
    ports: Vec<Arc<Port>>,
    /// Port rights table
    rights: Vec<(PortId, PortRights)>,
}

/// Port rights for a specific port
#[derive(Debug, Clone)]
pub struct PortRights {
    /// Has receive right
    pub receive: bool,
    /// Number of send rights
    pub send: u32,
    /// Number of send-once rights
    pub send_once: u32,
}

/// Task structure - the basic unit of resource allocation
pub struct Task {
    /// Task ID
    id: TaskId,
    /// Task state
    state: Mutex<TaskState>,
    /// Virtual memory map
    #[allow(dead_code)]
    vm_map: Mutex<VmMap>,
    /// Threads belonging to this task
    threads: Mutex<Vec<Thread>>,
    /// Port namespace
    ports: Mutex<PortNamespace>,
    /// Task port (for task control)
    #[allow(dead_code)]
    task_port: Arc<Port>,
}

impl Task {
    /// Create a new task
    pub fn new() -> Arc<Self> {
        let task_id = TaskId::new();
        let task_port = Port::new(task_id);
        
        Arc::new(Task {
            id: task_id,
            state: Mutex::new(TaskState::Running),
            vm_map: Mutex::new(VmMap::new(0x1000000, 0x1000000)), // 16MB at 16MB
            threads: Mutex::new(Vec::new()),
            ports: Mutex::new(PortNamespace {
                ports: Vec::new(),
                rights: Vec::new(),
            }),
            task_port,
        })
    }
    
    /// Get task ID
    pub fn id(&self) -> TaskId {
        self.id
    }
    
    /// Get task state
    pub fn state(&self) -> TaskState {
        *self.state.lock()
    }
    
    /// Set task state
    pub fn set_state(&self, new_state: TaskState) {
        *self.state.lock() = new_state;
    }
    
    /// Suspend the task
    pub fn suspend(&self) {
        *self.state.lock() = TaskState::Suspended;
        // Would also suspend all threads
    }
    
    /// Resume the task
    pub fn resume(&self) {
        *self.state.lock() = TaskState::Running;
        // Would also resume threads
    }
    
    /// Terminate the task
    pub fn terminate(&self) {
        *self.state.lock() = TaskState::Terminated;
        // Would also terminate all threads and release resources
    }
    
    /// Create a new thread in this task
    pub fn create_thread(&self) -> ThreadId {
        let thread_id = ThreadId::new();
        
        let thread = Thread {
            id: thread_id,
            task: self.id,
            state: ThreadState::Ready,
        };
        
        self.threads.lock().push(thread);
        thread_id
    }
    
    /// Allocate a port for this task
    pub fn allocate_port(&self) -> Arc<Port> {
        let port = Port::new(self.id);
        self.ports.lock().ports.push(port.clone());
        port
    }
    
    /// Add port rights
    pub fn add_port_rights(&self, port_id: PortId, rights: PortRights) {
        self.ports.lock().rights.push((port_id, rights));
    }
}

/// Global task management
pub struct TaskManager {
    /// All tasks in the system
    tasks: Mutex<Vec<Arc<Task>>>,
    /// Kernel task (task 0)
    kernel_task: Option<Arc<Task>>,
}

impl TaskManager {
    /// Create a new task manager
    pub const fn new() -> Self {
        TaskManager {
            tasks: Mutex::new(Vec::new()),
            kernel_task: None,
        }
    }
    
    /// Create the kernel task
    pub fn create_kernel_task(&mut self) -> Arc<Task> {
        let kernel_task = Task::new();
        self.kernel_task = Some(kernel_task.clone());
        self.tasks.lock().push(kernel_task.clone());
        kernel_task
    }
    
    /// Create a new user task
    pub fn create_task(&self) -> Arc<Task> {
        let task = Task::new();
        self.tasks.lock().push(task.clone());
        task
    }
    
    /// Get a task by ID
    pub fn get_task(&self, id: TaskId) -> Option<Arc<Task>> {
        self.tasks.lock()
            .iter()
            .find(|t| t.id() == id)
            .cloned()
    }
    
    /// Destroy a task (move to terminated state and cleanup)
    pub fn destroy_task(&self, id: TaskId) {
        let mut tasks = self.tasks.lock();
        if let Some(task) = tasks.iter().find(|t| t.id() == id) {
            task.set_state(TaskState::Terminated);
        }
        // Remove terminated tasks from the list
        tasks.retain(|t| t.state() != TaskState::Terminated);
    }
}

/// Global task manager instance
static mut TASK_MANAGER: TaskManager = TaskManager::new();

/// Initialize task management
pub fn init() {
    unsafe {
        // Create the kernel task
        let task_manager = &mut *core::ptr::addr_of_mut!(TASK_MANAGER);
        task_manager.create_kernel_task();
    }
}

/// Get the global task manager
pub fn manager() -> &'static TaskManager {
    unsafe { &*core::ptr::addr_of!(TASK_MANAGER) }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_task_creation() {
        let task = Task::new();
        assert_eq!(task.state(), TaskState::Running);
    }
    
    #[test]
    fn test_thread_creation() {
        let task = Task::new();
        let thread_id = task.create_thread();
        
        let threads = task.threads.lock();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, thread_id);
    }
    
    #[test]
    fn test_task_state_transitions() {
        let task = Task::new();
        
        task.suspend();
        assert_eq!(task.state(), TaskState::Suspended);
        
        task.resume();
        assert_eq!(task.state(), TaskState::Running);
        
        task.terminate();
        assert_eq!(task.state(), TaskState::Terminated);
    }
}