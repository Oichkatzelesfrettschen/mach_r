//! Run Queue Management
//!
//! Based on Mach4 kern/sched.h run queue structures
//!
//! Run queues are priority-based FIFO queues for runnable threads.
//! Each processor has a local run queue, and processor sets have shared queues.
//!
//! ## Priority Levels
//!
//! Mach uses 128 priority levels (0-127):
//! - 0-31: System priorities (highest)
//! - 32-63: Server/kernel priorities
//! - 64-95: Normal user priorities
//! - 96-127: Background/idle priorities (lowest)
//!
//! ## Bitmap Optimization
//!
//! A 128-bit bitmap tracks which priority levels have threads, allowing O(1)
//! lookup of the highest priority runnable thread using leading-zero count.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::kern::priority::Priority;
use crate::types::ThreadId;

// ============================================================================
// Constants
// ============================================================================

/// Number of priority levels
pub const NRQS: usize = 128;

/// Number of priority levels per bitmap word
pub const BITMAP_BITS: usize = 64;

/// Idle priority (lowest)
pub const IDLE_PRI: u32 = 127;

/// Maximum priority (highest)
pub const MAX_PRI: u32 = 0;

/// Default priority for user threads
pub const DEFAULT_USER_PRI: u32 = 80;

/// Default priority for kernel threads
pub const DEFAULT_KERNEL_PRI: u32 = 32;

// ============================================================================
// Run Queue Entry
// ============================================================================

/// An entry in the run queue representing a runnable thread
#[derive(Debug, Clone)]
pub struct RunqEntry {
    /// Thread ID
    pub thread_id: ThreadId,
    /// Thread priority
    pub priority: Priority,
    /// Time when thread was enqueued (for aging)
    pub enqueue_time: u64,
    /// CPU affinity hint (which CPU last ran this thread)
    pub last_processor: Option<u32>,
}

impl RunqEntry {
    /// Create a new run queue entry
    pub fn new(thread_id: ThreadId, priority: Priority) -> Self {
        Self {
            thread_id,
            priority,
            enqueue_time: 0, // Would be set by timer
            last_processor: None,
        }
    }

    /// Create entry with CPU affinity
    pub fn with_affinity(thread_id: ThreadId, priority: Priority, last_cpu: u32) -> Self {
        Self {
            thread_id,
            priority,
            enqueue_time: 0,
            last_processor: Some(last_cpu),
        }
    }
}

// ============================================================================
// Priority Queue (single priority level)
// ============================================================================

/// Queue of threads at a single priority level
#[derive(Debug, Default)]
pub struct PriorityQueue {
    /// Threads at this priority (FIFO order)
    threads: VecDeque<RunqEntry>,
}

impl PriorityQueue {
    /// Create new empty priority queue
    pub const fn new() -> Self {
        Self {
            threads: VecDeque::new(),
        }
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.threads.is_empty()
    }

    /// Get number of threads
    pub fn len(&self) -> usize {
        self.threads.len()
    }

    /// Add thread to end of queue (FIFO)
    pub fn push(&mut self, entry: RunqEntry) {
        self.threads.push_back(entry);
    }

    /// Remove thread from front of queue
    pub fn pop(&mut self) -> Option<RunqEntry> {
        self.threads.pop_front()
    }

    /// Peek at front thread
    pub fn front(&self) -> Option<&RunqEntry> {
        self.threads.front()
    }

    /// Remove specific thread by ID
    pub fn remove(&mut self, thread_id: ThreadId) -> Option<RunqEntry> {
        if let Some(pos) = self.threads.iter().position(|e| e.thread_id == thread_id) {
            self.threads.remove(pos)
        } else {
            None
        }
    }

    /// Clear the queue
    pub fn clear(&mut self) {
        self.threads.clear();
    }
}

// ============================================================================
// Run Queue
// ============================================================================

