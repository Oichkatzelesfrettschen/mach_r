//! Thread Activations
//!
//! Based on OSF/Mach 3.0 thread activation model (kern/act.h/c)
//!
//! Thread activations provide a separation between the thread's user-level
//! execution context (the "activation") and the kernel-level execution
//! context (the "shuttle"). This enables:
//!
//! - **User-level scheduling**: User threads can migrate between kernel threads
//! - **Stack recycling**: Kernel stacks can be reused across activations
//! - **Efficient upcalls**: Kernel can invoke user-level handlers efficiently
//!
//! ## Activation Stack
//!
//! Activations can be stacked (for nested upcalls/downcalls):
//! ```text
//! ┌──────────────┐
//! │  Higher Act  │  <- Most recent (current)
//! ├──────────────┤
//! │  Middle Act  │  <- Previous
//! ├──────────────┤
//! │  Lower Act   │  <- Oldest
//! └──────────────┘
//! ```
//!
//! ## Key Operations
//!
//! - `act_create`: Create new activation
//! - `act_attach`: Attach activation to shuttle
//! - `act_detach`: Detach activation from shuttle
//! - `act_alert`: Send alert to activation (for asynchronous interruption)

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::ipc::PortName;
use crate::kern::continuation::{BlockReason, Continuation, ThreadContinuationState, WaitResult};
use crate::kern::lock::SimpleLock;
use crate::kern::thread::TaskId;
use crate::types::ThreadId;

// ============================================================================
// Activation ID
// ============================================================================

/// Unique activation identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActivationId(pub u64);

impl ActivationId {
    /// Null activation ID
    pub const NULL: Self = Self(0);
}

// ============================================================================
// Activation State
// ============================================================================

/// Activation state flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActivationState(pub u32);

impl ActivationState {
    /// Activation is active (has shuttle)
    pub const ACTIVE: Self = Self(0x0001);
    /// Activation is waiting
    pub const WAITING: Self = Self(0x0002);
    /// Activation is suspended
    pub const SUSPENDED: Self = Self(0x0004);
    /// Activation has been alerted
    pub const ALERTED: Self = Self(0x0008);
    /// Activation is in kernel mode
    pub const IN_KERNEL: Self = Self(0x0010);
    /// Activation is handling exception
    pub const EXCEPTION: Self = Self(0x0020);
    /// Activation is being terminated
    pub const TERMINATED: Self = Self(0x0040);
    /// Activation is pooled (reusable)
    pub const POOLED: Self = Self(0x0080);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl core::ops::BitOr for ActivationState {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for ActivationState {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

// ============================================================================
// Alert Flags
// ============================================================================

/// Alert types that can be sent to an activation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AlertType {
    /// No alert
    None = 0,
    /// Abort current operation
    Abort = 1,
    /// Thread should halt
    Halt = 2,
    /// Resume from halt
    Resume = 3,
    /// Terminate the activation
    Terminate = 4,
    /// User-defined alert
    User = 5,
}

impl Default for AlertType {
    fn default() -> Self {
        AlertType::None
    }
}

// ============================================================================
// User State (Machine-Dependent)
// ============================================================================

/// User-mode register state (architecture-specific)
#[derive(Debug, Clone, Default)]
pub struct UserState {
    /// Program counter
    pub pc: u64,
    /// Stack pointer
    pub sp: u64,
    /// Frame pointer
    pub fp: u64,
    /// General purpose registers (simplified)
    pub gpr: [u64; 31],
    /// Floating point state (simplified)
    pub fpr: [u64; 32],
    /// Condition flags
    pub flags: u64,
}

impl UserState {
    /// Create new user state with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the entry point
    pub fn set_entry_point(&mut self, entry: u64) {
        self.pc = entry;
    }

    /// Set the stack pointer
    pub fn set_stack(&mut self, sp: u64) {
        self.sp = sp;
    }

    /// Get the program counter
    pub fn pc(&self) -> u64 {
        self.pc
    }

    /// Get the stack pointer
    pub fn sp(&self) -> u64 {
        self.sp
    }
}

// ============================================================================
// Activation Structure
// ============================================================================

/// Thread Activation - user-level thread context
#[derive(Debug)]
pub struct Activation {
    /// Unique activation ID
    pub id: ActivationId,

    /// Containing task
    pub task: TaskId,

    /// Current state flags
    state: AtomicU32,

    /// Reference count
    ref_count: AtomicU32,

    /// User-mode register state
    pub user_state: Mutex<UserState>,

    /// Current shuttle (kernel thread) - None if not active
    pub shuttle: Mutex<Option<ThreadId>>,

    /// Lower activation in stack
    pub lower: Mutex<Option<Arc<Activation>>>,

    /// Higher activation in stack
    pub higher: Mutex<Option<Arc<Activation>>>,

    /// Continuation state (for blocking)
    pub continuation: ThreadContinuationState,

    /// Current alert
    alert: AtomicU32,

    /// Exception port
    pub exception_port: Mutex<Option<PortName>>,

    /// Special port (for migration, etc.)
    pub special_port: Mutex<Option<PortName>>,

    /// Suspend count (nested suspends)
    suspend_count: AtomicU32,

    /// Lock for state changes
    lock: SimpleLock,

    /// Started flag
    started: AtomicBool,

    /// Active flag
    active: AtomicBool,
}

impl Activation {
    /// Create a new activation
    pub fn new(id: ActivationId, task: TaskId) -> Self {
        Self {
            id,
            task,
            state: AtomicU32::new(ActivationState::empty().0),
            ref_count: AtomicU32::new(1),
            user_state: Mutex::new(UserState::new()),
            shuttle: Mutex::new(None),
            lower: Mutex::new(None),
            higher: Mutex::new(None),
            continuation: ThreadContinuationState::new(),
            alert: AtomicU32::new(AlertType::None as u32),
            exception_port: Mutex::new(None),
            special_port: Mutex::new(None),
            suspend_count: AtomicU32::new(0),
            lock: SimpleLock::new(),
            started: AtomicBool::new(false),
            active: AtomicBool::new(false),
        }
    }

    /// Get current state
    pub fn get_state(&self) -> ActivationState {
        ActivationState(self.state.load(Ordering::SeqCst))
    }

    /// Set state flags
    pub fn set_state(&self, state: ActivationState) {
        self.state.store(state.0, Ordering::SeqCst);
    }

    /// Add state flags
    pub fn add_state(&self, flags: ActivationState) {
        self.state.fetch_or(flags.0, Ordering::SeqCst);
    }

    /// Remove state flags
    pub fn clear_state(&self, flags: ActivationState) {
        self.state.fetch_and(!flags.0, Ordering::SeqCst);
    }

    /// Check if active (has shuttle)
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Check if started
    pub fn is_started(&self) -> bool {
        self.started.load(Ordering::SeqCst)
    }

    /// Check if suspended
    pub fn is_suspended(&self) -> bool {
        self.get_state().contains(ActivationState::SUSPENDED)
    }

    /// Check if alerted
    pub fn is_alerted(&self) -> bool {
        self.get_state().contains(ActivationState::ALERTED)
    }

    /// Get current alert
    pub fn get_alert(&self) -> AlertType {
        match self.alert.load(Ordering::SeqCst) {
            0 => AlertType::None,
            1 => AlertType::Abort,
            2 => AlertType::Halt,
            3 => AlertType::Resume,
            4 => AlertType::Terminate,
            _ => AlertType::User,
        }
    }

    /// Increment reference count
    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement reference count
    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    /// Get reference count
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::SeqCst)
    }

