//! External Memory Objects
//!
//! Based on Mach4 mach/memory_object.h by Michael Wayne Young
//!
//! External memory management (EMM) allows user-space "pagers" to
//! provide the backing store for memory objects. This is one of Mach's
//! most distinctive features, enabling:
//! - Memory-mapped files
//! - Distributed shared memory
//! - Copy-on-write with external pagers
//! - Network-transparent memory
//!
//! The memory object protocol:
//! 1. Kernel sends `memory_object_init` to pager when object is created
//! 2. Pager replies with `memory_object_set_attributes`
//! 3. On page fault, kernel sends `memory_object_data_request`
//! 4. Pager replies with `memory_object_data_supply`
//! 5. On pageout, kernel sends `memory_object_data_return`
//! 6. Kernel sends `memory_object_terminate` when object destroyed

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::ipc::PortName;
use crate::mach_vm::vm_object::VmObjectId;

// ============================================================================
// Copy Strategy
// ============================================================================

/// Memory object copy strategy
///
/// Defines how the kernel handles copy-on-write for this object
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum CopyStrategy {
    /// No special support - make copy on write
    #[default]
    None = 0,
    /// Call memory manager on copy
    Call = 1,
    /// Delay copy until write (manager doesn't modify)
    Delay = 2,
    /// Temporary object (manager doesn't change data, doesn't need to see changes)
    Temporary = 3,
}

impl CopyStrategy {
    pub fn from_i32(val: i32) -> Option<Self> {
        match val {
            0 => Some(Self::None),
            1 => Some(Self::Call),
            2 => Some(Self::Delay),
            3 => Some(Self::Temporary),
            _ => None,
        }
    }

    /// Can the kernel make optimized copy-on-write?
    pub fn allows_cow_optimization(&self) -> bool {
        matches!(self, Self::Delay | Self::Temporary)
    }

    /// Does the manager need to see modifications?
    pub fn needs_writeback(&self) -> bool {
        !matches!(self, Self::Temporary)
    }
}

// ============================================================================
// Return Policy
// ============================================================================

/// Memory object return policy
///
/// Which pages to return to the manager on lock_request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum ReturnPolicy {
    /// Don't return any pages
    #[default]
    None = 0,
    /// Only return dirty pages
    Dirty = 1,
    /// Return dirty and precious pages
    All = 2,
}

impl ReturnPolicy {
    pub fn from_i32(val: i32) -> Option<Self> {
        match val {
            0 => Some(Self::None),
            1 => Some(Self::Dirty),
            2 => Some(Self::All),
            _ => None,
        }
    }
}

// ============================================================================
// Memory Object Attributes
// ============================================================================

/// Attributes for a memory object
#[derive(Debug, Clone)]
pub struct MemoryObjectAttributes {
    /// Copy strategy
    pub copy_strategy: CopyStrategy,
    /// Object ready for paging?
    pub ready: bool,
    /// May kernel cache pages?
    pub may_cache: bool,
    /// Is object temporary (no persistence)?
    pub temporary: bool,
    /// Pager cluster size (pages)
    pub cluster_size: u32,
}

impl Default for MemoryObjectAttributes {
    fn default() -> Self {
        Self {
            copy_strategy: CopyStrategy::Delay,
            ready: false,
            may_cache: true,
            temporary: false,
            cluster_size: 1,
        }
    }
}

impl MemoryObjectAttributes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create temporary object attributes
    pub fn temporary() -> Self {
        Self {
            copy_strategy: CopyStrategy::Temporary,
            ready: true,
            may_cache: true,
            temporary: true,
            cluster_size: 1,
        }
    }
}

// ============================================================================
// Memory Object State
// ============================================================================

/// Memory object pager state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum PagerState {
    /// Not initialized
    #[default]
    Uninitialized = 0,
    /// Initialization in progress
    Initializing = 1,
    /// Ready for requests
    Ready = 2,
    /// Terminating
    Terminating = 3,
    /// Terminated
    Terminated = 4,
}

impl PagerState {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Initializing | Self::Ready)
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

// ============================================================================
// Memory Object Control
// ============================================================================

/// Memory object control port
///
/// Given to the pager to control the memory object
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemoryObjectControl(pub PortName);

impl MemoryObjectControl {
    pub const NULL: Self = Self(PortName(0));

    pub fn new(port: PortName) -> Self {
        Self(port)
    }

    pub fn port(&self) -> PortName {
        self.0
    }

