//! Mach message implementation
//!
//! Messages are the data transferred through ports. They can contain
//! simple data, out-of-line memory, and port rights.

use crate::mach::abi as mach_abi;
use crate::types::PortId;
use alloc::vec::Vec as StdVec;
use heapless::Vec;

/// Message type codes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageType {
    /// Normal message
    Normal,
    /// Emergency message (higher priority)
    Emergency,
    /// Notification message
    Notification,
}

/// Message header - based on Mach's mach_msg_header_t
#[derive(Debug, Clone)]
pub struct MessageHeader {
    /// Message size in bytes
    pub size: u32,
    /// Message type
    pub msg_type: MessageType,
    /// Remote port (destination)
    pub remote_port: Option<PortId>,
    /// Local port (reply port)
    pub local_port: Option<PortId>,
    /// Sequence number
    pub sequence: u64,
    /// Message ID (user-defined)
    pub id: u32,
}

/// Message body types
#[derive(Debug, Clone)]
pub enum MessageBody {
    /// Inline data (small messages)
    Inline(Vec<u8, 256>),
    /// Out-of-line data (large messages)
    OutOfLine(StdVec<u8>),
    /// Port right transfer
    PortRight {
        port: PortId,
        right_type: PortRightType,
    },
    /// Array of port rights (OOL-Ports descriptor analogue)
    PortArray {
        ports: StdVec<PortId>,
        right_type: PortRightType,
    },
}

/// Types of port rights that can be transferred
#[derive(Debug, Clone, Copy)]
pub enum PortRightType {
    /// Send right
    Send,
    /// Duplicate a send right (sender retains)
    CopySend,
    /// Create a new send right from receive right
    MakeSend,
    /// Send-once right
    SendOnce,
    /// Receive right
    Receive,
}

/// Complete Mach message
#[derive(Debug, Clone)]
pub struct Message {
    /// Message header
    pub header: MessageHeader,
    /// Message body
    pub body: MessageBody,
}

impl Message {
    /// Create a simple inline message
    pub fn new_inline(remote_port: PortId, data: &[u8]) -> Result<Self, &'static str> {
        if data.len() > 256 {
            return Err("Data too large for inline message");
        }

        let mut inline_data = Vec::new();
        inline_data
            .extend_from_slice(data)
            .map_err(|_| "Failed to copy data")?;

        Ok(Message {
            header: MessageHeader {
                size: data.len() as u32,
                msg_type: MessageType::Normal,
                remote_port: Some(remote_port),
                local_port: None,
                sequence: 0,
                id: 0,
            },
            body: MessageBody::Inline(inline_data),
        })
    }

    /// Create an out-of-line message for large data
    pub fn new_out_of_line(remote_port: PortId, data: StdVec<u8>) -> Self {
        Message {
            header: MessageHeader {
                size: data.len() as u32,
                msg_type: MessageType::Normal,
                remote_port: Some(remote_port),
                local_port: None,
                sequence: 0,
                id: 0,
            },
            body: MessageBody::OutOfLine(data),
        }
    }

    /// Attach a reply (local) port to the message header
    pub fn with_reply_port(mut self, reply_port: PortId) -> Self {
        self.header.local_port = Some(reply_port);
        self
    }

    /// Create a port right transfer message
    pub fn new_port_transfer(
        remote_port: PortId,
        transferred_port: PortId,
        right_type: PortRightType,
    ) -> Self {
        Message {
            header: MessageHeader {
                size: core::mem::size_of::<PortId>() as u32,
                msg_type: MessageType::Normal,
                remote_port: Some(remote_port),
                local_port: None,
                sequence: 0,
                id: 0,
            },
            body: MessageBody::PortRight {
                port: transferred_port,
                right_type,
            },
        }
    }

    /// Get message size
    pub fn size(&self) -> usize {
        self.header.size as usize
    }

    /// Check if message is a notification
    pub fn is_notification(&self) -> bool {
        self.header.msg_type == MessageType::Notification
    }

    /// Get the destination port
    pub fn remote_port(&self) -> PortId {
        self.header.remote_port.unwrap_or(PortId(0))
    }

    /// Get message data
    pub fn data(&self) -> &[u8] {
        match &self.body {
            MessageBody::Inline(data) => data.as_slice(),
            MessageBody::OutOfLine(data) => data.as_slice(),
            MessageBody::PortRight { .. } => &[], // No data for port rights
            MessageBody::PortArray { .. } => &[],
        }
    }

    /// Compute a minimal Mach-like header bits field for compatibility
    pub fn header_bits(&self) -> mach_abi::MachMsgBits {
        // For now we only encode presence of remote/local port names (non-zero)
        let remote = if self.header.remote_port.is_some() {
            1
        } else {
            0
        };
        let local = if self.header.local_port.is_some() {
            1
        } else {
            0
        };
        mach_abi::mach_msgh_bits(remote, local)
    }
}

