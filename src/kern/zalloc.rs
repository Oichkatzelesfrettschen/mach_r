//! Zone-based Memory Allocator
//!
//! Based on Mach4 kern/zalloc.c by Avadis Tevanian, Jr.
//!
//! A zone is a collection of fixed-size data blocks for which quick
//! allocation/deallocation is possible. Kernel routines can use zones
//! to manage data structures dynamically, creating a zone for each type
//! of data structure to be managed.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::mach_vm::vm_page::PAGE_SIZE;

// ============================================================================
// Zone Type Flags
// ============================================================================

/// Zone memory type flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZoneType(pub u32);

impl ZoneType {
    /// Zone elements can be paged out
    pub const PAGEABLE: Self = Self(0x00000001);
    /// Garbage-collect this zone when memory runs low
    pub const COLLECTABLE: Self = Self(0x00000002);
    /// zalloc() on this zone is allowed to fail
    pub const EXHAUSTIBLE: Self = Self(0x00000004);
    /// Panic if zone is exhausted
    pub const FIXED: Self = Self(0x00000008);
    /// Zone never expands after initial allocation
    pub const PERMANENT: Self = Self(0x00000010);
    /// Zone memory is wired
    pub const WIRED: Self = Self(0x00000020);

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl core::ops::BitOr for ZoneType {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

// ============================================================================
// Zone Element
// ============================================================================

/// Free element in a zone
///
/// Free elements form a singly-linked list. The first word of each
/// free element points to the next free element.
#[repr(C)]
struct FreeElement {
    next: usize, // Pointer to next free element
}

// ============================================================================
// Zone Statistics
// ============================================================================

/// Zone statistics for debugging and monitoring
#[derive(Debug, Default)]
pub struct ZoneStats {
    /// Total allocations from this zone
    pub alloc_count: AtomicU64,
    /// Total deallocations to this zone
    pub free_count: AtomicU64,
    /// Current elements in use
    pub in_use: AtomicU32,
    /// Maximum elements ever in use
    pub max_in_use: AtomicU32,
    /// Times zone had to expand
    pub expansions: AtomicU32,
    /// Failed allocation attempts
    pub failures: AtomicU32,
}

impl ZoneStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_alloc(&self) {
        self.alloc_count.fetch_add(1, Ordering::Relaxed);
        let in_use = self.in_use.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.max_in_use.fetch_max(in_use, Ordering::Relaxed);
    }

    pub fn record_free(&self) {
        self.free_count.fetch_add(1, Ordering::Relaxed);
        self.in_use.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_expansion(&self) {
        self.expansions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }
}

// ============================================================================
// Zone
// ============================================================================

/// Zone identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZoneId(pub u32);

/// A zone is a collection of fixed-size blocks
#[derive(Debug)]
pub struct Zone {
    /// Zone identifier
    pub id: ZoneId,

    /// Zone name (for debugging)
    pub name: String,

    /// Size of each element
    pub elem_size: usize,

    /// Allocation granularity (how much to expand by)
    pub alloc_size: usize,

    /// Current memory used by this zone
    pub cur_size: AtomicU64,

    /// Maximum memory this zone can use
    pub max_size: u64,

    /// Zone type flags
    pub zone_type: ZoneType,

    /// Free element list (head pointer)
    free_elements: Mutex<usize>,

    /// Number of free elements
    free_count: AtomicU32,

    /// Is the zone currently expanding?
    doing_alloc: AtomicBool,

    /// Zone statistics
    pub stats: ZoneStats,

    /// Backing memory pages
    pages: Mutex<Vec<ZonePage>>,
}

/// A page of memory backing a zone
#[derive(Debug)]
struct ZonePage {
    /// Base address of this page
    base: usize,
    /// Size of this page
    size: usize,
    /// Elements allocated from this page
    alloc_count: u32,
}

impl Zone {
    /// Create a new zone
    ///
    /// # Arguments
    /// * `id` - Zone identifier
    /// * `name` - Zone name for debugging
    /// * `elem_size` - Size of each element (will be aligned)
    /// * `max_size` - Maximum memory this zone can use
    /// * `alloc_size` - How much memory to allocate when expanding
    /// * `zone_type` - Zone type flags
    pub fn new(
        id: ZoneId,
        name: &str,
        elem_size: usize,
        max_size: u64,
        alloc_size: usize,
        zone_type: ZoneType,
    ) -> Self {
        // Ensure element size is at least pointer-sized and aligned
        let elem_size = elem_size
            .max(core::mem::size_of::<usize>())
            .next_power_of_two()
            .max(8);

        // Default allocation size to one page if not specified
        let alloc_size = if alloc_size == 0 {
            PAGE_SIZE
        } else {
            alloc_size
        };

        Self {
            id,
            name: String::from(name),
            elem_size,
            alloc_size,
            cur_size: AtomicU64::new(0),
            max_size,
            zone_type,
            free_elements: Mutex::new(0),
            free_count: AtomicU32::new(0),
            doing_alloc: AtomicBool::new(false),
            stats: ZoneStats::new(),
            pages: Mutex::new(Vec::new()),
        }
    }

    /// Allocate an element from the zone
    ///
    /// Returns None if the zone is exhausted and EXHAUSTIBLE flag is set.
    /// Otherwise may block waiting for memory to become available.
    pub fn alloc(&self) -> Option<NonNull<u8>> {
        // Fast path: try to get from free list
        let mut free_head = self.free_elements.lock();

        if *free_head != 0 {
            let element = *free_head;
            // Read next pointer from the free element
            let next = unsafe { *(element as *const usize) };
            *free_head = next;
            drop(free_head);

            self.free_count.fetch_sub(1, Ordering::Relaxed);
            self.stats.record_alloc();

            return NonNull::new(element as *mut u8);
        }

        drop(free_head);

        // Slow path: need to expand the zone
        self.expand_and_alloc()
    }

    /// Try to allocate without blocking or expanding
    pub fn try_alloc(&self) -> Option<NonNull<u8>> {
        let mut free_head = self.free_elements.lock();

        if *free_head != 0 {
            let element = *free_head;
            let next = unsafe { *(element as *const usize) };
            *free_head = next;
            drop(free_head);

            self.free_count.fetch_sub(1, Ordering::Relaxed);
            self.stats.record_alloc();

            return NonNull::new(element as *mut u8);
        }

        None
    }

    /// Free an element back to the zone
    ///
    /// # Safety
    /// The element must have been allocated from this zone.
    pub unsafe fn free(&self, element: NonNull<u8>) {
        let elem_ptr = element.as_ptr() as usize;

        // Add to free list
        let mut free_head = self.free_elements.lock();
        let elem = elem_ptr as *mut usize;
        *elem = *free_head;
        *free_head = elem_ptr;
        drop(free_head);

        self.free_count.fetch_add(1, Ordering::Relaxed);
        self.stats.record_free();
    }

    /// Expand the zone and allocate an element
    fn expand_and_alloc(&self) -> Option<NonNull<u8>> {
        // Check if another thread is already expanding
        if self.doing_alloc.swap(true, Ordering::SeqCst) {
            // Another thread is expanding, spin briefly then retry
            for _ in 0..100 {
                core::hint::spin_loop();
            }
            return self.try_alloc();
        }

        // Check if we can expand
        let cur_size = self.cur_size.load(Ordering::Relaxed);
        if cur_size >= self.max_size {
            self.doing_alloc.store(false, Ordering::SeqCst);
            if self.zone_type.contains(ZoneType::EXHAUSTIBLE) {
                self.stats.record_failure();
                return None;
            }
            // For non-exhaustible zones, this is a panic condition
            // In a real kernel, we'd wait for memory
            self.stats.record_failure();
            return None;
        }

        // Calculate how much to allocate
        let alloc_size = self.alloc_size.min((self.max_size - cur_size) as usize);

        // Try to get memory (would normally come from vm_kern)
        // For now, use the global allocator as a backing store
        let layout = core::alloc::Layout::from_size_align(alloc_size, PAGE_SIZE).ok()?;

        let mem = unsafe { alloc::alloc::alloc_zeroed(layout) };
        if mem.is_null() {
            self.doing_alloc.store(false, Ordering::SeqCst);
            self.stats.record_failure();
            return None;
        }

        let base = mem as usize;

        // Record the new page
        self.pages.lock().push(ZonePage {
            base,
            size: alloc_size,
            alloc_count: 0,
        });

        // Add elements to free list
        self.zcram(base, alloc_size);

        // Update cur_size
        self.cur_size
            .fetch_add(alloc_size as u64, Ordering::Relaxed);
        self.stats.record_expansion();

        self.doing_alloc.store(false, Ordering::SeqCst);

        // Now allocate from the freshly added elements
        self.try_alloc()
    }

    /// Add memory to the zone's free list
    ///
    /// This is used to pre-populate zones or add memory chunks.
    pub fn zcram(&self, base: usize, size: usize) {
        let elem_size = self.elem_size;
        let num_elements = size / elem_size;

        if num_elements == 0 {
            return;
        }

        let mut free_head = self.free_elements.lock();

        // Link all elements into the free list
        for i in 0..num_elements {
            let elem_addr = base + (i * elem_size);
            let elem = elem_addr as *mut usize;

            unsafe {
                *elem = *free_head;
            }
            *free_head = elem_addr;
        }

        drop(free_head);
        self.free_count
            .fetch_add(num_elements as u32, Ordering::Relaxed);
    }

    /// Get the number of free elements
    pub fn free_count(&self) -> u32 {
        self.free_count.load(Ordering::Relaxed)
    }

    /// Get current memory usage
    pub fn current_size(&self) -> u64 {
        self.cur_size.load(Ordering::Relaxed)
    }

    /// Check if zone is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.free_count() == 0 && self.cur_size.load(Ordering::Relaxed) >= self.max_size
    }
}

// ============================================================================
// Zone Manager
// ============================================================================

/// Global zone manager
pub struct ZoneManager {
    /// All zones
    zones: BTreeMap<ZoneId, Arc<Zone>>,