/// Multi-priority run queue with bitmap optimization
#[derive(Debug)]
pub struct RunQueue {
    /// Priority queues (0 = highest, 127 = lowest)
    queues: [Mutex<PriorityQueue>; NRQS],

    /// Bitmap of non-empty queues (high word: priorities 0-63)
    bitmap_high: AtomicU64,

    /// Bitmap of non-empty queues (low word: priorities 64-127)
    bitmap_low: AtomicU64,

    /// Total count of threads in queue
    count: AtomicU32,

    /// Highest priority with threads (hint, may be stale)
    highest_priority: AtomicU32,
}

impl RunQueue {
    /// Create a new run queue
    pub fn new() -> Self {
        // Initialize all priority queues
        // This is a bit ugly due to const initialization requirements
        const EMPTY_QUEUE: Mutex<PriorityQueue> = Mutex::new(PriorityQueue::new());
        let queues: [Mutex<PriorityQueue>; NRQS] = [EMPTY_QUEUE; NRQS];

        Self {
            queues,
            bitmap_high: AtomicU64::new(0),
            bitmap_low: AtomicU64::new(0),
            count: AtomicU32::new(0),
            highest_priority: AtomicU32::new(IDLE_PRI + 1), // No threads
        }
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.count.load(Ordering::SeqCst) == 0
    }

    /// Get total thread count
    pub fn count(&self) -> u32 {
        self.count.load(Ordering::SeqCst)
    }

    /// Enqueue a thread at its priority level
    pub fn enqueue(&self, entry: RunqEntry) {
        let pri = entry.priority.value() as usize;
        assert!(pri < NRQS, "Priority {} out of range", pri);

        // Add to appropriate priority queue
        {
            let mut queue = self.queues[pri].lock();
            queue.push(entry);
        }

        // Update bitmap
        self.set_bitmap_bit(pri);

        // Update count
        self.count.fetch_add(1, Ordering::SeqCst);

        // Update highest priority hint
        self.update_highest_priority(pri as u32);
    }

    /// Dequeue the highest priority thread
    pub fn dequeue(&self) -> Option<RunqEntry> {
        // Find highest priority with threads
        let pri = self.find_highest_priority()?;

        // Dequeue from that priority
        let entry = {
            let mut queue = self.queues[pri].lock();
            queue.pop()
        };

        if let Some(ref _e) = entry {
            // Update count
            self.count.fetch_sub(1, Ordering::SeqCst);

            // Check if queue is now empty and update bitmap
            if self.queues[pri].lock().is_empty() {
                self.clear_bitmap_bit(pri);
            }
        }

        entry
    }

    /// Dequeue a specific thread by ID
    pub fn dequeue_thread(&self, thread_id: ThreadId) -> Option<RunqEntry> {
        // Need to search all priority levels
        for pri in 0..NRQS {
            let mut queue = self.queues[pri].lock();
            if let Some(entry) = queue.remove(thread_id) {
                // Update count
                self.count.fetch_sub(1, Ordering::SeqCst);

                // Update bitmap if queue is now empty
                if queue.is_empty() {
                    drop(queue);
                    self.clear_bitmap_bit(pri);
                }

                return Some(entry);
            }
        }
        None
    }

    /// Peek at highest priority thread without removing
    pub fn peek(&self) -> Option<ThreadId> {
        let pri = self.find_highest_priority()?;
        let queue = self.queues[pri].lock();
        queue.front().map(|e| e.thread_id)
    }

    /// Get highest priority value (lower = higher priority)
    pub fn highest_priority(&self) -> Option<u32> {
        if self.is_empty() {
            None
        } else {
            self.find_highest_priority().map(|p| p as u32)
        }
    }

