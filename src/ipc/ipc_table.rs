//! IPC Table Management - Dynamic capability table allocation
//!
//! Based on Mach4 ipc/ipc_table.h/c by Rich Draves (1989)
//!
//! This module manages dynamic allocation and resizing of IPC capability tables.
//! Tables are used for:
//! - IPC entries (ipc_entry_t) - port name to capability mapping
//! - Dead-name requests (ipc_port_request_t) - notification tracking
//!
//! ## Table Size Strategy
//!
//! The table sizes follow a growth pattern:
//! - First: powers of 2 up to page size
//! - Then: increments of page size (1, 2, 4, 8 pages)
//!
//! This provides efficient memory usage while minimizing reallocations.

use alloc::vec::Vec;
use core::mem::size_of;
use spin::Mutex;

// ============================================================================
// Constants
// ============================================================================

/// Page size for table calculations
pub const PAGE_SIZE: usize = 4096;

/// Number of entry table size entries
pub const IPC_TABLE_ENTRIES_SIZE: usize = 512;

/// Number of dead-name request table size entries
pub const IPC_TABLE_DNREQUESTS_SIZE: usize = 64;

/// Minimum number of entries in an entry table
pub const MIN_ENTRIES: usize = 4;

/// Minimum number of entries in a dead-name request table
pub const MIN_DNREQUESTS: usize = 2;

// ============================================================================
// Table Size Structure
// ============================================================================

/// Table size descriptor
///
/// Points to the size configuration for a table.
/// The `is_table_next` field of an IPC space points to the next larger size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpcTableSize {
    /// Number of elements in table
    pub its_size: usize,
}

impl IpcTableSize {
    pub const NULL: Self = Self { its_size: 0 };

    pub fn new(size: usize) -> Self {
        Self { its_size: size }
    }

    pub fn is_null(&self) -> bool {
        self.its_size == 0
    }

    /// Check if a table of this size can be reallocated (page-aligned)
    pub fn is_reallocable<T>(&self) -> bool {
        self.its_size * size_of::<T>() >= PAGE_SIZE
    }

    /// Get byte size for a table of element type T
    pub fn byte_size<T>(&self) -> usize {
        self.its_size * size_of::<T>()
    }
}

impl Default for IpcTableSize {
    fn default() -> Self {
        Self::NULL
    }
}

// ============================================================================
// Table Size Array
// ============================================================================

/// Array of table sizes for growth strategy
#[derive(Debug)]
pub struct IpcTableSizes {
    /// The size entries
    sizes: Vec<IpcTableSize>,
    /// Element size this array was computed for
    elem_size: usize,
}

impl IpcTableSizes {
    /// Create a new table sizes array
    pub fn new(num: usize, min_elems: usize, elem_size: usize) -> Self {
        let sizes = Self::fill(num, min_elems, elem_size);
        Self { sizes, elem_size }
    }

    /// Fill table sizes following Mach's growth strategy
    fn fill(num: usize, min_elems: usize, elem_size: usize) -> Vec<IpcTableSize> {
        let mut sizes = Vec::with_capacity(num);
        let min_size = min_elems * elem_size;

        // First use powers of two, up to the page size
        let mut size = 1usize;
        while sizes.len() < num.saturating_sub(1) && size < PAGE_SIZE {
            if size >= min_size {
                sizes.push(IpcTableSize::new(size / elem_size));
            }
            size = size.saturating_mul(2);
        }

        // Then increments of a page, then two pages, etc.
        let mut incr_size = PAGE_SIZE;
        while sizes.len() < num.saturating_sub(1) {
            for _ in 0..15 {
                if sizes.len() >= num.saturating_sub(1) {
                    break;
                }
                if size >= min_size {
                    sizes.push(IpcTableSize::new(size / elem_size));
                }
                size = size.saturating_add(incr_size);
            }
            if incr_size < PAGE_SIZE << 3 {
                incr_size <<= 1;
            }
        }

        // Ensure we have at least one entry
        if sizes.is_empty() {
            sizes.push(IpcTableSize::new(min_elems));
        }

        // The last element should duplicate the previous for termination
        if let Some(last) = sizes.last().copied() {
            sizes.push(last);
        }

        sizes
    }