    /// Zones by name
    by_name: BTreeMap<String, ZoneId>,

    /// Next zone ID
    next_id: u32,

    /// The zone that holds zone structures themselves
    zone_zone: Option<ZoneId>,
}

impl ZoneManager {
    pub fn new() -> Self {
        Self {
            zones: BTreeMap::new(),
            by_name: BTreeMap::new(),
            next_id: 1,
            zone_zone: None,
        }
    }

    /// Create a new zone
    pub fn zinit(
        &mut self,
        name: &str,
        elem_size: usize,
        max_size: u64,
        alloc_size: usize,
        zone_type: ZoneType,
    ) -> Arc<Zone> {
        let id = ZoneId(self.next_id);
        self.next_id += 1;

        let zone = Arc::new(Zone::new(
            id, name, elem_size, max_size, alloc_size, zone_type,
        ));

        self.zones.insert(id, Arc::clone(&zone));
        self.by_name.insert(String::from(name), id);

        zone
    }

    /// Find zone by ID
    pub fn find(&self, id: ZoneId) -> Option<Arc<Zone>> {
        self.zones.get(&id).cloned()
    }

    /// Find zone by name
    pub fn find_by_name(&self, name: &str) -> Option<Arc<Zone>> {
        let id = self.by_name.get(name)?;
        self.zones.get(id).cloned()
    }

