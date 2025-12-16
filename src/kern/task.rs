//! Task Management
//!
//! Based on Mach4 kern/task.h/c by Avadis Tevanian, Jr.
//!
//! A task is a container for threads and resources. It provides:
//! - An address space (vm_map)
//! - IPC namespace (ipc_space)
//! - Collection of threads
//! - Resource accounting

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::ipc::{PortName, SpaceId};
use crate::kern::lock::SimpleLock;
use crate::kern::thread::{TaskId, ThreadId};
use crate::mach_vm::vm_map::VmMapId;

// ============================================================================
// Task Port Registers
// ============================================================================

/// Maximum number of task port registers
pub const TASK_PORT_REGISTER_MAX: usize = 3;

// ============================================================================
// Task Statistics
// ============================================================================

/// Time value (seconds + microseconds)
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeValue {
    pub seconds: u64,
    pub microseconds: u32,
}

impl TimeValue {
    pub const ZERO: Self = Self {
        seconds: 0,
        microseconds: 0,
    };

    pub fn new(seconds: u64, microseconds: u32) -> Self {
        Self {
            seconds,
            microseconds,
        }
    }

    pub fn from_micros(micros: u64) -> Self {
        Self {
            seconds: micros / 1_000_000,
            microseconds: (micros % 1_000_000) as u32,
        }
    }

    pub fn to_micros(&self) -> u64 {
        self.seconds * 1_000_000 + self.microseconds as u64
    }

    pub fn add(&self, other: &Self) -> Self {
        let total_micros = self.to_micros() + other.to_micros();
        Self::from_micros(total_micros)
    }
}

/// Task statistics
#[derive(Debug, Default)]
pub struct TaskStats {
    /// Total user time for dead threads
    pub total_user_time: Mutex<TimeValue>,
    /// Total system time for dead threads
    pub total_system_time: Mutex<TimeValue>,
    /// Virtual memory size
    pub virtual_size: AtomicU64,
    /// Resident memory size
    pub resident_size: AtomicU64,
    /// Page faults
    pub faults: AtomicU64,
    /// Copy-on-write faults
    pub cow_faults: AtomicU64,
    /// Messages sent
    pub messages_sent: AtomicU64,
    /// Messages received
    pub messages_received: AtomicU64,
}

impl TaskStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_thread_times(&self, user_micros: u64, system_micros: u64) {
        let mut user = self.total_user_time.lock();
        *user = user.add(&TimeValue::from_micros(user_micros));
        drop(user);

        let mut system = self.total_system_time.lock();
        *system = system.add(&TimeValue::from_micros(system_micros));
    }
}

// ============================================================================
// Task IPC Ports
// ============================================================================

/// Task IPC ports
#[derive(Debug)]
pub struct TaskPorts {
    /// Lock for port operations
    lock: SimpleLock,

    /// Task's self port (not a right, doesn't hold ref)
    pub self_port: Mutex<Option<PortName>>,

    /// Task's send-right self port
    pub sself: Mutex<Option<PortName>>,

    /// Exception port
    pub exception: Mutex<Option<PortName>>,

    /// Bootstrap port
    pub bootstrap: Mutex<Option<PortName>>,

    /// Registered ports
    pub registered: Mutex<[Option<PortName>; TASK_PORT_REGISTER_MAX]>,
}

impl TaskPorts {
    pub fn new() -> Self {
        Self {
            lock: SimpleLock::new(),
            self_port: Mutex::new(None),
            sself: Mutex::new(None),
            exception: Mutex::new(None),
            bootstrap: Mutex::new(None),
            registered: Mutex::new([None; TASK_PORT_REGISTER_MAX]),
        }
    }

    pub fn lock(&self) {
        self.lock.lock();
    }

    pub fn unlock(&self) {
        self.lock.unlock();
    }
}

impl Default for TaskPorts {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Task Structure
// ============================================================================

/// A Mach task
#[derive(Debug)]
pub struct Task {
    /// Task identifier
    pub id: TaskId,

    /// Lock for task state
    lock: SimpleLock,

    /// Reference count
    ref_count: AtomicU32,

    /// Is task active (not terminated)?
    pub active: AtomicBool,

    // === Address Space ===
    /// Address space (VM map)
    pub map_id: Mutex<Option<VmMapId>>,

    // === Threads ===
    /// Thread IDs in this task
    threads: Mutex<Vec<ThreadId>>,

    /// Thread count
    thread_count: AtomicU32,

    // === Suspension ===
    /// Internal suspend count
    suspend_count: AtomicU32,

    /// User-visible stop count
    user_stop_count: AtomicU32,

    // === Scheduling ===
    /// Default priority for new threads
    pub priority: AtomicU32,

    /// Can processor set assignment be changed?
    pub may_assign: AtomicBool,

    /// Waiting for may_assign
    pub assign_active: AtomicBool,

