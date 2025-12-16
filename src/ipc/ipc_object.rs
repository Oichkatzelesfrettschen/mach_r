//! IPC Object Abstraction
//!
//! Based on Mach4 ipc/ipc_object.h/c by CMU (1991)
//!
//! This module provides the generic IPC object abstraction that underlies
//! all IPC primitives in Mach. Both ports and port sets are represented
//! as IPC objects, allowing polymorphic handling in the IPC subsystem.
//!
//! ## Object Types
//!
//! - **Port**: Communication endpoint for message passing
//! - **Port Set**: Collection of ports for receiving from multiple sources
//!
//! ## Design Notes
//!
//! IPC objects are reference-counted and have an intrinsic lock for
//! thread-safe operations. The object header contains common fields,
//! while port and port-set specific data extends this.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

// ============================================================================
// Object Type
// ============================================================================

/// Type of IPC object
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum IpcObjectType {
    /// No object / invalid
    #[default]
    None = 0,
    /// Send right to a port
    PortSend = 1,
    /// Receive right to a port
    PortReceive = 2,
    /// Send-once right to a port
    PortSendOnce = 3,
    /// Port set
    PortSet = 4,
    /// Dead name (port was destroyed)
    DeadName = 5,
}

impl IpcObjectType {
    /// Check if this is a port type
    pub fn is_port(&self) -> bool {
        matches!(
            self,
            Self::PortSend | Self::PortReceive | Self::PortSendOnce
        )
    }

    /// Check if this is a port set
    pub fn is_port_set(&self) -> bool {
        matches!(self, Self::PortSet)
    }

    /// Check if this is a dead name
    pub fn is_dead(&self) -> bool {
        matches!(self, Self::DeadName)
    }

    /// Get type name for debugging
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PortSend => "send",
            Self::PortReceive => "receive",
            Self::PortSendOnce => "send-once",
            Self::PortSet => "port-set",
            Self::DeadName => "dead-name",
        }
    }
}

// ============================================================================
// Object ID
// ============================================================================

/// Unique identifier for IPC objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct IpcObjectId(u64);

impl IpcObjectId {
    /// Invalid object ID
    pub const INVALID: Self = Self(0);

    /// Create a new object ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw ID value
    pub fn value(self) -> u64 {
        self.0
    }

    /// Check if this is a valid ID
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }

    /// Generate a new unique object ID
    pub fn generate() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for IpcObjectId {
    fn default() -> Self {
        Self::INVALID
    }
}

impl From<u64> for IpcObjectId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

// ============================================================================
// Object Bits / State
// ============================================================================

/// IPC object state bits
#[derive(Debug, Clone, Copy, Default)]
pub struct IpcObjectBits {
    /// Object type
    pub obj_type: IpcObjectType,
    /// Reference count
    pub references: u32,
    /// Object is active (not being destroyed)
    pub active: bool,
    /// Object has waiters blocked on it
    pub has_waiters: bool,
}

impl IpcObjectBits {
    pub fn new(obj_type: IpcObjectType) -> Self {
        Self {
            obj_type,
            references: 1,
            active: true,
            has_waiters: false,
        }
    }
}

// ============================================================================
// IPC Object Header
// ============================================================================

/// Common header for all IPC objects
///
/// This structure contains the fields shared by ports and port sets.
#[derive(Debug)]
pub struct IpcObjectHeader {
    /// Unique object identifier
    pub id: IpcObjectId,
    /// Object type and state bits
    pub bits: Mutex<IpcObjectBits>,
    /// Reference count (atomic for fast path)
    refs: AtomicU32,
}

impl IpcObjectHeader {
    /// Create a new object header
    pub fn new(obj_type: IpcObjectType) -> Self {
        Self {
            id: IpcObjectId::generate(),
            bits: Mutex::new(IpcObjectBits::new(obj_type)),
            refs: AtomicU32::new(1),
        }
    }

    /// Get the object type
    pub fn object_type(&self) -> IpcObjectType {
        self.bits.lock().obj_type
    }

