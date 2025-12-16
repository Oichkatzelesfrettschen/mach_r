//! Port Operations - User-callable port manipulation
//!
//! Based on Mach4 ipc/mach_port.c
//!
//! This module provides the user-level operations for port manipulation:
//! - mach_port_allocate: Create new ports and port sets
//! - mach_port_deallocate: Release a port right
//! - mach_port_destroy: Destroy a port right completely
//! - mach_port_insert_right: Insert a right into a task's space
//! - mach_port_extract_right: Extract a right from a task's space
//! - mach_port_get_refs: Query reference count
//! - mach_port_mod_refs: Modify reference count

use alloc::sync::Arc;
use spin::Mutex;

use super::entry::{IpcObject, MachPortName};
use super::port::Port;
use super::right::{MsgTypeName, PortRight};
use super::space::{current_space, IpcSpace};
use super::IpcError;
use crate::kern::syscall_sw::{KernReturn, KERN_INVALID_ARGUMENT, KERN_SUCCESS};

// ============================================================================
// Port Right Types (for allocate)
// ============================================================================

/// Port rights that can be allocated
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MachPortRight {
    /// Allocate send right
    Send = 0,
    /// Allocate receive right (creates new port)
    Receive = 1,
    /// Allocate send-once right (requires existing port)
    SendOnce = 2,
    /// Allocate port set
    PortSet = 3,
    /// Allocate dead name (placeholder)
    DeadName = 4,
}

impl MachPortRight {
    /// Convert from u32
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(MachPortRight::Send),
            1 => Some(MachPortRight::Receive),
            2 => Some(MachPortRight::SendOnce),
            3 => Some(MachPortRight::PortSet),
            4 => Some(MachPortRight::DeadName),
            _ => None,
        }
    }

    /// Convert to PortRight
    pub fn to_port_right(&self) -> PortRight {
        match self {
            MachPortRight::Send => PortRight::Send,
            MachPortRight::Receive => PortRight::Receive,
            MachPortRight::SendOnce => PortRight::SendOnce,
            MachPortRight::PortSet => PortRight::PortSet,
            MachPortRight::DeadName => PortRight::DeadName,
        }
    }
}

// ============================================================================
// Return Codes
// ============================================================================

/// Port operation return codes
pub const KERN_INVALID_NAME: KernReturn = 15;
pub const KERN_INVALID_RIGHT: KernReturn = 16;
pub const KERN_INVALID_VALUE: KernReturn = 18;
pub const KERN_UREFS_OVERFLOW: KernReturn = 19;
pub const KERN_INVALID_CAPABILITY: KernReturn = 20;

// ============================================================================
// mach_port_allocate - Allocate a new port right
// ============================================================================

/// Allocate a new port right in the task's space
///
/// # Arguments
/// * `space` - IPC space to allocate in
/// * `right_type` - Type of right to allocate
/// * `name` - Output: the allocated port name
///
/// # Returns
/// KERN_SUCCESS on success
pub fn mach_port_allocate(
    space: &IpcSpace,
    right_type: MachPortRight,
    name: &mut MachPortName,
) -> KernReturn {
    match right_type {
        MachPortRight::Receive => {
            // Create a new port and allocate receive right
            let port = Arc::new(Mutex::new(Port::new()));
            match space.alloc_receive_right(port) {
                Ok(n) => {
                    *name = n;
                    KERN_SUCCESS
                }
                Err(_) => KERN_INVALID_ARGUMENT,
            }
        }
        MachPortRight::PortSet => {
            // Create a new port set
            match space.alloc_port_set() {
                Ok(n) => {
                    *name = n;
                    KERN_SUCCESS
                }
                Err(_) => KERN_INVALID_ARGUMENT,
            }
        }
        MachPortRight::DeadName => {
            // Allocate a dead name placeholder
            match space.entry_alloc() {
                Ok(n) => {
                    *name = n;
                    KERN_SUCCESS
                }
                Err(_) => KERN_INVALID_ARGUMENT,
            }
        }
        _ => {
            // Can't allocate send or send-once without a port
            KERN_INVALID_VALUE
        }
    }
}