    // === IPC ===
    /// Task ports
    pub ports: TaskPorts,

    /// IPC space
    pub ipc_space: Mutex<Option<SpaceId>>,

    // === Statistics ===
    /// Task statistics
    pub stats: TaskStats,
}

impl Task {
    /// Create a new task
    pub fn new(id: TaskId) -> Self {
        Self {
            id,
            lock: SimpleLock::new(),
            ref_count: AtomicU32::new(1),
            active: AtomicBool::new(true),
            map_id: Mutex::new(None),
            threads: Mutex::new(Vec::new()),
            thread_count: AtomicU32::new(0),
            suspend_count: AtomicU32::new(0),
            user_stop_count: AtomicU32::new(0),
            priority: AtomicU32::new(31), // Default priority
            may_assign: AtomicBool::new(true),
            assign_active: AtomicBool::new(false),
            ports: TaskPorts::new(),
            ipc_space: Mutex::new(None),
            stats: TaskStats::new(),
        }
    }

    /// Create the kernel task
    pub fn kernel_task() -> Self {
        let task = Self::new(TaskId::KERNEL);
        task.may_assign.store(false, Ordering::Release);
        task
    }

    // === Lock operations ===

    pub fn lock(&self) {
        self.lock.lock();
    }

    pub fn unlock(&self) {
        self.lock.unlock();
    }

    // === Reference counting ===

    /// Increment reference count
    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement reference count, returns true if deallocated
    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    /// Get reference count
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::Relaxed)
    }

    // === Active state ===

    /// Check if task is active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Mark task as terminated
    pub fn terminate(&self) {
        self.active.store(false, Ordering::Release);
    }

    // === Thread management ===

    /// Add a thread to this task
    pub fn add_thread(&self, thread_id: ThreadId) {
        self.threads.lock().push(thread_id);
        self.thread_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Remove a thread from this task
    pub fn remove_thread(&self, thread_id: ThreadId) {
        let mut threads = self.threads.lock();
        if let Some(pos) = threads.iter().position(|&id| id == thread_id) {
            threads.remove(pos);
            self.thread_count.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Get thread count
    pub fn thread_count(&self) -> u32 {
        self.thread_count.load(Ordering::Relaxed)
    }

    /// Get all thread IDs
    pub fn thread_ids(&self) -> Vec<ThreadId> {
        self.threads.lock().clone()
    }

    // === Suspension ===

    /// Suspend the task (and all its threads)
    pub fn suspend(&self) -> bool {
        if !self.is_active() {
            return false;
        }

        self.lock();
        let _count = self.suspend_count.fetch_add(1, Ordering::AcqRel);
        self.unlock();

        true
    }

    /// Resume the task
    pub fn resume(&self) -> bool {
        self.lock();

        let count = self.suspend_count.load(Ordering::Acquire);
        if count == 0 {
            self.unlock();
            return false;
        }

        self.suspend_count.fetch_sub(1, Ordering::AcqRel);
        self.unlock();

        true
    }

    /// Get suspend count
    pub fn suspend_count(&self) -> u32 {
        self.suspend_count.load(Ordering::Relaxed)
    }

    /// Is task suspended?
    pub fn is_suspended(&self) -> bool {
        self.suspend_count() > 0
    }

    // === Address space ===

    /// Set the VM map for this task
    pub fn set_map(&self, map_id: VmMapId) {
        *self.map_id.lock() = Some(map_id);
    }

    /// Get the VM map ID
    pub fn get_map_id(&self) -> Option<VmMapId> {
        *self.map_id.lock()
    }

    // === IPC space ===

    /// Set the IPC space for this task
    pub fn set_ipc_space(&self, space_id: SpaceId) {
        *self.ipc_space.lock() = Some(space_id);
    }

    /// Get the IPC space ID
    pub fn get_ipc_space_id(&self) -> Option<SpaceId> {
        *self.ipc_space.lock()
    }

    // === Priority ===

    /// Get default priority for new threads
    pub fn get_priority(&self) -> i32 {
        self.priority.load(Ordering::Relaxed) as i32
    }

    /// Set default priority for new threads
    pub fn set_priority(&self, pri: i32) {
        let pri = pri.clamp(0, 63) as u32;
        self.priority.store(pri, Ordering::Release);
    }
}

// ============================================================================
// Task Manager
// ============================================================================

/// Task manager
pub struct TaskManager {
    /// All tasks by ID
    tasks: BTreeMap<TaskId, Arc<Task>>,

    /// Next task ID
    next_id: u64,

    /// Total task count
    count: u32,

    /// Kernel task
    kernel_task: Option<Arc<Task>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            next_id: 2, // 1 is reserved for kernel
            count: 0,
            kernel_task: None,
        }
    }

    /// Initialize with kernel task
    pub fn bootstrap(&mut self) -> Arc<Task> {
        let kernel = Arc::new(Task::kernel_task());
        self.tasks.insert(TaskId::KERNEL, Arc::clone(&kernel));
        self.kernel_task = Some(Arc::clone(&kernel));
        self.count += 1;
        kernel
    }

    /// Create a new task
    pub fn create(&mut self, _parent: Option<&Task>) -> Arc<Task> {
        let id = TaskId(self.next_id);
        self.next_id += 1;

        let task = Arc::new(Task::new(id));

        // TODO: If parent provided, inherit certain properties
        // - Copy VM map (with COW)
        // - Copy IPC space
        // - Inherit exception ports

        self.tasks.insert(id, Arc::clone(&task));
        self.count += 1;

        task
    }

    /// Find task by ID
    pub fn find(&self, id: TaskId) -> Option<Arc<Task>> {
        self.tasks.get(&id).cloned()
    }

    /// Get kernel task
    pub fn kernel_task(&self) -> Option<Arc<Task>> {
        self.kernel_task.clone()
    }

    /// Terminate a task
    pub fn terminate(&mut self, id: TaskId) -> bool {
        if id == TaskId::KERNEL {
            return false; // Can't terminate kernel task
        }

        if let Some(task) = self.tasks.remove(&id) {
            task.terminate();
            self.count -= 1;
            true
        } else {
            false
        }
    }

    /// Get task count
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Get all tasks
    pub fn all_tasks(&self) -> Vec<Arc<Task>> {
        self.tasks.values().cloned().collect()
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

static TASK_MANAGER: spin::Once<Mutex<TaskManager>> = spin::Once::new();

fn task_manager() -> &'static Mutex<TaskManager> {
    TASK_MANAGER.call_once(|| {
        let mut mgr = TaskManager::new();
        mgr.bootstrap();
        Mutex::new(mgr)
    });
    TASK_MANAGER.get().unwrap()
}

/// Initialize task subsystem
pub fn init() {
    let _ = task_manager();
}

/// Create a task
pub fn task_create(parent: Option<&Task>) -> Arc<Task> {
    task_manager().lock().create(parent)
}

/// Find task by ID
pub fn task_find(id: TaskId) -> Option<Arc<Task>> {
    task_manager().lock().find(id)
}

/// Get kernel task
pub fn kernel_task() -> Option<Arc<Task>> {
    task_manager().lock().kernel_task()
}

/// Terminate a task
pub fn task_terminate(id: TaskId) -> bool {
    task_manager().lock().terminate(id)
}

/// Get task count
pub fn task_count() -> u32 {
    task_manager().lock().count()
}

// ============================================================================
// Task Info (for debugging/inspection)
// ============================================================================

/// Task information for user space
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub id: TaskId,
    pub active: bool,
    pub thread_count: u32,
    pub suspend_count: u32,
    pub priority: i32,
    pub virtual_size: u64,
    pub resident_size: u64,
    pub user_time: TimeValue,
    pub system_time: TimeValue,
}

