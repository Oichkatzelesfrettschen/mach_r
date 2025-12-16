//! VM Page Management - Physical Page Abstraction
//!
//! Based on Mach4 vm/vm_page.h/c
//! Manages physical memory pages and their states.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::mach_vm::vm_object::VmObjectId;

// ============================================================================
// Constants
// ============================================================================

/// Page size (4KB on most platforms)
pub const PAGE_SIZE: usize = 4096;

/// Page shift (log2 of PAGE_SIZE)
pub const PAGE_SHIFT: usize = 12;

/// Invalid physical address
pub const PHYS_ADDR_INVALID: u64 = u64::MAX;

// ============================================================================
// Page Flags
// ============================================================================

/// Page state flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlags(u32);

impl PageFlags {
    /// Page is in active queue
    pub const ACTIVE: Self = Self(0x0001);
    /// Page is in inactive queue
    pub const INACTIVE: Self = Self(0x0002);
    /// Page is in laundry (being cleaned)
    pub const LAUNDRY: Self = Self(0x0004);
    /// Page is free
    pub const FREE: Self = Self(0x0008);
    /// Page is busy (I/O in progress)
    pub const BUSY: Self = Self(0x0010);
    /// Page wanted by someone
    pub const WANTED: Self = Self(0x0020);
    /// Page is tabled (in object's page table)
    pub const TABLED: Self = Self(0x0040);
    /// Page is fictitious (not backed by real memory)
    pub const FICTITIOUS: Self = Self(0x0080);
    /// Page is private to an object
    pub const PRIVATE: Self = Self(0x0100);
    /// Page is dirty (modified)
    pub const DIRTY: Self = Self(0x0200);
    /// Page was referenced recently
    pub const REFERENCED: Self = Self(0x0400);
    /// Page is locked
    pub const LOCKED: Self = Self(0x0800);
    /// Page is precious (don't discard)
    pub const PRECIOUS: Self = Self(0x1000);
    /// Page is absent (expected but not present)
    pub const ABSENT: Self = Self(0x2000);
    /// Page has error
    pub const ERROR: Self = Self(0x4000);
    /// Page is wired (pinned in memory)
    pub const WIRED: Self = Self(0x8000);

    /// Empty flags
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Get bits
    pub const fn bits(&self) -> u32 {
        self.0
    }

    /// Create from bits
    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits)
    }

    /// Check if contains flags
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Union with another flags
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Intersection with another flags
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Difference from another flags
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl Default for PageFlags {
    fn default() -> Self {
        Self::empty()
    }
}

// ============================================================================
// Page Queue Type
// ============================================================================

/// Page queue type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PageQueueType {
    /// Not on any queue
    None = 0,
    /// Free page queue
    Free = 1,
    /// Active page queue
    Active = 2,
    /// Inactive page queue
    Inactive = 3,
    /// Speculative queue
    Speculative = 4,
    /// Throttled queue
    Throttled = 5,
    /// Wire queue (pinned pages)
    Wire = 6,
}

// ============================================================================
// VM Page Structure
// ============================================================================

/// Virtual Memory Page
///
/// Represents a single physical page of memory.
/// Based on Mach4 vm_page structure.
#[derive(Debug)]
pub struct VmPage {
    /// Physical address of this page
    pub phys_addr: u64,

    /// Object this page belongs to (if any)
    pub object: Mutex<Option<VmObjectId>>,

    /// Offset within the object
    pub offset: AtomicU64,

    /// Which queue this page is on
    pub queue: Mutex<PageQueueType>,

    /// Page flags
    pub flags: AtomicU32,

    /// Wire count (reference count for wiring)
    pub wire_count: AtomicU32,

    /// Reference count
    pub ref_count: AtomicU32,

    /// Page number in system
    pub page_num: u32,

    /// Busy flag (separate for fast checking)
    pub busy: AtomicBool,

    /// Wanted flag (someone waiting for this page)
    pub wanted: AtomicBool,
}

