//! Scheduling Primitives - Wait/Wakeup and Event Management
//!
//! Based on Mach4 kern/sched_prim.h/c
//! Provides thread blocking, wakeup, and event-based synchronization.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

use crate::types::ThreadId;

// ============================================================================
// Wait Results
// ============================================================================

/// Result of a wait operation (from Mach4)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WaitResult {
    /// Thread was awakened normally
    Normal = 0,
    /// Thread was awakened abnormally (clear wait)
    Abnormal = 1,
    /// Thread was interrupted
    Interrupted = 2,
    /// Wait timed out
    TimedOut = 3,
    /// Restart the system call
    Restart = 4,
}

// ============================================================================
// Wait Event
// ============================================================================

/// Event that threads can wait on
/// In Mach, this is typically a pointer cast to an integer
pub type WaitEvent = u64;

/// Special value indicating no event
pub const EVENT_NULL: WaitEvent = 0;

/// Wait reason (for debugging/statistics)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum WaitReason {
    /// No reason
    None = 0,
    /// Waiting for IPC message
    IpcReceive = 1,
    /// Waiting for IPC send
    IpcSend = 2,
    /// Waiting for VM page
    VmPage = 3,
    /// Waiting for lock
    Lock = 4,
    /// Waiting for timer
    Timer = 5,
    /// Waiting for I/O
    Io = 6,
    /// Suspended
    Suspended = 7,
}

// ============================================================================
// Waiter Entry
// ============================================================================

/// A thread waiting on an event
#[derive(Debug, Clone)]
pub struct Waiter {
    /// The waiting thread
    pub thread_id: ThreadId,
    /// The event being waited on
    pub event: WaitEvent,
    /// Wait reason
    pub reason: WaitReason,
    /// Wait result (set when woken)
    pub result: WaitResult,
    /// Is this wait interruptible?
    pub interruptible: bool,
    /// Timeout (if any) - absolute time
    pub timeout: Option<u64>,
}

impl Waiter {
    /// Create a new waiter
    pub fn new(thread_id: ThreadId, event: WaitEvent) -> Self {
        Self {
            thread_id,
            event,
            reason: WaitReason::None,
            result: WaitResult::Normal,
            interruptible: true,
            timeout: None,
        }
    }

    /// Create waiter with reason
    pub fn with_reason(thread_id: ThreadId, event: WaitEvent, reason: WaitReason) -> Self {
        Self {
            thread_id,
            event,
            reason,
            result: WaitResult::Normal,
            interruptible: true,
            timeout: None,
        }
    }
}

// ============================================================================
// Wait Queue
// ============================================================================

/// Queue of threads waiting on events
#[derive(Debug)]
pub struct WaitQueue {
    /// Waiters indexed by event
    waiters: BTreeMap<WaitEvent, Vec<Waiter>>,
    /// Total waiter count
    count: usize,
}

impl WaitQueue {
    /// Create a new wait queue
    pub const fn new() -> Self {
        Self {
            waiters: BTreeMap::new(),
            count: 0,
        }
    }

    /// Add a waiter
    pub fn add(&mut self, waiter: Waiter) {
        let event = waiter.event;
        self.waiters.entry(event).or_default().push(waiter);
        self.count += 1;
    }

    /// Remove a specific waiter
    pub fn remove(&mut self, thread_id: ThreadId) -> Option<Waiter> {
        for (_event, waiters) in self.waiters.iter_mut() {
            if let Some(pos) = waiters.iter().position(|w| w.thread_id == thread_id) {
                self.count -= 1;
                return Some(waiters.remove(pos));
            }
        }
        None
    }

    /// Wake one thread waiting on an event
    pub fn wakeup_one(&mut self, event: WaitEvent) -> Option<Waiter> {
        if let Some(waiters) = self.waiters.get_mut(&event) {
            if let Some(waiter) = waiters.pop() {
                self.count -= 1;
                if waiters.is_empty() {
                    self.waiters.remove(&event);
                }
                return Some(waiter);
            }
        }
        None
    }

