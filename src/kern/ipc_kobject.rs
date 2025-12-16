//! IPC Kernel Objects
//!
//! Based on Mach4 kern/ipc_kobject.h by Rich Draves (1989)
//!
//! This module allows IPC ports to represent kernel objects. When a message
//! is sent to such a port, the kernel handles it directly rather than
//! queuing it for a user-space receiver.
//!
//! Kernel objects include:
//! - Tasks and threads
//! - Hosts and processors
//! - Devices and pagers
//! - Semaphores and lock sets
//! - Clocks

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use crate::ipc::PortName;

// ============================================================================
// Kernel Object Types
// ============================================================================

/// Kernel object type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum KobjectType {
    /// No kernel object (user port)
    #[default]
    None = 0,
    /// Thread
    Thread = 1,
    /// Task
    Task = 2,
    /// Host (normal)
    Host = 3,
    /// Host (privileged)
    HostPriv = 4,
    /// Processor
    Processor = 5,
    /// Processor set
    ProcessorSet = 6,
    /// Processor set name
    ProcessorSetName = 7,
    /// Memory object (pager)
    Pager = 8,
    /// Paging request
    PagingRequest = 9,
    /// Device
    Device = 10,
    /// XMM object
    XmmObject = 11,
    /// XMM pager
    XmmPager = 12,
    /// XMM kernel
    XmmKernel = 13,
    /// XMM reply
    XmmReply = 14,
    /// Pager terminating
    PagerTerminating = 15,
    /// Paging name
    PagingName = 16,
    /// Host security
    HostSecurity = 17,
    /// Ledger
    Ledger = 18,
    /// Master device
    MasterDevice = 19,
    /// Activation
    Activation = 20,
    /// Subsystem
    Subsystem = 21,
    /// I/O done queue
    IoDoneQueue = 22,
    /// Semaphore
    Semaphore = 23,
    /// Lock set
    LockSet = 24,
    /// Clock
    Clock = 25,
    /// Clock control
    ClockControl = 26,
    /// Unknown (catchall)
    Unknown = 27,
}

/// Maximum kernel object type value
pub const IKOT_MAX_TYPE: u32 = 28;

impl KobjectType {
    /// Convert from u32
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(Self::None),
            1 => Some(Self::Thread),
            2 => Some(Self::Task),
            3 => Some(Self::Host),
            4 => Some(Self::HostPriv),
            5 => Some(Self::Processor),
            6 => Some(Self::ProcessorSet),
            7 => Some(Self::ProcessorSetName),
            8 => Some(Self::Pager),
            9 => Some(Self::PagingRequest),
            10 => Some(Self::Device),
            11 => Some(Self::XmmObject),
            12 => Some(Self::XmmPager),
            13 => Some(Self::XmmKernel),
            14 => Some(Self::XmmReply),
            15 => Some(Self::PagerTerminating),
            16 => Some(Self::PagingName),
            17 => Some(Self::HostSecurity),
            18 => Some(Self::Ledger),
            19 => Some(Self::MasterDevice),
            20 => Some(Self::Activation),
            21 => Some(Self::Subsystem),
            22 => Some(Self::IoDoneQueue),
            23 => Some(Self::Semaphore),
            24 => Some(Self::LockSet),
            25 => Some(Self::Clock),
            26 => Some(Self::ClockControl),
            27 => Some(Self::Unknown),
            _ => None,
        }
    }

    /// Get type name for debugging
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Thread => "thread",
            Self::Task => "task",
            Self::Host => "host",
            Self::HostPriv => "host_priv",
            Self::Processor => "processor",
            Self::ProcessorSet => "processor_set",
            Self::ProcessorSetName => "pset_name",
            Self::Pager => "pager",
            Self::PagingRequest => "paging_request",
            Self::Device => "device",
            Self::XmmObject => "xmm_object",
            Self::XmmPager => "xmm_pager",
            Self::XmmKernel => "xmm_kernel",
            Self::XmmReply => "xmm_reply",
            Self::PagerTerminating => "pager_terminating",
            Self::PagingName => "paging_name",
            Self::HostSecurity => "host_security",
            Self::Ledger => "ledger",
            Self::MasterDevice => "master_device",
            Self::Activation => "activation",
            Self::Subsystem => "subsystem",
            Self::IoDoneQueue => "io_done_queue",
            Self::Semaphore => "semaphore",
            Self::LockSet => "lock_set",
            Self::Clock => "clock",
            Self::ClockControl => "clock_ctrl",
            Self::Unknown => "unknown",
        }
    }

    /// Check if this is a valid kernel object type
    pub fn is_kobject(&self) -> bool {
        *self != Self::None
    }

    /// Check if this type uses page lists for copyin/copyout
    pub fn uses_page_list(&self) -> bool {
        matches!(self, Self::PagingRequest | Self::Device)
    }

    /// Check if this type can steal pages
    pub fn can_steal_pages(&self) -> bool {
        matches!(self, Self::PagingRequest)
    }
}