/// Allocate a port with a specific name
pub fn mach_port_allocate_name(
    space: &IpcSpace,
    right_type: MachPortRight,
    name: MachPortName,
) -> KernReturn {
    // Check if name is already in use
    if space.lookup(name).is_ok() {
        return KERN_INVALID_ARGUMENT;
    }

    match right_type {
        MachPortRight::Receive => {
            let port = Arc::new(Mutex::new(Port::new()));
            match space.alloc_receive_right_with_name(port, name) {
                Ok(()) => KERN_SUCCESS,
                Err(_) => KERN_INVALID_ARGUMENT,
            }
        }
        MachPortRight::PortSet => match space.alloc_port_set_with_name(name) {
            Ok(()) => KERN_SUCCESS,
            Err(_) => KERN_INVALID_ARGUMENT,
        },
        _ => KERN_INVALID_VALUE,
    }
}

// ============================================================================
// mach_port_deallocate - Release a port right
// ============================================================================

/// Deallocate a port right (decrement user reference count)
///
/// This decrements the user reference count. When it reaches zero,
/// the entry is removed from the space.
///
/// # Arguments
/// * `space` - IPC space containing the right
/// * `name` - Port name to deallocate
///
/// # Returns
/// KERN_SUCCESS on success
pub fn mach_port_deallocate(space: &IpcSpace, name: MachPortName) -> KernReturn {
    if name == 0 {
        return KERN_SUCCESS; // Deallocating NULL is OK
    }

    match super::right::dealloc(space, name) {
        Ok(()) => KERN_SUCCESS,
        Err(IpcError::InvalidPort) => KERN_INVALID_NAME,
        Err(IpcError::InvalidRight) => KERN_INVALID_RIGHT,
        Err(_) => KERN_INVALID_ARGUMENT,
    }
}

// ============================================================================
// mach_port_destroy - Completely destroy a port right
// ============================================================================

/// Destroy a port right completely
///
/// For receive rights, this destroys the port itself.
/// For send rights, this removes all user references.
///
/// # Arguments
/// * `space` - IPC space containing the right
/// * `name` - Port name to destroy
///
/// # Returns
/// KERN_SUCCESS on success
pub fn mach_port_destroy(space: &IpcSpace, name: MachPortName) -> KernReturn {
    if name == 0 {
        return KERN_INVALID_NAME;
    }

    match super::right::destroy(space, name) {
        Ok(()) => KERN_SUCCESS,
        Err(IpcError::InvalidPort) => KERN_INVALID_NAME,
        Err(IpcError::InvalidRight) => KERN_INVALID_RIGHT,
        Err(_) => KERN_INVALID_ARGUMENT,
    }
}

// ============================================================================
// mach_port_mod_refs - Modify reference count
// ============================================================================

/// Modify the user reference count for a port right
///
/// # Arguments
/// * `space` - IPC space containing the right
/// * `name` - Port name
/// * `right` - Type of right to modify
/// * `delta` - Change in reference count (can be negative)
///
/// # Returns
/// KERN_SUCCESS on success
pub fn mach_port_mod_refs(
    space: &IpcSpace,
    name: MachPortName,
    right: MachPortRight,
    delta: i32,
) -> KernReturn {
    if name == 0 {
        return KERN_INVALID_NAME;
    }

    if delta == 0 {
        return KERN_SUCCESS;
    }

    match super::right::delta(space, name, right.to_port_right(), delta) {
        Ok(()) => KERN_SUCCESS,
        Err(IpcError::InvalidPort) => KERN_INVALID_NAME,
        Err(IpcError::InvalidRight) => KERN_INVALID_RIGHT,
        Err(_) => KERN_INVALID_ARGUMENT,
    }
}

// ============================================================================
// mach_port_get_refs - Query reference count
// ============================================================================

/// Get the user reference count for a port right
///
/// # Arguments
/// * `space` - IPC space containing the right
/// * `name` - Port name
/// * `right` - Type of right to query
/// * `refs` - Output: reference count
///
/// # Returns
/// KERN_SUCCESS on success
pub fn mach_port_get_refs(
    space: &IpcSpace,
    name: MachPortName,
    right: MachPortRight,
    refs: &mut u32,
) -> KernReturn {
    if name == 0 {
        return KERN_INVALID_NAME;
    }

    let entry = match space.lookup(name) {
        Ok(e) => e,
        Err(_) => return KERN_INVALID_NAME,
    };

    // Check that the entry has the right type
    let has_right = match right {
        MachPortRight::Send => entry.has_send(),
        MachPortRight::Receive => entry.has_receive(),
        MachPortRight::SendOnce => entry.has_send_once(),
        MachPortRight::PortSet => matches!(entry.object(), IpcObject::PortSet(_)),
        MachPortRight::DeadName => entry.is_dead_name(),
    };

    if !has_right {
        return KERN_INVALID_RIGHT;
    }

    *refs = entry.urefs() as u32;
    KERN_SUCCESS
}

