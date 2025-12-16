//! VM Map - Address Space Management
//!
//! Based on Mach4 vm/vm_map.h/c
//! VM maps represent the virtual address space of a task.
//! Each map contains a set of map entries that describe the mappings.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::mach_vm::vm_object::VmObject;
use crate::mach_vm::vm_page::PAGE_SIZE;
use crate::types::TaskId;

// ============================================================================
// VM Map Types
// ============================================================================

/// VM Map ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VmMapId(pub u64);

impl VmMapId {
    pub const NULL: Self = Self(0);
}

/// Memory protection flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmProt(u32);

impl VmProt {
    /// No access
    pub const NONE: Self = Self(0);
    /// Read access
    pub const READ: Self = Self(1);
    /// Write access
    pub const WRITE: Self = Self(2);
    /// Execute access
    pub const EXECUTE: Self = Self(4);
    /// Default (read/write)
    pub const DEFAULT: Self = Self(3); // READ | WRITE
    /// All permissions
    pub const ALL: Self = Self(7); // READ | WRITE | EXECUTE

    /// Empty (no permissions)
    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn new(bits: u32) -> Self {
        Self(bits & 0x7)
    }

    pub fn bits(&self) -> u32 {
        self.0
    }

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn can_read(&self) -> bool {
        self.contains(Self::READ)
    }

    pub fn can_write(&self) -> bool {
        self.contains(Self::WRITE)
    }

    pub fn can_execute(&self) -> bool {
        self.contains(Self::EXECUTE)
    }
}

impl core::ops::BitOr for VmProt {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for VmProt {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl Default for VmProt {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Inheritance on fork
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum VmInherit {
    /// Don't inherit
    None = 0,
    /// Share the mapping
    Share = 1,
    /// Copy the mapping
    #[default]
    Copy = 2,
    /// Inherit mapping using copy-on-write
    CopyOnWrite = 3,
}

// ============================================================================
// VM Map Entry Flags
// ============================================================================

/// Map entry flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryFlags(u32);

impl EntryFlags {
    /// Entry is wired
    pub const WIRED: Self = Self(0x0001);
    /// Entry is in transition
    pub const IN_TRANSITION: Self = Self(0x0002);
    /// Entry needs wake up
    pub const NEEDS_WAKEUP: Self = Self(0x0004);
    /// Entry allows copy-on-write
    pub const NEEDS_COPY: Self = Self(0x0008);
    /// Entry is a submap
    pub const IS_SUBMAP: Self = Self(0x0010);
    /// Entry shares an object
    pub const IS_SHARED: Self = Self(0x0020);
    /// Entry was never accessed
    pub const ZERO_WIRED: Self = Self(0x0040);
    /// Don't merge with adjacent entries
    pub const NO_COALESCE: Self = Self(0x0080);
    /// User cannot unwire
    pub const USER_WIRED: Self = Self(0x0100);
    /// Entry has been cleaned
    pub const CLEANING: Self = Self(0x0200);

    /// Empty flags
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Get bits
    pub const fn bits(&self) -> u32 {
        self.0
    }

    /// Check if contains flags
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Union with another flags (for |=)
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Remove flags
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl core::ops::BitOr for EntryFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for EntryFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl Default for EntryFlags {
    fn default() -> Self {
        Self::empty()
    }
}

// ============================================================================
// VM Map Entry
// ============================================================================

/// VM Map Entry - describes a single mapping in the address space
#[derive(Debug)]
pub struct VmMapEntry {
    /// Start address of this entry
    pub start: u64,

    /// End address of this entry
    pub end: u64,

    /// Backing object
    pub object: Option<Arc<VmObject>>,

    /// Offset into the object
    pub offset: u64,

    /// Protection (current)
    pub protection: VmProt,

    /// Maximum protection
    pub max_protection: VmProt,

    /// Inheritance behavior
    pub inheritance: VmInherit,

    /// Wire count
    pub wired_count: u32,

    /// User wire count
    pub user_wired_count: u32,

    /// Entry flags
    pub flags: EntryFlags,