impl VmPage {
    /// Create a new VM page
    pub fn new(phys_addr: u64, page_num: u32) -> Self {
        Self {
            phys_addr,
            object: Mutex::new(None),
            offset: AtomicU64::new(0),
            queue: Mutex::new(PageQueueType::None),
            flags: AtomicU32::new(0),
            wire_count: AtomicU32::new(0),
            ref_count: AtomicU32::new(1),
            page_num,
            busy: AtomicBool::new(false),
            wanted: AtomicBool::new(false),
        }
    }

    /// Create a fictitious page (not backed by real memory)
    pub fn fictitious() -> Self {
        let page = Self::new(PHYS_ADDR_INVALID, 0);
        page.set_flags(PageFlags::FICTITIOUS);
        page
    }

    /// Get page flags
    pub fn get_flags(&self) -> PageFlags {
        PageFlags::from_bits_truncate(self.flags.load(Ordering::SeqCst))
    }

    /// Set page flags
    pub fn set_flags(&self, flags: PageFlags) {
        self.flags.fetch_or(flags.bits(), Ordering::SeqCst);
    }

    /// Clear page flags
    pub fn clear_flags(&self, flags: PageFlags) {
        self.flags.fetch_and(!flags.bits(), Ordering::SeqCst);
    }

    /// Check if page has specific flags
    pub fn has_flags(&self, flags: PageFlags) -> bool {
        self.get_flags().contains(flags)
    }

    /// Mark page as dirty
    pub fn set_dirty(&self) {
        self.set_flags(PageFlags::DIRTY);
    }

    /// Clear dirty flag
    pub fn clear_dirty(&self) {
        self.clear_flags(PageFlags::DIRTY);
    }

    /// Check if page is dirty
    pub fn is_dirty(&self) -> bool {
        self.has_flags(PageFlags::DIRTY)
    }

    /// Mark page as referenced
    pub fn set_referenced(&self) {
        self.set_flags(PageFlags::REFERENCED);
    }

    /// Clear referenced flag
    pub fn clear_referenced(&self) {
        self.clear_flags(PageFlags::REFERENCED);
    }

    /// Check if page is referenced
    pub fn is_referenced(&self) -> bool {
        self.has_flags(PageFlags::REFERENCED)
    }

    /// Lock the page (mark busy)
    pub fn lock(&self) -> bool {
        !self.busy.swap(true, Ordering::SeqCst)
    }

    /// Unlock the page
    pub fn unlock(&self) {
        self.busy.store(false, Ordering::SeqCst);
        // Wake anyone waiting
        if self.wanted.swap(false, Ordering::SeqCst) {
            // In real implementation, would wake waiters
        }
    }

    /// Check if page is busy
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// Wire the page (pin in memory)
    pub fn wire(&self) {
        self.wire_count.fetch_add(1, Ordering::SeqCst);
        self.set_flags(PageFlags::WIRED);
    }

    /// Unwire the page
    pub fn unwire(&self) {
        let prev = self.wire_count.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            self.clear_flags(PageFlags::WIRED);
        }
    }

    /// Check if page is wired
    pub fn is_wired(&self) -> bool {
        self.wire_count.load(Ordering::SeqCst) > 0
    }

    /// Increment reference count
    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement reference count, returns true if page should be freed
    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    /// Get the object this page belongs to
    pub fn get_object(&self) -> Option<VmObjectId> {
        *self.object.lock()
    }

    /// Set the object this page belongs to
    pub fn set_object(&self, object: Option<VmObjectId>, offset: u64) {
        *self.object.lock() = object;
        self.offset.store(offset, Ordering::SeqCst);
        if object.is_some() {
            self.set_flags(PageFlags::TABLED);
        } else {
            self.clear_flags(PageFlags::TABLED);
        }
    }
}

// ============================================================================
// Page Queue
// ============================================================================

/// A queue of pages
#[derive(Debug)]
pub struct PageQueue {
    /// Pages in this queue
    pages: VecDeque<u32>, // Page numbers
    /// Queue type
    queue_type: PageQueueType,
    /// Page count
    count: usize,
}