// ============================================================================
// mach_port_insert_right - Insert a right into space
// ============================================================================

/// Insert a port right into a task's space
///
/// # Arguments
/// * `space` - Target IPC space
/// * `name` - Port name to use
/// * `port` - Port to insert (as PortName in source space)
/// * `msg_type` - Type of right to insert
///
/// # Returns
/// KERN_SUCCESS on success
pub fn mach_port_insert_right(
    space: &IpcSpace,
    name: MachPortName,
    port: Arc<Mutex<Port>>,
    msg_type: MsgTypeName,
) -> KernReturn {
    // Determine the right type from message type
    let right_type = match msg_type {
        MsgTypeName::MoveReceive => PortRight::Receive,
        MsgTypeName::MoveSend | MsgTypeName::CopySend | MsgTypeName::MakeSend => PortRight::Send,
        MsgTypeName::MoveSendOnce | MsgTypeName::MakeSendOnce => PortRight::SendOnce,
    };

    // Try to insert the right
    match right_type {
        PortRight::Receive => match space.alloc_receive_right_with_name(port, name) {
            Ok(()) => KERN_SUCCESS,
            Err(_) => KERN_INVALID_ARGUMENT,
        },
        PortRight::Send => {
            // Add send right reference to the port
            {
                let port_guard = port.lock();
                port_guard.add_send_right();
            }
            match space.alloc_send_right_with_name(port, name, 1) {
                Ok(()) => KERN_SUCCESS,
                Err(_) => KERN_INVALID_ARGUMENT,
            }
        }
        PortRight::SendOnce => {
            // Add send-once right reference to the port
            {
                let port_guard = port.lock();
                port_guard.make_send_once_right();
            }
            match space.alloc_send_once_right_with_name(port, name) {
                Ok(()) => KERN_SUCCESS,
                Err(_) => KERN_INVALID_ARGUMENT,
            }
        }
        _ => KERN_INVALID_RIGHT,
    }
}

// ============================================================================
// mach_port_extract_right - Extract a right from space
// ============================================================================

/// Extract a port right from a task's space
///
/// # Arguments
/// * `space` - Source IPC space
/// * `name` - Port name to extract
/// * `msg_type` - Type of extraction to perform
/// * `port` - Output: the extracted port
/// * `acquired_type` - Output: the type of right acquired
///
/// # Returns
/// KERN_SUCCESS on success
pub fn mach_port_extract_right(
    space: &IpcSpace,
    name: MachPortName,
    msg_type: MsgTypeName,
    port: &mut Option<Arc<Mutex<Port>>>,
    acquired_type: &mut MsgTypeName,
) -> KernReturn {
    if name == 0 {
        *port = None;
        return KERN_SUCCESS;
    }

    match super::right::copyin(space, name, msg_type) {
        Ok(result) => {
            *port = result.port;
            // The acquired type is the same as requested for move operations
            *acquired_type = msg_type;
            KERN_SUCCESS
        }
        Err(IpcError::InvalidPort) => KERN_INVALID_NAME,
        Err(IpcError::InvalidRight) => KERN_INVALID_RIGHT,
        Err(_) => KERN_INVALID_ARGUMENT,
    }
}

// ============================================================================
// mach_port_move_member - Add/remove port from port set
// ============================================================================

