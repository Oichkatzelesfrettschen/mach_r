//! Thread Management
//!
//! Based on Mach4 kern/thread.h/c by Avadis Tevanian, Jr.
//!
//! Threads are the unit of execution in Mach. A thread belongs to exactly
//! one task, which provides its address space. Multiple threads can run
//! concurrently within a single task.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

use crate::ipc::PortName;
use crate::kern::lock::SimpleLock;
use crate::kern::sched_prim::WaitEvent;

// ============================================================================
// Thread State Flags
// ============================================================================

/// Thread state flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThreadState(pub u32);

impl ThreadState {
    /// Thread is queued for waiting
    pub const WAIT: Self = Self(0x01);
    /// Thread has been asked to stop
    pub const SUSP: Self = Self(0x02);
    /// Thread is running or on run queue
    pub const RUN: Self = Self(0x04);
    /// Thread is waiting uninterruptibly
    pub const UNINT: Self = Self(0x08);
    /// Thread is halted at clean point
    pub const HALTED: Self = Self(0x10);
    /// Thread is an idle thread
    pub const IDLE: Self = Self(0x80);
    /// Thread has no kernel stack (swapped out)
    pub const SWAPPED: Self = Self(0x0100);
    /// Thread is waiting for kernel stack
    pub const SW_COMING_IN: Self = Self(0x0200);

    /// Scheduling-relevant states
    pub const SCHED_STATE: Self = Self(0x01 | 0x02 | 0x04 | 0x08);

    /// Swap-related states
    pub const SWAP_STATE: Self = Self(0x0100 | 0x0200);

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn is_runnable(self) -> bool {
        self.contains(Self::RUN) && !self.contains(Self::WAIT) && !self.contains(Self::SUSP)
    }

    pub fn is_waiting(self) -> bool {
        self.contains(Self::WAIT)
    }

    pub fn is_suspended(self) -> bool {
        self.contains(Self::SUSP)
    }
}

impl core::ops::BitOr for ThreadState {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for ThreadState {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for ThreadState {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl core::ops::Not for ThreadState {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

// ============================================================================
// Thread Priority
// ============================================================================

/// Thread priority levels
pub mod priority {
    /// Minimum priority
    pub const MIN: i32 = 0;
    /// Default priority
    pub const DEFAULT: i32 = 31;
    /// Maximum priority
    pub const MAX: i32 = 63;
    /// Idle thread priority
    pub const IDLE: i32 = -1;
    /// Real-time priority base
    pub const RT_BASE: i32 = 64;
}

/// Scheduling policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum SchedPolicy {
    /// Time-sharing (default)
    #[default]
    Timeshare = 0,
    /// Fixed priority
    FixedPri = 1,
    /// Real-time
    RealTime = 2,
}

// ============================================================================
// Thread IPC State
// ============================================================================

/// Thread IPC state (for message operations)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum IpcState {
    #[default]
    None = 0,
    /// Waiting to receive
    Receiving = 1,
    /// Waiting to send
    Sending = 2,
    /// Message received
    MsgReceived = 3,
    /// Send complete
    SendComplete = 4,
    /// Timed out
    TimedOut = 5,
    /// Interrupted
    Interrupted = 6,
}

// ============================================================================
// Thread Identifier
// ============================================================================

/// Thread identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThreadId(pub u64);

impl ThreadId {
    pub const NULL: Self = Self(0);

    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

// ============================================================================
// Task Identifier (forward reference)
// ============================================================================

/// Task identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u64);

impl TaskId {
    pub const NULL: Self = Self(0);
    pub const KERNEL: Self = Self(1);
}

// ============================================================================
// Thread Statistics
// ============================================================================

/// Thread timing statistics
#[derive(Debug, Default)]
pub struct ThreadStats {
    /// User mode time (microseconds)
    pub user_time: AtomicU64,
    /// System mode time (microseconds)
    pub system_time: AtomicU64,
    /// CPU usage (decaying average)
    pub cpu_usage: AtomicU32,
    /// Load-weighted CPU usage
    pub sched_usage: AtomicU32,
    /// Last time priority was updated
    pub sched_stamp: AtomicU64,
    /// Context switches
    pub context_switches: AtomicU64,
}

impl ThreadStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_user_time(&self, micros: u64) {
        self.user_time.fetch_add(micros, Ordering::Relaxed);
    }