    /// Get suspend count
    pub fn suspend_count(&self) -> u32 {
        self.suspend_count.load(Ordering::SeqCst)
    }

    /// Lock the activation
    pub fn lock(&self) {
        self.lock.lock();
    }

    /// Unlock the activation
    pub fn unlock(&self) {
        self.lock.unlock();
    }
}

// ============================================================================
// Shuttle (Kernel Thread Context)
// ============================================================================

/// Shuttle - kernel-level thread execution context
///
/// The shuttle provides the kernel stack and scheduling context.
/// Activations "ride" on shuttles to execute.
#[derive(Debug)]
pub struct Shuttle {
    /// Thread ID
    pub thread_id: ThreadId,

    /// Current activation (top of activation stack)
    pub activation: Mutex<Option<Arc<Activation>>>,

    /// Kernel stack base address
    pub kstack_base: AtomicU64,

    /// Kernel stack size
    pub kstack_size: AtomicU64,

    /// Is shuttle idle?
    idle: AtomicBool,

    /// Is shuttle bound to a specific processor?
    bound: AtomicBool,

    /// Bound processor ID
    bound_processor: AtomicU32,
}

impl Shuttle {
    /// Create a new shuttle
    pub fn new(thread_id: ThreadId) -> Self {
        Self {
            thread_id,
            activation: Mutex::new(None),
            kstack_base: AtomicU64::new(0),
            kstack_size: AtomicU64::new(0),
            idle: AtomicBool::new(true),
            bound: AtomicBool::new(false),
            bound_processor: AtomicU32::new(0),
        }
    }

    /// Check if shuttle has an activation
    pub fn has_activation(&self) -> bool {
        self.activation.lock().is_some()
    }

    /// Check if shuttle is idle
    pub fn is_idle(&self) -> bool {
        self.idle.load(Ordering::SeqCst)
    }

