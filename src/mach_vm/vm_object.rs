//! VM Object - Memory Object Abstraction
//!
//! Based on Mach4 vm/vm_object.h/c
//! VM objects represent backing store for virtual memory regions.
//! They can be backed by:
//! - Physical memory (anonymous memory)
//! - External pagers (files, devices)
//! - Shadow chains (copy-on-write)

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::ipc::PortName;

// ============================================================================
// VM Object Types
// ============================================================================

/// VM Object ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VmObjectId(pub u64);

impl VmObjectId {
    /// Null object ID
    pub const NULL: Self = Self(0);
}

/// Object pager type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerType {
    /// Anonymous memory (no pager)
    Anonymous,
    /// Default pager (swap)
    Default,
    /// External pager (vnode, device, etc.)
    External,
}

/// Object copy strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyStrategy {
    /// Symmetric copy (both share until modified)
    Symmetric,
    /// Asymmetric copy (delay source copy)
    Delay,
    /// No copy
    None,
}

// ============================================================================
// VM Object Flags
// ============================================================================

/// VM Object flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectFlags(u32);

impl ObjectFlags {
    /// Object is internal (anonymous)
    pub const INTERNAL: Self = Self(0x0001);
    /// Object is temporary
    pub const TEMPORARY: Self = Self(0x0002);
    /// Object can be paged out
    pub const PAGEABLE: Self = Self(0x0004);
    /// Object is shadowed
    pub const SHADOWED: Self = Self(0x0008);
    /// Object is alive
    pub const ALIVE: Self = Self(0x0010);
    /// Object is being paged
    pub const PAGING: Self = Self(0x0020);
    /// Object is cached
    pub const CACHED: Self = Self(0x0040);
    /// Object is locked
    pub const LOCKED: Self = Self(0x0080);
    /// Object should not be coalesced
    pub const NO_COALESCE: Self = Self(0x0100);
    /// Object is copy-on-write source
    pub const COW_SOURCE: Self = Self(0x0200);
    /// Object is being terminated
    pub const TERMINATING: Self = Self(0x0400);
    /// Object is for kernel use only
    pub const KERNEL_ONLY: Self = Self(0x0800);

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

    /// Difference from another flags
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl core::ops::BitOr for ObjectFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl Default for ObjectFlags {
    fn default() -> Self {
        Self(Self::ALIVE.0 | Self::INTERNAL.0 | Self::PAGEABLE.0)
    }
}

// ============================================================================
// VM Object Structure
// ============================================================================

/// VM Object - represents a unit of backing store
#[derive(Debug)]
pub struct VmObject {
    /// Object ID
    pub id: VmObjectId,

    /// Reference count
    ref_count: AtomicU32,

    /// Resident reference count (maps using this object)
    resident_count: AtomicU32,

    /// Size in bytes
    size: AtomicU64,

    /// Object flags
    flags: AtomicU32,

    /// Pager type
    pub pager_type: PagerType,

    /// Pager port (for external pagers)
    pub pager: Mutex<Option<PortName>>,

    /// Pager control port
    pub pager_control: Mutex<Option<PortName>>,

    /// Shadow object (for copy-on-write)
    pub shadow: Mutex<Option<Arc<VmObject>>>,

    /// Offset into shadow
    pub shadow_offset: AtomicU64,

    /// Copy strategy
    pub copy_strategy: Mutex<CopyStrategy>,

    /// Resident pages (offset -> page number)
    pub pages: Mutex<BTreeMap<u64, u32>>,

    /// Paging offset (for pager)
    pub paging_offset: AtomicU64,

    /// Last page-in offset
    pub last_page_in_offset: AtomicU64,

    /// Copy object (for delayed copy)
    pub copy: Mutex<Option<Arc<VmObject>>>,

    /// Is this object busy?
    pub busy: AtomicBool,