    pub fn add_system_time(&self, micros: u64) {
        self.system_time.fetch_add(micros, Ordering::Relaxed);
    }

    pub fn record_context_switch(&self) {
        self.context_switches.fetch_add(1, Ordering::Relaxed);
    }
}

// ============================================================================
// Thread Wait Information
// ============================================================================

/// Thread wait information
#[derive(Debug)]
pub struct WaitInfo {
    /// Event we're waiting on
    pub event: Mutex<Option<WaitEvent>>,
    /// Wait result
    pub result: AtomicU32,
    /// Is someone waiting for this thread to stop?
    pub wake_active: AtomicBool,
}

impl Default for WaitInfo {
    fn default() -> Self {
        Self {
            event: Mutex::new(None),
            result: AtomicU32::new(0),
            wake_active: AtomicBool::new(false),
        }
    }
}

/// Wait result values
pub mod wait_result {
    pub const SUCCESS: u32 = 0;
    pub const INTERRUPTED: u32 = 1;
    pub const TIMEOUT: u32 = 2;
    pub const RESTART: u32 = 3;
}

// ============================================================================
// Thread IPC Ports
// ============================================================================

/// Thread IPC ports
#[derive(Debug, Default)]
pub struct ThreadPorts {
    /// Thread's self port (not a right)
    pub self_port: Mutex<Option<PortName>>,
    /// Thread's send-right self port
    pub sself: Mutex<Option<PortName>>,
    /// Exception port
    pub exception: Mutex<Option<PortName>>,
    /// MIG reply port
    pub mig_reply: Mutex<Option<PortName>>,
    /// Kernel RPC reply port
    pub rpc_reply: Mutex<Option<PortName>>,
}

// ============================================================================
// Thread Structure
// ============================================================================

/// A Mach thread
#[derive(Debug)]
pub struct Thread {
    /// Thread identifier
    pub id: ThreadId,

    /// Task this thread belongs to
    pub task_id: TaskId,

    /// Thread state
    state: AtomicU32,

    /// Reference count
    ref_count: AtomicU32,

    /// Lock for thread state
    lock: SimpleLock,

    // === Scheduling ===
    /// Current priority
    pub priority: AtomicU32,

    /// Maximum priority
    pub max_priority: AtomicU32,

    /// Computed scheduling priority
    pub sched_pri: AtomicU32,

    /// Scheduling policy
    pub policy: Mutex<SchedPolicy>,

    /// Depressed priority (for priority inversion)
    pub depress_priority: AtomicU32,

    // === Suspension ===
    /// Internal suspend count
    suspend_count: AtomicU32,

    /// User-visible stop count
    user_stop_count: AtomicU32,

    // === Wait information ===
    /// Wait state
    pub wait: WaitInfo,

    // === Stack ===
    /// Kernel stack address
    pub kernel_stack: AtomicUsize,

    /// Reserved kernel stack (privilege)
    pub stack_privilege: AtomicUsize,

    // === IPC ===
    /// Thread ports
    pub ports: ThreadPorts,

    /// IPC state
    pub ipc_state: AtomicU32,

    /// IPC sequence number
    pub ipc_seqno: AtomicU32,

    // === Statistics ===
    /// Thread statistics
    pub stats: ThreadStats,

    // === VM ===
    /// Can use reserved memory?
    pub vm_privilege: AtomicBool,

    /// Page fault recovery address
    pub recover: AtomicUsize,

    // === Processor binding ===
    /// Processor this thread last ran on
    pub last_processor: AtomicU32,

    /// Processor this thread is bound to (0 = unbound)
    pub bound_processor: AtomicU32,

    // === User-mode entry state ===
    /// Program counter for user mode entry
    pub user_pc: AtomicU64,