    /// Get size at index
    pub fn get(&self, index: usize) -> Option<IpcTableSize> {
        self.sizes.get(index).copied()
    }

    /// Get the next larger size
    pub fn next_size(&self, current: IpcTableSize) -> Option<IpcTableSize> {
        for (i, &size) in self.sizes.iter().enumerate() {
            if size.its_size > current.its_size {
                return Some(size);
            }
            // Check if we've hit the terminating duplicate
            if i > 0 && size.its_size == self.sizes[i - 1].its_size {
                return None;
            }
        }
        None
    }

    /// Get the initial (smallest) size
    pub fn initial_size(&self) -> IpcTableSize {
        self.sizes.first().copied().unwrap_or(IpcTableSize::NULL)
    }

    /// Get the maximum size
    pub fn max_size(&self) -> IpcTableSize {
        // Second to last is the max (last is duplicate terminator)
        if self.sizes.len() >= 2 {
            self.sizes[self.sizes.len() - 2]
        } else {
            self.sizes.last().copied().unwrap_or(IpcTableSize::NULL)
        }
    }

    /// Get number of size entries
    pub fn len(&self) -> usize {
        self.sizes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.sizes.is_empty()
    }

    /// Get element size this was computed for
    pub fn elem_size(&self) -> usize {
        self.elem_size
    }
}

// ============================================================================
// Table Allocation
// ============================================================================

/// Statistics for table allocation
#[derive(Debug, Clone, Default)]
pub struct IpcTableStats {
    /// Number of tables allocated
    pub allocs: u64,
    /// Number of tables freed
    pub frees: u64,
    /// Number of tables reallocated
    pub reallocs: u64,
    /// Total bytes currently allocated
    pub bytes_allocated: u64,
    /// Peak bytes allocated
    pub peak_bytes: u64,
}

/// Table allocator
#[derive(Debug)]
pub struct IpcTableAllocator {
    /// Allocation statistics
    stats: Mutex<IpcTableStats>,
}

impl IpcTableAllocator {
    pub const fn new() -> Self {
        Self {
            stats: Mutex::new(IpcTableStats {
                allocs: 0,
                frees: 0,
                reallocs: 0,
                bytes_allocated: 0,
                peak_bytes: 0,
            }),
        }
    }

    /// Allocate a table
    ///
    /// For small tables (< PAGE_SIZE), uses kalloc.
    /// For large tables, uses kmem_alloc from kalloc_map.
    pub fn alloc(&self, size: usize) -> Option<*mut u8> {
        if size == 0 {
            return None;
        }

        // In real implementation, this would call kalloc or kmem_alloc
        // For now, we use the Rust allocator
        let layout = core::alloc::Layout::from_size_align(size, 8).ok()?;
        let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };

        if ptr.is_null() {
            return None;
        }

        // Update stats
        {
            let mut stats = self.stats.lock();
            stats.allocs += 1;
            stats.bytes_allocated += size as u64;
            if stats.bytes_allocated > stats.peak_bytes {
                stats.peak_bytes = stats.bytes_allocated;
            }
        }

        Some(ptr)
    }

    /// Reallocate a table (remap without copying)
    ///
    /// Only works for page-sized or bigger tables.
    pub fn realloc(&self, old_ptr: *mut u8, old_size: usize, new_size: usize) -> Option<*mut u8> {
        if old_size < PAGE_SIZE {
            // Small tables can't be remapped, need copy
            return self.realloc_copy(old_ptr, old_size, new_size);
        }

        // In real implementation, this would use kmem_realloc to remap
        // without copying. For now, we do a copy.
        self.realloc_copy(old_ptr, old_size, new_size)
    }

    /// Reallocate by copying (fallback)
    fn realloc_copy(&self, old_ptr: *mut u8, old_size: usize, new_size: usize) -> Option<*mut u8> {
        let new_ptr = self.alloc(new_size)?;

        // Copy old data
        let copy_size = old_size.min(new_size);
        unsafe {
            core::ptr::copy_nonoverlapping(old_ptr, new_ptr, copy_size);
        }

        // Free old table
        // SAFETY: old_ptr was allocated by this allocator with old_size
        unsafe {
            self.free(old_ptr, old_size);
        }

        // Update stats
        {
            let mut stats = self.stats.lock();
            stats.reallocs += 1;
        }

        Some(new_ptr)
    }

    /// Free a table
    ///
    /// # Safety
    /// The caller must ensure that `ptr` was allocated by this allocator
    /// with the given `size`.
    pub unsafe fn free(&self, ptr: *mut u8, size: usize) {
        if ptr.is_null() || size == 0 {
            return;
        }

        // In real implementation, this would call kfree or kmem_free
        let layout = match core::alloc::Layout::from_size_align(size, 8) {
            Ok(l) => l,
            Err(_) => return,
        };

        alloc::alloc::dealloc(ptr, layout);

        // Update stats
        {
            let mut stats = self.stats.lock();
            stats.frees += 1;
            stats.bytes_allocated = stats.bytes_allocated.saturating_sub(size as u64);
        }
    }

    /// Get allocation statistics
    pub fn stats(&self) -> IpcTableStats {
        self.stats.lock().clone()
    }
}

