//! IPC Rights - Capability transfer (copyin/copyout)
//!
//! Based on Mach4 ipc/ipc_right.c
//! Handles transfer of port rights between tasks during message passing.

use alloc::sync::Arc;
use spin::Mutex;

use super::entry::{IpcEntry, IpcObject, MachPortName};
use super::port::Port;
use super::space::IpcSpace;
use super::IpcError;

// ============================================================================
// Right Types for Message Passing
// ============================================================================

/// Message type names - how a right appears in a message
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MsgTypeName {
    /// Move receive right
    MoveReceive = 16,
    /// Move send right
    MoveSend = 17,
    /// Move send-once right
    MoveSendOnce = 18,
    /// Copy send right
    CopySend = 19,
    /// Make send right (from receive)
    MakeSend = 20,
    /// Make send-once right (from receive)
    MakeSendOnce = 21,
}

impl MsgTypeName {
    /// Convert from u32
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            16 => Some(MsgTypeName::MoveReceive),
            17 => Some(MsgTypeName::MoveSend),
            18 => Some(MsgTypeName::MoveSendOnce),
            19 => Some(MsgTypeName::CopySend),
            20 => Some(MsgTypeName::MakeSend),
            21 => Some(MsgTypeName::MakeSendOnce),
            _ => None,
        }
    }

    /// Check if this type name moves the right (vs copies)
    pub fn is_move(&self) -> bool {
        matches!(
            self,
            MsgTypeName::MoveReceive | MsgTypeName::MoveSend | MsgTypeName::MoveSendOnce
        )
    }

    /// Check if this creates a new right from receive
    pub fn is_make(&self) -> bool {
        matches!(self, MsgTypeName::MakeSend | MsgTypeName::MakeSendOnce)
    }
}

/// Port right types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PortRight {
    /// Send right
    Send = 0,
    /// Receive right (only one per port)
    Receive = 1,
    /// Send-once right (consumed on use)
    SendOnce = 2,
    /// Port set membership
    PortSet = 3,
    /// Dead name (port was destroyed)
    DeadName = 4,
}

// ============================================================================
// Copyin Result - What we get from copying in a right
// ============================================================================

/// Result of copying in a port right from user space
#[derive(Debug, Clone)]
pub struct CopyinResult {
    /// The port (or None for dead name)
    pub port: Option<Arc<Mutex<Port>>>,
    /// The type of right that was copied in
    pub right_type: PortRight,
    /// Was this a send-once that should generate a reply?
    pub reply_port: Option<Arc<Mutex<Port>>>,
}

impl CopyinResult {
    /// Create result for a valid port
    pub fn port(port: Arc<Mutex<Port>>, right_type: PortRight) -> Self {
        Self {
            port: Some(port),
            right_type,
            reply_port: None,
        }
    }

    /// Create result for dead name
    pub fn dead() -> Self {
        Self {
            port: None,
            right_type: PortRight::DeadName,
            reply_port: None,
        }
    }
}

// ============================================================================
// Copyin Operations - Transfer rights FROM user space
// ============================================================================

/// Check if a copyin operation is valid (without performing it)
pub fn copyin_check(
    space: &IpcSpace,
    name: MachPortName,
    msg_type: MsgTypeName,
) -> Result<bool, IpcError> {
    if name == 0 {
        return Ok(true); // NULL port is always valid
    }

    let entry = space.lookup(name)?;

    match msg_type {
        MsgTypeName::MoveReceive => Ok(entry.has_receive()),
        MsgTypeName::MoveSend | MsgTypeName::CopySend => Ok(entry.has_send() && entry.urefs() > 0),
        MsgTypeName::MoveSendOnce => Ok(entry.has_send_once()),
        MsgTypeName::MakeSend | MsgTypeName::MakeSendOnce => Ok(entry.has_receive()),
    }
}

/// Copy in a port right from a task's space (for message sending)
///
/// This is called when a message is being sent to copy the port
/// right from the sender's space into the kernel message.
pub fn copyin(
    space: &IpcSpace,
    name: MachPortName,
    msg_type: MsgTypeName,
) -> Result<CopyinResult, IpcError> {
    // NULL port maps to no capability
    if name == 0 {
        return Ok(CopyinResult::dead());
    }

    let entry = space.lookup(name)?;

    match msg_type {
        MsgTypeName::MoveReceive => copyin_move_receive(space, name, &entry),
        MsgTypeName::MoveSend => copyin_move_send(space, name, &entry),
        MsgTypeName::MoveSendOnce => copyin_move_send_once(space, name, &entry),
        MsgTypeName::CopySend => copyin_copy_send(&entry),
        MsgTypeName::MakeSend => copyin_make_send(&entry),
        MsgTypeName::MakeSendOnce => copyin_make_send_once(&entry),
    }
}

