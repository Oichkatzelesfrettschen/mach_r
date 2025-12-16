//! General Kernel Memory Allocator
//!
//! Based on Mach4 kern/kalloc.c by Avadis Tevanian, Jr.
//!
//! This allocator is designed to be used by the kernel to manage
//! dynamic memory fast. It uses the zone allocator for small allocations
//! and direct VM allocation for large ones.
//!
//! All allocations of size less than kalloc_max are rounded to the
//! next highest power of 2. A zone is created for each potential size.

use alloc::sync::Arc;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::kern::zalloc::{zinit, Zone, ZoneType};
use crate::mach_vm::vm_page::PAGE_SIZE;

// ============================================================================
// Constants
// ============================================================================

/// Minimum allocation size (smaller requests are rounded up)
pub const MINSIZE: usize = 16;

/// Maximum size for zone-based allocation
/// Larger allocations go directly to the VM system
pub const KALLOC_MAX: usize = 16 * 1024; // 16KB

/// Number of kalloc zones (one per power of 2)
const NUM_K_ZONES: usize = 16;

/// Zone names for debugging
const K_ZONE_NAMES: [&str; NUM_K_ZONES] = [
    "kalloc.1",
    "kalloc.2",
    "kalloc.4",
    "kalloc.8",
    "kalloc.16",
    "kalloc.32",
    "kalloc.64",
    "kalloc.128",
    "kalloc.256",
    "kalloc.512",
    "kalloc.1024",
    "kalloc.2048",
    "kalloc.4096",
    "kalloc.8192",
    "kalloc.16384",
    "kalloc.32768",
];

/// Maximum elements per zone
const K_ZONE_MAX: [u64; NUM_K_ZONES] = [
    1024, // 1 byte
    1024, // 2 bytes
    1024, // 4 bytes
    1024, // 8 bytes
    1024, // 16 bytes
    4096, // 32 bytes
    4096, // 64 bytes
    4096, // 128 bytes
    4096, // 256 bytes
    1024, // 512 bytes
    1024, // 1024 bytes
    1024, // 2048 bytes
    1024, // 4096 bytes
    4096, // 8192 bytes
    64,   // 16384 bytes
    64,   // 32768 bytes
];

// ============================================================================
// Kalloc State
// ============================================================================

/// Kalloc allocator state
pub struct KallocState {
    /// Zones for each power-of-2 size
    k_zones: [Option<Arc<Zone>>; NUM_K_ZONES],

    /// Index of first active zone (for MINSIZE)
    first_k_zone: usize,

    /// Maximum size handled by zones
    kalloc_max: usize,

    /// Is initialized?
    initialized: AtomicBool,

    /// Statistics
    pub stats: KallocStats,
}

/// Kalloc statistics
#[derive(Debug, Default)]
pub struct KallocStats {
    /// Total allocations
    pub alloc_count: AtomicU64,
    /// Total frees
    pub free_count: AtomicU64,
    /// Bytes currently allocated
    pub bytes_allocated: AtomicU64,
    /// Large allocations (bypassing zones)
    pub large_allocs: AtomicU64,
    /// Large frees
    pub large_frees: AtomicU64,
}

impl KallocState {
    /// Create uninitialized kalloc state
    pub const fn new() -> Self {
        Self {
            k_zones: [const { None }; NUM_K_ZONES],
            first_k_zone: 0,
            kalloc_max: KALLOC_MAX,
            initialized: AtomicBool::new(false),
            stats: KallocStats {
                alloc_count: AtomicU64::new(0),
                free_count: AtomicU64::new(0),
                bytes_allocated: AtomicU64::new(0),
                large_allocs: AtomicU64::new(0),
                large_frees: AtomicU64::new(0),
            },
        }
    }

    /// Get zone index for a given size
    fn zone_index(size: usize) -> usize {
        // Find the power of 2 that covers this size
        if size == 0 {
            return 0;
        }
        (usize::BITS - (size - 1).leading_zeros()) as usize
    }

    /// Get the actual allocation size for a zone index
    fn zone_size(index: usize) -> usize {
        1 << index
    }
}

// ============================================================================
// Global Kalloc State
// ============================================================================

static KALLOC: spin::Once<Mutex<KallocState>> = spin::Once::new();