    /// Find the highest priority level with threads
    fn find_highest_priority(&self) -> Option<usize> {
        // Check high bitmap (priorities 0-63) first
        let high = self.bitmap_high.load(Ordering::SeqCst);
        if high != 0 {
            // Find first set bit (highest priority)
            let bit = high.leading_zeros() as usize;
            let pri = 63 - bit; // Convert to priority
            return Some(pri);
        }

        // Check low bitmap (priorities 64-127)
        let low = self.bitmap_low.load(Ordering::SeqCst);
        if low != 0 {
            let bit = low.leading_zeros() as usize;
            let pri = 64 + (63 - bit);
            return Some(pri);
        }

        None
    }

    /// Set a bit in the priority bitmap
    fn set_bitmap_bit(&self, pri: usize) {
        if pri < 64 {
            let mask = 1u64 << (63 - pri);
            self.bitmap_high.fetch_or(mask, Ordering::SeqCst);
        } else {
            let mask = 1u64 << (63 - (pri - 64));
            self.bitmap_low.fetch_or(mask, Ordering::SeqCst);
        }
    }

    /// Clear a bit in the priority bitmap
    fn clear_bitmap_bit(&self, pri: usize) {
        if pri < 64 {
            let mask = !(1u64 << (63 - pri));
            self.bitmap_high.fetch_and(mask, Ordering::SeqCst);
        } else {
            let mask = !(1u64 << (63 - (pri - 64)));
            self.bitmap_low.fetch_and(mask, Ordering::SeqCst);
        }
    }