    /// Check if object is active
    pub fn is_active(&self) -> bool {
        self.bits.lock().active
    }

    /// Increment reference count
    pub fn reference(&self) {
        self.refs.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count, returns true if should be freed
    pub fn dereference(&self) -> bool {
        let old = self.refs.fetch_sub(1, Ordering::Release);
        if old == 1 {
            core::sync::atomic::fence(Ordering::Acquire);
            true
        } else {
            false
        }
    }

    /// Get current reference count
    pub fn ref_count(&self) -> u32 {
        self.refs.load(Ordering::Relaxed)
    }

    /// Mark object as inactive (being destroyed)
    pub fn deactivate(&self) {
        self.bits.lock().active = false;
    }
}

// ============================================================================
// IPC Object Trait
// ============================================================================

/// Trait for all IPC objects
pub trait IpcObject: Send + Sync {
    /// Get the object header
    fn header(&self) -> &IpcObjectHeader;

    /// Get object type
    fn object_type(&self) -> IpcObjectType {
        self.header().object_type()
    }

    /// Get object ID
    fn id(&self) -> IpcObjectId {
        self.header().id
    }

    /// Increment reference count
    fn reference(&self) {
        self.header().reference();
    }

    /// Decrement reference count
    fn dereference(&self) -> bool {
        self.header().dereference()
    }

    /// Check if object is active
    fn is_active(&self) -> bool {
        self.header().is_active()
    }

    /// Destroy the object (implementation-specific)
    fn destroy(&self);
}

// ============================================================================
// Port Object
// ============================================================================

/// IPC Port - communication endpoint
#[derive(Debug)]
pub struct IpcPort {
    /// Common header
    header: IpcObjectHeader,
    /// Port-specific data
    data: Mutex<IpcPortData>,
}

/// Port-specific data
#[derive(Debug, Default)]
pub struct IpcPortData {
    /// Number of send rights
    pub send_count: u32,
    /// Number of send-once rights
    pub sorights: u32,
    /// Make-send count (for notifications)
    pub mscount: u32,
    /// Sequence number for messages
    pub seqno: u32,
    /// Message queue depth limit
    pub qlimit: u32,
    /// Current message count
    pub msgcount: u32,
    /// Port set this port belongs to (if any)
    pub pset_id: Option<IpcObjectId>,
    /// Special port flags
    pub flags: PortFlags,
}

/// Port flags
#[derive(Debug, Clone, Copy, Default)]
pub struct PortFlags {
    /// Port is no-senders notification requested
    pub nsrequest: bool,
    /// Port is dead-name notification requested
    pub dnrequest: bool,
    /// Port is a kernel port
    pub kernel: bool,
    /// Port is immovable
    pub immovable: bool,
    /// Port is guarded
    pub guarded: bool,
}

impl IpcPort {
    /// Create a new port
    pub fn new() -> Self {
        Self {
            header: IpcObjectHeader::new(IpcObjectType::PortReceive),
            data: Mutex::new(IpcPortData {
                qlimit: 5, // Default queue limit
                ..Default::default()
            }),
        }
    }

    /// Create a kernel port
    pub fn kernel() -> Self {
        let port = Self::new();
        port.data.lock().flags.kernel = true;
        port
    }

    /// Get port data
    pub fn data(&self) -> spin::MutexGuard<'_, IpcPortData> {
        self.data.lock()
    }

    /// Add a send right
    pub fn add_send(&self) {
        let mut data = self.data.lock();
        data.send_count += 1;
        data.mscount += 1;
    }

    /// Remove a send right
    pub fn remove_send(&self) -> bool {
        let mut data = self.data.lock();
        if data.send_count > 0 {
            data.send_count -= 1;
            true
        } else {
            false
        }
    }

    /// Add a send-once right
    pub fn add_send_once(&self) {
        self.data.lock().sorights += 1;
    }

    /// Remove a send-once right
    pub fn remove_send_once(&self) -> bool {
        let mut data = self.data.lock();
        if data.sorights > 0 {
            data.sorights -= 1;
            true
        } else {
            false
        }
    }

