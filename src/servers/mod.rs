//! System servers for Mach_R
//!
//! Implements the core system servers that provide essential services:
//! - Name Server: Port name resolution and registration
//! - File Server: File system operations via MIG
//! - VM Server: Virtual memory management
//! - Device Server: Hardware device management

pub mod file_server;
pub mod name_server;
pub mod pager_server;
pub mod vm_server;

use crate::types::PortId;
use alloc::collections::BTreeMap;
use spin::Mutex;

/// Server registry for system services
pub struct ServerRegistry {
    servers: Mutex<BTreeMap<&'static str, PortId>>,
}

impl ServerRegistry {
    pub const fn new() -> Self {
        Self {
            servers: Mutex::new(BTreeMap::new()),
        }
    }

    /// Register a system server
    pub fn register_server(&self, name: &'static str, port: PortId) {
        let mut servers = self.servers.lock();
        servers.insert(name, port);
    }

    /// Look up a system server
    pub fn lookup_server(&self, name: &str) -> Option<PortId> {
        let servers = self.servers.lock();
        servers.get(name).copied()
    }

    /// Unregister a system server
    pub fn unregister_server(&self, name: &str) -> Option<PortId> {
        let mut servers = self.servers.lock();
        servers.remove(name)
    }

    /// List all registered servers
    pub fn list_servers(&self) -> alloc::vec::Vec<(&'static str, PortId)> {
        let servers = self.servers.lock();
        servers.iter().map(|(&name, &port)| (name, port)).collect()
    }
}

/// Global server registry
pub static SERVER_REGISTRY: ServerRegistry = ServerRegistry::new();

/// Initialize all system servers
pub fn init_system_servers() {
    // Initialize Name Server first as other servers depend on it
    name_server::init();

    // Initialize File Server
    file_server::init();

    // Initialize VM Server
    vm_server::init();

    // Initialize Pager Server
    pager_server::init();

    crate::println!("System servers initialized successfully");
}

/// Poll each system server once for incoming requests and dispatch replies
pub fn poll_once() {
    // Name Server
    let _ =
        core::hint::black_box(unsafe { (*core::ptr::addr_of!(name_server::NAME_SERVER)).as_ref() })
            .map(|ns| ns.poll_once());
    // VM Server
    let _ = core::hint::black_box(unsafe { (*core::ptr::addr_of!(vm_server::VM_SERVER)).as_ref() })
        .map(|vm| vm.poll_once());
    // Pager Server
    let _ = core::hint::black_box(unsafe {
        (*core::ptr::addr_of!(pager_server::PAGER_SERVER)).as_ref()
    })
    .map(|pg| pg.poll_once());
}