    /// Update highest priority hint
    fn update_highest_priority(&self, new_pri: u32) {
        loop {
            let current = self.highest_priority.load(Ordering::SeqCst);
            if new_pri >= current {
                break; // New priority is lower or equal, no update needed
            }
            if self
                .highest_priority
                .compare_exchange(current, new_pri, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Get count at specific priority
    pub fn count_at_priority(&self, pri: u32) -> usize {
        if pri as usize >= NRQS {
            return 0;
        }
        self.queues[pri as usize].lock().len()
    }

    /// Requeue thread at new priority (priority change)
    pub fn requeue(&self, thread_id: ThreadId, new_priority: Priority) {
        if let Some(mut entry) = self.dequeue_thread(thread_id) {
            entry.priority = new_priority;
            self.enqueue(entry);
        }
    }
}

impl Default for RunQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Per-Processor Run Queue
// ============================================================================

/// Per-processor run queue state
#[derive(Debug)]
pub struct ProcessorRunQueue {
    /// Processor ID
    processor_id: u32,

    /// Local run queue (processor-bound threads)
    local: RunQueue,

    /// Reference to shared processor set queue
    shared: Option<Arc<RunQueue>>,

    /// Currently running thread (if any)
    current: Mutex<Option<ThreadId>>,

    /// Next thread to run (direct dispatch)
    next: Mutex<Option<ThreadId>>,

    /// Is this processor idle?
    idle: AtomicU32,
}

impl ProcessorRunQueue {
    /// Create a new processor run queue
    pub fn new(processor_id: u32) -> Self {
        Self {
            processor_id,
            local: RunQueue::new(),
            shared: None,
            current: Mutex::new(None),
            next: Mutex::new(None),
            idle: AtomicU32::new(1), // Start idle
        }
    }

    /// Create with shared queue
    pub fn with_shared(processor_id: u32, shared: Arc<RunQueue>) -> Self {
        Self {
            processor_id,
            local: RunQueue::new(),
            shared: Some(shared),
            current: Mutex::new(None),
            next: Mutex::new(None),
            idle: AtomicU32::new(1),
        }
    }

    /// Get processor ID
    pub fn id(&self) -> u32 {
        self.processor_id
    }

    /// Check if processor is idle
    pub fn is_idle(&self) -> bool {
        self.idle.load(Ordering::SeqCst) != 0
    }

    /// Set idle state
    pub fn set_idle(&self, idle: bool) {
        self.idle.store(if idle { 1 } else { 0 }, Ordering::SeqCst);
    }

    /// Get current thread
    pub fn current_thread(&self) -> Option<ThreadId> {
        *self.current.lock()
    }

    /// Set current thread
    pub fn set_current(&self, thread: Option<ThreadId>) {
        *self.current.lock() = thread;
        self.set_idle(thread.is_none());
    }

    /// Get next thread (direct dispatch)
    pub fn next_thread(&self) -> Option<ThreadId> {
        self.next.lock().take()
    }

    /// Set next thread for direct dispatch
    pub fn set_next(&self, thread: ThreadId) {
        *self.next.lock() = Some(thread);
    }

    /// Enqueue thread on local queue
    pub fn enqueue_local(&self, entry: RunqEntry) {
        self.local.enqueue(entry);
    }

    /// Dequeue from local queue
    pub fn dequeue_local(&self) -> Option<RunqEntry> {
        self.local.dequeue()
    }

    /// Get local queue reference
    pub fn local_queue(&self) -> &RunQueue {
        &self.local
    }

    /// Get shared queue reference
    pub fn shared_queue(&self) -> Option<&Arc<RunQueue>> {
        self.shared.as_ref()
    }

    /// Select next thread to run (checks local then shared)
    pub fn select_thread(&self) -> Option<RunqEntry> {
        // Check for direct dispatch first
        if let Some(thread_id) = self.next_thread() {
            // Need to find the thread's entry (simplified)
            return Some(RunqEntry::new(thread_id, Priority::default()));
        }

        // Try local queue first
        if let Some(entry) = self.local.dequeue() {
            return Some(entry);
        }

        // Try shared queue
        if let Some(shared) = &self.shared {
            return shared.dequeue();
        }

        None
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_queue() {
        let mut pq = PriorityQueue::new();
        assert!(pq.is_empty());

        let entry = RunqEntry::new(ThreadId(1), Priority::new(64));
        pq.push(entry);
        assert_eq!(pq.len(), 1);

        let popped = pq.pop().unwrap();
        assert_eq!(popped.thread_id.0, 1);
        assert!(pq.is_empty());
    }

    #[test]
    fn test_run_queue_basic() {
        let rq = RunQueue::new();
        assert!(rq.is_empty());

        // Enqueue threads at different priorities
        rq.enqueue(RunqEntry::new(ThreadId(1), Priority::new(64)));
        rq.enqueue(RunqEntry::new(ThreadId(2), Priority::new(32)));
        rq.enqueue(RunqEntry::new(ThreadId(3), Priority::new(96)));

        assert_eq!(rq.count(), 3);

        // Should dequeue in priority order (lower number = higher priority)
        let e1 = rq.dequeue().unwrap();
        assert_eq!(e1.thread_id.0, 2); // Priority 32

        let e2 = rq.dequeue().unwrap();
        assert_eq!(e2.thread_id.0, 1); // Priority 64

        let e3 = rq.dequeue().unwrap();
        assert_eq!(e3.thread_id.0, 3); // Priority 96

        assert!(rq.is_empty());
    }

    #[test]
    fn test_bitmap_operations() {
        let rq = RunQueue::new();

        // Add thread at priority 0 (highest)
        rq.enqueue(RunqEntry::new(ThreadId(1), Priority::new(0)));
        assert_eq!(rq.highest_priority(), Some(0));

        // Add thread at priority 127 (lowest)
        rq.enqueue(RunqEntry::new(ThreadId(2), Priority::new(127)));

        // Should still get priority 0 first
        assert_eq!(rq.highest_priority(), Some(0));

        rq.dequeue();
        assert_eq!(rq.highest_priority(), Some(127));
    }

    #[test]
    fn test_processor_run_queue() {
        let prq = ProcessorRunQueue::new(0);
        assert!(prq.is_idle());

        prq.enqueue_local(RunqEntry::new(ThreadId(1), Priority::new(64)));
        assert!(!prq.local_queue().is_empty());

        let entry = prq.select_thread().unwrap();
        assert_eq!(entry.thread_id.0, 1);
    }
}
