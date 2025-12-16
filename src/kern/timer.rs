//! Kernel Timers - Time measurement and scheduling timers
//!
//! Based on Mach4 kern/timer.h/c
//! Provides timer management for thread scheduling and system timing.

use alloc::collections::BinaryHeap;
use alloc::vec::Vec;
use core::cmp::Ordering as CmpOrdering;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::types::ThreadId;

// ============================================================================
// Time Types
// ============================================================================

/// Time value in microseconds
pub type TimeValue = u64;

/// Time value structure (seconds + microseconds)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MachTimeValue {
    /// Seconds
    pub seconds: u32,
    /// Microseconds (0-999999)
    pub microseconds: u32,
}

impl MachTimeValue {
    /// Create new time value
    pub const fn new(seconds: u32, microseconds: u32) -> Self {
        Self {
            seconds,
            microseconds,
        }
    }

    /// Create from total microseconds
    pub fn from_usecs(usecs: u64) -> Self {
        Self {
            seconds: (usecs / 1_000_000) as u32,
            microseconds: (usecs % 1_000_000) as u32,
        }
    }

    /// Convert to total microseconds
    pub fn to_usecs(&self) -> u64 {
        (self.seconds as u64) * 1_000_000 + (self.microseconds as u64)
    }

    /// Add another time value
    pub fn add(&mut self, other: &Self) {
        self.microseconds += other.microseconds;
        if self.microseconds >= 1_000_000 {
            self.microseconds -= 1_000_000;
            self.seconds += 1;
        }
        self.seconds += other.seconds;
    }
}

// ============================================================================
// Timer Data
// ============================================================================

/// Timer data for a thread
#[derive(Debug, Clone, Default)]
pub struct TimerData {
    /// Low bits of timestamp
    low_bits: u32,
    /// High bits of timestamp
    high_bits: u32,
    /// High bits at last read
    high_bits_check: u32,
    /// Timestamp
    timestamp: u64,
}

impl TimerData {
    /// Create new timer data
    pub const fn new() -> Self {
        Self {
            low_bits: 0,
            high_bits: 0,
            high_bits_check: 0,
            timestamp: 0,
        }
    }

    /// Read current value
    pub fn read(&self) -> u64 {
        self.timestamp
    }

    /// Update timestamp
    pub fn update(&mut self, ticks: u64) {
        self.timestamp = ticks;
        self.low_bits = ticks as u32;
        self.high_bits = (ticks >> 32) as u32;
        self.high_bits_check = self.high_bits;
    }
}

/// Saved timer value
#[derive(Debug, Clone, Copy, Default)]
pub struct TimerSaveData {
    /// Saved value
    pub value: u64,
}

// ============================================================================
// Timer Element (for callouts/timeouts)
// ============================================================================

/// Timer element for scheduled callbacks
#[derive(Debug, Clone)]
pub struct TimerElement {
    /// When the timer fires (absolute ticks)
    pub deadline: u64,
    /// Callback function
    pub callback: TimerCallback,
    /// Associated thread (if any)
    pub thread_id: Option<ThreadId>,
    /// Timer ID
    pub id: u32,
    /// Is this timer active?
    pub active: bool,
}

/// Timer callback type
#[derive(Debug, Clone, Copy)]
pub enum TimerCallback {
    /// No callback
    None,
    /// Wake a thread
    WakeThread(ThreadId),
    /// Call a function (via index into callback table)
    Function(usize),
}

impl PartialEq for TimerElement {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TimerElement {}

impl PartialOrd for TimerElement {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerElement {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // Reverse order for min-heap (earliest deadline first)
        other.deadline.cmp(&self.deadline)
    }
}

// ============================================================================
// Timer Queue
// ============================================================================

/// Queue of pending timers
#[derive(Debug)]
pub struct TimerQueue {
    /// Pending timers (min-heap by deadline)
    timers: BinaryHeap<TimerElement>,
    /// Next timer ID
    next_id: u32,
}

impl TimerQueue {
    /// Create new timer queue
    pub fn new() -> Self {
        Self {
            timers: BinaryHeap::new(),
            next_id: 1,
        }
    }

    /// Schedule a timer
    pub fn schedule(&mut self, deadline: u64, callback: TimerCallback) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let thread_id = match callback {
            TimerCallback::WakeThread(tid) => Some(tid),
            _ => None,
        };

        self.timers.push(TimerElement {
            deadline,
            callback,
            thread_id,
            id,
            active: true,
        });

        id
    }

