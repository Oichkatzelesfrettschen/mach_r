//! XMM - External Memory Manager Methods
//!
//! Based on OSF/Mach xmm/xmm_methods.h
//!
//! The XMM (External Memory Manager) subsystem provides a polymorphic interface
//! for memory object management. It allows different types of memory managers
//! (default pager, external pagers, network pagers) to implement the same
//! interface.
//!
//! ## Method Categories
//!
//! Manager-side methods (m_*): Called by kernel on memory object
//! Kernel-side methods (k_*): Called by pager to supply data to kernel
//!
//! ## Key Features from OSF/Mach
//!
//! - **Freeze/Thaw**: Checkpoint support for migration
//! - **Synchronize**: Force sync to backing store
//! - **Lock requests**: Page lock/unlock for protection changes
//! - **Existence maps**: Bitmap tracking which pages exist in backing store

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;

use crate::kern::syscall_sw::{KernReturn, KERN_SUCCESS};
use crate::mach_vm::vm_map::VmProt;

// ============================================================================
// Types
// ============================================================================

/// Memory object identifier
pub type MemoryObjectId = u64;

/// Offset within a memory object
pub type VmOffset = usize;

/// Size of a memory region
pub type VmSize = usize;

/// Lock value for page locking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum LockValue {
    #[default]
    Unlock = 0,
    ReadLock = 1,
    WriteLock = 2,
    ExclusiveLock = 3,
}

/// Copy strategy for memory objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum CopyStrategy {
    /// No special copy strategy
    #[default]
    None = 0,
    /// Copy on reference (delay copy)
    Delay = 1,
    /// Copy on write
    CopyOnWrite = 2,
    /// Call pager for copy semantics
    Call = 3,
}

/// Synchronization flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SyncFlags(pub u32);

impl SyncFlags {
    pub const SYNC: Self = Self(0x01);
    pub const ASYNC: Self = Self(0x02);
    pub const INVALIDATE: Self = Self(0x04);
    pub const FLUSH: Self = Self(0x08);
}

/// Memory object state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum MemoryObjectState {
    /// Not yet initialized
    #[default]
    Uncalled = 0,
    /// m_init() called, awaiting k_set_ready()
    Called = 1,
    /// Fully initialized and ready
    Ready = 2,
    /// Shutdown requested
    ShouldTerminate = 3,
    /// Cleanup complete
    Terminated = 4,
}

/// Page data supplied by pager
#[derive(Debug)]
pub struct PageData {
    /// Page content
    pub data: Vec<u8>,
    /// Is this data precious (pager owns updates)?
    pub precious: bool,
    /// Lock value applied to page
    pub lock: LockValue,
}

impl PageData {
    /// Create new page data
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            precious: false,
            lock: LockValue::Unlock,
        }
    }

    /// Create zero-filled page
    pub fn zeroed(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
            precious: false,
            lock: LockValue::Unlock,
        }
    }
}

/// Existence map - bitmap of which pages exist in backing store
#[derive(Debug, Clone)]
pub struct ExistenceMap {
    /// Bitmap data
    bits: Vec<u64>,
    /// Number of pages tracked
    page_count: usize,
}

impl ExistenceMap {
    /// Create new existence map
    pub fn new(page_count: usize) -> Self {
        let words = (page_count + 63) / 64;
        Self {
            bits: vec![0u64; words],
            page_count,
        }
    }

    /// Check if page exists
    pub fn exists(&self, page: usize) -> bool {
        if page >= self.page_count {
            return false;
        }
        let word = page / 64;
        let bit = page % 64;
        (self.bits[word] & (1u64 << bit)) != 0
    }

    /// Mark page as existing
    pub fn set(&mut self, page: usize) {
        if page < self.page_count {
            let word = page / 64;
            let bit = page % 64;
            self.bits[word] |= 1u64 << bit;
        }
    }

    /// Mark page as not existing
    pub fn clear(&mut self, page: usize) {
        if page < self.page_count {
            let word = page / 64;
            let bit = page % 64;
            self.bits[word] &= !(1u64 << bit);
        }
    }

    /// Get raw bits for serialization
    pub fn as_bytes(&self) -> &[u64] {
        &self.bits
    }

    /// Number of pages
    pub fn len(&self) -> usize {
        self.page_count
    }

    /// Check if map is empty
    pub fn is_empty(&self) -> bool {
        self.page_count == 0
    }
}

// ============================================================================
// XMM Methods Trait
// ============================================================================

