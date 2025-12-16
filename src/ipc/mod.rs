//! Mach-style Inter-Process Communication
//!
//! This is the core of the microkernel - all communication happens through ports.
//! Based on Mach4 IPC subsystem from CMU/Utah.
//!
//! ## Architecture
//!
//! The IPC subsystem consists of:
//! - **entry**: Port name to capability translation (ipc_entry)
//! - **space**: Per-task IPC namespace (ipc_space)
//! - **right**: Capability transfer operations (ipc_right)
//! - **kmsg**: Kernel message representation (ipc_kmsg)
//! - **mqueue**: Message queue management (ipc_mqueue)
//! - **notify**: Port death notifications (ipc_notify)
//! - **pset**: Port sets for multiplexed receive (ipc_pset)

use core::sync::atomic::{AtomicU32, Ordering};

// Core IPC modules
pub mod entry;
pub mod ipc_hash;
pub mod ipc_object;
pub mod ipc_table;
pub mod kmsg;
pub mod mach_msg;
pub mod mqueue;
pub mod notify;
pub mod pset;
pub mod right;
pub mod space;

// Original modules (being integrated)
pub mod message;
pub mod port;
pub mod port_ops;
pub mod rights;

/// Global port name counter
static NEXT_PORT_NAME: AtomicU32 = AtomicU32::new(1000);

/// Port name type (like Mach's mach_port_t)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PortName(pub u32);

impl PortName {
    pub const NULL: Self = Self(0);

    pub fn new() -> Self {
        let name = NEXT_PORT_NAME.fetch_add(1, Ordering::SeqCst);
        Self(name)
    }

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Get the underlying ID of the port name.
    pub fn id(&self) -> u32 {
        self.0
    }
}

/// IPC error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    InvalidPort,
    PortDead,
    NoSpace,
    InvalidRight,
    WouldBlock,
    MessageTooLarge,
    NoMemory,
    InvalidThread,
}

/// Result type for IPC operations
pub type IpcResult<T> = Result<T, IpcError>;

/// Initialize the IPC subsystem
pub fn init() {
    ipc_table::init();
    port::init();
    space::init_kernel_space();
    notify::init();
    ipc_hash::init();
    pset::init_pset_registry();
}

// Re-export commonly used types
pub use entry::{IpcEntry, IpcEntryTable, IpcObject, MachPortName};
pub use kmsg::{
    IpcKmsg, IpcKmsgQueue, KmsgHeader, KmsgOolRegion, KmsgPortRight, MachMsgBodyDescriptor,
    MachMsgHeader, MachMsgOolDescriptor, MachMsgOolPortsDescriptor, MachMsgPortDescriptor,
    OolCopyOption, OolDescriptor, PortDescriptor, MACH_MSGH_BITS_COMPLEX, MACH_MSG_OOL_DESCRIPTOR,
    MACH_MSG_OOL_PORTS_DESCRIPTOR, MACH_MSG_OOL_VOLATILE_DESCRIPTOR, MACH_MSG_PORT_DESCRIPTOR,
};
pub use mqueue::{IpcMqueue, SyncMqueue};
pub use port_ops::{
    mach_port_allocate, mach_port_deallocate, mach_port_destroy, mach_port_extract_right,
    mach_port_get_refs, mach_port_insert_right, mach_port_mod_refs, mach_port_move_member,
    mach_port_type, MachPortRight,
};
pub use pset::{
    add_port_to_set, create_port_set, destroy_port_set, lookup_port_set, move_port_between_sets,
    remove_port_from_set, IpcPortSet, PortSetId, SyncPortSet,
};
pub use right::{CopyinResult, MsgTypeName, PortRight};
pub use space::{IpcSpace, SpaceId};