    /// Cancel a timer
    pub fn cancel(&mut self, id: u32) -> bool {
        // BinaryHeap doesn't support efficient removal
        // Mark as inactive instead
        let timers: Vec<_> = self.timers.drain().collect();
        let mut found = false;
        for mut timer in timers {
            if timer.id == id {
                timer.active = false;
                found = true;
            }
            self.timers.push(timer);
        }
        found
    }

    /// Process expired timers
    pub fn process(&mut self, current_time: u64) -> Vec<TimerElement> {
        let mut expired = Vec::new();

        while let Some(timer) = self.timers.peek() {
            if timer.deadline <= current_time && timer.active {
                if let Some(timer) = self.timers.pop() {
                    if timer.active {
                        expired.push(timer);
                    }
                }
            } else {
                break;
            }
        }

        expired
    }

    /// Get next deadline
    pub fn next_deadline(&self) -> Option<u64> {
        self.timers.peek().map(|t| t.deadline)
    }
}

impl Default for TimerQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Timer Management
// ============================================================================

/// Global timer queue
static TIMER_QUEUE: spin::Once<Mutex<TimerQueue>> = spin::Once::new();

/// System tick counter
static SYSTEM_TICKS: AtomicU64 = AtomicU64::new(0);

/// Scheduler tick (increments every second)
static SCHED_TICK: AtomicU32 = AtomicU32::new(0);

/// Ticks per second
pub const TICKS_PER_SECOND: u64 = 1000;

/// Initialize timer subsystem
pub fn init() {
    TIMER_QUEUE.call_once(|| Mutex::new(TimerQueue::new()));
}

/// Get timer queue
fn timer_queue() -> &'static Mutex<TimerQueue> {
    TIMER_QUEUE.get().expect("Timer subsystem not initialized")
}

/// Get current system ticks
pub fn system_ticks() -> u64 {
    SYSTEM_TICKS.load(Ordering::SeqCst)
}

/// Get scheduler tick
pub fn sched_tick() -> u32 {
    SCHED_TICK.load(Ordering::SeqCst)
}

/// Timer interrupt handler - called every tick
pub fn timer_tick() {
    let ticks = SYSTEM_TICKS.fetch_add(1, Ordering::SeqCst) + 1;

    // Update sched_tick every second
    if ticks.is_multiple_of(TICKS_PER_SECOND) {
        SCHED_TICK.fetch_add(1, Ordering::SeqCst);
    }

    // Process expired timers
    let expired = timer_queue().lock().process(ticks);

    for timer in expired {
        match timer.callback {
            TimerCallback::WakeThread(thread_id) => {
                crate::scheduler::wake_thread(thread_id);
            }
            TimerCallback::Function(idx) => {
                // Would call registered callback function
                let _ = idx;
            }
            TimerCallback::None => {}
        }
    }

    // Update scheduling primitives
    super::sched_prim::tick();
    super::sched_prim::check_timeouts();
}

/// Schedule a thread wakeup timer
pub fn schedule_wakeup(thread_id: ThreadId, delay_ticks: u64) -> u32 {
    let deadline = system_ticks() + delay_ticks;
    timer_queue()
        .lock()
        .schedule(deadline, TimerCallback::WakeThread(thread_id))
}

/// Cancel a timer
pub fn cancel_timer(id: u32) -> bool {
    timer_queue().lock().cancel(id)
}

/// Get next timer deadline
pub fn next_deadline() -> Option<u64> {
    timer_queue().lock().next_deadline()
}

/// Convert milliseconds to ticks
pub const fn ms_to_ticks(ms: u64) -> u64 {
    (ms * TICKS_PER_SECOND) / 1000
}

/// Convert ticks to milliseconds
pub const fn ticks_to_ms(ticks: u64) -> u64 {
    (ticks * 1000) / TICKS_PER_SECOND
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_value() {
        let tv = MachTimeValue::new(1, 500000);
        assert_eq!(tv.to_usecs(), 1_500_000);

        let tv2 = MachTimeValue::from_usecs(2_750_000);
        assert_eq!(tv2.seconds, 2);
        assert_eq!(tv2.microseconds, 750_000);
    }

    #[test]
    fn test_timer_queue() {
        let mut tq = TimerQueue::new();

        let id1 = tq.schedule(100, TimerCallback::None);
        let id2 = tq.schedule(50, TimerCallback::None);
        let id3 = tq.schedule(200, TimerCallback::None);

        // Process at time 75 - should get id2
        let expired = tq.process(75);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, id2);

        // Process at time 150 - should get id1
        let expired = tq.process(150);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, id1);

        // Cancel id3
        assert!(tq.cancel(id3));
    }
}