    /// Is someone waiting on this object?
    pub wanted: AtomicBool,
}

impl VmObject {
    /// Create a new VM object
    pub fn new(id: VmObjectId, size: u64) -> Self {
        Self {
            id,
            ref_count: AtomicU32::new(1),
            resident_count: AtomicU32::new(0),
            size: AtomicU64::new(size),
            flags: AtomicU32::new(ObjectFlags::default().bits()),
            pager_type: PagerType::Anonymous,
            pager: Mutex::new(None),
            pager_control: Mutex::new(None),
            shadow: Mutex::new(None),
            shadow_offset: AtomicU64::new(0),
            copy_strategy: Mutex::new(CopyStrategy::Symmetric),
            pages: Mutex::new(BTreeMap::new()),
            paging_offset: AtomicU64::new(0),
            last_page_in_offset: AtomicU64::new(0),
            copy: Mutex::new(None),
            busy: AtomicBool::new(false),
            wanted: AtomicBool::new(false),
        }
    }

    /// Create an anonymous (internal) object
    pub fn anonymous(id: VmObjectId, size: u64) -> Self {
        let obj = Self::new(id, size);
        obj.flags.store(
            (ObjectFlags::ALIVE
                | ObjectFlags::INTERNAL
                | ObjectFlags::PAGEABLE
                | ObjectFlags::TEMPORARY)
                .bits(),
            Ordering::SeqCst,
        );
        obj
    }

    /// Create an object for external paging
    pub fn with_pager(id: VmObjectId, size: u64, pager: PortName) -> Self {
        let mut obj = Self::new(id, size);
        obj.pager_type = PagerType::External;
        *obj.pager.lock() = Some(pager);
        obj.flags.store(
            (ObjectFlags::ALIVE | ObjectFlags::PAGEABLE).bits(),
            Ordering::SeqCst,
        );
        obj
    }

    /// Get object size
    pub fn size(&self) -> u64 {
        self.size.load(Ordering::SeqCst)
    }

    /// Set object size
    pub fn set_size(&self, size: u64) {
        self.size.store(size, Ordering::SeqCst);
    }

    /// Get object flags
    pub fn get_flags(&self) -> ObjectFlags {
        ObjectFlags::from_bits_truncate(self.flags.load(Ordering::SeqCst))
    }

    /// Set object flags
    pub fn set_flags(&self, flags: ObjectFlags) {
        self.flags.fetch_or(flags.bits(), Ordering::SeqCst);
    }

    /// Clear object flags
    pub fn clear_flags(&self, flags: ObjectFlags) {
        self.flags.fetch_and(!flags.bits(), Ordering::SeqCst);
    }

    /// Check if object is alive
    pub fn is_alive(&self) -> bool {
        self.get_flags().contains(ObjectFlags::ALIVE)
    }

    /// Check if object is internal
    pub fn is_internal(&self) -> bool {
        self.get_flags().contains(ObjectFlags::INTERNAL)
    }

