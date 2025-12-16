//! Generic Queue - Doubly-linked list for kernel data structures
//!
//! Based on Mach4 kern/queue.h by Avadis Tevanian Jr. (1985)
//!
//! This module provides a generic doubly-linked list (queue) that is the
//! fundamental data structure used throughout the Mach kernel. The queue
//! is maintained within the objects being queued, supporting fast removal
//! from anywhere in the queue.
//!
//! ## Design Notes
//!
//! Unlike typical linked lists where nodes point to data, Mach queues embed
//! the queue linkage directly in the data structures. This allows:
//! - O(1) removal without knowing which queue contains the element
//! - Multiple queue chains through the same object
//! - No separate memory allocation for queue nodes
//!
//! ## Usage Pattern
//!
//! ```ignore
//! struct MyItem {
//!     links: QueueChain,  // Queue linkage embedded in structure
//!     data: u32,
//! }
//!
//! let mut queue = QueueHead::new();
//! queue.enqueue_tail(&mut item.links);
//! ```

use core::ptr::NonNull;

// ============================================================================
// Queue Entry - The fundamental building block
// ============================================================================

/// A queue chain entry - embedded in structures that can be queued
///
/// This is the doubly-linked list node that gets embedded directly in
/// data structures. Each structure can have multiple QueueChain fields
/// to be on multiple queues simultaneously.
#[derive(Debug)]
#[repr(C)]
pub struct QueueChain {
    /// Next element in the queue
    next: Option<NonNull<QueueChain>>,
    /// Previous element in the queue
    prev: Option<NonNull<QueueChain>>,
}

impl QueueChain {
    /// Create a new unlinked queue chain
    pub const fn new() -> Self {
        Self {
            next: None,
            prev: None,
        }
    }

    /// Check if this entry is linked into a queue
    pub fn is_linked(&self) -> bool {
        self.next.is_some() || self.prev.is_some()
    }

    /// Get the next entry
    pub fn next(&self) -> Option<NonNull<QueueChain>> {
        self.next
    }

    /// Get the previous entry
    pub fn prev(&self) -> Option<NonNull<QueueChain>> {
        self.prev
    }

    /// Unlink this entry (internal use)
    fn unlink(&mut self) {
        self.next = None;
        self.prev = None;
    }
}

impl Default for QueueChain {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Queue Head - The queue itself
// ============================================================================

/// A queue head - represents the queue itself
///
/// The queue is circular: an empty queue has head pointing to itself.
/// This allows O(1) operations for both head and tail.
#[derive(Debug)]
pub struct QueueHead {
    /// The sentinel entry (points to first/last elements)
    head: QueueChain,
}

impl QueueHead {
    /// Create a new empty queue
    pub const fn new() -> Self {
        Self {
            head: QueueChain::new(),
        }
    }

    /// Initialize the queue (for already-allocated queues)
    pub fn init(&mut self) {
        // Empty queue: head points to itself
        let ptr = NonNull::new(&mut self.head as *mut _);
        self.head.next = ptr;
        self.head.prev = ptr;
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        match self.head.next {
            Some(next) => core::ptr::eq(next.as_ptr(), &self.head),
            None => true, // Uninitialized queue is empty
        }
    }

    /// Get the first element (returns None if empty)
    pub fn first(&self) -> Option<NonNull<QueueChain>> {
        let next = self.head.next?;
        if core::ptr::eq(next.as_ptr(), &self.head) {
            None // Queue is empty
        } else {
            Some(next)
        }
    }

    /// Get the last element (returns None if empty)
    pub fn last(&self) -> Option<NonNull<QueueChain>> {
        let prev = self.head.prev?;
        if core::ptr::eq(prev.as_ptr(), &self.head) {
            None // Queue is empty
        } else {
            Some(prev)
        }
    }

    /// Enqueue an element at the tail of the queue
    ///
    /// # Safety
    /// The element must not already be in a queue.
    pub fn enqueue_tail(&mut self, elt: &mut QueueChain) {
        debug_assert!(!elt.is_linked(), "Element already in a queue");

        let elt_ptr = NonNull::new(elt as *mut _).unwrap();

        if let Some(mut prev) = self.head.prev {
            // Queue not empty - link after previous tail
            unsafe {
                prev.as_mut().next = Some(elt_ptr);
            }
            elt.prev = Some(prev);
        } else {
            // Queue empty or uninitialized - initialize and set as first
            let head_ptr = NonNull::new(&mut self.head as *mut _).unwrap();
            self.head.next = Some(elt_ptr);
            elt.prev = Some(head_ptr);
        }

        // New element becomes the tail
        let head_ptr = NonNull::new(&mut self.head as *mut _).unwrap();
        elt.next = Some(head_ptr);
        self.head.prev = Some(elt_ptr);
    }