// ============================================================================
// Kernel Object Reference
// ============================================================================

/// Kernel object reference (opaque pointer)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Kobject(pub usize);

impl Kobject {
    pub const NULL: Self = Self(0);

    pub fn new(ptr: usize) -> Self {
        Self(ptr)
    }

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    pub fn as_ptr(&self) -> usize {
        self.0
    }
}

impl Default for Kobject {
    fn default() -> Self {
        Self::NULL
    }
}

// ============================================================================
// Port Kernel Object Association
// ============================================================================

/// Association between a port and a kernel object
#[derive(Debug, Clone, Copy, Default)]
pub struct PortKobject {
    /// The kernel object
    pub kobject: Kobject,
    /// Type of kernel object
    pub kotype: KobjectType,
}

impl PortKobject {
    pub fn new(kobject: Kobject, kotype: KobjectType) -> Self {
        Self { kobject, kotype }
    }

    pub fn none() -> Self {
        Self::default()
    }

    pub fn is_kobject(&self) -> bool {
        self.kotype.is_kobject()
    }
}

// ============================================================================
// Kernel Object Manager
// ============================================================================

/// Statistics for kobject operations
#[derive(Debug, Default)]
pub struct KobjectStats {
    /// Total kobject ports created
    pub created: AtomicU32,
    /// Total kobject ports destroyed
    pub destroyed: AtomicU32,
    /// Total server calls dispatched
    pub dispatched: AtomicU32,
    /// Counts by type
    pub by_type: [AtomicU32; IKOT_MAX_TYPE as usize],
}

impl KobjectStats {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Kernel object manager
pub struct KobjectManager {
    /// Port to kobject mapping
    ports: BTreeMap<PortName, PortKobject>,

    /// Statistics
    pub stats: KobjectStats,
}

impl KobjectManager {
    pub fn new() -> Self {
        Self {
            ports: BTreeMap::new(),
            stats: KobjectStats::new(),
        }
    }

