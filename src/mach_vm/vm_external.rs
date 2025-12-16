//! VM External Page Management
//!
//! Based on Mach4 vm/vm_external.h/c by CMU (1989)
//!
//! This module maintains a (potentially incomplete) map of pages written to
//! external storage for a range of virtual memory. This is used by the
//! external memory management (EMM) interface to track which pages have been
//! paged out to backing store.
//!
//! ## Use Cases
//!
//! - Track which pages of a memory object have been written to disk
//! - Avoid unnecessary reads from backing store for pages known to be absent
//! - Optimize pager behavior by tracking existence state

use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;

use super::PAGE_SIZE;

// ============================================================================
// Constants
// ============================================================================

/// Small existence map size (bytes)
pub const VM_EXTERNAL_SMALL_SIZE: usize = 128;

/// Large existence map size (bytes)
pub const VM_EXTERNAL_LARGE_SIZE: usize = 8192;

/// Bits per byte in existence map
const BITS_PER_BYTE: usize = 8;

// ============================================================================
// External Page State
// ============================================================================

/// States that may be recorded for a page of external storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum VmExternalState {
    /// Page exists on external storage
    Exists = 1,
    /// Page state is unknown
    #[default]
    Unknown = 2,
    /// Page is known to be absent from external storage
    Absent = 3,
}

impl From<u32> for VmExternalState {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::Exists,
            2 => Self::Unknown,
            3 => Self::Absent,
            _ => Self::Unknown,
        }
    }
}

// ============================================================================
// VM External Structure
// ============================================================================

/// External page existence tracking structure
///
/// Maintains a bitmap tracking which pages have been written to external storage.
#[derive(Debug)]
pub struct VmExternal {
    /// The existence bitmap
    existence_map: Vec<u8>,
    /// Size of the bitmap in bytes
    existence_size: usize,
    /// Number of bits set in the map (pages known to exist)
    existence_count: usize,
    /// Starting offset this map covers
    offset: usize,
}

impl VmExternal {
    /// Create a new VM external tracker
    pub fn new(size_hint: usize) -> Self {
        // Choose appropriate bitmap size
        let existence_size = if size_hint <= VM_EXTERNAL_SMALL_SIZE * BITS_PER_BYTE * PAGE_SIZE {
            VM_EXTERNAL_SMALL_SIZE
        } else {
            VM_EXTERNAL_LARGE_SIZE
        };

        Self {
            existence_map: vec![0u8; existence_size],
            existence_size,
            existence_count: 0,
            offset: 0,
        }
    }

    /// Create a VM external tracker with specific size
    pub fn with_size(existence_size: usize) -> Self {
        Self {
            existence_map: vec![0u8; existence_size],
            existence_size,
            existence_count: 0,
            offset: 0,
        }
    }

    /// Create a small tracker
    pub fn small() -> Self {
        Self::with_size(VM_EXTERNAL_SMALL_SIZE)
    }

    /// Create a large tracker
    pub fn large() -> Self {
        Self::with_size(VM_EXTERNAL_LARGE_SIZE)
    }

    /// Get the size of the existence map in bytes
    pub fn size(&self) -> usize {
        self.existence_size
    }

    /// Get the number of pages this can track
    pub fn capacity(&self) -> usize {
        self.existence_size * BITS_PER_BYTE
    }

    /// Get the number of pages known to exist
    pub fn count(&self) -> usize {
        self.existence_count
    }

    /// Check if empty (no pages tracked as existing)
    pub fn is_empty(&self) -> bool {
        self.existence_count == 0
    }

    /// Convert page offset to bit index
    fn offset_to_bit(&self, offset: usize) -> Option<(usize, u8)> {
        let page_num = offset / PAGE_SIZE;
        if page_num >= self.capacity() {
            return None;
        }
        let byte_index = page_num / BITS_PER_BYTE;
        let bit_offset = (page_num % BITS_PER_BYTE) as u8;
        Some((byte_index, bit_offset))
    }

    /// Get the state of a page at the given offset
    pub fn get_state(&self, offset: usize) -> VmExternalState {
        match self.offset_to_bit(offset) {
            Some((byte_idx, bit_off)) => {
                if byte_idx < self.existence_map.len() {
                    if (self.existence_map[byte_idx] & (1 << bit_off)) != 0 {
                        VmExternalState::Exists
                    } else {
                        VmExternalState::Absent
                    }
                } else {
                    VmExternalState::Unknown
                }
            }
            None => VmExternalState::Unknown,
        }
    }