    /// Enqueue an element at the head of the queue
    ///
    /// # Safety
    /// The element must not already be in a queue.
    pub fn enqueue_head(&mut self, elt: &mut QueueChain) {
        debug_assert!(!elt.is_linked(), "Element already in a queue");

        let elt_ptr = NonNull::new(elt as *mut _).unwrap();
        let head_ptr = NonNull::new(&mut self.head as *mut _).unwrap();

        if let Some(mut next) = self.head.next {
            // Check if queue is not empty (next != head)
            if !core::ptr::eq(next.as_ptr(), &self.head) {
                // Queue not empty - link before current head
                unsafe {
                    next.as_mut().prev = Some(elt_ptr);
                }
                elt.next = Some(next);
            } else {
                // Queue empty - element becomes both head and tail
                elt.next = Some(head_ptr);
                self.head.prev = Some(elt_ptr);
            }
        } else {
            // Uninitialized - element is first and last
            elt.next = Some(head_ptr);
            self.head.prev = Some(elt_ptr);
        }

        // New element becomes the head
        elt.prev = Some(head_ptr);
        self.head.next = Some(elt_ptr);
    }

    /// Dequeue and return the first element
    pub fn dequeue_head(&mut self) -> Option<NonNull<QueueChain>> {
        let first = self.first()?;

        // Remove from queue
        unsafe {
            let elt = first.as_ptr();
            let next = (*elt).next;
            let head_ptr = NonNull::new(&mut self.head as *mut _).unwrap();

            // Update head to point to next element
            self.head.next = next;

            // Update next element's prev pointer
            if let Some(mut next_entry) = next {
                if !core::ptr::eq(next_entry.as_ptr(), &self.head) {
                    next_entry.as_mut().prev = Some(head_ptr);
                } else {
                    // Queue is now empty
                    self.head.prev = Some(head_ptr);
                }
            }

            // Unlink the removed element
            (*elt).unlink();
        }

        Some(first)
    }

    /// Dequeue and return the last element
    pub fn dequeue_tail(&mut self) -> Option<NonNull<QueueChain>> {
        let last = self.last()?;

        // Remove from queue
        unsafe {
            let elt = last.as_ptr();
            let prev = (*elt).prev;
            let head_ptr = NonNull::new(&mut self.head as *mut _).unwrap();

            // Update head to point to new tail
            self.head.prev = prev;

            // Update prev element's next pointer
            if let Some(mut prev_entry) = prev {
                if !core::ptr::eq(prev_entry.as_ptr(), &self.head) {
                    prev_entry.as_mut().next = Some(head_ptr);
                } else {
                    // Queue is now empty
                    self.head.next = Some(head_ptr);
                }
            }

            // Unlink the removed element
            (*elt).unlink();
        }

        Some(last)
    }

    /// Remove an arbitrary element from the queue
    ///
    /// # Safety
    /// The element must be in THIS queue.
    pub fn remove(&mut self, elt: &mut QueueChain) {
        if !elt.is_linked() {
            return;
        }

        let prev = elt.prev;
        let next = elt.next;

        // Update prev's next pointer
        if let Some(mut p) = prev {
            unsafe {
                p.as_mut().next = next;
            }
        }

        // Update next's prev pointer
        if let Some(mut n) = next {
            unsafe {
                n.as_mut().prev = prev;
            }
        }

        elt.unlink();
    }

    /// Get the head pointer for low-level operations
    pub fn head_ptr(&mut self) -> NonNull<QueueChain> {
        NonNull::new(&mut self.head as *mut _).unwrap()
    }
}

impl Default for QueueHead {
    fn default() -> Self {
        let mut q = Self::new();
        q.init();
        q
    }
}

// ============================================================================
// Convenience Aliases (matching Mach naming)
// ============================================================================

/// Alias for enqueue (defaults to tail)
pub fn enqueue(queue: &mut QueueHead, elt: &mut QueueChain) {
    queue.enqueue_tail(elt);
}

/// Alias for dequeue (defaults to head)
pub fn dequeue(queue: &mut QueueHead) -> Option<NonNull<QueueChain>> {
    queue.dequeue_head()
}

// ============================================================================
// MP Queue - Queue with lock for multiprocessor use
// ============================================================================

/// A queue with an integrated lock for MP-safe access
#[derive(Debug)]
pub struct MpQueueHead {
    /// The underlying queue
    queue: QueueHead,
    /// Lock protecting the queue
    lock: spin::Mutex<()>,
}

impl MpQueueHead {
    /// Create a new locked queue
    pub fn new() -> Self {
        Self {
            queue: QueueHead::default(),
            lock: spin::Mutex::new(()),
        }
    }