    /// Stack pointer for user mode entry
    pub user_sp: AtomicU64,
}

impl Thread {
    /// Create a new thread
    pub fn new(id: ThreadId, task_id: TaskId) -> Self {
        Self {
            id,
            task_id,
            state: AtomicU32::new(ThreadState::SUSP.0),
            ref_count: AtomicU32::new(1),
            lock: SimpleLock::new(),
            priority: AtomicU32::new(priority::DEFAULT as u32),
            max_priority: AtomicU32::new(priority::MAX as u32),
            sched_pri: AtomicU32::new(priority::DEFAULT as u32),
            policy: Mutex::new(SchedPolicy::default()),
            depress_priority: AtomicU32::new(0),
            suspend_count: AtomicU32::new(1),
            user_stop_count: AtomicU32::new(0),
            wait: WaitInfo::default(),
            kernel_stack: AtomicUsize::new(0),
            stack_privilege: AtomicUsize::new(0),
            ports: ThreadPorts::default(),
            ipc_state: AtomicU32::new(IpcState::None as u32),
            ipc_seqno: AtomicU32::new(0),
            stats: ThreadStats::new(),
            vm_privilege: AtomicBool::new(false),
            recover: AtomicUsize::new(0),
            last_processor: AtomicU32::new(0),
            bound_processor: AtomicU32::new(0),
            user_pc: AtomicU64::new(0),
            user_sp: AtomicU64::new(0),
        }
    }

    /// Create a kernel thread
    pub fn kernel_thread(id: ThreadId) -> Self {
        let thread = Self::new(id, TaskId::KERNEL);
        thread.vm_privilege.store(true, Ordering::Release);
        thread.set_state(ThreadState::RUN);
        thread.suspend_count.store(0, Ordering::Release);
        thread
    }

    // === State management ===

    /// Get thread state
    pub fn get_state(&self) -> ThreadState {
        ThreadState(self.state.load(Ordering::Acquire))
    }

    /// Set thread state
    pub fn set_state(&self, state: ThreadState) {
        self.state.store(state.0, Ordering::Release);
    }

    /// Add state flags
    pub fn add_state(&self, flags: ThreadState) {
        self.state.fetch_or(flags.0, Ordering::AcqRel);
    }

    /// Remove state flags
    pub fn remove_state(&self, flags: ThreadState) {
        self.state.fetch_and(!flags.0, Ordering::AcqRel);
    }

    /// Check if thread is runnable
    pub fn is_runnable(&self) -> bool {
        self.get_state().is_runnable() && self.suspend_count.load(Ordering::Acquire) == 0
    }

