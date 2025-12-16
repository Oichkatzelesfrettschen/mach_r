//! Thread Swapping - Stack management for idle threads
//!
//! Based on Mach4 kern/thread_swap.c by Avadis Tevanian Jr. (1987)
//!
//! The Mach thread swapper manages kernel stack resources:
//! - Swaps out idle threads to free kernel stack memory
//! - Swaps in threads that need to run by allocating stacks
//!
//! ## Important Note
//!
//! "Swapping" in Mach doesn't mean forcing memory to secondary storage.
//! Thread memory is paged out through the normal paging mechanism.
//! Here, swapping refers specifically to kernel stack allocation/deallocation.

use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

use crate::kern::counters::{C_STACKS_CURRENT, C_SWAPIN_THREAD_BLOCK};
use crate::kern::thread::ThreadId;

// ============================================================================
// Swap State Constants
// ============================================================================

/// Thread is not swapped (has a stack)
pub const TH_SW_IN: u32 = 0;
/// Thread is swapped out (no stack)
pub const TH_SWAPPED: u32 = 1;
/// Thread is queued for swap in
pub const TH_SW_COMING_IN: u32 = 2;
/// Thread is being swapped out
pub const TH_SW_GOING_OUT: u32 = 3;

/// Mask for swap state bits
pub const TH_SWAP_STATE_MASK: u32 = 0x0F;

// ============================================================================
// Swap Request
// ============================================================================

/// A request to swap in a thread
#[derive(Debug, Clone, Copy)]
pub struct SwapinRequest {
    /// Thread ID to swap in
    pub thread_id: ThreadId,
    /// Priority hint
    pub priority: i32,
    /// Timestamp of request
    pub timestamp: u64,
}

impl SwapinRequest {
    pub fn new(thread_id: ThreadId, priority: i32) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self {
            thread_id,
            priority,
            timestamp: COUNTER.fetch_add(1, Ordering::Relaxed),
        }
    }
}

// ============================================================================
// Swap Queue
// ============================================================================

/// Queue of threads waiting to be swapped in
#[derive(Debug)]
pub struct SwapinQueue {
    /// The queue of requests
    queue: VecDeque<SwapinRequest>,
    /// Statistics
    stats: SwapStats,
}

impl SwapinQueue {
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            stats: SwapStats::new(),
        }
    }

    /// Add a thread to the swapin queue
    pub fn enqueue(&mut self, request: SwapinRequest) {
        self.queue.push_back(request);
        self.stats.enqueues += 1;
    }

    /// Remove a thread from the swapin queue
    pub fn dequeue(&mut self) -> Option<SwapinRequest> {
        let request = self.queue.pop_front();
        if request.is_some() {
            self.stats.dequeues += 1;
        }
        request
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get queue length
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Get statistics
    pub fn stats(&self) -> &SwapStats {
        &self.stats
    }
}

impl Default for SwapinQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Swapper Statistics
// ============================================================================

/// Statistics for the swapper
#[derive(Debug, Clone, Default)]
pub struct SwapStats {
    /// Threads queued for swapin
    pub enqueues: u64,
    /// Threads dequeued and swapped in
    pub dequeues: u64,
    /// Successful swapins
    pub swapins: u64,
    /// Failed swapins (no stack available)
    pub swapin_failures: u64,
    /// Successful swapouts
    pub swapouts: u64,
    /// Swapin thread wakeups
    pub wakeups: u64,
}

impl SwapStats {
    pub const fn new() -> Self {
        Self {
            enqueues: 0,
            dequeues: 0,
            swapins: 0,
            swapin_failures: 0,
            swapouts: 0,
            wakeups: 0,
        }
    }
}

// ============================================================================
// Stack Management
// ============================================================================

/// Result of stack allocation attempt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackAllocResult {
    /// Stack allocated successfully
    Success,
    /// No stack available, try later
    NoStack,
    /// Thread doesn't need a stack (already has one)
    NotNeeded,
}

/// Stack allocation context
#[derive(Debug, Clone, Copy)]
pub struct StackContext {
    /// Base address of stack
    pub base: usize,
    /// Size of stack in bytes
    pub size: usize,
    /// Stack pointer (top of stack)
    pub sp: usize,
}

impl StackContext {
    pub const fn new(base: usize, size: usize) -> Self {
        Self {
            base,
            size,
            sp: base + size, // Stack grows downward
        }
    }