fn kalloc_state() -> &'static Mutex<KallocState> {
    KALLOC.call_once(|| Mutex::new(KallocState::new()));
    KALLOC.get().unwrap()
}

/// Initialize the kalloc allocator
///
/// This should be called once during system initialization.
pub fn kalloc_init() {
    let mut state = kalloc_state().lock();

    if state.initialized.load(Ordering::SeqCst) {
        return;
    }

    // Determine kalloc_max based on page size
    state.kalloc_max = if PAGE_SIZE < 16 * 1024 {
        16 * 1024
    } else {
        PAGE_SIZE
    };

    // Create zones for each power-of-2 size
    let mut size: usize = 1;
    for i in 0..NUM_K_ZONES {
        if size >= state.kalloc_max {
            break;
        }

        if size < MINSIZE {
            state.k_zones[i] = None;
            size <<= 1;
            continue;
        }

        if size == MINSIZE {
            state.first_k_zone = i;
        }

        // Create zone for this size
        let zone_type = if size >= PAGE_SIZE {
            ZoneType::COLLECTABLE
        } else {
            ZoneType::default()
        };

        let zone = zinit(
            K_ZONE_NAMES[i],
            size,
            K_ZONE_MAX[i] * size as u64,
            size,
            zone_type,
        );

        state.k_zones[i] = Some(zone);
        size <<= 1;
    }

    state.initialized.store(true, Ordering::SeqCst);
}

/// Allocate kernel memory
///
/// Allocates `size` bytes of kernel memory. For sizes up to `kalloc_max`,
/// allocation comes from zones. Larger allocations come directly from VM.
///
/// Returns None on allocation failure.
pub fn kalloc(size: usize) -> Option<NonNull<u8>> {
    if size == 0 {
        return None;
    }

    let state = kalloc_state().lock();

    // Make sure we're initialized
    if !state.initialized.load(Ordering::SeqCst) {
        drop(state);
        kalloc_init();
        return kalloc(size);
    }

    // Check if this goes to a zone
    if size < state.kalloc_max {
        let zindex = KallocState::zone_index(size).max(state.first_k_zone);

        if zindex < NUM_K_ZONES {
            if let Some(ref zone) = state.k_zones[zindex] {
                let result = zone.alloc();
                if result.is_some() {
                    let actual_size = KallocState::zone_size(zindex);
                    state.stats.alloc_count.fetch_add(1, Ordering::Relaxed);
                    state
                        .stats
                        .bytes_allocated
                        .fetch_add(actual_size as u64, Ordering::Relaxed);
                }
                return result;
            }
        }
    }

    // Large allocation - go directly to VM/heap
    state.stats.large_allocs.fetch_add(1, Ordering::Relaxed);
    state.stats.alloc_count.fetch_add(1, Ordering::Relaxed);
    state
        .stats
        .bytes_allocated
        .fetch_add(size as u64, Ordering::Relaxed);

    // Use the global allocator for large allocations
    let layout = core::alloc::Layout::from_size_align(size, 8).ok()?;
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    NonNull::new(ptr)
}

/// Try to allocate without blocking
pub fn kget(size: usize) -> Option<NonNull<u8>> {
    if size == 0 {
        return None;
    }

    let state = kalloc_state().lock();

    if !state.initialized.load(Ordering::SeqCst) {
        return None;
    }

    if size < state.kalloc_max {
        let zindex = KallocState::zone_index(size).max(state.first_k_zone);

        if zindex < NUM_K_ZONES {
            if let Some(ref zone) = state.k_zones[zindex] {
                let result = zone.try_alloc();
                if result.is_some() {
                    let actual_size = KallocState::zone_size(zindex);
                    state.stats.alloc_count.fetch_add(1, Ordering::Relaxed);
                    state
                        .stats
                        .bytes_allocated
                        .fetch_add(actual_size as u64, Ordering::Relaxed);
                }
                return result;
            }
        }
    }

    None
}