    /// Check if thread is waiting
    pub fn is_waiting(&self) -> bool {
        self.get_state().is_waiting()
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

    // === Suspension ===

    /// Suspend the thread
    pub fn suspend(&self) -> bool {
        self.lock.lock();

        let count = self.suspend_count.fetch_add(1, Ordering::AcqRel);
        if count == 0 {
            // First suspension - mark as suspended
            self.add_state(ThreadState::SUSP);
        }

        self.lock.unlock();
        true
    }

    /// Resume the thread
    pub fn resume(&self) -> bool {
        self.lock.lock();

        let count = self.suspend_count.load(Ordering::Acquire);
        if count == 0 {
            self.lock.unlock();
            return false;
        }

        let new_count = self.suspend_count.fetch_sub(1, Ordering::AcqRel) - 1;
        if new_count == 0 {
            // No longer suspended
            self.remove_state(ThreadState::SUSP);
        }

        self.lock.unlock();
        true
    }

    /// Get suspend count
    pub fn suspend_count(&self) -> u32 {
        self.suspend_count.load(Ordering::Relaxed)
    }

    // === Priority ===

    /// Get current priority
    pub fn get_priority(&self) -> i32 {
        self.priority.load(Ordering::Relaxed) as i32
    }

    /// Set priority
    pub fn set_priority(&self, pri: i32) {
        let pri = pri.clamp(priority::MIN, priority::MAX) as u32;
        self.priority.store(pri, Ordering::Release);
        self.update_sched_priority();
    }

    /// Get maximum priority
    pub fn get_max_priority(&self) -> i32 {
        self.max_priority.load(Ordering::Relaxed) as i32
    }

    /// Set maximum priority
    pub fn set_max_priority(&self, pri: i32) {
        let pri = pri.clamp(priority::MIN, priority::MAX) as u32;
        self.max_priority.store(pri, Ordering::Release);

        // Ensure current priority doesn't exceed max
        let current = self.priority.load(Ordering::Relaxed);
        if current > pri {
            self.priority.store(pri, Ordering::Release);
        }
        self.update_sched_priority();
    }

    /// Update computed scheduling priority
    pub fn update_sched_priority(&self) {
        let base = self.priority.load(Ordering::Relaxed);
        let usage = self.stats.cpu_usage.load(Ordering::Relaxed);

        // Simple priority decay based on CPU usage
        let decay = (usage / 4).min(31);
        let sched_pri = base.saturating_sub(decay);

        self.sched_pri.store(sched_pri, Ordering::Release);
    }

    /// Depress priority temporarily
    pub fn depress_priority(&self, pri: i32) {
        let current = self.priority.load(Ordering::Relaxed);
        self.depress_priority.store(current, Ordering::Release);
        self.priority.store(pri as u32, Ordering::Release);
        self.update_sched_priority();
    }

    /// Restore depressed priority
    pub fn restore_priority(&self) {
        let depressed = self.depress_priority.load(Ordering::Acquire);
        if depressed != 0 {
            self.priority.store(depressed, Ordering::Release);
            self.depress_priority.store(0, Ordering::Release);
            self.update_sched_priority();
        }
    }

    // === Stack ===

    /// Set kernel stack
    pub fn set_kernel_stack(&self, stack: usize) {
        self.kernel_stack.store(stack, Ordering::Release);
    }

    /// Get kernel stack
    pub fn get_kernel_stack(&self) -> usize {
        self.kernel_stack.load(Ordering::Acquire)
    }

    /// Check if thread has a kernel stack
    pub fn has_stack(&self) -> bool {
        !self.get_state().contains(ThreadState::SWAPPED)
    }

    // === User mode state ===

    /// Set program counter for user mode entry
    pub fn set_pc(&self, pc: u64) {
        self.user_pc.store(pc, Ordering::Release);
    }

    /// Get program counter for user mode entry
    pub fn get_pc(&self) -> u64 {
        self.user_pc.load(Ordering::Acquire)
    }

    /// Set stack pointer for user mode entry
    pub fn set_sp(&self, sp: u64) {
        self.user_sp.store(sp, Ordering::Release);
    }

    /// Get stack pointer for user mode entry
    pub fn get_sp(&self) -> u64 {
        self.user_sp.load(Ordering::Acquire)
    }
}

// ============================================================================
// Thread Manager
// ============================================================================

/// Thread manager
pub struct ThreadManager {
    /// All threads by ID
    threads: BTreeMap<ThreadId, Arc<Thread>>,

    /// Threads by task
    by_task: BTreeMap<TaskId, Vec<ThreadId>>,

    /// Next thread ID
    next_id: u64,

    /// Total thread count
    count: u32,
}

impl ThreadManager {
    pub fn new() -> Self {
        Self {
            threads: BTreeMap::new(),
            by_task: BTreeMap::new(),
            next_id: 1,
            count: 0,
        }
    }

    /// Create a new thread in a task
    pub fn create(&mut self, task_id: TaskId) -> Arc<Thread> {
        let id = ThreadId(self.next_id);
        self.next_id += 1;

        let thread = Arc::new(Thread::new(id, task_id));

        self.threads.insert(id, Arc::clone(&thread));
        self.by_task.entry(task_id).or_default().push(id);
        self.count += 1;

        thread
    }

    /// Create a kernel thread
    pub fn create_kernel(&mut self) -> Arc<Thread> {
        let id = ThreadId(self.next_id);
        self.next_id += 1;

        let thread = Arc::new(Thread::kernel_thread(id));

        self.threads.insert(id, Arc::clone(&thread));
        self.by_task.entry(TaskId::KERNEL).or_default().push(id);
        self.count += 1;

        thread
    }

    /// Find thread by ID
    pub fn find(&self, id: ThreadId) -> Option<Arc<Thread>> {
        self.threads.get(&id).cloned()
    }