impl From<&Task> for TaskInfo {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id,
            active: task.is_active(),
            thread_count: task.thread_count(),
            suspend_count: task.suspend_count(),
            priority: task.get_priority(),
            virtual_size: task.stats.virtual_size.load(Ordering::Relaxed),
            resident_size: task.stats.resident_size.load(Ordering::Relaxed),
            user_time: *task.stats.total_user_time.lock(),
            system_time: *task.stats.total_system_time.lock(),
        }
    }
}

/// Get task info
pub fn task_info(id: TaskId) -> Option<TaskInfo> {
    task_find(id).map(|t| TaskInfo::from(t.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_time_value() {
        let tv1 = TimeValue::new(1, 500_000);
        let tv2 = TimeValue::new(0, 700_000);
        let sum = tv1.add(&tv2);

        assert_eq!(sum.seconds, 2);
        assert_eq!(sum.microseconds, 200_000);
    }

    #[test]
    fn test_task_creation() {
        let task = Task::new(TaskId(1));
        assert_eq!(task.id, TaskId(1));
        assert!(task.is_active());
        assert_eq!(task.thread_count(), 0);
    }

    #[test]
    fn test_task_threads() {
        let task = Task::new(TaskId(1));

        task.add_thread(ThreadId(1));
        task.add_thread(ThreadId(2));
        assert_eq!(task.thread_count(), 2);

        task.remove_thread(ThreadId(1));
        assert_eq!(task.thread_count(), 1);

        let ids = task.thread_ids();
        assert_eq!(ids, vec![ThreadId(2)]);
    }

    #[test]
    fn test_task_suspend() {
        let task = Task::new(TaskId(1));

        assert!(!task.is_suspended());

        task.suspend();
        assert!(task.is_suspended());
        assert_eq!(task.suspend_count(), 1);

        task.resume();
        assert!(!task.is_suspended());
    }
}