/// Move receive right from space
fn copyin_move_receive(
    space: &IpcSpace,
    name: MachPortName,
    entry: &IpcEntry,
) -> Result<CopyinResult, IpcError> {
    if !entry.has_receive() {
        return Err(IpcError::InvalidRight);
    }

    let port = match entry.object() {
        IpcObject::Port(p) => Arc::clone(p),
        _ => return Err(IpcError::InvalidPort),
    };

    // Remove the receive right from the space
    space.entry_dealloc(name)?;

    Ok(CopyinResult::port(port, PortRight::Receive))
}

/// Move send right from space
fn copyin_move_send(
    space: &IpcSpace,
    name: MachPortName,
    entry: &IpcEntry,
) -> Result<CopyinResult, IpcError> {
    if !entry.has_send() {
        return Err(IpcError::InvalidRight);
    }

    if entry.urefs() == 0 {
        return Err(IpcError::InvalidRight);
    }

    let port = match entry.object() {
        IpcObject::Port(p) => Arc::clone(p),
        _ => return Err(IpcError::InvalidPort),
    };

    // Decrement user refs
    if entry.urefs() == 1 {
        // Last reference, remove entry
        space.entry_dealloc(name)?;
    } else {
        space.modify_refs(name, -1)?;
    }

    Ok(CopyinResult::port(port, PortRight::Send))
}

/// Move send-once right from space
fn copyin_move_send_once(
    space: &IpcSpace,
    name: MachPortName,
    entry: &IpcEntry,
) -> Result<CopyinResult, IpcError> {
    if !entry.has_send_once() {
        return Err(IpcError::InvalidRight);
    }

    let port = match entry.object() {
        IpcObject::Port(p) => Arc::clone(p),
        _ => return Err(IpcError::InvalidPort),
    };

    // Send-once is always consumed
    space.entry_dealloc(name)?;

    Ok(CopyinResult::port(port, PortRight::SendOnce))
}

/// Copy send right (doesn't modify source)
fn copyin_copy_send(entry: &IpcEntry) -> Result<CopyinResult, IpcError> {
    if !entry.has_send() {
        return Err(IpcError::InvalidRight);
    }

    if entry.urefs() == 0 {
        return Err(IpcError::InvalidRight);
    }

    let port = match entry.object() {
        IpcObject::Port(p) => Arc::clone(p),
        _ => return Err(IpcError::InvalidPort),
    };

    // Add a send right reference to the port
    {
        let port_guard = port.lock();
        port_guard.add_send_right();
    }

    Ok(CopyinResult::port(port, PortRight::Send))
}

/// Make send right from receive
fn copyin_make_send(entry: &IpcEntry) -> Result<CopyinResult, IpcError> {
    if !entry.has_receive() {
        return Err(IpcError::InvalidRight);
    }

    let port = match entry.object() {
        IpcObject::Port(p) => Arc::clone(p),
        _ => return Err(IpcError::InvalidPort),
    };

    // Create a new send right from the port
    {
        let port_guard = port.lock();
        port_guard.make_send_right();
    }

    Ok(CopyinResult::port(port, PortRight::Send))
}

/// Make send-once right from receive
fn copyin_make_send_once(entry: &IpcEntry) -> Result<CopyinResult, IpcError> {
    if !entry.has_receive() {
        return Err(IpcError::InvalidRight);
    }

    let port = match entry.object() {
        IpcObject::Port(p) => Arc::clone(p),
        _ => return Err(IpcError::InvalidPort),
    };

    // Create a new send-once right from the port
    {
        let port_guard = port.lock();
        port_guard.make_send_once_right();
    }

    Ok(CopyinResult::port(port, PortRight::SendOnce))
}

// ============================================================================
// Copyout Operations - Transfer rights TO user space
// ============================================================================

/// Copy out a port right to a task's space (for message receiving)
///
/// This is called when a message is being received to install the port
/// right into the receiver's space.
pub fn copyout(
    space: &IpcSpace,
    port: Arc<Mutex<Port>>,
    msg_type: MsgTypeName,
) -> Result<MachPortName, IpcError> {
    // Determine what type of right to create
    let right_type = match msg_type {
        MsgTypeName::MoveReceive => PortRight::Receive,
        MsgTypeName::MoveSend | MsgTypeName::CopySend | MsgTypeName::MakeSend => PortRight::Send,
        MsgTypeName::MoveSendOnce | MsgTypeName::MakeSendOnce => PortRight::SendOnce,
    };

    copyout_right(space, port, right_type)
}