    /// Check if port has no senders
    pub fn has_no_senders(&self) -> bool {
        let data = self.data.lock();
        data.send_count == 0 && data.sorights == 0
    }

    /// Get next message sequence number
    pub fn next_seqno(&self) -> u32 {
        let mut data = self.data.lock();
        let seqno = data.seqno;
        data.seqno = seqno.wrapping_add(1);
        seqno
    }

    /// Check if message queue is full
    pub fn is_full(&self) -> bool {
        let data = self.data.lock();
        data.msgcount >= data.qlimit
    }

    /// Set queue limit
    pub fn set_qlimit(&self, limit: u32) {
        self.data.lock().qlimit = limit.min(1024); // Max queue limit
    }
}

impl Default for IpcPort {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcObject for IpcPort {
    fn header(&self) -> &IpcObjectHeader {
        &self.header
    }

    fn destroy(&self) {
        self.header.deactivate();
        // Cleanup port-specific resources
        // In real implementation: notify dead-name holders, etc.
    }
}

// ============================================================================
// Port Set Object
// ============================================================================

/// IPC Port Set - collection of ports
#[derive(Debug)]
pub struct IpcPortSet {
    /// Common header
    header: IpcObjectHeader,
    /// Set-specific data
    data: Mutex<IpcPortSetData>,
}

/// Port set-specific data
#[derive(Debug, Default)]
pub struct IpcPortSetData {
    /// Member ports (by ID)
    pub members: Vec<IpcObjectId>,
    /// Whether set is active
    pub active: bool,
}

impl IpcPortSet {
    /// Create a new port set
    pub fn new() -> Self {
        Self {
            header: IpcObjectHeader::new(IpcObjectType::PortSet),
            data: Mutex::new(IpcPortSetData {
                active: true,
                ..Default::default()
            }),
        }
    }

    /// Add a port to the set
    pub fn add_member(&self, port_id: IpcObjectId) -> bool {
        let mut data = self.data.lock();
        if !data.members.contains(&port_id) {
            data.members.push(port_id);
            true
        } else {
            false
        }
    }