    pub fn is_null(&self) -> bool {
        self.0 == PortName(0)
    }
}

impl Default for MemoryObjectControl {
    fn default() -> Self {
        Self::NULL
    }
}

// ============================================================================
// Memory Object Name
// ============================================================================

/// Memory object name port
///
/// Used to identify the object in vm_region() calls
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemoryObjectName(pub PortName);

impl MemoryObjectName {
    pub const NULL: Self = Self(PortName(0));

    pub fn new(port: PortName) -> Self {
        Self(port)
    }

    pub fn port(&self) -> PortName {
        self.0
    }

    pub fn is_null(&self) -> bool {
        self.0 == PortName(0)
    }
}

impl Default for MemoryObjectName {
    fn default() -> Self {
        Self::NULL
    }
}

// ============================================================================
// Memory Object
// ============================================================================

/// External memory object
///
/// Represents the kernel's side of an external pager connection
#[derive(Debug)]
pub struct MemoryObject {
    /// Object ID
    pub id: MemoryObjectId,

    /// Pager port (the external pager)
    pub pager_port: Mutex<Option<PortName>>,

    /// Control port (given to pager)
    pub control_port: Mutex<MemoryObjectControl>,

    /// Name port (for vm_region)
    pub name_port: Mutex<MemoryObjectName>,

    /// Associated VM object (if any)
    pub vm_object: Mutex<Option<VmObjectId>>,

    /// Pager state
    pub state: Mutex<PagerState>,

    /// Attributes
    pub attributes: Mutex<MemoryObjectAttributes>,

    /// Reference count
    ref_count: AtomicU32,

    /// Is this a default pager object?
    pub is_default_pager: AtomicBool,
}

/// Memory object identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MemoryObjectId(pub u64);

impl MemoryObjectId {
    pub const NULL: Self = Self(0);
}

impl MemoryObject {
    /// Create a new memory object
    pub fn new(id: MemoryObjectId) -> Self {
        Self {
            id,
            pager_port: Mutex::new(None),
            control_port: Mutex::new(MemoryObjectControl::NULL),
            name_port: Mutex::new(MemoryObjectName::NULL),
            vm_object: Mutex::new(None),
            state: Mutex::new(PagerState::Uninitialized),
            attributes: Mutex::new(MemoryObjectAttributes::default()),
            ref_count: AtomicU32::new(1),
            is_default_pager: AtomicBool::new(false),
        }
    }

    /// Create a temporary memory object (no external pager)
    pub fn temporary(id: MemoryObjectId) -> Self {
        let obj = Self::new(id);
        *obj.state.lock() = PagerState::Ready;
        *obj.attributes.lock() = MemoryObjectAttributes::temporary();
        obj
    }

    // === Reference counting ===

    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::Relaxed)
    }

    // === State management ===

    pub fn get_state(&self) -> PagerState {
        *self.state.lock()
    }

    pub fn is_ready(&self) -> bool {
        self.state.lock().is_ready()
    }

    pub fn set_ready(&self) {
        *self.state.lock() = PagerState::Ready;
    }

    pub fn set_terminating(&self) {
        *self.state.lock() = PagerState::Terminating;
    }

    pub fn set_terminated(&self) {
        *self.state.lock() = PagerState::Terminated;
    }

    // === Port management ===

    pub fn set_pager_port(&self, port: PortName) {
        *self.pager_port.lock() = Some(port);
    }

    pub fn get_pager_port(&self) -> Option<PortName> {
        *self.pager_port.lock()
    }

    pub fn set_control_port(&self, control: MemoryObjectControl) {
        *self.control_port.lock() = control;
    }

    pub fn get_control_port(&self) -> MemoryObjectControl {
        *self.control_port.lock()
    }

    // === Attributes ===

    pub fn get_attributes(&self) -> MemoryObjectAttributes {
        self.attributes.lock().clone()
    }

    pub fn set_attributes(&self, attrs: MemoryObjectAttributes) {
        let mut lock = self.attributes.lock();
        *lock = attrs;
        // If attributes set, object becomes ready
        if !self.is_ready() {
            drop(lock);
            *self.state.lock() = PagerState::Ready;
        }
    }

    pub fn get_copy_strategy(&self) -> CopyStrategy {
        self.attributes.lock().copy_strategy
    }

    pub fn set_copy_strategy(&self, strategy: CopyStrategy) {
        self.attributes.lock().copy_strategy = strategy;
    }
}

// ============================================================================
// Memory Object Manager
// ============================================================================