/// Copy out a specific right type
pub fn copyout_right(
    space: &IpcSpace,
    port: Arc<Mutex<Port>>,
    right_type: PortRight,
) -> Result<MachPortName, IpcError> {
    // Check if we already have this port in the space
    let object = IpcObject::Port(Arc::clone(&port));
    if let Some((existing_name, existing_entry)) = space.find_entry(&object) {
        // Already have a right to this port
        match right_type {
            PortRight::Send => {
                // Can combine with existing send right
                if existing_entry.has_send() {
                    space.modify_refs(existing_name, 1)?;
                    return Ok(existing_name);
                }
                // Can't combine send with receive, need new entry
            }
            PortRight::Receive => {
                // Can't have two receive rights
                if existing_entry.has_receive() {
                    return Err(IpcError::InvalidRight);
                }
                // Could potentially combine with send right
            }
            PortRight::SendOnce => {
                // Send-once always needs a new entry
            }
            _ => {}
        }
    }

    // Allocate new entry based on right type
    match right_type {
        PortRight::Send => space.alloc_send_right(port, 1),
        PortRight::Receive => space.alloc_receive_right(port),
        PortRight::SendOnce => space.alloc_send_once_right(port),
        PortRight::DeadName => {
            // Allocate dead name entry
            let name = space.entry_alloc()?;
            // Entry needs to be set up as dead name
            // For now just return the allocated name
            Ok(name)
        }
        PortRight::PortSet => Err(IpcError::InvalidRight),
    }
}

/// Copy out a dead name (port was destroyed)
pub fn copyout_dead_name(space: &IpcSpace) -> Result<MachPortName, IpcError> {
    let name = space.entry_alloc()?;
    // The entry would be marked as dead name
    // For now we just allocate it
    Ok(name)
}

// ============================================================================
// Right Manipulation
// ============================================================================

/// Destroy a right (deallocate from space)
pub fn destroy(space: &IpcSpace, name: MachPortName) -> Result<(), IpcError> {
    let entry = space.lookup(name)?;

    match entry.object() {
        IpcObject::Port(port) => {
            if entry.has_receive() {
                // Destroying receive right destroys the port
                let mut port_guard = port.lock();
                port_guard.destroy();
                drop(port_guard);

                // Trigger dead-name notifications for all watchers of this port
                super::notify::trigger_dead_name(name);
                // Also trigger port-destroyed notification for the former receive right holder
                super::notify::trigger_port_destroyed(name);
            } else if entry.has_send() {
                // Release send rights
                let port_guard = port.lock();
                let urefs = entry.urefs();
                for _ in 0..urefs {
                    port_guard.release_send_right();
                }

                // Check if port now has no senders
                if port_guard.send_right_count() == 0 {
                    drop(port_guard);
                    super::notify::trigger_no_senders(name);
                }
            } else if entry.has_send_once() {
                // Release send-once right
                let port_guard = port.lock();
                port_guard.release_send_once_right();
            }
        }
        IpcObject::PortSet(_) => {
            // Would destroy port set
        }
        IpcObject::None => {
            // Dead name, just deallocate
        }
    }

    space.entry_dealloc(name)
}

/// Deallocate a right (reduce reference count)
pub fn dealloc(space: &IpcSpace, name: MachPortName) -> Result<(), IpcError> {
    let entry = space.lookup(name)?;

    if entry.has_send() {
        // Get port to check send right count after deallocation
        let port = match entry.object() {
            IpcObject::Port(p) => Some(Arc::clone(p)),
            _ => None,
        };

        // Reduce send right count in space
        space.modify_refs(name, -1)?;

        // If this was the last user reference, release the port's send right
        // and check for no-senders condition
        if entry.urefs() == 1 {
            // This was the last reference, entry was deallocated
            if let Some(port) = port {
                let port_guard = port.lock();
                port_guard.release_send_right();

                // Check for no-senders notification
                if port_guard.send_right_count() == 0 {
                    drop(port_guard);
                    super::notify::trigger_no_senders(name);
                }
            }
        }

        Ok(())
    } else {
        // Other rights just get destroyed
        destroy(space, name)
    }
}

/// Modify reference delta for a right
pub fn delta(
    space: &IpcSpace,
    name: MachPortName,
    right_type: PortRight,
    delta: i32,
) -> Result<(), IpcError> {
    let entry = space.lookup(name)?;

    // Verify the right type matches
    let type_ok = match right_type {
        PortRight::Send => entry.has_send(),
        PortRight::Receive => entry.has_receive(),
        PortRight::SendOnce => entry.has_send_once(),
        PortRight::DeadName => entry.is_dead_name(),
        PortRight::PortSet => false, // Not supported yet
    };

    if !type_ok {
        return Err(IpcError::InvalidRight);
    }

    if delta == 0 {
        return Ok(());
    }

    space.modify_refs(name, delta)
}

/// Get information about a right
pub fn info(space: &IpcSpace, name: MachPortName) -> Result<(u32, u16), IpcError> {
    let entry = space.lookup(name)?;
    Ok((entry.entry_type(), entry.urefs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_type_name() {
        assert!(MsgTypeName::MoveReceive.is_move());
        assert!(MsgTypeName::MoveSend.is_move());
        assert!(!MsgTypeName::CopySend.is_move());
        assert!(MsgTypeName::MakeSend.is_make());
    }
}