    /// Wake all threads waiting on an event
    pub fn wakeup_all(&mut self, event: WaitEvent) -> Vec<Waiter> {
        if let Some(mut waiters) = self.waiters.remove(&event) {
            self.count -= waiters.len();
            core::mem::take(&mut waiters)
        } else {
            Vec::new()
        }
    }

    /// Wake one thread waiting on an event with a specific result
    pub fn wakeup_one_with_result(
        &mut self,
        event: WaitEvent,
        result: WaitResult,
    ) -> Option<Waiter> {
        self.wakeup_one(event).map(|mut w| {
            w.result = result;
            w
        })
    }

    /// Get count of waiters
    pub fn count(&self) -> usize {
        self.count
    }

    /// Check if any waiters for an event
    pub fn has_waiters(&self, event: WaitEvent) -> bool {
        self.waiters.get(&event).is_some_and(|w| !w.is_empty())
    }
}

impl Default for WaitQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Wait Queue Manager
// ============================================================================

/// Global wait queue
static WAIT_QUEUE: spin::Once<Mutex<WaitQueue>> = spin::Once::new();

/// Current tick count (for timeouts)
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// Initialize wait queue
pub fn init() {
    WAIT_QUEUE.call_once(|| Mutex::new(WaitQueue::new()));
}

/// Get the global wait queue
fn wait_queue() -> &'static Mutex<WaitQueue> {
    WAIT_QUEUE.get().expect("Wait queue not initialized")
}

/// Get current tick count
pub fn current_ticks() -> u64 {
    TICK_COUNT.load(Ordering::SeqCst)
}

/// Increment tick count (called by timer interrupt)
pub fn tick() {
    TICK_COUNT.fetch_add(1, Ordering::SeqCst);
}

// ============================================================================
// Assert Wait / Thread Block Operations
// ============================================================================

/// Assert that thread will wait on an event
///
/// This sets up the wait but doesn't actually block.
/// Call thread_block() after to actually block.
pub fn assert_wait(thread_id: ThreadId, event: WaitEvent, interruptible: bool) {
    let mut wq = wait_queue().lock();
    let mut waiter = Waiter::new(thread_id, event);
    waiter.interruptible = interruptible;
    wq.add(waiter);
}

/// Assert wait with timeout
pub fn assert_wait_timeout(
    thread_id: ThreadId,
    event: WaitEvent,
    interruptible: bool,
    timeout_ticks: u64,
) {
    let mut wq = wait_queue().lock();
    let mut waiter = Waiter::new(thread_id, event);
    waiter.interruptible = interruptible;
    waiter.timeout = Some(current_ticks() + timeout_ticks);
    wq.add(waiter);
}

/// Clear a thread's wait (wake it up abnormally)
pub fn clear_wait(thread_id: ThreadId, result: WaitResult) -> bool {
    let mut wq = wait_queue().lock();
    if let Some(mut waiter) = wq.remove(thread_id) {
        waiter.result = result;
        // Actually wake the thread through scheduler
        crate::scheduler::wake_thread(thread_id);
        true
    } else {
        false
    }
}

/// Wake up one thread waiting on an event
pub fn thread_wakeup_one(event: WaitEvent) -> bool {
    let mut wq = wait_queue().lock();
    if let Some(waiter) = wq.wakeup_one(event) {
        crate::scheduler::wake_thread(waiter.thread_id);
        true
    } else {
        false
    }
}

/// Wake up all threads waiting on an event
pub fn thread_wakeup(event: WaitEvent) -> usize {
    let mut wq = wait_queue().lock();
    let waiters = wq.wakeup_all(event);
    let count = waiters.len();
    for waiter in waiters {
        crate::scheduler::wake_thread(waiter.thread_id);
    }
    count
}