/// Statistics for memory objects
#[derive(Debug, Default)]
pub struct MemoryObjectStats {
    /// Total objects created
    pub created: AtomicU64,
    /// Total objects destroyed
    pub destroyed: AtomicU64,
    /// Data requests sent
    pub data_requests: AtomicU64,
    /// Data supplies received
    pub data_supplies: AtomicU64,
    /// Data returns sent
    pub data_returns: AtomicU64,
    /// Lock requests sent
    pub lock_requests: AtomicU64,
}

impl MemoryObjectStats {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Memory object manager
pub struct MemoryObjectManager {
    /// All memory objects by ID
    objects: BTreeMap<MemoryObjectId, MemoryObject>,

    /// Port to object mapping
    port_map: BTreeMap<PortName, MemoryObjectId>,

    /// Next object ID
    next_id: u64,

    /// Default memory manager port
    pub default_manager: Option<PortName>,

    /// Statistics
    pub stats: MemoryObjectStats,
}

impl MemoryObjectManager {
    pub fn new() -> Self {
        Self {
            objects: BTreeMap::new(),
            port_map: BTreeMap::new(),
            next_id: 1,
            default_manager: None,
            stats: MemoryObjectStats::new(),
        }
    }

    /// Create a new memory object
    pub fn create(&mut self, pager_port: Option<PortName>) -> MemoryObjectId {
        let id = MemoryObjectId(self.next_id);
        self.next_id += 1;

        let obj = if pager_port.is_some() {
            let obj = MemoryObject::new(id);
            if let Some(port) = pager_port {
                obj.set_pager_port(port);
                self.port_map.insert(port, id);
            }
            obj
        } else {
            // No pager - create temporary object
            MemoryObject::temporary(id)
        };

        self.objects.insert(id, obj);
        self.stats.created.fetch_add(1, Ordering::Relaxed);

        id
    }

    /// Create a temporary memory object
    pub fn create_temporary(&mut self) -> MemoryObjectId {
        self.create(None)
    }

    /// Find object by ID
    pub fn find(&self, id: MemoryObjectId) -> Option<&MemoryObject> {
        self.objects.get(&id)
    }

    /// Find object by ID (mutable)
    pub fn find_mut(&mut self, id: MemoryObjectId) -> Option<&mut MemoryObject> {
        self.objects.get_mut(&id)
    }

    /// Find object by pager port
    pub fn find_by_port(&self, port: PortName) -> Option<&MemoryObject> {
        self.port_map.get(&port).and_then(|id| self.objects.get(id))
    }