/// External Memory Manager methods trait
///
/// This trait defines all the methods a memory manager must implement.
/// Based on OSF/Mach xmm_methods.h with 26 method pointers.
pub trait XmmMethods: Send + Sync {
    // ========================================================================
    // Manager-side methods (m_*) - Called by kernel
    // ========================================================================

    /// Initialize the memory object
    ///
    /// Called when kernel first associates with memory object.
    fn m_init(&mut self, page_size: usize, internal: bool, size: VmSize) -> KernReturn;

    /// Terminate the memory object
    ///
    /// Called when kernel no longer needs the object.
    fn m_terminate(&mut self, release: bool) -> KernReturn;

    /// Deallocate memory object resources
    fn m_deallocate(&mut self) -> KernReturn;

    /// Create a copy of this memory object
    fn m_copy(&mut self) -> Result<Arc<Mutex<dyn XmmMethods>>, KernReturn>;

    /// Request page data from pager
    ///
    /// Kernel requests pages from backing store.
    fn m_data_request(
        &mut self,
        offset: VmOffset,
        length: VmSize,
        desired_access: VmProt,
    ) -> KernReturn;

    /// Unlock pages
    fn m_data_unlock(
        &mut self,
        offset: VmOffset,
        length: VmSize,
        desired_access: VmProt,
    ) -> KernReturn;

    /// Return dirty pages to pager
    ///
    /// Kernel returns modified pages for writeback.
    fn m_data_return(
        &mut self,
        offset: VmOffset,
        data: &[u8],
        dirty: bool,
        kernel_copy: bool,
    ) -> KernReturn;

    /// Acknowledge lock completion
    fn m_lock_completed(&mut self, offset: VmOffset, length: VmSize) -> KernReturn;

    /// Acknowledge supply completion
    fn m_supply_completed(
        &mut self,
        offset: VmOffset,
        length: VmSize,
        result: KernReturn,
        error_offset: VmOffset,
    ) -> KernReturn;

    /// Acknowledge attribute change completion
    fn m_change_completed(&mut self, may_cache: bool, copy_strategy: CopyStrategy) -> KernReturn;

    /// Synchronize pages to backing store
    ///
    /// Used for checkpointing and migration.
    fn m_synchronize(&mut self, offset: VmOffset, length: VmSize, flags: SyncFlags) -> KernReturn;

    /// Freeze memory object for migration/checkpoint
    ///
    /// Returns existence map showing which pages exist.
    fn m_freeze(&mut self) -> Result<ExistenceMap, KernReturn>;

    /// Thaw memory object after migration/checkpoint
    fn m_thaw(&mut self) -> KernReturn;

    /// Enable page sharing
    fn m_share(&mut self) -> KernReturn;

    /// Declare a single page to manager
    fn m_declare_page(&mut self, offset: VmOffset, size: VmSize) -> KernReturn;

    /// Declare multiple pages via existence map
    fn m_declare_pages(&mut self, existence_map: &ExistenceMap, frozen: bool) -> KernReturn;

    /// Enable caching
    fn m_caching(&mut self) -> KernReturn;

    /// Disable caching
    fn m_uncaching(&mut self) -> KernReturn;

    // ========================================================================
    // Kernel-side methods (k_*) - Called by pager to kernel
    // ========================================================================

    /// Report unavailable pages
    fn k_data_unavailable(&mut self, offset: VmOffset, length: VmSize) -> KernReturn;

    /// Get memory object attributes
    fn k_get_attributes(&mut self) -> Result<MemoryObjectAttributes, KernReturn>;

    /// Request page locks
    fn k_lock_request(
        &mut self,
        offset: VmOffset,
        length: VmSize,
        should_clean: bool,
        should_flush: bool,
        lock_value: LockValue,
    ) -> KernReturn;

    /// Report I/O error
    fn k_data_error(&mut self, offset: VmOffset, length: VmSize, error: u32) -> KernReturn;

    /// Signal that memory object is ready
    fn k_set_ready(
        &mut self,
        may_cache: bool,
        copy_strategy: CopyStrategy,
        cluster_size: VmSize,
        temporary: bool,
        existence_map: Option<ExistenceMap>,
    ) -> KernReturn;

    /// Destroy memory object
    fn k_destroy(&mut self, reason: u32) -> KernReturn;

    /// Supply page data to kernel
    fn k_data_supply(&mut self, offset: VmOffset, data: PageData) -> KernReturn;

    /// Create copy object handle
    fn k_create_copy(&mut self) -> Result<MemoryObjectId, KernReturn>;

    /// Check if uncaching is permitted
    fn k_uncaching_permitted(&self) -> bool;
}