    /// Get current activation
    pub fn current_activation(&self) -> Option<Arc<Activation>> {
        self.activation.lock().clone()
    }

    /// Set kernel stack
    pub fn set_kstack(&self, base: u64, size: u64) {
        self.kstack_base.store(base, Ordering::SeqCst);
        self.kstack_size.store(size, Ordering::SeqCst);
    }
}

// ============================================================================
// Activation Operations
// ============================================================================

/// Create a new activation for a task
pub fn act_create(task: TaskId) -> Arc<Activation> {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    let id = ActivationId(NEXT_ID.fetch_add(1, Ordering::SeqCst));
    Arc::new(Activation::new(id, task))
}

/// Attach an activation to a shuttle
///
/// The activation becomes the current execution context for the shuttle.
/// If the shuttle already has an activation, it is pushed down on the stack.
pub fn act_attach(act: &Arc<Activation>, shuttle: &Shuttle) -> bool {
    act.lock();

    // Check if activation is already attached somewhere
    if act.is_active() {
        act.unlock();
        return false;
    }

    // Get current activation (if any) to stack under
    let previous = shuttle.activation.lock().take();
    if let Some(ref prev) = previous {
        // Link the activations
        *act.lower.lock() = Some(Arc::clone(prev));
        *prev.higher.lock() = Some(Arc::clone(act));
    }

    // Attach the new activation
    *shuttle.activation.lock() = Some(Arc::clone(act));
    *act.shuttle.lock() = Some(shuttle.thread_id);

    act.active.store(true, Ordering::SeqCst);
    act.add_state(ActivationState::ACTIVE);

    act.unlock();
    true
}

/// Detach the current activation from a shuttle
///
/// Returns the detached activation. The next activation in the stack
/// becomes current.
pub fn act_detach(shuttle: &Shuttle) -> Option<Arc<Activation>> {
    let mut act_guard = shuttle.activation.lock();
    let current = act_guard.take()?;

    current.lock();

    // Get lower activation to become current
    let lower = current.lower.lock().take();
    if let Some(ref low) = lower {
        *low.higher.lock() = None;
    }

    *act_guard = lower;
    *current.shuttle.lock() = None;

    current.active.store(false, Ordering::SeqCst);
    current.clear_state(ActivationState::ACTIVE);

    current.unlock();
    Some(current)
}

/// Alert an activation
///
/// Sends an alert to the activation, potentially waking it from a wait.
pub fn act_alert(act: &Arc<Activation>, alert: AlertType) {
    act.lock();

    act.alert.store(alert as u32, Ordering::SeqCst);
    act.add_state(ActivationState::ALERTED);

    // If the activation is waiting, wake it up
    if act.get_state().contains(ActivationState::WAITING) {
        act.continuation.clear(WaitResult::Interrupted);
        act.clear_state(ActivationState::WAITING);
    }

    act.unlock();
}

/// Clear an alert
pub fn act_alert_clear(act: &Arc<Activation>) {
    act.alert.store(AlertType::None as u32, Ordering::SeqCst);
    act.clear_state(ActivationState::ALERTED);
}

/// Suspend an activation
pub fn act_suspend(act: &Arc<Activation>) {
    act.lock();

    act.suspend_count.fetch_add(1, Ordering::SeqCst);
    act.add_state(ActivationState::SUSPENDED);

    act.unlock();
}

/// Resume an activation
pub fn act_resume(act: &Arc<Activation>) {
    act.lock();

    let count = act.suspend_count.fetch_sub(1, Ordering::SeqCst);
    if count == 1 {
        // Last resume - clear suspended state
        act.clear_state(ActivationState::SUSPENDED);
    }

    act.unlock();
}

/// Terminate an activation
pub fn act_terminate(act: &Arc<Activation>) {
    act.lock();

    act.add_state(ActivationState::TERMINATED);
    act.alert
        .store(AlertType::Terminate as u32, Ordering::SeqCst);

    // If waiting, wake up
    if act.get_state().contains(ActivationState::WAITING) {
        act.continuation.clear(WaitResult::Aborted);
    }

    act.unlock();
}

/// Set up activation to block with a continuation
pub fn act_block(
    act: &Arc<Activation>,
    continuation: Continuation,
    event: u64,
    reason: BlockReason,
) {
    act.continuation
        .setup_continuation(continuation, event, reason);
    act.add_state(ActivationState::WAITING);
}

/// Wake up an activation
pub fn act_wakeup(act: &Arc<Activation>, result: WaitResult) {
    act.lock();

    if act.get_state().contains(ActivationState::WAITING) {
        act.continuation.clear(result);
        act.clear_state(ActivationState::WAITING);
    }

    act.unlock();
}

// ============================================================================
// Activation Pool
// ============================================================================