    /// Destroy a memory object
    pub fn destroy(&mut self, id: MemoryObjectId) -> bool {
        if let Some(obj) = self.objects.remove(&id) {
            // Remove port mapping
            if let Some(port) = obj.get_pager_port() {
                self.port_map.remove(&port);
            }
            self.stats.destroyed.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Set the default memory manager
    pub fn set_default_manager(&mut self, port: PortName) {
        self.default_manager = Some(port);
    }

    /// Get the default memory manager
    pub fn get_default_manager(&self) -> Option<PortName> {
        self.default_manager
    }

    /// Get object count
    pub fn count(&self) -> usize {
        self.objects.len()
    }

    /// Get all object IDs
    pub fn all_ids(&self) -> Vec<MemoryObjectId> {
        self.objects.keys().copied().collect()
    }
}

impl Default for MemoryObjectManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Memory Object Requests
// ============================================================================

/// Data request from kernel to pager
#[derive(Debug, Clone)]
pub struct DataRequest {
    /// Memory object control port
    pub control: MemoryObjectControl,
    /// Offset in object
    pub offset: u64,
    /// Number of bytes requested
    pub length: u64,
    /// Desired access (read/write)
    pub desired_access: u32,
}

impl DataRequest {
    pub fn new(
        control: MemoryObjectControl,
        offset: u64,
        length: u64,
        desired_access: u32,
    ) -> Self {
        Self {
            control,
            offset,
            length,
            desired_access,
        }
    }
}

/// Data supply from pager to kernel
#[derive(Debug)]
pub struct DataSupply {
    /// Memory object control port
    pub control: MemoryObjectControl,
    /// Offset in object
    pub offset: u64,
    /// Data (page contents)
    pub data: Vec<u8>,
    /// Lock value (none, read, write)
    pub lock: u32,
    /// Is data precious (don't discard)?
    pub precious: bool,
    /// Reply port for errors
    pub reply_port: Option<PortName>,
}

impl DataSupply {
    pub fn new(control: MemoryObjectControl, offset: u64, data: Vec<u8>) -> Self {
        Self {
            control,
            offset,
            data,
            lock: 0,
            precious: false,
            reply_port: None,
        }
    }
}

/// Data return from kernel to pager
#[derive(Debug)]
pub struct DataReturn {
    /// Memory object control port (of the pager)
    pub memory_object: PortName,
    /// Offset in object
    pub offset: u64,
    /// Data (page contents)
    pub data: Vec<u8>,
    /// Is data dirty?
    pub dirty: bool,
    /// Kernel finished with this region?
    pub kernel_copy: bool,
}

// ============================================================================
// Global State
// ============================================================================

static MEMORY_OBJECT_MANAGER: spin::Once<Mutex<MemoryObjectManager>> = spin::Once::new();

fn memory_object_manager() -> &'static Mutex<MemoryObjectManager> {
    MEMORY_OBJECT_MANAGER.call_once(|| Mutex::new(MemoryObjectManager::new()));
    MEMORY_OBJECT_MANAGER.get().unwrap()
}

/// Initialize memory object subsystem
pub fn init() {
    let _ = memory_object_manager();
}

// ============================================================================
// Public API
// ============================================================================

/// Create a memory object
pub fn memory_object_create(pager_port: Option<PortName>) -> MemoryObjectId {
    memory_object_manager().lock().create(pager_port)
}

/// Create a temporary memory object
pub fn memory_object_create_temporary() -> MemoryObjectId {
    memory_object_manager().lock().create_temporary()
}

/// Find memory object by ID
pub fn memory_object_find(id: MemoryObjectId) -> bool {
    memory_object_manager().lock().find(id).is_some()
}

/// Destroy memory object
pub fn memory_object_destroy(id: MemoryObjectId) -> bool {
    memory_object_manager().lock().destroy(id)
}

/// Set default memory manager
pub fn memory_manager_default_init(port: PortName) {
    memory_object_manager().lock().set_default_manager(port);
}

/// Get default memory manager port
pub fn memory_manager_default() -> Option<PortName> {
    memory_object_manager().lock().get_default_manager()
}

/// Record a data request (for statistics)
pub fn record_data_request() {
    memory_object_manager()
        .lock()
        .stats
        .data_requests
        .fetch_add(1, Ordering::Relaxed);
}

/// Record a data supply (for statistics)
pub fn record_data_supply() {
    memory_object_manager()
        .lock()
        .stats
        .data_supplies
        .fetch_add(1, Ordering::Relaxed);
}

/// Record a data return (for statistics)
pub fn record_data_return() {
    memory_object_manager()
        .lock()
        .stats
        .data_returns
        .fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_strategy() {
        assert!(CopyStrategy::Delay.allows_cow_optimization());
        assert!(CopyStrategy::Temporary.allows_cow_optimization());
        assert!(!CopyStrategy::None.allows_cow_optimization());
        assert!(!CopyStrategy::Call.allows_cow_optimization());

        assert!(CopyStrategy::Delay.needs_writeback());
        assert!(!CopyStrategy::Temporary.needs_writeback());
    }

    #[test]
    fn test_memory_object() {
        let obj = MemoryObject::new(MemoryObjectId(1));
        assert_eq!(obj.get_state(), PagerState::Uninitialized);
        assert!(!obj.is_ready());

        obj.set_ready();
        assert!(obj.is_ready());
        assert_eq!(obj.get_state(), PagerState::Ready);
    }

    #[test]
    fn test_memory_object_temporary() {
        let obj = MemoryObject::temporary(MemoryObjectId(2));
        assert!(obj.is_ready());
        assert_eq!(obj.get_copy_strategy(), CopyStrategy::Temporary);
    }

    #[test]
    fn test_memory_object_manager() {
        let mut mgr = MemoryObjectManager::new();

        let id1 = mgr.create(None);
        let id2 = mgr.create(Some(PortName(100)));

        assert!(mgr.find(id1).is_some());
        assert!(mgr.find(id2).is_some());

        // Temporary object should be ready
        assert!(mgr.find(id1).unwrap().is_ready());

        // Object with pager should not be ready yet
        assert!(!mgr.find(id2).unwrap().is_ready());

        // Find by port
        assert!(mgr.find_by_port(PortName(100)).is_some());

        // Destroy
        assert!(mgr.destroy(id1));
        assert!(mgr.find(id1).is_none());
    }
}