    /// Set the state of a page at the given offset
    pub fn set_state(&mut self, offset: usize, state: VmExternalState) {
        if let Some((byte_idx, bit_off)) = self.offset_to_bit(offset) {
            if byte_idx < self.existence_map.len() {
                let old_bit = (self.existence_map[byte_idx] & (1 << bit_off)) != 0;

                match state {
                    VmExternalState::Exists => {
                        self.existence_map[byte_idx] |= 1 << bit_off;
                        if !old_bit {
                            self.existence_count += 1;
                        }
                    }
                    VmExternalState::Absent => {
                        self.existence_map[byte_idx] &= !(1 << bit_off);
                        if old_bit {
                            self.existence_count = self.existence_count.saturating_sub(1);
                        }
                    }
                    VmExternalState::Unknown => {
                        // Unknown doesn't change the bitmap
                    }
                }
            }
        }
    }

    /// Mark a page as existing on external storage
    pub fn mark_exists(&mut self, offset: usize) {
        self.set_state(offset, VmExternalState::Exists);
    }

    /// Mark a page as absent from external storage
    pub fn mark_absent(&mut self, offset: usize) {
        self.set_state(offset, VmExternalState::Absent);
    }

    /// Clear all existence information
    pub fn clear(&mut self) {
        self.existence_map.fill(0);
        self.existence_count = 0;
    }

    /// Copy from another VmExternal
    pub fn copy_from(&mut self, other: &VmExternal) {
        let copy_size = self.existence_map.len().min(other.existence_map.len());
        self.existence_map[..copy_size].copy_from_slice(&other.existence_map[..copy_size]);

        // Recount bits
        self.existence_count = 0;
        for &byte in &self.existence_map {
            self.existence_count += byte.count_ones() as usize;
        }
    }

    /// Get the raw bitmap for inspection
    pub fn bitmap(&self) -> &[u8] {
        &self.existence_map
    }
}

impl Default for VmExternal {
    fn default() -> Self {
        Self::small()
    }
}

impl Clone for VmExternal {
    fn clone(&self) -> Self {
        Self {
            existence_map: self.existence_map.clone(),
            existence_size: self.existence_size,
            existence_count: self.existence_count,
            offset: self.offset,
        }
    }
}

// ============================================================================
// Global Module State
// ============================================================================

/// Statistics for external page tracking
#[derive(Debug, Clone, Default)]
pub struct VmExternalStats {
    /// Number of VmExternal structures created
    pub creates: u64,
    /// Number of VmExternal structures destroyed
    pub destroys: u64,
    /// Number of state get operations
    pub state_gets: u64,
    /// Number of state set operations
    pub state_sets: u64,
    /// Number of pages marked as existing
    pub pages_marked_exists: u64,
    /// Number of pages marked as absent
    pub pages_marked_absent: u64,
}

impl VmExternalStats {
    pub const fn new() -> Self {
        Self {
            creates: 0,
            destroys: 0,
            state_gets: 0,
            state_sets: 0,
            pages_marked_exists: 0,
            pages_marked_absent: 0,
        }
    }
}

static STATS: Mutex<VmExternalStats> = Mutex::new(VmExternalStats::new());

/// Get module statistics
pub fn stats() -> VmExternalStats {
    STATS.lock().clone()
}

/// Record a create operation
pub fn record_create() {
    STATS.lock().creates += 1;
}

/// Record a destroy operation
pub fn record_destroy() {
    STATS.lock().destroys += 1;
}

/// Record state operations
pub fn record_state_get() {
    STATS.lock().state_gets += 1;
}