/// Wake up with specific result
pub fn thread_wakeup_with_result(event: WaitEvent, result: WaitResult) -> usize {
    let mut wq = wait_queue().lock();
    let mut waiters = wq.wakeup_all(event);
    let count = waiters.len();
    for waiter in waiters.iter_mut() {
        waiter.result = result;
        crate::scheduler::wake_thread(waiter.thread_id);
    }
    count
}

/// Check for timed-out waiters (called periodically)
pub fn check_timeouts() {
    let now = current_ticks();
    let mut wq = wait_queue().lock();

    // Collect timed-out waiters
    let mut timed_out = Vec::new();

    for (_event, waiters) in wq.waiters.iter_mut() {
        let mut i = 0;
        while i < waiters.len() {
            if let Some(timeout) = waiters[i].timeout {
                if now >= timeout {
                    let mut waiter = waiters.remove(i);
                    waiter.result = WaitResult::TimedOut;
                    timed_out.push(waiter);
                    continue;
                }
            }
            i += 1;
        }
    }

    // Update count
    wq.count -= timed_out.len();

    // Drop lock before waking threads
    drop(wq);

    // Wake timed-out threads
    for waiter in timed_out {
        crate::scheduler::wake_thread(waiter.thread_id);
    }
}

// ============================================================================
// Continuation Support
// ============================================================================

/// Continuation function type
/// When a thread is blocked and later resumed, it can continue
/// from a specific function instead of where it left off.
pub type Continuation = fn() -> !;

/// Thread continuation (for resuming blocked threads)
static CONTINUATIONS: spin::Once<Mutex<BTreeMap<ThreadId, Continuation>>> = spin::Once::new();

/// Initialize continuations map
fn init_continuations() {
    CONTINUATIONS.call_once(|| Mutex::new(BTreeMap::new()));
}

/// Get continuations map
fn continuations() -> &'static Mutex<BTreeMap<ThreadId, Continuation>> {
    CONTINUATIONS.get().expect("Continuations not initialized")
}

/// Set continuation for a thread
pub fn thread_set_continuation(thread_id: ThreadId, cont: Continuation) {
    init_continuations();
    continuations().lock().insert(thread_id, cont);
}

/// Get and clear continuation for a thread
pub fn thread_get_continuation(thread_id: ThreadId) -> Option<Continuation> {
    let conts = CONTINUATIONS.get()?;
    conts.lock().remove(&thread_id)
}

// ============================================================================
// Simple Event Operations
// ============================================================================

/// Create an event from a pointer/address
pub fn event_from_addr<T>(addr: &T) -> WaitEvent {
    addr as *const T as usize as WaitEvent
}

/// Block the current thread (full context save)
///
/// This is the main entry point for blocking. The thread will be removed
/// from the run queue and context switched away. When the thread is woken,
/// it will resume from where it left off (unless a continuation is set).
///
/// Must be called after assert_wait() to set up what the thread is waiting for.
pub fn thread_block(reason: WaitReason) -> WaitResult {
    let thread_id = crate::scheduler::current_thread()
        .map(|t| t.thread_id)
        .unwrap_or(ThreadId(0));

    // Update the waiter's reason if we have one
    {
        let mut wq = wait_queue().lock();
        for (_event, waiters) in wq.waiters.iter_mut() {
            for waiter in waiters.iter_mut() {
                if waiter.thread_id == thread_id {
                    waiter.reason = reason;
                    break;
                }
            }
        }
    }

    // Block through the scheduler
    crate::scheduler::block_current();

    // When we wake up, return the result
    // In a full implementation, this would be stored in the thread struct
    WaitResult::Normal
}