    /// Check if stack is valid
    pub fn is_valid(&self) -> bool {
        self.base != 0 && self.size > 0
    }
}

impl Default for StackContext {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

// ============================================================================
// Swapper Thread State
// ============================================================================

/// State of the swapper thread
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapperState {
    /// Swapper is idle/sleeping
    Idle,
    /// Swapper is processing swapin requests
    Running,
    /// Swapper is blocked waiting for resources
    Blocked,
}

/// The swapper module state
#[derive(Debug)]
pub struct Swapper {
    /// Swapin queue
    queue: Mutex<SwapinQueue>,
    /// Swapper thread state
    state: Mutex<SwapperState>,
    /// Whether swapper has been initialized
    initialized: bool,
}

impl Swapper {
    pub const fn new() -> Self {
        Self {
            queue: Mutex::new(SwapinQueue::new()),
            state: Mutex::new(SwapperState::Idle),
            initialized: false,
        }
    }

    /// Initialize the swapper
    pub fn init(&mut self) {
        self.initialized = true;
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Queue a thread for swapin
    pub fn queue_swapin(&self, thread_id: ThreadId, priority: i32) {
        let request = SwapinRequest::new(thread_id, priority);
        self.queue.lock().enqueue(request);
    }

    /// Process the swapin queue (called by swapin_thread)
    pub fn process_queue(&self) -> Option<SwapinRequest> {
        self.queue.lock().dequeue()
    }

    /// Check if there are pending swapins
    pub fn has_pending(&self) -> bool {
        !self.queue.lock().is_empty()
    }

    /// Get queue length
    pub fn queue_len(&self) -> usize {
        self.queue.lock().len()
    }

    /// Get swapper state
    pub fn get_state(&self) -> SwapperState {
        *self.state.lock()
    }

    /// Set swapper state
    pub fn set_state(&self, state: SwapperState) {
        *self.state.lock() = state;
    }

    /// Record a wakeup
    pub fn record_wakeup(&self) {
        self.queue.lock().stats.wakeups += 1;
    }

    /// Get statistics
    pub fn stats(&self) -> SwapStats {
        self.queue.lock().stats.clone()
    }
}

impl Default for Swapper {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

static SWAPPER: spin::Once<Swapper> = spin::Once::new();

fn swapper() -> &'static Swapper {
    SWAPPER.call_once(|| {
        let mut s = Swapper::new();
        s.init();
        s
    })
}

/// Initialize the swapper module
pub fn swapper_init() {
    let _ = swapper();
}

/// Alias for init
pub fn init() {
    swapper_init();
}

// ============================================================================
// Thread Swapin Operations
// ============================================================================

/// Queue a thread for swapin
///
/// Called when a thread needs to run but doesn't have a stack.
/// The thread is added to the swapin queue and the swapin thread
/// will allocate a stack for it.
pub fn thread_swapin(thread_id: ThreadId, current_state: u32, priority: i32) -> u32 {
    match current_state & TH_SWAP_STATE_MASK {
        TH_SWAPPED => {
            // Thread is swapped out - queue for swapin
            swapper().queue_swapin(thread_id, priority);
            // Wake up swapin thread
            thread_wakeup_swapin();
            // Return new state with SW_COMING_IN
            (current_state & !TH_SWAP_STATE_MASK) | TH_SW_COMING_IN
        }
        TH_SW_COMING_IN => {
            // Already queued - no change
            current_state
        }
        _ => {
            // Already swapped in or invalid state
            current_state
        }
    }
}

/// Actually perform the swapin for a thread
///
/// This allocates a kernel stack and prepares the thread to run.
/// Should be called without locks held as it may sleep.
pub fn thread_doswapin(thread_id: ThreadId) -> StackAllocResult {
    // Allocate a kernel stack
    // In real implementation, this would call stack_alloc()
    let result = stack_alloc_try(thread_id);

    if result == StackAllocResult::Success {
        // Record stack allocation
        C_STACKS_CURRENT.inc();

        // Update swapper stats
        swapper().queue.lock().stats.swapins += 1;
    } else {
        swapper().queue.lock().stats.swapin_failures += 1;
    }

    result
}

/// Try to allocate a stack (fast path)
fn stack_alloc_try(_thread_id: ThreadId) -> StackAllocResult {
    // In real implementation, would try to get a stack from free list
    // For now, always succeed
    StackAllocResult::Success
}

/// Wake up the swapin thread
fn thread_wakeup_swapin() {
    swapper().record_wakeup();
    // In real implementation, would call thread_wakeup(&swapin_queue)
}

// ============================================================================
// Thread Swapout Operations
// ============================================================================

/// Swap out a thread (free its kernel stack)
///
/// Called when a thread is idle and its stack can be reclaimed.
pub fn thread_swapout(thread_id: ThreadId, current_state: u32) -> u32 {
    if (current_state & TH_SWAP_STATE_MASK) != TH_SW_IN {
        // Already swapped or in transition
        return current_state;
    }

    // Free the kernel stack
    stack_free(thread_id);

    // Record stack deallocation
    C_STACKS_CURRENT.dec();

    // Update stats
    swapper().queue.lock().stats.swapouts += 1;

    // Return new state
    (current_state & !TH_SWAP_STATE_MASK) | TH_SWAPPED
}

/// Free a thread's stack
fn stack_free(_thread_id: ThreadId) {
    // In real implementation, would return stack to free list
}

// ============================================================================
// Swapin Thread
// ============================================================================

/// The swapin thread entry point
///
/// This kernel thread processes the swapin queue, allocating stacks
/// for threads that need them.
pub fn swapin_thread_loop() {
    loop {
        // Set state to idle while waiting
        swapper().set_state(SwapperState::Idle);

        // Wait for work
        if !swapper().has_pending() {
            // Block until woken
            C_SWAPIN_THREAD_BLOCK.inc();
            // In real implementation: thread_block()
            // For now, just return (would be an infinite loop in kernel)
            break;
        }

        // Process pending swapins
        swapper().set_state(SwapperState::Running);

        while let Some(request) = swapper().process_queue() {
            // Swap in the thread
            let _result = thread_doswapin(request.thread_id);

            // In real implementation:
            // - If TH_RUN is set, put thread on run queue
            // - Clear TH_SWAPPED and TH_SW_COMING_IN bits
        }
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Get swapper statistics
pub fn swap_stats() -> SwapStats {
    swapper().stats()
}

/// Check if swapper is initialized
pub fn is_initialized() -> bool {
    swapper().is_initialized()
}

/// Get number of threads waiting for swapin
pub fn pending_swapins() -> usize {
    swapper().queue_len()
}

/// Get swap state name
pub fn swap_state_name(state: u32) -> &'static str {
    match state & TH_SWAP_STATE_MASK {
        TH_SW_IN => "IN",
        TH_SWAPPED => "SWAPPED",
        TH_SW_COMING_IN => "COMING_IN",
        TH_SW_GOING_OUT => "GOING_OUT",
        _ => "UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kern::thread::ThreadId;

    #[test]
    fn test_swap_states() {
        assert_eq!(swap_state_name(TH_SW_IN), "IN");
        assert_eq!(swap_state_name(TH_SWAPPED), "SWAPPED");
        assert_eq!(swap_state_name(TH_SW_COMING_IN), "COMING_IN");
    }

    #[test]
    fn test_swapin_queue() {
        let mut queue = SwapinQueue::new();
        assert!(queue.is_empty());

        queue.enqueue(SwapinRequest::new(ThreadId(1), 0));
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);

        let request = queue.dequeue();
        assert!(request.is_some());
        assert!(queue.is_empty());
    }

    #[test]
    fn test_stack_context() {
        let stack = StackContext::new(0x1000, 0x4000);
        assert!(stack.is_valid());
        assert_eq!(stack.base, 0x1000);
        assert_eq!(stack.sp, 0x5000); // base + size

        let empty = StackContext::default();
        assert!(!empty.is_valid());
    }

    #[test]
    fn test_thread_swapin() {
        // Thread in SWAPPED state
        let new_state = thread_swapin(ThreadId(1), TH_SWAPPED, 0);
        assert_eq!(new_state & TH_SWAP_STATE_MASK, TH_SW_COMING_IN);

        // Thread already coming in
        let same_state = thread_swapin(ThreadId(1), TH_SW_COMING_IN, 0);
        assert_eq!(same_state & TH_SWAP_STATE_MASK, TH_SW_COMING_IN);

        // Thread already in
        let in_state = thread_swapin(ThreadId(1), TH_SW_IN, 0);
        assert_eq!(in_state & TH_SWAP_STATE_MASK, TH_SW_IN);
    }
}