    /// Set a port to represent a kernel object
    pub fn set(&mut self, port: PortName, kobject: Kobject, kotype: KobjectType) {
        // Remove any existing association
        if self.ports.contains_key(&port) {
            self.destroy(port);
        }

        // Add new association
        self.ports.insert(port, PortKobject::new(kobject, kotype));

        // Update statistics
        self.stats.created.fetch_add(1, Ordering::Relaxed);
        if (kotype as usize) < IKOT_MAX_TYPE as usize {
            self.stats.by_type[kotype as usize].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get the kernel object for a port
    pub fn get(&self, port: PortName) -> Option<&PortKobject> {
        self.ports.get(&port)
    }

    /// Get the kernel object type for a port
    pub fn get_type(&self, port: PortName) -> KobjectType {
        self.ports
            .get(&port)
            .map(|pk| pk.kotype)
            .unwrap_or(KobjectType::None)
    }

    /// Check if a port represents a kernel object
    pub fn is_kobject(&self, port: PortName) -> bool {
        self.ports
            .get(&port)
            .map(|pk| pk.is_kobject())
            .unwrap_or(false)
    }

    /// Destroy the kernel object association for a port
    pub fn destroy(&mut self, port: PortName) {
        if let Some(pk) = self.ports.remove(&port) {
            self.stats.destroyed.fetch_add(1, Ordering::Relaxed);

            // Update type count
            if (pk.kotype as usize) < IKOT_MAX_TYPE as usize {
                self.stats.by_type[pk.kotype as usize].fetch_sub(1, Ordering::Relaxed);
            }
        }
    }

    /// Find all ports of a given type
    pub fn find_by_type(&self, kotype: KobjectType) -> Vec<PortName> {
        self.ports
            .iter()
            .filter(|(_, pk)| pk.kotype == kotype)
            .map(|(&port, _)| port)
            .collect()
    }

    /// Get total number of kobject ports
    pub fn count(&self) -> usize {
        self.ports.len()
    }

    /// Get count by type
    pub fn count_by_type(&self, kotype: KobjectType) -> u32 {
        if (kotype as usize) < IKOT_MAX_TYPE as usize {
            self.stats.by_type[kotype as usize].load(Ordering::Relaxed)
        } else {
            0
        }
    }
}

impl Default for KobjectManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Server Dispatch
// ============================================================================

/// Result of kernel server dispatch
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerResult {
    /// Request handled successfully
    Success,
    /// Request requires reply
    NeedsReply,
    /// No handler for this request
    NoHandler,
    /// Invalid port
    InvalidPort,
    /// Invalid request
    InvalidRequest,
    /// Resource busy
    ResourceBusy,
}

/// Kernel server request
#[derive(Debug)]
pub struct ServerRequest {
    /// Target port
    pub port: PortName,
    /// Message ID
    pub msg_id: u32,
    /// Request data
    pub data: alloc::vec::Vec<u8>,
}

impl ServerRequest {
    pub fn new(port: PortName, msg_id: u32) -> Self {
        Self {
            port,
            msg_id,
            data: alloc::vec::Vec::new(),
        }
    }

    pub fn with_data(port: PortName, msg_id: u32, data: alloc::vec::Vec<u8>) -> Self {
        Self { port, msg_id, data }
    }
}

/// Kernel server reply
#[derive(Debug)]
pub struct ServerReply {
    /// Result code
    pub result: i32,
    /// Reply data
    pub data: alloc::vec::Vec<u8>,
}

impl ServerReply {
    pub fn success() -> Self {
        Self {
            result: 0,
            data: alloc::vec::Vec::new(),
        }
    }

    pub fn error(code: i32) -> Self {
        Self {
            result: code,
            data: alloc::vec::Vec::new(),
        }
    }

    pub fn with_data(result: i32, data: alloc::vec::Vec<u8>) -> Self {
        Self { result, data }
    }
}

// ============================================================================
// Global State
// ============================================================================

static KOBJECT_MANAGER: spin::Once<Mutex<KobjectManager>> = spin::Once::new();

fn kobject_manager() -> &'static Mutex<KobjectManager> {
    KOBJECT_MANAGER.call_once(|| Mutex::new(KobjectManager::new()));
    KOBJECT_MANAGER.get().unwrap()
}

/// Initialize the kobject subsystem
pub fn init() {
    let _ = kobject_manager();
}

// ============================================================================
// Public API
// ============================================================================

/// Set a port to represent a kernel object
pub fn ipc_kobject_set(port: PortName, kobject: Kobject, kotype: KobjectType) {
    kobject_manager().lock().set(port, kobject, kotype);
}

/// Get the kernel object for a port
pub fn ipc_kobject_get(port: PortName) -> Option<PortKobject> {
    kobject_manager().lock().get(port).copied()
}

/// Get the kernel object type for a port
pub fn ipc_kobject_type(port: PortName) -> KobjectType {
    kobject_manager().lock().get_type(port)
}

/// Check if a port represents a kernel object
pub fn is_ipc_kobject(port: PortName) -> bool {
    kobject_manager().lock().is_kobject(port)
}

/// Destroy the kernel object association for a port
pub fn ipc_kobject_destroy(port: PortName) {
    kobject_manager().lock().destroy(port);
}

/// Dispatch a request to the appropriate kernel server
pub fn ipc_kobject_server(request: ServerRequest) -> ServerResult {
    let mgr = kobject_manager().lock();

    // Get the kobject for this port
    let pk = match mgr.get(request.port) {
        Some(pk) if pk.is_kobject() => *pk,
        _ => return ServerResult::NoHandler,
    };

    drop(mgr);

    // Increment dispatch counter
    kobject_manager()
        .lock()
        .stats
        .dispatched
        .fetch_add(1, Ordering::Relaxed);

    // Dispatch based on type
    // In a full implementation, we would call the appropriate MIG-generated
    // server routines here based on the kobject type and message ID
    match pk.kotype {
        KobjectType::Task => {
            // task_server(request)
            ServerResult::Success
        }
        KobjectType::Thread => {
            // thread_server(request)
            ServerResult::Success
        }
        KobjectType::Host | KobjectType::HostPriv => {
            // host_server(request)
            ServerResult::Success
        }
        KobjectType::Processor => {
            // processor_server(request)
            ServerResult::Success
        }
        KobjectType::ProcessorSet | KobjectType::ProcessorSetName => {
            // processor_set_server(request)
            ServerResult::Success
        }
        KobjectType::Device | KobjectType::MasterDevice => {
            // device_server(request)
            ServerResult::Success
        }
        KobjectType::Pager | KobjectType::PagingRequest | KobjectType::PagingName => {
            // memory_object_server(request)
            ServerResult::Success
        }
        KobjectType::Semaphore => {
            // semaphore_server(request)
            ServerResult::Success
        }
        KobjectType::LockSet => {
            // lock_set_server(request)
            ServerResult::Success
        }
        KobjectType::Clock | KobjectType::ClockControl => {
            // clock_server(request)
            ServerResult::Success
        }
        _ => ServerResult::NoHandler,
    }
}

/// Get kobject statistics
pub fn ipc_kobject_stats() -> (u32, u32, u32) {
    let mgr = kobject_manager().lock();
    (
        mgr.stats.created.load(Ordering::Relaxed),
        mgr.stats.destroyed.load(Ordering::Relaxed),
        mgr.stats.dispatched.load(Ordering::Relaxed),
    )
}

// ============================================================================
// Conversion Helpers
// ============================================================================

/// Create a thread port
pub fn thread_to_port(thread_ptr: usize, port: PortName) {
    ipc_kobject_set(port, Kobject::new(thread_ptr), KobjectType::Thread);
}

/// Create a task port
pub fn task_to_port(task_ptr: usize, port: PortName) {
    ipc_kobject_set(port, Kobject::new(task_ptr), KobjectType::Task);
}

/// Create a host port
pub fn host_to_port(host_ptr: usize, port: PortName) {
    ipc_kobject_set(port, Kobject::new(host_ptr), KobjectType::Host);
}

/// Create a privileged host port
pub fn host_priv_to_port(host_ptr: usize, port: PortName) {
    ipc_kobject_set(port, Kobject::new(host_ptr), KobjectType::HostPriv);
}

/// Create a processor port
pub fn processor_to_port(processor_ptr: usize, port: PortName) {
    ipc_kobject_set(port, Kobject::new(processor_ptr), KobjectType::Processor);
}

/// Create a processor set port
pub fn pset_to_port(pset_ptr: usize, port: PortName) {
    ipc_kobject_set(port, Kobject::new(pset_ptr), KobjectType::ProcessorSet);
}

/// Create a device port
pub fn device_to_port(device_ptr: usize, port: PortName) {
    ipc_kobject_set(port, Kobject::new(device_ptr), KobjectType::Device);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kobject_type() {
        assert_eq!(KobjectType::Task.name(), "task");
        assert!(KobjectType::Task.is_kobject());
        assert!(!KobjectType::None.is_kobject());

        assert!(KobjectType::PagingRequest.uses_page_list());
        assert!(KobjectType::Device.uses_page_list());
        assert!(!KobjectType::Task.uses_page_list());

        assert!(KobjectType::PagingRequest.can_steal_pages());
        assert!(!KobjectType::Device.can_steal_pages());
    }

    #[test]
    fn test_kobject() {
        let ko = Kobject::new(0x1000);
        assert!(!ko.is_null());
        assert_eq!(ko.as_ptr(), 0x1000);

        let null = Kobject::NULL;
        assert!(null.is_null());
    }

    #[test]
    fn test_port_kobject() {
        let pk = PortKobject::new(Kobject::new(0x2000), KobjectType::Task);
        assert!(pk.is_kobject());
        assert_eq!(pk.kotype, KobjectType::Task);

        let none = PortKobject::none();
        assert!(!none.is_kobject());
    }

    #[test]
    fn test_kobject_manager() {
        let mut mgr = KobjectManager::new();

        let port = PortName(100);
        let ko = Kobject::new(0x3000);

        mgr.set(port, ko, KobjectType::Thread);

        assert!(mgr.is_kobject(port));
        assert_eq!(mgr.get_type(port), KobjectType::Thread);

        let found = mgr.find_by_type(KobjectType::Thread);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], port);

        mgr.destroy(port);
        assert!(!mgr.is_kobject(port));
    }
}