/// Pool of reusable activations
#[derive(Debug)]
pub struct ActivationPool {
    /// Free activations
    free: Mutex<Vec<Arc<Activation>>>,
    /// Maximum pool size
    max_size: usize,
    /// Current count
    count: AtomicU32,
}

impl ActivationPool {
    /// Create a new activation pool
    pub const fn new(max_size: usize) -> Self {
        Self {
            free: Mutex::new(Vec::new()),
            max_size,
            count: AtomicU32::new(0),
        }
    }

    /// Get an activation from the pool (or create one)
    pub fn get(&self, task: TaskId) -> Arc<Activation> {
        // Try to get from free list
        if let Some(act) = self.free.lock().pop() {
            self.count.fetch_sub(1, Ordering::SeqCst);
            // Reset the activation for reuse
            act.set_state(ActivationState::empty());
            act.alert.store(AlertType::None as u32, Ordering::SeqCst);
            act.suspend_count.store(0, Ordering::SeqCst);
            act.started.store(false, Ordering::SeqCst);
            return act;
        }

        // Create a new one
        act_create(task)
    }

    /// Return an activation to the pool
    pub fn put(&self, act: Arc<Activation>) -> bool {
        if self.count.load(Ordering::SeqCst) >= self.max_size as u32 {
            return false; // Pool is full, let it drop
        }

        // Mark as pooled
        act.add_state(ActivationState::POOLED);

        self.free.lock().push(act);
        self.count.fetch_add(1, Ordering::SeqCst);
        true
    }

    /// Get pool count
    pub fn count(&self) -> u32 {
        self.count.load(Ordering::SeqCst)
    }
}

/// Global activation pool
static ACTIVATION_POOL: ActivationPool = ActivationPool::new(64);

/// Get activation pool
pub fn activation_pool() -> &'static ActivationPool {
    &ACTIVATION_POOL
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activation_create() {
        let act = act_create(TaskId(1));
        assert!(!act.is_active());
        assert!(!act.is_started());
        assert!(!act.is_suspended());
    }

    #[test]
    fn test_activation_state() {
        let act = act_create(TaskId(1));

        act.add_state(ActivationState::SUSPENDED);
        assert!(act.is_suspended());

        act.clear_state(ActivationState::SUSPENDED);
        assert!(!act.is_suspended());
    }

    #[test]
    fn test_shuttle_attach_detach() {
        let act = act_create(TaskId(1));
        let shuttle = Shuttle::new(ThreadId(1));

        assert!(act_attach(&act, &shuttle));
        assert!(act.is_active());
        assert!(shuttle.has_activation());

        let detached = act_detach(&shuttle);
        assert!(detached.is_some());
        assert!(!act.is_active());
        assert!(!shuttle.has_activation());
    }

    #[test]
    fn test_activation_stacking() {
        let act1 = act_create(TaskId(1));
        let act2 = act_create(TaskId(1));
        let shuttle = Shuttle::new(ThreadId(1));

        // Attach first activation
        assert!(act_attach(&act1, &shuttle));

        // Attach second (should stack on top)
        assert!(act_attach(&act2, &shuttle));

        // Check the stack
        assert!(act2.lower.lock().is_some());
        assert!(act1.higher.lock().is_some());

        // Detach top
        let top = act_detach(&shuttle).unwrap();
        assert_eq!(top.id, act2.id);

        // Next should be act1
        let next = act_detach(&shuttle).unwrap();
        assert_eq!(next.id, act1.id);
    }

    #[test]
    fn test_activation_suspend_resume() {
        let act = act_create(TaskId(1));

        // Nested suspends
        act_suspend(&act);
        assert!(act.is_suspended());
        assert_eq!(act.suspend_count(), 1);

        act_suspend(&act);
        assert_eq!(act.suspend_count(), 2);

        act_resume(&act);
        assert!(act.is_suspended()); // Still suspended
        assert_eq!(act.suspend_count(), 1);

        act_resume(&act);
        assert!(!act.is_suspended()); // Now resumed
    }

    #[test]
    fn test_activation_alert() {
        let act = act_create(TaskId(1));

        act_alert(&act, AlertType::Abort);
        assert!(act.is_alerted());
        assert_eq!(act.get_alert(), AlertType::Abort);

        act_alert_clear(&act);
        assert!(!act.is_alerted());
    }

    #[test]
    fn test_activation_pool() {
        let pool = ActivationPool::new(10);

        // Get a new activation
        let act1 = pool.get(TaskId(1));
        let id1 = act1.id;

        // Return to pool
        assert!(pool.put(act1));
        assert_eq!(pool.count(), 1);

        // Get again - should be the same one
        let act2 = pool.get(TaskId(1));
        assert_eq!(act2.id, id1);
        assert_eq!(pool.count(), 0);
    }
}