    /// Increment reference count
    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement reference count, returns true if object should be destroyed
    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    /// Get reference count
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::SeqCst)
    }

    /// Lock the object
    pub fn lock(&self) -> bool {
        !self.busy.swap(true, Ordering::SeqCst)
    }

    /// Unlock the object
    pub fn unlock(&self) {
        self.busy.store(false, Ordering::SeqCst);
        if self.wanted.swap(false, Ordering::SeqCst) {
            // Would wake waiters
        }
    }

    /// Insert a page into this object
    pub fn page_insert(&self, offset: u64, page_num: u32) {
        self.pages.lock().insert(offset, page_num);
        self.resident_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Remove a page from this object
    pub fn page_remove(&self, offset: u64) -> Option<u32> {
        let page = self.pages.lock().remove(&offset)?;
        self.resident_count.fetch_sub(1, Ordering::SeqCst);
        Some(page)
    }

    /// Lookup a page in this object
    pub fn page_lookup(&self, offset: u64) -> Option<u32> {
        self.pages.lock().get(&offset).copied()
    }

    /// Get resident page count
    pub fn resident_page_count(&self) -> usize {
        self.pages.lock().len()
    }

    /// Set shadow object for copy-on-write
    pub fn set_shadow(&self, shadow: Arc<VmObject>, offset: u64) {
        shadow.reference();
        shadow.set_flags(ObjectFlags::SHADOWED);
        *self.shadow.lock() = Some(shadow);
        self.shadow_offset.store(offset, Ordering::SeqCst);
    }

    /// Get shadow object
    pub fn get_shadow(&self) -> Option<Arc<VmObject>> {
        self.shadow.lock().clone()
    }

    /// Create a shadow copy of this object
    pub fn shadow_copy(self: &Arc<Self>, id: VmObjectId, offset: u64, size: u64) -> VmObject {
        let copy = VmObject::new(id, size);
        copy.set_shadow(Arc::clone(self), offset);
        copy
    }

    /// Collapse shadow chain if possible
    /// Returns true if collapse occurred
    pub fn collapse(&self) -> bool {
        let shadow = self.shadow.lock().clone();
        if let Some(shadow_obj) = shadow {
            // Only collapse if shadow has no other references
            if shadow_obj.ref_count() == 1
                && !shadow_obj.get_flags().contains(ObjectFlags::SHADOWED)
            {
                // Move pages from shadow to us
                let shadow_offset = self.shadow_offset.load(Ordering::SeqCst);
                let shadow_pages = shadow_obj.pages.lock().clone();

                for (offset, page_num) in shadow_pages {
                    let our_offset = offset.saturating_sub(shadow_offset);
                    // Only copy if we don't have this page
                    if self.page_lookup(our_offset).is_none() {
                        self.page_insert(our_offset, page_num);
                    }
                }

                // Clear shadow
                *self.shadow.lock() = None;
                return true;
            }
        }
        false
    }

    /// Copy pages from this object to a new object (slow copy)
    ///
    /// This is used when we need to actually copy pages rather than
    /// use shadow chains (e.g., when shadow depth is too high).
    ///
    /// Based on Mach4 vm_object_copy_slowly
    pub fn copy_slowly(&self, src_offset: u64, size: u64, dst: &Arc<VmObject>) -> bool {
        let src_pages = self.pages.lock();
        let page_size = crate::mach_vm::vm_page::PAGE_SIZE as u64;

        // Iterate through the range and copy each resident page
        let mut offset = src_offset;
        while offset < src_offset + size {
            if let Some(&src_page_num) = src_pages.get(&offset) {
                // Allocate destination page
                if let Some(dst_addr) = crate::mach_vm::vm_page::alloc_page() {
                    let dst_page_num = crate::mach_vm::vm_page::addr_to_page(dst_addr);

                    // In real implementation: copy page contents
                    // copy_phys_page(src_page_num, dst_page_num);
                    let _ = src_page_num; // Suppress warning

                    // Insert into destination object
                    dst.page_insert(offset - src_offset, dst_page_num);
                } else {
                    // Out of memory
                    return false;
                }
            }
            offset += page_size;
        }
        true
    }

    /// Copy object using shadow chains (fast copy)
    ///
    /// Based on Mach4 vm_object_copy_strategically
    /// Returns the new copy object
    pub fn copy_strategically(self: &Arc<Self>, offset: u64, size: u64) -> Option<Arc<VmObject>> {
        let strategy = *self.copy_strategy.lock();

        match strategy {
            CopyStrategy::Symmetric => {
                // Both source and copy share pages via shadow
                Some(crate::mach_vm::vm_object::shadow(
                    Arc::clone(self),
                    offset,
                    size,
                ))
            }
            CopyStrategy::Delay => {
                // Delay copy by using source's copy object
                let mut copy_guard = self.copy.lock();
                if copy_guard.is_none() {
                    // Create copy object on first use
                    let copy_obj =
                        crate::mach_vm::vm_object::shadow(Arc::clone(self), offset, size);
                    *copy_guard = Some(Arc::clone(&copy_obj));
                    Some(copy_obj)
                } else {
                    copy_guard.clone()
                }
            }
            CopyStrategy::None => {
                // No copy - caller must use copy_slowly
                None
            }
        }
    }

    /// Check shadow chain depth
    pub fn shadow_depth(&self) -> u32 {
        let mut depth = 0;
        let mut current = self.shadow.lock().clone();
        while let Some(obj) = current {
            depth += 1;
            if depth > 100 {
                // Prevent infinite loops
                break;
            }
            current = obj.shadow.lock().clone();
        }
        depth
    }

    /// Bypass shadow chain to find original object
    pub fn shadow_root(self: &Arc<Self>) -> Arc<VmObject> {
        let mut current = Arc::clone(self);
        loop {
            let next = current.shadow.lock().clone();
            match next {
                Some(shadow) => current = shadow,
                None => return current,
            }
        }
    }

    /// Check if object needs shadow chain collapse
    ///
    /// Returns true if shadow depth exceeds threshold
    pub fn needs_collapse(&self) -> bool {
        const MAX_SHADOW_DEPTH: u32 = 10;
        self.shadow_depth() > MAX_SHADOW_DEPTH
    }

    /// Terminate this object
    pub fn terminate(&self) {
        self.set_flags(ObjectFlags::TERMINATING);
        self.clear_flags(ObjectFlags::ALIVE);

        // Free all resident pages
        let pages: Vec<_> = self.pages.lock().values().copied().collect();
        for page_num in pages {
            crate::mach_vm::vm_page::free_page(crate::mach_vm::vm_page::page_to_addr(page_num));
        }
        self.pages.lock().clear();

        // Clear shadow reference
        if let Some(shadow) = self.shadow.lock().take() {
            let _ = shadow.deallocate();
        }
    }
}