    /// Enqueue with locking
    pub fn enqueue_tail(&mut self, elt: &mut QueueChain) {
        let _guard = self.lock.lock();
        self.queue.enqueue_tail(elt);
    }

    /// Dequeue with locking
    pub fn dequeue_head(&mut self) -> Option<NonNull<QueueChain>> {
        let _guard = self.lock.lock();
        self.queue.dequeue_head()
    }

    /// Check if empty (with locking)
    pub fn is_empty(&self) -> bool {
        let _guard = self.lock.lock();
        self.queue.is_empty()
    }
}

impl Default for MpQueueHead {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Type-Safe Queue Iterator
// ============================================================================

/// Iterator over queue elements
pub struct QueueIter<'a> {
    head: &'a QueueHead,
    current: Option<NonNull<QueueChain>>,
}

impl<'a> QueueIter<'a> {
    pub fn new(queue: &'a QueueHead) -> Self {
        Self {
            head: queue,
            current: queue.first(),
        }
    }
}

impl<'a> Iterator for QueueIter<'a> {
    type Item = NonNull<QueueChain>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;

        // Advance to next
        unsafe {
            let next = (*current.as_ptr()).next;
            if let Some(n) = next {
                if core::ptr::eq(n.as_ptr(), &self.head.head) {
                    self.current = None; // Reached end
                } else {
                    self.current = Some(n);
                }
            } else {
                self.current = None;
            }
        }

        Some(current)
    }
}

// ============================================================================
// Queue Statistics
// ============================================================================

/// Statistics for queue operations
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    pub enqueues: u64,
    pub dequeues: u64,
    pub removes: u64,
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the queue subsystem
pub fn init() {
    // Queue is a pure data structure, no global initialization needed
}

// ============================================================================
// Helper Macros (would be macros in C, here as inline functions)
// ============================================================================

/// Check if we've reached the end of iteration
#[inline]
pub fn queue_end(head: &QueueHead, entry: NonNull<QueueChain>) -> bool {
    core::ptr::eq(entry.as_ptr(), &head.head)
}

/// Get the next entry in iteration
#[inline]
pub fn queue_next(entry: NonNull<QueueChain>) -> Option<NonNull<QueueChain>> {
    unsafe { (*entry.as_ptr()).next }
}

/// Get the previous entry
#[inline]
pub fn queue_prev(entry: NonNull<QueueChain>) -> Option<NonNull<QueueChain>> {
    unsafe { (*entry.as_ptr()).prev }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_init() {
        let mut queue = QueueHead::default();
        assert!(queue.is_empty());
        assert!(queue.first().is_none());
        assert!(queue.last().is_none());
    }

    #[test]
    fn test_enqueue_dequeue_tail() {
        let mut queue = QueueHead::default();
        let mut e1 = QueueChain::new();
        let mut e2 = QueueChain::new();
        let mut e3 = QueueChain::new();

        queue.enqueue_tail(&mut e1);
        queue.enqueue_tail(&mut e2);
        queue.enqueue_tail(&mut e3);

        assert!(!queue.is_empty());

        // Dequeue should return in FIFO order
        let d1 = queue.dequeue_head();
        assert!(d1.is_some());

        let d2 = queue.dequeue_head();
        assert!(d2.is_some());

        let d3 = queue.dequeue_head();
        assert!(d3.is_some());

        assert!(queue.is_empty());
        assert!(queue.dequeue_head().is_none());
    }

    #[test]
    fn test_enqueue_head() {
        let mut queue = QueueHead::default();
        let mut e1 = QueueChain::new();
        let mut e2 = QueueChain::new();

        queue.enqueue_head(&mut e1);
        queue.enqueue_head(&mut e2);

        // e2 should be first (LIFO at head)
        let first = queue.first();
        assert!(first.is_some());
    }

    #[test]
    fn test_remove_middle() {
        let mut queue = QueueHead::default();
        let mut e1 = QueueChain::new();
        let mut e2 = QueueChain::new();
        let mut e3 = QueueChain::new();

        queue.enqueue_tail(&mut e1);
        queue.enqueue_tail(&mut e2);
        queue.enqueue_tail(&mut e3);

        // Remove middle element
        queue.remove(&mut e2);

        // Should still have 2 elements
        assert!(!queue.is_empty());
        let _ = queue.dequeue_head();
        let _ = queue.dequeue_head();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_chain_linking() {
        let chain = QueueChain::new();
        assert!(!chain.is_linked());
    }

    #[test]
    fn test_mp_queue() {
        let mut mpq = MpQueueHead::new();
        assert!(mpq.is_empty());

        let mut e1 = QueueChain::new();
        mpq.enqueue_tail(&mut e1);

        assert!(!mpq.is_empty());

        let _ = mpq.dequeue_head();
        assert!(mpq.is_empty());
    }
}