impl PageQueue {
    /// Create a new page queue
    pub fn new(queue_type: PageQueueType) -> Self {
        Self {
            pages: VecDeque::new(),
            queue_type,
            count: 0,
        }
    }

    /// Add a page to the queue
    pub fn enqueue(&mut self, page_num: u32) {
        self.pages.push_back(page_num);
        self.count += 1;
    }

    /// Add a page to the front of the queue
    pub fn enqueue_front(&mut self, page_num: u32) {
        self.pages.push_front(page_num);
        self.count += 1;
    }

    /// Remove a page from the front of the queue
    pub fn dequeue(&mut self) -> Option<u32> {
        let page = self.pages.pop_front()?;
        self.count -= 1;
        Some(page)
    }

    /// Remove a specific page from the queue
    pub fn remove(&mut self, page_num: u32) -> bool {
        if let Some(pos) = self.pages.iter().position(|&p| p == page_num) {
            self.pages.remove(pos);
            self.count -= 1;
            true
        } else {
            false
        }
    }

    /// Get queue length
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get queue type
    pub fn queue_type(&self) -> PageQueueType {
        self.queue_type
    }
}

// ============================================================================
// Global Page Manager
// ============================================================================

/// Page manager state
pub struct PageManager {
    /// All pages in the system (indexed by page number)
    pages: Vec<VmPage>,

    /// Free page queue
    free_queue: PageQueue,

    /// Active page queue
    active_queue: PageQueue,

    /// Inactive page queue
    inactive_queue: PageQueue,

    /// Wire queue (pinned pages)
    wire_queue: PageQueue,

    /// Total page count
    page_count: usize,

    /// Free page count
    free_count: AtomicU32,

    /// Active page count
    active_count: AtomicU32,

    /// Inactive page count
    inactive_count: AtomicU32,

    /// Wired page count
    wire_count: AtomicU32,

    /// Pages reserved for kernel
    reserved_count: AtomicU32,

    /// Low memory threshold (start reclaiming)
    pages_free_target: u32,

    /// Critical memory threshold
    pages_free_min: u32,
}

impl PageManager {
    /// Create a new page manager
    pub fn new() -> Self {
        Self {
            pages: Vec::new(),
            free_queue: PageQueue::new(PageQueueType::Free),
            active_queue: PageQueue::new(PageQueueType::Active),
            inactive_queue: PageQueue::new(PageQueueType::Inactive),
            wire_queue: PageQueue::new(PageQueueType::Wire),
            page_count: 0,
            free_count: AtomicU32::new(0),
            active_count: AtomicU32::new(0),
            inactive_count: AtomicU32::new(0),
            wire_count: AtomicU32::new(0),
            reserved_count: AtomicU32::new(0),
            pages_free_target: 0,
            pages_free_min: 0,
        }
    }

    /// Initialize with physical memory range
    pub fn init_with_memory(&mut self, start_addr: u64, end_addr: u64) {
        let start_page = (start_addr / PAGE_SIZE as u64) as u32;
        let end_page = (end_addr / PAGE_SIZE as u64) as u32;

        self.page_count = (end_page - start_page) as usize;
        self.pages.reserve(self.page_count);

        for i in 0..self.page_count {
            let page_num = start_page + i as u32;
            let phys_addr = (page_num as u64) * PAGE_SIZE as u64;
            let page = VmPage::new(phys_addr, page_num);
            self.pages.push(page);

            // Add to free queue
            self.free_queue.enqueue(page_num);
        }

        self.free_count
            .store(self.page_count as u32, Ordering::SeqCst);

        // Set memory thresholds
        self.pages_free_target = (self.page_count / 20) as u32; // 5%
        self.pages_free_min = (self.page_count / 50) as u32; // 2%
    }