/// Memory object attributes
#[derive(Debug, Clone, Default)]
pub struct MemoryObjectAttributes {
    /// Can pages be cached?
    pub may_cache: bool,
    /// Copy strategy
    pub copy_strategy: CopyStrategy,
    /// Cluster size for I/O
    pub cluster_size: VmSize,
    /// Is this temporary?
    pub temporary: bool,
    /// Is object ready?
    pub ready: bool,
}

// ============================================================================
// Default Memory Object (Kernel-managed anonymous memory)
// ============================================================================

/// Default memory object for anonymous memory
#[derive(Debug)]
pub struct DefaultMemoryObject {
    /// Object ID
    id: MemoryObjectId,
    /// Object state
    state: MemoryObjectState,
    /// Size in bytes
    size: VmSize,
    /// Page size
    page_size: usize,
    /// Attributes
    attributes: MemoryObjectAttributes,
    /// Existence map
    existence_map: Option<ExistenceMap>,
}

impl DefaultMemoryObject {
    /// Create new default memory object
    pub fn new(id: MemoryObjectId, size: VmSize) -> Self {
        Self {
            id,
            state: MemoryObjectState::Uncalled,
            size,
            page_size: 4096,
            attributes: MemoryObjectAttributes::default(),
            existence_map: None,
        }
    }
}

impl XmmMethods for DefaultMemoryObject {
    fn m_init(&mut self, page_size: usize, _internal: bool, size: VmSize) -> KernReturn {
        self.page_size = page_size;
        self.size = size;
        self.state = MemoryObjectState::Called;
        KERN_SUCCESS
    }

    fn m_terminate(&mut self, _release: bool) -> KernReturn {
        self.state = MemoryObjectState::ShouldTerminate;
        KERN_SUCCESS
    }

    fn m_deallocate(&mut self) -> KernReturn {
        self.state = MemoryObjectState::Terminated;
        KERN_SUCCESS
    }

    fn m_copy(&mut self) -> Result<Arc<Mutex<dyn XmmMethods>>, KernReturn> {
        // Create a copy of this memory object
        let copy = DefaultMemoryObject {
            id: self.id + 1, // Simplified ID generation
            state: MemoryObjectState::Ready,
            size: self.size,
            page_size: self.page_size,
            attributes: self.attributes.clone(),
            existence_map: self.existence_map.clone(),
        };
        Ok(Arc::new(Mutex::new(copy)))
    }

    fn m_data_request(
        &mut self,
        _offset: VmOffset,
        _length: VmSize,
        _desired_access: VmProt,
    ) -> KernReturn {
        // Default object provides zero-filled pages
        // In a real implementation, this would coordinate with vm_fault
        KERN_SUCCESS
    }

    fn m_data_unlock(
        &mut self,
        _offset: VmOffset,
        _length: VmSize,
        _desired_access: VmProt,
    ) -> KernReturn {
        KERN_SUCCESS
    }

    fn m_data_return(
        &mut self,
        _offset: VmOffset,
        _data: &[u8],
        _dirty: bool,
        _kernel_copy: bool,
    ) -> KernReturn {
        // Default object doesn't persist data
        KERN_SUCCESS
    }

    fn m_lock_completed(&mut self, _offset: VmOffset, _length: VmSize) -> KernReturn {
        KERN_SUCCESS
    }

    fn m_supply_completed(
        &mut self,
        _offset: VmOffset,
        _length: VmSize,
        _result: KernReturn,
        _error_offset: VmOffset,
    ) -> KernReturn {
        KERN_SUCCESS
    }

    fn m_change_completed(&mut self, may_cache: bool, copy_strategy: CopyStrategy) -> KernReturn {
        self.attributes.may_cache = may_cache;
        self.attributes.copy_strategy = copy_strategy;
        KERN_SUCCESS
    }

    fn m_synchronize(
        &mut self,
        _offset: VmOffset,
        _length: VmSize,
        _flags: SyncFlags,
    ) -> KernReturn {
        // Default object has nothing to sync
        KERN_SUCCESS
    }

    fn m_freeze(&mut self) -> Result<ExistenceMap, KernReturn> {
        let page_count = self.size / self.page_size;
        let map = self
            .existence_map
            .clone()
            .unwrap_or_else(|| ExistenceMap::new(page_count));
        Ok(map)
    }

    fn m_thaw(&mut self) -> KernReturn {
        KERN_SUCCESS
    }

    fn m_share(&mut self) -> KernReturn {
        KERN_SUCCESS
    }