    /// Get all threads for a task
    pub fn threads_for_task(&self, task_id: TaskId) -> Vec<Arc<Thread>> {
        self.by_task
            .get(&task_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.threads.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Terminate a thread
    pub fn terminate(&mut self, id: ThreadId) -> bool {
        if let Some(thread) = self.threads.remove(&id) {
            // Remove from task's thread list
            if let Some(task_threads) = self.by_task.get_mut(&thread.task_id) {
                task_threads.retain(|&tid| tid != id);
            }
            self.count -= 1;
            true
        } else {
            false
        }
    }

    /// Terminate all threads in a task
    pub fn terminate_task_threads(&mut self, task_id: TaskId) {
        if let Some(thread_ids) = self.by_task.remove(&task_id) {
            for id in thread_ids {
                self.threads.remove(&id);
                self.count -= 1;
            }
        }
    }

    /// Get thread count
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Get all threads
    pub fn all_threads(&self) -> Vec<Arc<Thread>> {
        self.threads.values().cloned().collect()
    }
}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

static THREAD_MANAGER: spin::Once<Mutex<ThreadManager>> = spin::Once::new();

fn thread_manager() -> &'static Mutex<ThreadManager> {
    THREAD_MANAGER.call_once(|| Mutex::new(ThreadManager::new()));
    THREAD_MANAGER.get().unwrap()
}

/// Initialize thread subsystem
pub fn init() {
    let _ = thread_manager();
}

/// Create a thread
pub fn thread_create(task_id: TaskId) -> Arc<Thread> {
    thread_manager().lock().create(task_id)
}

/// Create a kernel thread
pub fn kernel_thread_create() -> Arc<Thread> {
    thread_manager().lock().create_kernel()
}

/// Find thread by ID
pub fn thread_find(id: ThreadId) -> Option<Arc<Thread>> {
    thread_manager().lock().find(id)
}

/// Terminate a thread
pub fn thread_terminate(id: ThreadId) -> bool {
    thread_manager().lock().terminate(id)
}

/// Get thread count
pub fn thread_count() -> u32 {
    thread_manager().lock().count()
}

// ============================================================================
// Thread Info (for debugging/inspection)
// ============================================================================

/// Thread information for user space
#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub id: ThreadId,
    pub task_id: TaskId,
    pub state: ThreadState,
    pub priority: i32,
    pub sched_pri: i32,
    pub suspend_count: u32,
    pub user_time: u64,
    pub system_time: u64,
}

impl From<&Thread> for ThreadInfo {
    fn from(thread: &Thread) -> Self {
        Self {
            id: thread.id,
            task_id: thread.task_id,
            state: thread.get_state(),
            priority: thread.get_priority(),
            sched_pri: thread.sched_pri.load(Ordering::Relaxed) as i32,
            suspend_count: thread.suspend_count(),
            user_time: thread.stats.user_time.load(Ordering::Relaxed),
            system_time: thread.stats.system_time.load(Ordering::Relaxed),
        }
    }
}

/// Get thread info
pub fn thread_info(id: ThreadId) -> Option<ThreadInfo> {
    thread_find(id).map(|t| ThreadInfo::from(t.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_state() {
        let state = ThreadState::RUN | ThreadState::SUSP;
        assert!(state.contains(ThreadState::RUN));
        assert!(state.contains(ThreadState::SUSP));
        assert!(!state.contains(ThreadState::WAIT));
    }

    #[test]
    fn test_thread_creation() {
        let thread = Thread::new(ThreadId(1), TaskId(1));
        assert_eq!(thread.id, ThreadId(1));
        assert_eq!(thread.task_id, TaskId(1));
        assert!(thread.get_state().contains(ThreadState::SUSP));
    }

    #[test]
    fn test_thread_suspend_resume() {
        let thread = Thread::new(ThreadId(1), TaskId(1));

        // Already suspended with count 1
        assert_eq!(thread.suspend_count(), 1);

        // Suspend again
        thread.suspend();
        assert_eq!(thread.suspend_count(), 2);

        // Resume
        thread.resume();
        assert_eq!(thread.suspend_count(), 1);

        thread.resume();
        assert_eq!(thread.suspend_count(), 0);
        assert!(!thread.get_state().contains(ThreadState::SUSP));
    }

    #[test]
    fn test_thread_priority() {
        let thread = Thread::new(ThreadId(1), TaskId(1));

        assert_eq!(thread.get_priority(), priority::DEFAULT);

        thread.set_priority(50);
        assert_eq!(thread.get_priority(), 50);

        // Clamp to max
        thread.set_priority(100);
        assert_eq!(thread.get_priority(), priority::MAX);
    }
}