    /// Submap (if IS_SUBMAP flag is set)
    pub submap: Option<Arc<VmMap>>,
}

impl VmMapEntry {
    /// Create a new map entry
    pub fn new(start: u64, end: u64, object: Option<Arc<VmObject>>, offset: u64) -> Self {
        Self {
            start,
            end,
            object,
            offset,
            protection: VmProt::DEFAULT,
            max_protection: VmProt::ALL,
            inheritance: VmInherit::default(),
            wired_count: 0,
            user_wired_count: 0,
            flags: EntryFlags::default(),
            submap: None,
        }
    }

    /// Get entry size
    pub fn size(&self) -> u64 {
        self.end - self.start
    }

    /// Check if address is in this entry
    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.start && addr < self.end
    }

    /// Check if this entry overlaps with a range
    pub fn overlaps(&self, start: u64, end: u64) -> bool {
        self.start < end && start < self.end
    }

    /// Check if entry is wired
    pub fn is_wired(&self) -> bool {
        self.wired_count > 0
    }

    /// Wire this entry
    pub fn wire(&mut self) {
        self.wired_count += 1;
        self.flags |= EntryFlags::WIRED;
    }

    /// Unwire this entry
    pub fn unwire(&mut self) -> bool {
        if self.wired_count > 0 {
            self.wired_count -= 1;
            if self.wired_count == 0 {
                self.flags.remove(EntryFlags::WIRED);
            }
            true
        } else {
            false
        }
    }

    /// Split this entry at address, returning the upper portion
    pub fn split(&mut self, at: u64) -> Option<VmMapEntry> {
        if at <= self.start || at >= self.end {
            return None;
        }

        // Create upper entry
        let mut upper = VmMapEntry::new(at, self.end, self.object.clone(), 0);
        upper.offset = self.offset + (at - self.start);
        upper.protection = self.protection;
        upper.max_protection = self.max_protection;
        upper.inheritance = self.inheritance;
        upper.wired_count = self.wired_count;
        upper.user_wired_count = self.user_wired_count;
        upper.flags = self.flags;

        // Update this entry
        self.end = at;

        Some(upper)
    }

    /// Check if can merge with another entry
    pub fn can_merge(&self, other: &VmMapEntry) -> bool {
        // Must be adjacent
        if self.end != other.start {
            return false;
        }

        // Must have same attributes
        if self.protection != other.protection
            || self.max_protection != other.max_protection
            || self.inheritance != other.inheritance
            || self.flags != other.flags
        {
            return false;
        }

        // Must be same object with consecutive offsets
        match (&self.object, &other.object) {
            (Some(obj1), Some(obj2)) => {
                Arc::ptr_eq(obj1, obj2) && self.offset + self.size() == other.offset
            }
            (None, None) => true,
            _ => false,
        }
    }
}

// ============================================================================
// VM Map
// ============================================================================

/// VM Map - represents a task's address space
#[derive(Debug)]
pub struct VmMap {
    /// Map ID
    pub id: VmMapId,

    /// Reference count
    ref_count: AtomicU32,

    /// Owning task
    pub task: Mutex<Option<TaskId>>,

    /// Map entries (ordered by start address)
    pub entries: Mutex<BTreeMap<u64, VmMapEntry>>,

    /// Minimum address
    pub min_offset: u64,

    /// Maximum address
    pub max_offset: u64,

    /// Map size (sum of all entry sizes)
    pub size: AtomicU64,

    /// Is this map locked?
    pub locked: AtomicBool,

    /// Wait channel for lock
    pub wait_for_space: AtomicBool,

    /// Hint for next allocation
    first_free: Mutex<u64>,

    /// Number of entries
    entry_count: AtomicU32,

    /// Physical map (pmap) reference for hardware page tables
    pub pmap_id: Mutex<Option<super::pmap::PmapId>>,

    /// Is this the kernel map?
    pub is_kernel_map: bool,

    /// Timestamp for versioning
    pub timestamp: AtomicU64,
}

impl VmMap {
    /// Create a new VM map
    pub fn new(id: VmMapId, min: u64, max: u64) -> Self {
        Self {
            id,
            ref_count: AtomicU32::new(1),
            task: Mutex::new(None),
            entries: Mutex::new(BTreeMap::new()),
            min_offset: min,
            max_offset: max,
            size: AtomicU64::new(0),
            locked: AtomicBool::new(false),
            wait_for_space: AtomicBool::new(false),
            first_free: Mutex::new(min),
            entry_count: AtomicU32::new(0),
            pmap_id: Mutex::new(None),
            is_kernel_map: false,
            timestamp: AtomicU64::new(0),
        }
    }