    /// Get all zones
    pub fn all_zones(&self) -> Vec<Arc<Zone>> {
        self.zones.values().cloned().collect()
    }

    /// Bootstrap the zone allocator
    ///
    /// Creates the zone_zone which is used to allocate other zones.
    pub fn bootstrap(&mut self) {
        // Create the zone that holds other zones
        let zone_zone = self.zinit(
            "zones",
            core::mem::size_of::<Zone>(),
            64 * 1024, // 64KB for zone structures
            PAGE_SIZE,
            ZoneType::PERMANENT,
        );
        self.zone_zone = Some(zone_zone.id);
    }

    /// Perform garbage collection on collectable zones
    pub fn gc(&mut self) {
        // In a full implementation, this would:
        // 1. Find zones with COLLECTABLE flag
        // 2. Free empty pages back to the VM system
        // For now, we just record that GC was attempted
    }

    /// Get total memory used by all zones
    pub fn total_memory(&self) -> u64 {
        self.zones.values().map(|z| z.current_size()).sum()
    }
}

impl Default for ZoneManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

static ZONE_MANAGER: spin::Once<Mutex<ZoneManager>> = spin::Once::new();

fn zone_manager() -> &'static Mutex<ZoneManager> {
    ZONE_MANAGER.call_once(|| {
        let mut mgr = ZoneManager::new();
        mgr.bootstrap();
        Mutex::new(mgr)
    });
    ZONE_MANAGER.get().unwrap()
}