/// Move a port into or out of a port set
///
/// # Arguments
/// * `space` - IPC space
/// * `member` - Port to add/remove
/// * `after` - Port set to add to (or 0 to remove from current set)
///
/// # Returns
/// KERN_SUCCESS on success
pub fn mach_port_move_member(
    space: &IpcSpace,
    member: MachPortName,
    after: MachPortName,
) -> KernReturn {
    if member == 0 {
        return KERN_INVALID_NAME;
    }

    // Get the port entry
    let port_entry = match space.lookup(member) {
        Ok(e) => e,
        Err(_) => return KERN_INVALID_NAME,
    };

    if !port_entry.has_receive() {
        return KERN_INVALID_RIGHT;
    }

    let port = match port_entry.object() {
        IpcObject::Port(p) => Arc::clone(p),
        _ => return KERN_INVALID_RIGHT,
    };

    if after == 0 {
        // Remove from current port set
        let port_guard = port.lock();
        port_guard.set_port_set(None);
        KERN_SUCCESS
    } else {
        // Get the port set entry
        let pset_entry = match space.lookup(after) {
            Ok(e) => e,
            Err(_) => return KERN_INVALID_NAME,
        };

        // PortSet is currently stored as u32 ID, not a full object
        // PortSetId is a type alias for u32, so we use the value directly
        let pset_id: super::pset::PortSetId = match pset_entry.object() {
            IpcObject::PortSet(id) => *id,
            _ => return KERN_INVALID_RIGHT,
        };

        // Add port to port set
        let port_guard = port.lock();
        port_guard.set_port_set(Some(pset_id));

        KERN_SUCCESS
    }
}

// ============================================================================
// mach_port_type - Get the type of a port name
// ============================================================================

/// Get the type(s) of rights associated with a port name
///
/// # Arguments
/// * `space` - IPC space
/// * `name` - Port name to query
/// * `ptype` - Output: bitmap of right types
///
/// # Returns
/// KERN_SUCCESS on success
pub fn mach_port_type(space: &IpcSpace, name: MachPortName, ptype: &mut u32) -> KernReturn {
    if name == 0 {
        return KERN_INVALID_NAME;
    }

    let entry = match space.lookup(name) {
        Ok(e) => e,
        Err(_) => return KERN_INVALID_NAME,
    };

    *ptype = entry.entry_type();
    KERN_SUCCESS
}

// ============================================================================
// Port Set Operations (extend pset module)
// ============================================================================

/// Allocate a new port set
pub fn mach_port_allocate_pset(space: &IpcSpace, name: &mut MachPortName) -> KernReturn {
    mach_port_allocate(space, MachPortRight::PortSet, name)
}

/// Get members of a port set
pub fn mach_port_get_set_status(
    space: &IpcSpace,
    name: MachPortName,
    members: &mut alloc::vec::Vec<MachPortName>,
) -> KernReturn {
    let entry = match space.lookup(name) {
        Ok(e) => e,
        Err(_) => return KERN_INVALID_NAME,
    };

    // Get the port set ID
    let pset_id = match entry.object() {
        IpcObject::PortSet(id) => *id,
        _ => return KERN_INVALID_RIGHT,
    };

    // Clear output vector
    members.clear();

    // Iterate over all entries in the space to find ports belonging to this set
    // Port set membership is tracked via port's port_set field
    space.for_each_entry(|entry_name, entry| {
        if let IpcObject::Port(port) = entry.object() {
            let port_guard = port.lock();
            if let Some(port_pset_id) = port_guard.port_set() {
                if port_pset_id == pset_id {
                    members.push(entry_name);
                }
            }
        }
    });

    KERN_SUCCESS
}

// ============================================================================
// Convenience wrappers using current task's space
// ============================================================================

/// Allocate a port in the current task's space
pub fn port_allocate(right_type: MachPortRight, name: &mut MachPortName) -> KernReturn {
    if let Some(space) = current_space() {
        mach_port_allocate(&space, right_type, name)
    } else {
        KERN_INVALID_ARGUMENT
    }
}

/// Deallocate a port from the current task's space
pub fn port_deallocate(name: MachPortName) -> KernReturn {
    if let Some(space) = current_space() {
        mach_port_deallocate(&space, name)
    } else {
        KERN_INVALID_ARGUMENT
    }
}

/// Destroy a port in the current task's space
pub fn port_destroy(name: MachPortName) -> KernReturn {
    if let Some(space) = current_space() {
        mach_port_destroy(&space, name)
    } else {
        KERN_INVALID_ARGUMENT
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mach_port_right() {
        assert_eq!(MachPortRight::from_u32(0), Some(MachPortRight::Send));
        assert_eq!(MachPortRight::from_u32(1), Some(MachPortRight::Receive));
        assert_eq!(MachPortRight::from_u32(100), None);
    }
}