    /// Create the kernel map
    pub fn kernel(id: VmMapId, min: u64, max: u64) -> Self {
        let mut map = Self::new(id, min, max);
        map.is_kernel_map = true;
        map
    }

    /// Increment reference count
    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement reference count
    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    /// Lock the map
    pub fn lock(&self) -> bool {
        !self.locked.swap(true, Ordering::SeqCst)
    }

    /// Unlock the map
    pub fn unlock(&self) {
        self.locked.store(false, Ordering::SeqCst);
    }

    /// Get map size
    pub fn total_size(&self) -> u64 {
        self.size.load(Ordering::SeqCst)
    }

    /// Get entry count
    pub fn entry_count(&self) -> u32 {
        self.entry_count.load(Ordering::SeqCst)
    }

    /// Find entry containing address
    pub fn lookup(&self, addr: u64) -> Option<u64> {
        let entries = self.entries.lock();

        // Use BTreeMap range to find potential entry
        if let Some((&start, entry)) = entries.range(..=addr).next_back() {
            if entry.contains(addr) {
                return Some(start);
            }
        }
        None
    }

    /// Find free space of given size
    pub fn find_space(&self, size: u64, mask: u64) -> Option<u64> {
        let entries = self.entries.lock();
        let mut hint = *self.first_free.lock();

        // Align hint
        hint = (hint + mask) & !mask;

        if entries.is_empty() {
            if hint + size <= self.max_offset {
                return Some(hint);
            }
            return None;
        }

        // Search for gap
        let mut prev_end = self.min_offset;

        for (&start, entry) in entries.iter() {
            // Check gap before this entry
            let aligned_prev = (prev_end + mask) & !mask;
            if aligned_prev >= hint && aligned_prev + size <= start {
                return Some(aligned_prev);
            }
            prev_end = entry.end;
        }

        // Check gap after last entry
        let aligned_prev = (prev_end + mask) & !mask;
        if aligned_prev + size <= self.max_offset {
            return Some(aligned_prev);
        }

        None
    }

    /// Enter a new mapping
    #[allow(clippy::too_many_arguments)]
    pub fn enter(
        &self,
        start: u64,
        end: u64,
        object: Option<Arc<VmObject>>,
        offset: u64,
        protection: VmProt,
        max_protection: VmProt,
        inheritance: VmInherit,
    ) -> Result<(), MapError> {
        // Validate range
        if start >= end || start < self.min_offset || end > self.max_offset {
            return Err(MapError::InvalidRange);
        }

        let mut entries = self.entries.lock();

        // Check for overlap
        for (_, entry) in entries.iter() {
            if entry.overlaps(start, end) {
                return Err(MapError::NoSpace);
            }
        }

        // Create entry
        let mut entry = VmMapEntry::new(start, end, object, offset);
        entry.protection = protection;
        entry.max_protection = max_protection;
        entry.inheritance = inheritance;

        let size = entry.size();
        entries.insert(start, entry);

        self.size.fetch_add(size, Ordering::SeqCst);
        self.entry_count.fetch_add(1, Ordering::SeqCst);
        self.timestamp.fetch_add(1, Ordering::SeqCst);

        // Update first_free hint
        let mut first_free = self.first_free.lock();
        if *first_free <= start && *first_free < end {
            *first_free = end;
        }

        Ok(())
    }

    /// Remove a mapping
    pub fn remove(&self, start: u64, end: u64) -> Result<(), MapError> {
        let mut entries = self.entries.lock();

        // Find entries in range
        let to_remove: Vec<u64> = entries
            .iter()
            .filter(|(_, e)| e.start >= start && e.end <= end)
            .map(|(&k, _)| k)
            .collect();

        if to_remove.is_empty() {
            return Err(MapError::NotFound);
        }

        let mut total_removed = 0u64;
        for key in to_remove {
            if let Some(entry) = entries.remove(&key) {
                total_removed += entry.size();
            }
        }

        self.size.fetch_sub(total_removed, Ordering::SeqCst);
        self.entry_count.fetch_sub(1, Ordering::SeqCst);
        self.timestamp.fetch_add(1, Ordering::SeqCst);

        // Update first_free hint
        let mut first_free = self.first_free.lock();
        if start < *first_free {
            *first_free = start;
        }

        Ok(())
    }