impl PortRightType {
    /// Map to clean-room Mach disposition names
    pub fn to_mach_name(self) -> mach_abi::MachMsgTypeName {
        match self {
            PortRightType::Send => mach_abi::MachMsgTypeName::MoveSend,
            PortRightType::CopySend => mach_abi::MachMsgTypeName::CopySend,
            PortRightType::MakeSend => mach_abi::MachMsgTypeName::MakeSend,
            PortRightType::SendOnce => mach_abi::MachMsgTypeName::MoveSendOnce,
            PortRightType::Receive => mach_abi::MachMsgTypeName::MoveReceive,
        }
    }
}

/// Message queue for buffering (simplified version)
pub struct MessageBuffer {
    messages: spin::Mutex<StdVec<Message>>,
    max_size: usize,
}

impl MessageBuffer {
    /// Create a new message buffer
    pub fn new(max_size: usize) -> Self {
        MessageBuffer {
            messages: spin::Mutex::new(StdVec::new()),
            max_size,
        }
    }

    /// Add a message to the buffer
    pub fn push(&self, msg: Message) -> Result<(), Message> {
        let mut buffer = self.messages.lock();
        if buffer.len() >= self.max_size {
            return Err(msg);
        }
        buffer.push(msg);
        Ok(())
    }

    /// Remove and return the first message
    pub fn pop(&self) -> Option<Message> {
        let mut buffer = self.messages.lock();
        if buffer.is_empty() {
            None
        } else {
            Some(buffer.remove(0))
        }
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.messages.lock().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_message() {
        let port_id = PortId(1);
        let data = b"Hello, Mach_R!";

        let msg = Message::new_inline(port_id, data).unwrap();
        assert_eq!(msg.size(), data.len());

        if let MessageBody::Inline(ref body_data) = msg.body {
            assert_eq!(body_data.as_slice(), data);
        } else {
            panic!("Expected inline message");
        }
    }

    #[test]
    fn test_message_buffer() {
        let buffer = MessageBuffer::new(10);
        let port_id = PortId(1);

        let msg = Message::new_inline(port_id, b"Test").unwrap();
        assert!(buffer.push(msg).is_ok());
        assert!(!buffer.is_empty());

        let retrieved = buffer.pop().unwrap();
        assert_eq!(retrieved.size(), 4);
        assert!(buffer.is_empty());
    }

    #[test]
    fn port_right_mapping() {
        assert_eq!(PortRightType::Send.to_mach_name() as u32, 17);
        assert_eq!(PortRightType::CopySend.to_mach_name() as u32, 19);
        assert_eq!(PortRightType::MakeSend.to_mach_name() as u32, 20);
        assert_eq!(PortRightType::SendOnce.to_mach_name() as u32, 18);
        assert_eq!(PortRightType::Receive.to_mach_name() as u32, 16);
    }

    #[test]
    fn header_bits_encode() {
        let p = PortId(42);
        let m = Message::new_inline(p, b"hi").unwrap();
        let bits = m.header_bits();
        assert_eq!(mach_abi::mach_msgh_bits_remote(bits), 1);
        assert_eq!(mach_abi::mach_msgh_bits_local(bits), 0);
    }
}