/// Initialize the zone allocator
pub fn zone_bootstrap() {
    let _ = zone_manager();
}

/// Create a new zone
pub fn zinit(
    name: &str,
    elem_size: usize,
    max_size: u64,
    alloc_size: usize,
    zone_type: ZoneType,
) -> Arc<Zone> {
    zone_manager()
        .lock()
        .zinit(name, elem_size, max_size, alloc_size, zone_type)
}

/// Allocate from a zone
pub fn zalloc(zone: &Zone) -> Option<NonNull<u8>> {
    zone.alloc()
}

/// Try to allocate without blocking
pub fn zget(zone: &Zone) -> Option<NonNull<u8>> {
    zone.try_alloc()
}

/// Free to a zone
///
/// # Safety
/// Element must have been allocated from the given zone.
pub unsafe fn zfree(zone: &Zone, element: NonNull<u8>) {
    zone.free(element);
}

/// Add memory to a zone
pub fn zcram(zone: &Zone, base: usize, size: usize) {
    zone.zcram(base, size);
}

/// Consider garbage collection
pub fn consider_zone_gc() {
    zone_manager().lock().gc();
}

/// Get zone by name
pub fn zone_find(name: &str) -> Option<Arc<Zone>> {
    zone_manager().lock().find_by_name(name)
}

// ============================================================================
// Zone Info (for debugging)
// ============================================================================

/// Zone information for debugging
#[derive(Debug, Clone)]
pub struct ZoneInfo {
    pub name: String,
    pub elem_size: usize,
    pub cur_size: u64,
    pub max_size: u64,
    pub free_count: u32,
    pub alloc_count: u64,
    pub zone_type: ZoneType,
}

impl From<&Zone> for ZoneInfo {
    fn from(zone: &Zone) -> Self {
        Self {
            name: zone.name.clone(),
            elem_size: zone.elem_size,
            cur_size: zone.current_size(),
            max_size: zone.max_size,
            free_count: zone.free_count(),
            alloc_count: zone.stats.alloc_count.load(Ordering::Relaxed),
            zone_type: zone.zone_type,
        }
    }
}

/// Get information about all zones
pub fn zone_info() -> Vec<ZoneInfo> {
    zone_manager()
        .lock()
        .all_zones()
        .iter()
        .map(|z| ZoneInfo::from(z.as_ref()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_creation() {
        let zone = Zone::new(ZoneId(1), "test", 64, 4096, 1024, ZoneType::default());
        assert_eq!(zone.name, "test");
        assert!(zone.elem_size >= 64);
    }

    #[test]
    fn test_zone_alloc_free() {
        let zone = Zone::new(ZoneId(1), "test", 64, 8192, 4096, ZoneType::default());

        // Allocate
        let elem = zone.alloc().expect("allocation failed");
        assert!(!elem.as_ptr().is_null());

        // Free
        unsafe {
            zone.free(elem);
        }

        // Should be able to allocate again
        let elem2 = zone.alloc().expect("allocation failed");
        assert!(!elem2.as_ptr().is_null());

        unsafe {
            zone.free(elem2);
        }
    }

    #[test]
    fn test_zone_type_flags() {
        let pageable = ZoneType::PAGEABLE;
        let collectable = ZoneType::COLLECTABLE;
        let combined = pageable | collectable;

        assert!(combined.contains(pageable));
        assert!(combined.contains(collectable));
        assert!(!combined.contains(ZoneType::FIXED));
    }
}