impl Default for IpcTableAllocator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

/// Entry table sizes
static ENTRY_SIZES: spin::Once<IpcTableSizes> = spin::Once::new();

/// Dead-name request table sizes
static DNREQUEST_SIZES: spin::Once<IpcTableSizes> = spin::Once::new();

/// Global table allocator
static TABLE_ALLOCATOR: spin::Once<IpcTableAllocator> = spin::Once::new();

fn entry_sizes() -> &'static IpcTableSizes {
    ENTRY_SIZES.call_once(|| {
        // Size of IpcEntry (from entry.rs)
        let entry_size = 32; // Approximate size
        IpcTableSizes::new(IPC_TABLE_ENTRIES_SIZE, MIN_ENTRIES, entry_size)
    })
}

fn dnrequest_sizes() -> &'static IpcTableSizes {
    DNREQUEST_SIZES.call_once(|| {
        // Size of port request
        let request_size = 16; // Approximate size
        IpcTableSizes::new(IPC_TABLE_DNREQUESTS_SIZE, MIN_DNREQUESTS, request_size)
    })
}

fn table_allocator() -> &'static IpcTableAllocator {
    TABLE_ALLOCATOR.call_once(IpcTableAllocator::new)
}

/// Initialize the IPC table subsystem
pub fn init() {
    let _ = entry_sizes();
    let _ = dnrequest_sizes();
    let _ = table_allocator();
}

// ============================================================================
// Public API
// ============================================================================

/// Get the initial entry table size
pub fn initial_entry_table_size() -> IpcTableSize {
    entry_sizes().initial_size()
}

/// Get the next larger entry table size
pub fn next_entry_table_size(current: IpcTableSize) -> Option<IpcTableSize> {
    entry_sizes().next_size(current)
}

/// Get the maximum entry table size
pub fn max_entry_table_size() -> IpcTableSize {
    entry_sizes().max_size()
}

/// Get the initial dead-name request table size
pub fn initial_dnrequest_table_size() -> IpcTableSize {
    dnrequest_sizes().initial_size()
}

/// Get the next larger dead-name request table size
pub fn next_dnrequest_table_size(current: IpcTableSize) -> Option<IpcTableSize> {
    dnrequest_sizes().next_size(current)
}

/// Allocate an entry table
pub fn alloc_entry_table(size: IpcTableSize) -> Option<*mut u8> {
    let bytes = size.its_size * entry_sizes().elem_size();
    table_allocator().alloc(bytes)
}

/// Reallocate an entry table
pub fn realloc_entry_table(
    table: *mut u8,
    old_size: IpcTableSize,
    new_size: IpcTableSize,
) -> Option<*mut u8> {
    let old_bytes = old_size.its_size * entry_sizes().elem_size();
    let new_bytes = new_size.its_size * entry_sizes().elem_size();
    table_allocator().realloc(table, old_bytes, new_bytes)
}

/// Free an entry table
///
/// # Safety
/// The caller must ensure `table` was allocated with `alloc_entry_table`.
pub unsafe fn free_entry_table(table: *mut u8, size: IpcTableSize) {
    let bytes = size.its_size * entry_sizes().elem_size();
    table_allocator().free(table, bytes);
}

/// Allocate a dead-name request table
pub fn alloc_dnrequest_table(size: IpcTableSize) -> Option<*mut u8> {
    let bytes = size.its_size * dnrequest_sizes().elem_size();
    table_allocator().alloc(bytes)
}