impl Drop for VmObject {
    fn drop(&mut self) {
        if self.is_alive() {
            self.terminate();
        }
    }
}

// ============================================================================
// Object Cache (for recently freed objects)
// ============================================================================

/// Cached object entry
struct CachedObject {
    object: Arc<VmObject>,
    timestamp: u64,
}

/// Object cache for quick reuse
pub struct ObjectCache {
    objects: Vec<CachedObject>,
    max_size: usize,
}

impl ObjectCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            objects: Vec::new(),
            max_size,
        }
    }

    pub fn add(&mut self, object: Arc<VmObject>, timestamp: u64) {
        if self.objects.len() >= self.max_size {
            self.objects.remove(0);
        }
        self.objects.push(CachedObject { object, timestamp });
    }

    pub fn get(&mut self, size: u64) -> Option<Arc<VmObject>> {
        for i in 0..self.objects.len() {
            if self.objects[i].object.size() >= size {
                return Some(self.objects.remove(i).object);
            }
        }
        None
    }

    pub fn trim(&mut self, cutoff_timestamp: u64) {
        self.objects.retain(|c| c.timestamp > cutoff_timestamp);
    }
}

// ============================================================================
// Global Object Manager
// ============================================================================

/// VM Object manager
pub struct ObjectManager {
    /// All objects
    objects: BTreeMap<VmObjectId, Arc<VmObject>>,
    /// Next object ID
    next_id: u64,
    /// Object cache
    cache: ObjectCache,
    /// Kernel object (for wired kernel memory)
    kernel_object: Option<Arc<VmObject>>,
}

impl ObjectManager {
    pub fn new() -> Self {
        Self {
            objects: BTreeMap::new(),
            next_id: 1,
            cache: ObjectCache::new(32),
            kernel_object: None,
        }
    }

    /// Initialize kernel object
    pub fn init_kernel(&mut self) {
        let id = VmObjectId(self.next_id);
        self.next_id += 1;

        let obj = VmObject::new(id, u64::MAX);
        obj.set_flags(ObjectFlags::KERNEL_ONLY);
        obj.clear_flags(ObjectFlags::PAGEABLE);

        let arc_obj = Arc::new(obj);
        self.kernel_object = Some(Arc::clone(&arc_obj));
        self.objects.insert(id, arc_obj);
    }

    /// Get kernel object
    pub fn kernel_object(&self) -> Option<Arc<VmObject>> {
        self.kernel_object.clone()
    }