/// Free kernel memory
///
/// # Safety
///
/// The pointer must have been allocated by kalloc/kget, and `size` must
/// be the same size that was passed to kalloc.
pub unsafe fn kfree(ptr: NonNull<u8>, size: usize) {
    let state = kalloc_state().lock();

    if size < state.kalloc_max {
        let zindex = KallocState::zone_index(size).max(state.first_k_zone);

        if zindex < NUM_K_ZONES {
            if let Some(ref zone) = state.k_zones[zindex] {
                zone.free(ptr);
                let actual_size = KallocState::zone_size(zindex);
                state.stats.free_count.fetch_add(1, Ordering::Relaxed);
                state
                    .stats
                    .bytes_allocated
                    .fetch_sub(actual_size as u64, Ordering::Relaxed);
                return;
            }
        }
    }

    // Large free - return to global allocator
    state.stats.large_frees.fetch_add(1, Ordering::Relaxed);
    state.stats.free_count.fetch_add(1, Ordering::Relaxed);
    state
        .stats
        .bytes_allocated
        .fetch_sub(size as u64, Ordering::Relaxed);

    let layout = core::alloc::Layout::from_size_align(size, 8).expect("invalid layout");
    alloc::alloc::dealloc(ptr.as_ptr(), layout);
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Allocate and zero memory
pub fn kalloc_zeroed(size: usize) -> Option<NonNull<u8>> {
    let ptr = kalloc(size)?;
    unsafe {
        core::ptr::write_bytes(ptr.as_ptr(), 0, size);
    }
    Some(ptr)
}

/// Reallocate memory
///
/// # Safety
///
/// The old pointer must have been allocated by kalloc.
pub unsafe fn krealloc(
    old_ptr: Option<NonNull<u8>>,
    old_size: usize,
    new_size: usize,
) -> Option<NonNull<u8>> {
    let new_ptr = kalloc(new_size)?;

    if let Some(old) = old_ptr {
        let copy_size = old_size.min(new_size);
        core::ptr::copy_nonoverlapping(old.as_ptr(), new_ptr.as_ptr(), copy_size);
        kfree(old, old_size);
    }

    Some(new_ptr)
}

// ============================================================================
// Statistics
// ============================================================================

/// Get kalloc statistics
pub fn kalloc_stats() -> KallocStatsCopy {
    let state = kalloc_state().lock();
    KallocStatsCopy {
        alloc_count: state.stats.alloc_count.load(Ordering::Relaxed),
        free_count: state.stats.free_count.load(Ordering::Relaxed),
        bytes_allocated: state.stats.bytes_allocated.load(Ordering::Relaxed),
        large_allocs: state.stats.large_allocs.load(Ordering::Relaxed),
        large_frees: state.stats.large_frees.load(Ordering::Relaxed),
    }
}

/// Copy of kalloc statistics
#[derive(Debug, Clone, Copy)]
pub struct KallocStatsCopy {
    pub alloc_count: u64,
    pub free_count: u64,
    pub bytes_allocated: u64,
    pub large_allocs: u64,
    pub large_frees: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_index() {
        assert_eq!(KallocState::zone_index(1), 1);
        assert_eq!(KallocState::zone_index(2), 1);
        assert_eq!(KallocState::zone_index(3), 2);
        assert_eq!(KallocState::zone_index(4), 2);
        assert_eq!(KallocState::zone_index(5), 3);
        assert_eq!(KallocState::zone_index(16), 4);
        assert_eq!(KallocState::zone_index(17), 5);
    }

    #[test]
    fn test_zone_size() {
        assert_eq!(KallocState::zone_size(0), 1);
        assert_eq!(KallocState::zone_size(1), 2);
        assert_eq!(KallocState::zone_size(4), 16);
        assert_eq!(KallocState::zone_size(10), 1024);
    }

    #[test]
    fn test_kalloc_free() {
        kalloc_init();

        let ptr = kalloc(64).expect("allocation failed");
        assert!(!ptr.as_ptr().is_null());

        unsafe {
            kfree(ptr, 64);
        }
    }

    #[test]
    fn test_kalloc_zeroed() {
        kalloc_init();

        let ptr = kalloc_zeroed(128).expect("allocation failed");

        // Check that memory is zeroed
        let slice = unsafe { core::slice::from_raw_parts(ptr.as_ptr(), 128) };
        assert!(slice.iter().all(|&b| b == 0));

        unsafe {
            kfree(ptr, 128);
        }
    }
}