/// Block with continuation
///
/// Like thread_block(), but when the thread is woken, instead of resuming
/// from where it left off, it will jump to the continuation function.
/// This is used for operations that span multiple blocking points.
pub fn thread_block_with_continuation(reason: WaitReason, continuation: Continuation) -> ! {
    let thread_id = crate::scheduler::current_thread()
        .map(|t| t.thread_id)
        .unwrap_or(ThreadId(0));

    // Set continuation for when we wake up
    thread_set_continuation(thread_id, continuation);

    // Update the waiter's reason
    {
        let mut wq = wait_queue().lock();
        for (_event, waiters) in wq.waiters.iter_mut() {
            for waiter in waiters.iter_mut() {
                if waiter.thread_id == thread_id {
                    waiter.reason = reason;
                    break;
                }
            }
        }
    }

    // Block - we won't return from this, continuation will be called instead
    crate::scheduler::block_current();

    // This should never be reached - continuation should be called
    unreachable!("thread_block_with_continuation should never return");
}

/// Simple blocking wait on an event
/// Returns the wait result
pub fn thread_sleep(event: WaitEvent, interruptible: bool) -> WaitResult {
    let thread_id = crate::scheduler::current_thread()
        .map(|t| t.thread_id)
        .unwrap_or(ThreadId(0));

    assert_wait(thread_id, event, interruptible);
    thread_block(WaitReason::None)
}

/// Timed sleep
pub fn thread_sleep_timeout(event: WaitEvent, timeout_ticks: u64) -> WaitResult {
    let thread_id = crate::scheduler::current_thread()
        .map(|t| t.thread_id)
        .unwrap_or(ThreadId(0));

    assert_wait_timeout(thread_id, event, true, timeout_ticks);
    thread_block(WaitReason::Timer)
}

// ============================================================================
// Low-level Wakeup Primitives
// ============================================================================

/// Low-level wakeup primitive
///
/// Wakes up threads waiting on an event with priority boost consideration.
/// This is the core wakeup function used by higher-level wakeup operations.
pub fn thread_wakeup_prim(event: WaitEvent, one_thread: bool) -> usize {
    let mut wq = wait_queue().lock();

    if one_thread {
        // Wake just one thread
        if let Some(waiter) = wq.wakeup_one(event) {
            let thread_id = waiter.thread_id;
            drop(wq); // Release lock before scheduler call
            crate::scheduler::wake_thread(thread_id);
            1
        } else {
            0
        }
    } else {
        // Wake all threads waiting on this event
        let waiters = wq.wakeup_all(event);
        let count = waiters.len();
        let thread_ids: Vec<ThreadId> = waiters.iter().map(|w| w.thread_id).collect();
        drop(wq); // Release lock before scheduler calls

        for thread_id in thread_ids {
            crate::scheduler::wake_thread(thread_id);
        }
        count
    }
}

/// Wakeup with priority handoff
///
/// Like thread_wakeup_prim but gives the woken thread a temporary priority boost.
/// Used for lock handoff scenarios to prevent priority inversion.
pub fn thread_wakeup_prim_with_boost(event: WaitEvent, one_thread: bool, _boost: bool) -> usize {
    // TODO: Implement priority boost
    // For now, just delegate to the basic wakeup
    thread_wakeup_prim(event, one_thread)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_queue() {
        let mut wq = WaitQueue::new();

        let event1: WaitEvent = 0x1000;
        let event2: WaitEvent = 0x2000;

        wq.add(Waiter::new(ThreadId(1), event1));
        wq.add(Waiter::new(ThreadId(2), event1));
        wq.add(Waiter::new(ThreadId(3), event2));

        assert_eq!(wq.count(), 3);
        assert!(wq.has_waiters(event1));
        assert!(wq.has_waiters(event2));

        // Wake one from event1
        let waiter = wq.wakeup_one(event1).unwrap();
        assert!(waiter.thread_id == ThreadId(1) || waiter.thread_id == ThreadId(2));
        assert_eq!(wq.count(), 2);

        // Wake all from event1
        let woken = wq.wakeup_all(event1);
        assert_eq!(woken.len(), 1);
        assert_eq!(wq.count(), 1);
        assert!(!wq.has_waiters(event1));
    }

    #[test]
    fn test_wait_result() {
        assert_eq!(WaitResult::Normal as i32, 0);
        assert_eq!(WaitResult::TimedOut as i32, 3);
    }
}
