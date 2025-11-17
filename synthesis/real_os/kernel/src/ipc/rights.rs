//! Port rights management (capabilities)

use super::{PortName, IpcError, IpcResult};
use alloc::vec::Vec;

/// Types of port rights (like mach_port_right_t)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortRightType {
    /// Can send messages to port
    Send,
    /// Can receive messages from port (only one holder)
    Receive,
    /// Can send once then right is consumed
    SendOnce,
    /// Notification of port death
    DeadName,
    /// Port set membership
    PortSet,
}

/// A port right (capability)
#[derive(Debug, Clone)]
pub struct PortRight {
    pub port: PortName,
    pub right_type: PortRightType,
    pub refs: u32,  // Reference count for send rights
}

impl PortRight {
    /// Create a new send right
    pub fn new_send(port: PortName) -> Self {
        Self {
            port,
            right_type: PortRightType::Send,
            refs: 1,
        }
    }
    
    /// Create a new receive right
    pub fn new_receive(port: PortName) -> Self {
        Self {
            port,
            right_type: PortRightType::Receive,
            refs: 1,  // Receive rights always have exactly 1 ref
        }
    }
    
    /// Create a send-once right
    pub fn new_send_once(port: PortName) -> Self {
        Self {
            port,
            right_type: PortRightType::SendOnce,
            refs: 1,
        }
    }
    
    /// Add a reference
    pub fn add_ref(&mut self) -> IpcResult<()> {
        match self.right_type {
            PortRightType::Send => {
                self.refs += 1;
                Ok(())
            }
            PortRightType::Receive => {
                // Can't add refs to receive right
                Err(IpcError::InvalidRight)
            }
            _ => Ok(())
        }
    }
    
    /// Remove a reference
    pub fn remove_ref(&mut self) -> IpcResult<bool> {
        if self.refs > 0 {
            self.refs -= 1;
            Ok(self.refs == 0)
        } else {
            Err(IpcError::InvalidRight)
        }
    }
}

/// Port set for receiving from multiple ports
pub struct PortSet {
    pub name: PortName,
    pub members: Vec<PortName>,
}

impl PortSet {
    pub fn new() -> Self {
        Self {
            name: PortName::new(),
            members: Vec::new(),
        }
    }
    
    pub fn add_member(&mut self, port: PortName) -> IpcResult<()> {
        if self.members.contains(&port) {
            return Err(IpcError::InvalidPort);
        }
        self.members.push(port);
        Ok(())
    }
    
    pub fn remove_member(&mut self, port: PortName) -> IpcResult<()> {
        if let Some(pos) = self.members.iter().position(|&p| p == port) {
            self.members.swap_remove(pos);
            Ok(())
        } else {
            Err(IpcError::InvalidPort)
        }
    }
}

/// Rights disposition for message passing
#[derive(Debug, Clone, Copy)]
pub enum Disposition {
    /// Move the right (sender loses it)
    MoveSend,
    /// Copy the right 
    CopySend,
    /// Make a send right from receive right
    MakeSend,
    /// Make a send-once right
    MakeSendOnce,
    /// Move receive right
    MoveReceive,
}

impl Disposition {
    pub fn as_u32(&self) -> u32 {
        match self {
            Self::MoveSend => 17,
            Self::CopySend => 19,
            Self::MakeSend => 20,
            Self::MakeSendOnce => 21,
            Self::MoveReceive => 16,
        }
    }
}