    /// Allocate a new object
    pub fn allocate(&mut self, size: u64) -> Arc<VmObject> {
        // Try cache first
        if let Some(obj) = self.cache.get(size) {
            obj.set_size(size);
            return obj;
        }

        // Create new object
        let id = VmObjectId(self.next_id);
        self.next_id += 1;

        let obj = Arc::new(VmObject::anonymous(id, size));
        self.objects.insert(id, Arc::clone(&obj));
        obj
    }

    /// Allocate object with external pager
    pub fn allocate_with_pager(&mut self, size: u64, pager: PortName) -> Arc<VmObject> {
        let id = VmObjectId(self.next_id);
        self.next_id += 1;

        let obj = Arc::new(VmObject::with_pager(id, size, pager));
        self.objects.insert(id, Arc::clone(&obj));
        obj
    }

    /// Deallocate an object
    pub fn deallocate(&mut self, id: VmObjectId) {
        if let Some(obj) = self.objects.remove(&id) {
            if obj.deallocate() {
                obj.terminate();
            }
        }
    }

    /// Look up an object
    pub fn lookup(&self, id: VmObjectId) -> Option<Arc<VmObject>> {
        self.objects.get(&id).cloned()
    }

    /// Shadow an existing object (copy-on-write)
    pub fn shadow(&mut self, source: Arc<VmObject>, offset: u64, size: u64) -> Arc<VmObject> {
        let id = VmObjectId(self.next_id);
        self.next_id += 1;

        let obj = Arc::new(source.shadow_copy(id, offset, size));
        self.objects.insert(id, Arc::clone(&obj));
        obj
    }
}

impl Default for ObjectManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

static OBJECT_MANAGER: spin::Once<Mutex<ObjectManager>> = spin::Once::new();

/// Initialize object subsystem
pub fn init() {
    OBJECT_MANAGER.call_once(|| {
        let mut mgr = ObjectManager::new();
        mgr.init_kernel();
        Mutex::new(mgr)
    });
}

/// Get object manager
fn object_manager() -> &'static Mutex<ObjectManager> {
    OBJECT_MANAGER
        .get()
        .expect("Object manager not initialized")
}

/// Allocate an object
pub fn allocate(size: u64) -> Arc<VmObject> {
    object_manager().lock().allocate(size)
}

/// Allocate object with pager
pub fn allocate_with_pager(size: u64, pager: PortName) -> Arc<VmObject> {
    object_manager().lock().allocate_with_pager(size, pager)
}

/// Shadow an object
pub fn shadow(source: Arc<VmObject>, offset: u64, size: u64) -> Arc<VmObject> {
    object_manager().lock().shadow(source, offset, size)
}

/// Look up an object
pub fn lookup(id: VmObjectId) -> Option<Arc<VmObject>> {
    object_manager().lock().lookup(id)
}

/// Get kernel object
pub fn kernel_object() -> Option<Arc<VmObject>> {
    object_manager().lock().kernel_object()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_creation() {
        let obj = VmObject::anonymous(VmObjectId(1), 4096);
        assert!(obj.is_alive());
        assert!(obj.is_internal());
        assert_eq!(obj.size(), 4096);
    }

    #[test]
    fn test_object_pages() {
        let obj = VmObject::new(VmObjectId(1), 8192);

        obj.page_insert(0, 100);
        obj.page_insert(4096, 101);

        assert_eq!(obj.page_lookup(0), Some(100));
        assert_eq!(obj.page_lookup(4096), Some(101));
        assert_eq!(obj.page_lookup(8192), None);

        assert_eq!(obj.page_remove(0), Some(100));
        assert_eq!(obj.page_lookup(0), None);
    }

    #[test]
    fn test_object_reference() {
        let obj = VmObject::new(VmObjectId(1), 4096);
        assert_eq!(obj.ref_count(), 1);

        obj.reference();
        assert_eq!(obj.ref_count(), 2);

        assert!(!obj.deallocate()); // Still has references
        assert_eq!(obj.ref_count(), 1);

        assert!(obj.deallocate()); // Last reference
    }
}