    /// Allocate a free page
    pub fn alloc(&mut self) -> Option<&VmPage> {
        let page_num = self.free_queue.dequeue()?;
        self.free_count.fetch_sub(1, Ordering::SeqCst);

        // Find page by number
        if let Some(page) = self.pages.iter().find(|p| p.page_num == page_num) {
            page.clear_flags(PageFlags::FREE);
            *page.queue.lock() = PageQueueType::None;
            Some(page)
        } else {
            None
        }
    }

    /// Free a page
    pub fn free(&mut self, page_num: u32) {
        if let Some(page) = self.pages.iter().find(|p| p.page_num == page_num) {
            // Remove from current queue
            match *page.queue.lock() {
                PageQueueType::Active => {
                    self.active_queue.remove(page_num);
                    self.active_count.fetch_sub(1, Ordering::SeqCst);
                }
                PageQueueType::Inactive => {
                    self.inactive_queue.remove(page_num);
                    self.inactive_count.fetch_sub(1, Ordering::SeqCst);
                }
                PageQueueType::Wire => {
                    self.wire_queue.remove(page_num);
                    self.wire_count.fetch_sub(1, Ordering::SeqCst);
                }
                _ => {}
            }

            // Reset page state
            page.flags.store(PageFlags::FREE.bits(), Ordering::SeqCst);
            page.set_object(None, 0);
            *page.queue.lock() = PageQueueType::Free;

            // Add to free queue
            self.free_queue.enqueue(page_num);
            self.free_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Activate a page (move to active queue)
    pub fn activate(&mut self, page_num: u32) {
        if let Some(page) = self.pages.iter().find(|p| p.page_num == page_num) {
            let current_queue = *page.queue.lock();

            // Remove from current queue
            match current_queue {
                PageQueueType::Inactive => {
                    self.inactive_queue.remove(page_num);
                    self.inactive_count.fetch_sub(1, Ordering::SeqCst);
                }
                PageQueueType::Active => return, // Already active
                _ => {}
            }

            // Add to active queue
            self.active_queue.enqueue(page_num);
            self.active_count.fetch_add(1, Ordering::SeqCst);
            *page.queue.lock() = PageQueueType::Active;
            page.set_flags(PageFlags::ACTIVE);
            page.clear_flags(PageFlags::INACTIVE);
        }
    }

    /// Deactivate a page (move to inactive queue)
    pub fn deactivate(&mut self, page_num: u32) {
        if let Some(page) = self.pages.iter().find(|p| p.page_num == page_num) {
            let current_queue = *page.queue.lock();

            // Remove from current queue
            match current_queue {
                PageQueueType::Active => {
                    self.active_queue.remove(page_num);
                    self.active_count.fetch_sub(1, Ordering::SeqCst);
                }
                PageQueueType::Inactive => return, // Already inactive
                _ => {}
            }

            // Add to inactive queue
            self.inactive_queue.enqueue(page_num);
            self.inactive_count.fetch_add(1, Ordering::SeqCst);
            *page.queue.lock() = PageQueueType::Inactive;
            page.set_flags(PageFlags::INACTIVE);
            page.clear_flags(PageFlags::ACTIVE);
        }
    }

    /// Get number of free pages
    pub fn free_count(&self) -> u32 {
        self.free_count.load(Ordering::SeqCst)
    }

    /// Check if memory is low
    pub fn is_memory_low(&self) -> bool {
        self.free_count() < self.pages_free_target
    }

    /// Check if memory is critically low
    pub fn is_memory_critical(&self) -> bool {
        self.free_count() < self.pages_free_min
    }

    /// Get page by number
    pub fn get_page(&self, page_num: u32) -> Option<&VmPage> {
        self.pages.iter().find(|p| p.page_num == page_num)
    }

    /// Get statistics
    pub fn stats(&self) -> PageStats {
        PageStats {
            total: self.page_count as u32,
            free: self.free_count.load(Ordering::SeqCst),
            active: self.active_count.load(Ordering::SeqCst),
            inactive: self.inactive_count.load(Ordering::SeqCst),
            wired: self.wire_count.load(Ordering::SeqCst),
            reserved: self.reserved_count.load(Ordering::SeqCst),
        }
    }

    // ========================================================================
    // Page daemon helper methods
    // ========================================================================

    /// Dequeue from active queue (for pageout daemon)
    pub fn dequeue_active(&mut self) -> Option<u32> {
        let page_num = self.active_queue.dequeue()?;
        self.active_count.fetch_sub(1, Ordering::SeqCst);
        Some(page_num)
    }

    /// Enqueue to active queue (for pageout daemon)
    pub fn enqueue_active(&mut self, page_num: u32) {
        self.active_queue.enqueue(page_num);
        self.active_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Dequeue from inactive queue (for pageout daemon)
    pub fn dequeue_inactive(&mut self) -> Option<u32> {
        let page_num = self.inactive_queue.dequeue()?;
        self.inactive_count.fetch_sub(1, Ordering::SeqCst);
        Some(page_num)
    }

    /// Enqueue to inactive queue (for pageout daemon)
    pub fn enqueue_inactive(&mut self, page_num: u32) {
        self.inactive_queue.enqueue(page_num);
        self.inactive_count.fetch_add(1, Ordering::SeqCst);
    }
}

impl Default for PageManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Page statistics
#[derive(Debug, Clone, Copy)]
pub struct PageStats {
    pub total: u32,
    pub free: u32,
    pub active: u32,
    pub inactive: u32,
    pub wired: u32,
    pub reserved: u32,
}

// ============================================================================
// Global State
// ============================================================================

static PAGE_MANAGER: spin::Once<Mutex<PageManager>> = spin::Once::new();

/// Initialize page subsystem
pub fn init() {
    PAGE_MANAGER.call_once(|| Mutex::new(PageManager::new()));
}

/// Get page manager (internal use)
pub fn page_manager() -> &'static Mutex<PageManager> {
    PAGE_MANAGER.get().expect("Page manager not initialized")
}

/// Initialize with physical memory
pub fn init_memory(start: u64, end: u64) {
    page_manager().lock().init_with_memory(start, end);
}

/// Allocate a page
pub fn alloc_page() -> Option<u64> {
    page_manager().lock().alloc().map(|p| p.phys_addr)
}

/// Free a page
pub fn free_page(phys_addr: u64) {
    let page_num = (phys_addr / PAGE_SIZE as u64) as u32;
    page_manager().lock().free(page_num);
}

/// Get page statistics
pub fn page_stats() -> PageStats {
    page_manager().lock().stats()
}

/// Check if memory is low
pub fn memory_low() -> bool {
    page_manager().lock().is_memory_low()
}

/// Convert address to page number
pub const fn addr_to_page(addr: u64) -> u32 {
    (addr >> PAGE_SHIFT) as u32
}

/// Convert page number to address
pub const fn page_to_addr(page: u32) -> u64 {
    (page as u64) << PAGE_SHIFT
}

/// Round address down to page boundary
pub const fn trunc_page(addr: u64) -> u64 {
    addr & !(PAGE_SIZE as u64 - 1)
}

/// Round address up to page boundary
pub const fn round_page(addr: u64) -> u64 {
    (addr + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_flags() {
        let page = VmPage::new(0x1000, 1);
        assert!(!page.is_dirty());

        page.set_dirty();
        assert!(page.is_dirty());

        page.clear_dirty();
        assert!(!page.is_dirty());
    }

    #[test]
    fn test_page_queue() {
        let mut queue = PageQueue::new(PageQueueType::Free);
        assert!(queue.is_empty());

        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);

        assert_eq!(queue.len(), 3);
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));

        assert!(queue.remove(3));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_page_utils() {
        assert_eq!(addr_to_page(0x5000), 5);
        assert_eq!(page_to_addr(5), 0x5000);
        assert_eq!(trunc_page(0x5678), 0x5000);
        assert_eq!(round_page(0x5001), 0x6000);
    }
}