    fn m_declare_page(&mut self, offset: VmOffset, _size: VmSize) -> KernReturn {
        let page = offset / self.page_size;
        if let Some(ref mut map) = self.existence_map {
            map.set(page);
        } else {
            let page_count = self.size / self.page_size;
            let mut map = ExistenceMap::new(page_count);
            map.set(page);
            self.existence_map = Some(map);
        }
        KERN_SUCCESS
    }

    fn m_declare_pages(&mut self, existence_map: &ExistenceMap, _frozen: bool) -> KernReturn {
        self.existence_map = Some(existence_map.clone());
        KERN_SUCCESS
    }

    fn m_caching(&mut self) -> KernReturn {
        self.attributes.may_cache = true;
        KERN_SUCCESS
    }

    fn m_uncaching(&mut self) -> KernReturn {
        self.attributes.may_cache = false;
        KERN_SUCCESS
    }

    fn k_data_unavailable(&mut self, _offset: VmOffset, _length: VmSize) -> KernReturn {
        // Should signal page fault error
        KERN_SUCCESS
    }

    fn k_get_attributes(&mut self) -> Result<MemoryObjectAttributes, KernReturn> {
        Ok(self.attributes.clone())
    }

    fn k_lock_request(
        &mut self,
        _offset: VmOffset,
        _length: VmSize,
        _should_clean: bool,
        _should_flush: bool,
        _lock_value: LockValue,
    ) -> KernReturn {
        KERN_SUCCESS
    }

    fn k_data_error(&mut self, _offset: VmOffset, _length: VmSize, _error: u32) -> KernReturn {
        KERN_SUCCESS
    }

    fn k_set_ready(
        &mut self,
        may_cache: bool,
        copy_strategy: CopyStrategy,
        cluster_size: VmSize,
        temporary: bool,
        existence_map: Option<ExistenceMap>,
    ) -> KernReturn {
        self.state = MemoryObjectState::Ready;
        self.attributes.may_cache = may_cache;
        self.attributes.copy_strategy = copy_strategy;
        self.attributes.cluster_size = cluster_size;
        self.attributes.temporary = temporary;
        self.attributes.ready = true;
        self.existence_map = existence_map;
        KERN_SUCCESS
    }

    fn k_destroy(&mut self, _reason: u32) -> KernReturn {
        self.state = MemoryObjectState::Terminated;
        KERN_SUCCESS
    }

    fn k_data_supply(&mut self, offset: VmOffset, _data: PageData) -> KernReturn {
        // Mark page as present
        let page = offset / self.page_size;
        if let Some(ref mut map) = self.existence_map {
            map.set(page);
        }
        KERN_SUCCESS
    }

    fn k_create_copy(&mut self) -> Result<MemoryObjectId, KernReturn> {
        Ok(self.id + 1)
    }

    fn k_uncaching_permitted(&self) -> bool {
        true
    }
}

// ============================================================================
// XMM Object Wrapper
// ============================================================================

/// Thread-safe XMM object wrapper
pub type XmmObject = Arc<Mutex<dyn XmmMethods>>;

/// Create a new default memory object
pub fn create_default_object(id: MemoryObjectId, size: VmSize) -> XmmObject {
    Arc::new(Mutex::new(DefaultMemoryObject::new(id, size)))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_existence_map() {
        let mut map = ExistenceMap::new(1000);
        assert!(!map.exists(0));
        assert!(!map.exists(500));

        map.set(500);
        assert!(map.exists(500));
        assert!(!map.exists(501));

        map.clear(500);
        assert!(!map.exists(500));
    }

    #[test]
    fn test_default_memory_object() {
        let mut obj = DefaultMemoryObject::new(1, 4096 * 100);

        // Initialize
        assert_eq!(obj.m_init(4096, false, 4096 * 100), KERN_SUCCESS);
        assert_eq!(obj.state, MemoryObjectState::Called);

        // Set ready
        assert_eq!(
            obj.k_set_ready(true, CopyStrategy::CopyOnWrite, 65536, false, None),
            KERN_SUCCESS
        );
        assert_eq!(obj.state, MemoryObjectState::Ready);

        // Freeze
        let map = obj.m_freeze().unwrap();
        assert_eq!(map.len(), 100);
    }

    #[test]
    fn test_xmm_copy() {
        let mut obj = DefaultMemoryObject::new(1, 4096 * 10);
        obj.m_init(4096, false, 4096 * 10);

        let copy = obj.m_copy().unwrap();
        let copy_guard = copy.lock();
        // Verify copy exists
        assert!(copy_guard.k_uncaching_permitted());
    }
}