/// Free a dead-name request table
///
/// # Safety
/// The caller must ensure `table` was allocated with `alloc_dnrequest_table`.
pub unsafe fn free_dnrequest_table(table: *mut u8, size: IpcTableSize) {
    let bytes = size.its_size * dnrequest_sizes().elem_size();
    table_allocator().free(table, bytes);
}

/// Get table allocation statistics
pub fn table_stats() -> IpcTableStats {
    table_allocator().stats()
}

// ============================================================================
// Typed Table Wrapper
// ============================================================================

/// A typed wrapper for IPC tables
#[derive(Debug)]
pub struct IpcTable<T> {
    /// Pointer to table data
    ptr: *mut T,
    /// Current size configuration
    size: IpcTableSize,
    /// Number of elements in use
    count: usize,
}

impl<T: Default + Clone> IpcTable<T> {
    /// Create a new table with initial size
    pub fn new() -> Option<Self> {
        let size = initial_entry_table_size();
        let bytes = size.its_size * size_of::<T>();
        let ptr = table_allocator().alloc(bytes)? as *mut T;

        Some(Self {
            ptr,
            size,
            count: 0,
        })
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.size.its_size
    }

    /// Get element at index
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.size.its_size {
            unsafe { Some(&*self.ptr.add(index)) }
        } else {
            None
        }
    }

    /// Get mutable element at index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.size.its_size {
            unsafe { Some(&mut *self.ptr.add(index)) }
        } else {
            None
        }
    }

    /// Grow the table to next size
    pub fn grow(&mut self) -> bool {
        let new_size = match next_entry_table_size(self.size) {
            Some(s) => s,
            None => return false, // Already at max
        };

        let old_bytes = self.size.its_size * size_of::<T>();
        let new_bytes = new_size.its_size * size_of::<T>();

        let new_ptr = match table_allocator().realloc(self.ptr as *mut u8, old_bytes, new_bytes) {
            Some(p) => p as *mut T,
            None => return false,
        };

        self.ptr = new_ptr;
        self.size = new_size;
        true
    }

    /// Get current size configuration
    pub fn current_size(&self) -> IpcTableSize {
        self.size
    }
}

impl<T: Default + Clone> Default for IpcTable<T> {
    fn default() -> Self {
        Self::new().expect("Failed to allocate IPC table")
    }
}

impl<T> Drop for IpcTable<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            let bytes = self.size.its_size * size_of::<T>();
            // SAFETY: ptr was allocated by table_allocator with this size
            unsafe {
                table_allocator().free(self.ptr as *mut u8, bytes);
            }
        }
    }
}

// Note: IpcTable is not Send/Sync by default due to raw pointer
// In a real kernel, this would be managed with proper synchronization

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_size() {
        let size = IpcTableSize::new(16);
        assert_eq!(size.its_size, 16);
        assert!(!size.is_null());

        let null = IpcTableSize::NULL;
        assert!(null.is_null());
    }

    #[test]
    fn test_table_sizes_growth() {
        let sizes = IpcTableSizes::new(32, 4, 8);
        assert!(!sizes.is_empty());

        let initial = sizes.initial_size();
        assert!(initial.its_size >= 4);

        // Verify sizes are increasing
        let mut prev = initial.its_size;
        for i in 1..sizes.len() - 1 {
            if let Some(size) = sizes.get(i) {
                assert!(size.its_size >= prev);
                prev = size.its_size;
            }
        }
    }

    #[test]
    fn test_allocator() {
        let allocator = IpcTableAllocator::new();

        let ptr = allocator.alloc(64);
        assert!(ptr.is_some());

        let ptr = ptr.unwrap();
        // SAFETY: ptr was just allocated by allocator with size 64
        unsafe {
            allocator.free(ptr, 64);
        }

        let stats = allocator.stats();
        assert_eq!(stats.allocs, 1);
        assert_eq!(stats.frees, 1);
    }

    #[test]
    fn test_next_size() {
        let sizes = IpcTableSizes::new(16, 4, 8);
        let initial = sizes.initial_size();

        let next = sizes.next_size(initial);
        assert!(next.is_some());

        let next = next.unwrap();
        assert!(next.its_size > initial.its_size);
    }
}