    /// Remove a port from the set
    pub fn remove_member(&self, port_id: IpcObjectId) -> bool {
        let mut data = self.data.lock();
        if let Some(pos) = data.members.iter().position(|&id| id == port_id) {
            data.members.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if port is a member
    pub fn contains(&self, port_id: IpcObjectId) -> bool {
        self.data.lock().members.contains(&port_id)
    }

    /// Get number of members
    pub fn member_count(&self) -> usize {
        self.data.lock().members.len()
    }

    /// Get all members
    pub fn members(&self) -> Vec<IpcObjectId> {
        self.data.lock().members.clone()
    }
}

impl Default for IpcPortSet {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcObject for IpcPortSet {
    fn header(&self) -> &IpcObjectHeader {
        &self.header
    }

    fn destroy(&self) {
        self.header.deactivate();
        // Cleanup set-specific resources
        self.data.lock().members.clear();
    }
}

// ============================================================================
// Object References
// ============================================================================

/// A reference-counted handle to an IPC object
#[derive(Debug, Clone)]
pub enum IpcObjectRef {
    /// Port reference
    Port(Arc<IpcPort>),
    /// Port set reference
    PortSet(Arc<IpcPortSet>),
}

impl IpcObjectRef {
    /// Get the object ID
    pub fn id(&self) -> IpcObjectId {
        match self {
            Self::Port(p) => p.id(),
            Self::PortSet(ps) => ps.id(),
        }
    }

    /// Get the object type
    pub fn object_type(&self) -> IpcObjectType {
        match self {
            Self::Port(p) => p.object_type(),
            Self::PortSet(ps) => ps.object_type(),
        }
    }

    /// Check if this is a port
    pub fn is_port(&self) -> bool {
        matches!(self, Self::Port(_))
    }

    /// Check if this is a port set
    pub fn is_port_set(&self) -> bool {
        matches!(self, Self::PortSet(_))
    }

    /// Get as port reference
    pub fn as_port(&self) -> Option<&Arc<IpcPort>> {
        match self {
            Self::Port(p) => Some(p),
            _ => None,
        }
    }

    /// Get as port set reference
    pub fn as_port_set(&self) -> Option<&Arc<IpcPortSet>> {
        match self {
            Self::PortSet(ps) => Some(ps),
            _ => None,
        }
    }
}

// ============================================================================
// Object Table
// ============================================================================

/// Global IPC object table
#[derive(Debug)]
pub struct IpcObjectTable {
    /// Objects indexed by ID
    objects: Mutex<BTreeMap<IpcObjectId, IpcObjectRef>>,
    /// Statistics
    stats: Mutex<IpcObjectStats>,
}

/// Object table statistics
#[derive(Debug, Clone, Default)]
pub struct IpcObjectStats {
    pub ports_created: u64,
    pub ports_destroyed: u64,
    pub port_sets_created: u64,
    pub port_sets_destroyed: u64,
    pub lookups: u64,
    pub lookup_failures: u64,
}

impl IpcObjectTable {
    /// Create a new object table
    pub const fn new() -> Self {
        Self {
            objects: Mutex::new(BTreeMap::new()),
            stats: Mutex::new(IpcObjectStats {
                ports_created: 0,
                ports_destroyed: 0,
                port_sets_created: 0,
                port_sets_destroyed: 0,
                lookups: 0,
                lookup_failures: 0,
            }),
        }
    }

    /// Create a new port
    pub fn create_port(&self) -> Arc<IpcPort> {
        let port = Arc::new(IpcPort::new());
        let id = port.id();

        self.objects
            .lock()
            .insert(id, IpcObjectRef::Port(port.clone()));
        self.stats.lock().ports_created += 1;

        port
    }

    /// Create a new port set
    pub fn create_port_set(&self) -> Arc<IpcPortSet> {
        let pset = Arc::new(IpcPortSet::new());
        let id = pset.id();

        self.objects
            .lock()
            .insert(id, IpcObjectRef::PortSet(pset.clone()));
        self.stats.lock().port_sets_created += 1;

        pset
    }

    /// Look up an object by ID
    pub fn lookup(&self, id: IpcObjectId) -> Option<IpcObjectRef> {
        self.stats.lock().lookups += 1;

        match self.objects.lock().get(&id) {
            Some(obj) => Some(obj.clone()),
            None => {
                self.stats.lock().lookup_failures += 1;
                None
            }
        }
    }

    /// Look up a port by ID
    pub fn lookup_port(&self, id: IpcObjectId) -> Option<Arc<IpcPort>> {
        self.lookup(id).and_then(|obj| match obj {
            IpcObjectRef::Port(p) => Some(p),
            _ => None,
        })
    }

    /// Look up a port set by ID
    pub fn lookup_port_set(&self, id: IpcObjectId) -> Option<Arc<IpcPortSet>> {
        self.lookup(id).and_then(|obj| match obj {
            IpcObjectRef::PortSet(ps) => Some(ps),
            _ => None,
        })
    }

    /// Remove an object from the table
    pub fn remove(&self, id: IpcObjectId) -> Option<IpcObjectRef> {
        let obj = self.objects.lock().remove(&id);

        if let Some(ref o) = obj {
            let mut stats = self.stats.lock();
            match o {
                IpcObjectRef::Port(_) => stats.ports_destroyed += 1,
                IpcObjectRef::PortSet(_) => stats.port_sets_destroyed += 1,
            }
        }

        obj
    }

    /// Get the number of objects
    pub fn count(&self) -> usize {
        self.objects.lock().len()
    }

    /// Get statistics
    pub fn stats(&self) -> IpcObjectStats {
        self.stats.lock().clone()
    }
}

impl Default for IpcObjectTable {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global State
// ============================================================================

static OBJECT_TABLE: spin::Once<IpcObjectTable> = spin::Once::new();

fn object_table() -> &'static IpcObjectTable {
    OBJECT_TABLE.call_once(IpcObjectTable::new)
}

/// Create a new port in the global table
pub fn ipc_port_alloc() -> Arc<IpcPort> {
    object_table().create_port()
}

/// Create a new port set in the global table
pub fn ipc_pset_alloc() -> Arc<IpcPortSet> {
    object_table().create_port_set()
}

/// Look up an object by ID
pub fn ipc_object_lookup(id: IpcObjectId) -> Option<IpcObjectRef> {
    object_table().lookup(id)
}

/// Look up a port by ID
pub fn ipc_port_lookup(id: IpcObjectId) -> Option<Arc<IpcPort>> {
    object_table().lookup_port(id)
}

/// Look up a port set by ID
pub fn ipc_pset_lookup(id: IpcObjectId) -> Option<Arc<IpcPortSet>> {
    object_table().lookup_port_set(id)
}

/// Destroy an object by ID
pub fn ipc_object_destroy(id: IpcObjectId) -> bool {
    if let Some(obj) = object_table().remove(id) {
        match obj {
            IpcObjectRef::Port(p) => p.destroy(),
            IpcObjectRef::PortSet(ps) => ps.destroy(),
        }
        true
    } else {
        false
    }
}

/// Get object statistics
pub fn ipc_object_stats() -> IpcObjectStats {
    object_table().stats()
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the IPC object subsystem
pub fn init() {
    let _ = object_table();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_id() {
        let id1 = IpcObjectId::generate();
        let id2 = IpcObjectId::generate();

        assert!(id1.is_valid());
        assert!(id2.is_valid());
        assert_ne!(id1, id2);

        assert!(!IpcObjectId::INVALID.is_valid());
    }

    #[test]
    fn test_object_type() {
        assert!(IpcObjectType::PortSend.is_port());
        assert!(IpcObjectType::PortReceive.is_port());
        assert!(IpcObjectType::PortSendOnce.is_port());
        assert!(!IpcObjectType::PortSet.is_port());

        assert!(IpcObjectType::PortSet.is_port_set());
        assert!(IpcObjectType::DeadName.is_dead());
    }

    #[test]
    fn test_port_creation() {
        let port = IpcPort::new();

        assert!(port.is_active());
        assert_eq!(port.object_type(), IpcObjectType::PortReceive);
        assert!(port.id().is_valid());
    }

    #[test]
    fn test_port_send_rights() {
        let port = IpcPort::new();

        assert!(port.has_no_senders());

        port.add_send();
        assert!(!port.has_no_senders());
        assert_eq!(port.data().send_count, 1);

        port.remove_send();
        assert!(port.has_no_senders());
    }

    #[test]
    fn test_port_queue() {
        let port = IpcPort::new();

        port.set_qlimit(10);
        assert_eq!(port.data().qlimit, 10);

        assert!(!port.is_full());
    }

    #[test]
    fn test_port_set() {
        let pset = IpcPortSet::new();
        let port_id = IpcObjectId::generate();

        assert!(pset.add_member(port_id));
        assert!(!pset.add_member(port_id)); // Already added

        assert!(pset.contains(port_id));
        assert_eq!(pset.member_count(), 1);

        assert!(pset.remove_member(port_id));
        assert!(!pset.contains(port_id));
    }

    #[test]
    fn test_object_table() {
        let table = IpcObjectTable::new();

        let port = table.create_port();
        let port_id = port.id();

        let lookup = table.lookup_port(port_id);
        assert!(lookup.is_some());

        let pset = table.create_port_set();
        let pset_id = pset.id();

        assert!(table.lookup_port_set(pset_id).is_some());
        assert!(table.lookup_port(pset_id).is_none()); // Wrong type

        let stats = table.stats();
        assert_eq!(stats.ports_created, 1);
        assert_eq!(stats.port_sets_created, 1);
    }

    #[test]
    fn test_reference_counting() {
        let header = IpcObjectHeader::new(IpcObjectType::PortReceive);

        assert_eq!(header.ref_count(), 1);

        header.reference();
        assert_eq!(header.ref_count(), 2);

        assert!(!header.dereference());
        assert_eq!(header.ref_count(), 1);

        assert!(header.dereference());
    }
}