pub fn record_state_set(state: VmExternalState) {
    let mut stats = STATS.lock();
    stats.state_sets += 1;
    match state {
        VmExternalState::Exists => stats.pages_marked_exists += 1,
        VmExternalState::Absent => stats.pages_marked_absent += 1,
        _ => {}
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Create a new VM external structure
pub fn vm_external_create(size_hint: usize) -> VmExternal {
    record_create();
    VmExternal::new(size_hint)
}

/// Destroy a VM external structure (for symmetry with C API)
pub fn vm_external_destroy(_ext: VmExternal) {
    record_destroy();
    // Structure is dropped automatically
}

/// Set the state of a page
pub fn vm_external_state_set(ext: &mut VmExternal, offset: usize, state: VmExternalState) {
    record_state_set(state);
    ext.set_state(offset, state);
}

/// Get the state of a page
pub fn vm_external_state_get(ext: Option<&VmExternal>, offset: usize) -> VmExternalState {
    record_state_get();
    match ext {
        Some(e) => e.get_state(offset),
        None => VmExternalState::Unknown,
    }
}

/// Initialize the VM external module
pub fn init() {
    // Module is initialized on first use via lazy statics
}

/// Alias for module initialization
pub fn vm_external_module_initialize() {
    init();
}

// ============================================================================
// Helpers
// ============================================================================

/// Calculate the bitmap size needed to track a given number of bytes
pub fn bitmap_size_for_bytes(bytes: usize) -> usize {
    let pages = (bytes + PAGE_SIZE - 1) / PAGE_SIZE;
    let bits_needed = pages;
    (bits_needed + BITS_PER_BYTE - 1) / BITS_PER_BYTE
}

/// Check if a VmExternal can track the given range
pub fn can_track_range(ext: &VmExternal, _start: usize, end: usize) -> bool {
    let end_page = (end + PAGE_SIZE - 1) / PAGE_SIZE;
    end_page <= ext.capacity()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_external_create() {
        let ext = VmExternal::small();
        assert_eq!(ext.size(), VM_EXTERNAL_SMALL_SIZE);
        assert_eq!(ext.count(), 0);
        assert!(ext.is_empty());

        let large = VmExternal::large();
        assert_eq!(large.size(), VM_EXTERNAL_LARGE_SIZE);
    }

    #[test]
    fn test_set_get_state() {
        let mut ext = VmExternal::small();

        // Initially unknown/absent
        assert_eq!(ext.get_state(0), VmExternalState::Absent);

        // Mark as existing
        ext.set_state(0, VmExternalState::Exists);
        assert_eq!(ext.get_state(0), VmExternalState::Exists);
        assert_eq!(ext.count(), 1);

        // Mark as absent
        ext.set_state(0, VmExternalState::Absent);
        assert_eq!(ext.get_state(0), VmExternalState::Absent);
        assert_eq!(ext.count(), 0);
    }

    #[test]
    fn test_multiple_pages() {
        let mut ext = VmExternal::small();

        ext.mark_exists(0);
        ext.mark_exists(PAGE_SIZE);
        ext.mark_exists(PAGE_SIZE * 2);

        assert_eq!(ext.count(), 3);
        assert_eq!(ext.get_state(0), VmExternalState::Exists);
        assert_eq!(ext.get_state(PAGE_SIZE), VmExternalState::Exists);
        assert_eq!(ext.get_state(PAGE_SIZE * 2), VmExternalState::Exists);
        assert_eq!(ext.get_state(PAGE_SIZE * 3), VmExternalState::Absent);
    }

    #[test]
    fn test_clear() {
        let mut ext = VmExternal::small();

        ext.mark_exists(0);
        ext.mark_exists(PAGE_SIZE);
        assert_eq!(ext.count(), 2);

        ext.clear();
        assert_eq!(ext.count(), 0);
        assert_eq!(ext.get_state(0), VmExternalState::Absent);
    }

    #[test]
    fn test_capacity() {
        let ext = VmExternal::small();
        let expected = VM_EXTERNAL_SMALL_SIZE * BITS_PER_BYTE;
        assert_eq!(ext.capacity(), expected);
    }

    #[test]
    fn test_out_of_range() {
        let ext = VmExternal::small();
        let beyond_capacity = ext.capacity() * PAGE_SIZE + PAGE_SIZE;
        assert_eq!(ext.get_state(beyond_capacity), VmExternalState::Unknown);
    }

    #[test]
    fn test_copy() {
        let mut ext1 = VmExternal::small();
        ext1.mark_exists(0);
        ext1.mark_exists(PAGE_SIZE);

        let mut ext2 = VmExternal::small();
        ext2.copy_from(&ext1);

        assert_eq!(ext2.count(), 2);
        assert_eq!(ext2.get_state(0), VmExternalState::Exists);
        assert_eq!(ext2.get_state(PAGE_SIZE), VmExternalState::Exists);
    }

    #[test]
    fn test_bitmap_size_calculation() {
        assert_eq!(bitmap_size_for_bytes(PAGE_SIZE), 1);
        assert_eq!(bitmap_size_for_bytes(PAGE_SIZE * 8), 1);
        assert_eq!(bitmap_size_for_bytes(PAGE_SIZE * 9), 2);
    }
}