    /// Change protection on a range
    pub fn protect(&self, start: u64, end: u64, new_prot: VmProt) -> Result<(), MapError> {
        let mut entries = self.entries.lock();

        for (_, entry) in entries.iter_mut() {
            if entry.overlaps(start, end) {
                // Check max protection
                if !entry.max_protection.contains(new_prot) {
                    return Err(MapError::ProtectionFailure);
                }
                entry.protection = new_prot;
            }
        }

        self.timestamp.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Wire a range of addresses
    pub fn wire(&self, start: u64, end: u64) -> Result<(), MapError> {
        let mut entries = self.entries.lock();

        for (_, entry) in entries.iter_mut() {
            if entry.overlaps(start, end) {
                entry.wire();
            }
        }

        self.timestamp.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Unwire a range of addresses
    pub fn unwire(&self, start: u64, end: u64) -> Result<(), MapError> {
        let mut entries = self.entries.lock();

        for (_, entry) in entries.iter_mut() {
            if entry.overlaps(start, end) {
                entry.unwire();
            }
        }

        self.timestamp.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Fork this map for a new task
    pub fn fork(&self, new_id: VmMapId) -> VmMap {
        let new_map = VmMap::new(new_id, self.min_offset, self.max_offset);

        let entries = self.entries.lock();

        for (&start, entry) in entries.iter() {
            match entry.inheritance {
                VmInherit::None => continue,
                VmInherit::Share => {
                    // Share the same mapping
                    let mut new_entry =
                        VmMapEntry::new(entry.start, entry.end, entry.object.clone(), entry.offset);
                    new_entry.protection = entry.protection;
                    new_entry.max_protection = entry.max_protection;
                    new_entry.inheritance = entry.inheritance;
                    new_entry.flags = entry.flags | EntryFlags::IS_SHARED;
                    new_map.entries.lock().insert(start, new_entry);
                }
                VmInherit::Copy | VmInherit::CopyOnWrite => {
                    // Create shadow object for copy-on-write
                    let shadow_object = if let Some(ref obj) = entry.object {
                        let shadow = crate::mach_vm::vm_object::shadow(
                            Arc::clone(obj),
                            entry.offset,
                            entry.size(),
                        );
                        Some(shadow)
                    } else {
                        None
                    };

                    let mut new_entry = VmMapEntry::new(entry.start, entry.end, shadow_object, 0);
                    new_entry.protection = entry.protection;
                    new_entry.max_protection = entry.max_protection;
                    new_entry.inheritance = entry.inheritance;
                    new_entry.flags = entry.flags | EntryFlags::NEEDS_COPY;
                    new_map.entries.lock().insert(start, new_entry);
                }
            }
        }

        new_map
            .size
            .store(self.size.load(Ordering::SeqCst), Ordering::SeqCst);
        new_map
    }

    /// Allocate virtual space
    pub fn allocate(&self, size: u64, anywhere: bool, addr_hint: u64) -> Result<u64, MapError> {
        let aligned_size = (size + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1);

        let addr = if anywhere {
            self.find_space(aligned_size, PAGE_SIZE as u64 - 1)
                .ok_or(MapError::NoSpace)?
        } else {
            // Try exact address
            let entries = self.entries.lock();
            for (_, entry) in entries.iter() {
                if entry.overlaps(addr_hint, addr_hint + aligned_size) {
                    return Err(MapError::NoSpace);
                }
            }
            addr_hint
        };

        // Create anonymous object
        let object = crate::mach_vm::vm_object::allocate(aligned_size);

        self.enter(
            addr,
            addr + aligned_size,
            Some(object),
            0,
            VmProt::DEFAULT,
            VmProt::ALL,
            VmInherit::Copy,
        )?;

        Ok(addr)
    }
}

// ============================================================================
// Map Errors
// ============================================================================

/// VM Map operation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    /// Invalid address range
    InvalidRange,
    /// No space available
    NoSpace,
    /// Entry not found
    NotFound,
    /// Protection failure
    ProtectionFailure,
    /// Resource shortage
    ResourceShortage,
    /// Map is busy
    Busy,
    /// Invalid argument
    InvalidArgument,
}

// ============================================================================
// Global Map Manager
// ============================================================================

/// Map manager
pub struct MapManager {
    /// All maps
    maps: BTreeMap<VmMapId, Arc<VmMap>>,
    /// Next map ID
    next_id: u64,
    /// Kernel map
    kernel_map: Option<Arc<VmMap>>,
}

impl MapManager {
    pub fn new() -> Self {
        Self {
            maps: BTreeMap::new(),
            next_id: 1,
            kernel_map: None,
        }
    }

    /// Initialize kernel map
    pub fn init_kernel(&mut self, min: u64, max: u64) {
        let id = VmMapId(self.next_id);
        self.next_id += 1;

        let map = Arc::new(VmMap::kernel(id, min, max));
        self.kernel_map = Some(Arc::clone(&map));
        self.maps.insert(id, map);
    }

    /// Get kernel map
    pub fn kernel_map(&self) -> Option<Arc<VmMap>> {
        self.kernel_map.clone()
    }

    /// Create a new map
    pub fn create(&mut self, min: u64, max: u64) -> Arc<VmMap> {
        let id = VmMapId(self.next_id);
        self.next_id += 1;

        let map = Arc::new(VmMap::new(id, min, max));
        self.maps.insert(id, Arc::clone(&map));
        map
    }

    /// Look up a map
    pub fn lookup(&self, id: VmMapId) -> Option<Arc<VmMap>> {
        self.maps.get(&id).cloned()
    }

    /// Deallocate a map
    pub fn deallocate(&mut self, id: VmMapId) {
        if let Some(map) = self.maps.remove(&id) {
            if map.deallocate() {
                // Map will be dropped
            }
        }
    }
}

impl Default for MapManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

static MAP_MANAGER: spin::Once<Mutex<MapManager>> = spin::Once::new();

/// Initialize map subsystem
pub fn init() {
    MAP_MANAGER.call_once(|| {
        let mut mgr = MapManager::new();
        // Initialize kernel map with typical kernel address range
        mgr.init_kernel(0xFFFF_8000_0000_0000, 0xFFFF_FFFF_FFFF_FFFF);
        Mutex::new(mgr)
    });
}

/// Get map manager
fn map_manager() -> &'static Mutex<MapManager> {
    MAP_MANAGER.get().expect("Map manager not initialized")
}

/// Create a map
pub fn create(min: u64, max: u64) -> Arc<VmMap> {
    map_manager().lock().create(min, max)
}

/// Get kernel map
pub fn kernel_map() -> Option<Arc<VmMap>> {
    map_manager().lock().kernel_map()
}

/// Look up a map
pub fn lookup(id: VmMapId) -> Option<Arc<VmMap>> {
    map_manager().lock().lookup(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_prot() {
        let prot = VmProt::DEFAULT;
        assert!(prot.can_read());
        assert!(prot.can_write());
        assert!(!prot.can_execute());
    }

    #[test]
    fn test_map_entry() {
        let entry = VmMapEntry::new(0x1000, 0x3000, None, 0);
        assert_eq!(entry.size(), 0x2000);
        assert!(entry.contains(0x1000));
        assert!(entry.contains(0x2FFF));
        assert!(!entry.contains(0x3000));
    }

    #[test]
    fn test_map_operations() {
        let map = VmMap::new(VmMapId(1), 0x1000, 0x10000);

        // Enter mapping
        assert!(map
            .enter(
                0x2000,
                0x4000,
                None,
                0,
                VmProt::DEFAULT,
                VmProt::ALL,
                VmInherit::Copy
            )
            .is_ok());

        // Lookup
        assert!(map.lookup(0x2500).is_some());
        assert!(map.lookup(0x5000).is_none());

        // Remove
        assert!(map.remove(0x2000, 0x4000).is_ok());
        assert!(map.lookup(0x2500).is_none());
    }

    #[test]
    fn test_find_space() {
        let map = VmMap::new(VmMapId(1), 0x1000, 0x100000);

        // Should find space at beginning
        let addr = map.find_space(0x1000, PAGE_SIZE as u64 - 1);
        assert_eq!(addr, Some(0x1000));

        // Add entry
        let _ = map.enter(
            0x1000,
            0x2000,
            None,
            0,
            VmProt::DEFAULT,
            VmProt::ALL,
            VmInherit::Copy,
        );

        // Should find space after entry
        let addr = map.find_space(0x1000, PAGE_SIZE as u64 - 1);
        assert_eq!(addr, Some(0x2000));
    }
}
