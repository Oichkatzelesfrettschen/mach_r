//! Mach-style message structure

use alloc::vec::Vec;
use alloc::vec;
use super::{PortName};

/// Message type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Normal,
    Emergency,
    Notification,
}

/// Message header (like mach_msg_header_t)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct MessageHeader {
    /// Message bits (type, complexity, etc)
    pub bits: u32,
    /// Size of the entire message
    pub size: u32,
    /// Remote port (sender for receive, destination for send)
    pub remote_port: PortName,
    /// Local port (for replies)
    pub local_port: PortName,
    /// Message ID
    pub id: u32,
}

impl MessageHeader {
    const BITS_COMPLEX: u32 = 0x80000000;
    const BITS_SIMPLE: u32 = 0x00000000;
    
    pub fn new_simple(size: u32, remote: PortName, local: PortName, id: u32) -> Self {
        Self {
            bits: Self::BITS_SIMPLE,
            size,
            remote_port: remote,
            local_port: local,
            id,
        }
    }
    
    pub fn is_complex(&self) -> bool {
        self.bits & Self::BITS_COMPLEX != 0
    }
}

/// Port descriptor for complex messages
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PortDescriptor {
    pub name: PortName,
    pub disposition: u32,  // MOVE, COPY, MAKE_SEND, etc
    pub type_: u32,
}

/// Memory descriptor for out-of-line data
#[repr(C)]
#[derive(Debug, Clone)]
pub struct MemoryDescriptor {
    pub address: usize,
    pub size: usize,
    pub deallocate: bool,
}

/// Complete message structure
#[derive(Debug, Clone)]
pub struct Message {
    pub header: MessageHeader,
    pub body: MessageBody,
}

#[derive(Debug, Clone)]
pub enum MessageBody {
    /// Simple inline data
    Simple(Vec<u8>),
    /// Complex message with descriptors
    Complex {
        port_descriptors: Vec<PortDescriptor>,
        memory_descriptors: Vec<MemoryDescriptor>,
        inline_data: Vec<u8>,
    },
}

impl Message {
    /// Create a simple message with inline data
    pub fn new_simple(remote: PortName, local: PortName, id: u32, data: Vec<u8>) -> Self {
        let size = core::mem::size_of::<MessageHeader>() as u32 + data.len() as u32;
        Self {
            header: MessageHeader::new_simple(size, remote, local, id),
            body: MessageBody::Simple(data),
        }
    }
    
    /// Create a notification message
    pub fn new_notification(port: PortName, id: u32) -> Self {
        Self::new_simple(port, PortName::NULL, id, Vec::new())
    }
    
    /// Get total message size
    pub fn size(&self) -> usize {
        self.header.size as usize
    }
    
    /// Add a port descriptor (makes message complex)
    pub fn add_port(&mut self, port: PortName, disposition: u32) {
        match &mut self.body {
            MessageBody::Simple(data) => {
                // Convert to complex
                let inline_data = core::mem::take(data);
                self.body = MessageBody::Complex {
                    port_descriptors: vec![PortDescriptor {
                        name: port,
                        disposition,
                        type_: 0,
                    }],
                    memory_descriptors: Vec::new(),
                    inline_data,
                };
                self.header.bits |= MessageHeader::BITS_COMPLEX;
            }
            MessageBody::Complex { port_descriptors, .. } => {
                port_descriptors.push(PortDescriptor {
                    name: port,
                    disposition,
                    type_: 0,
                });
            }
        }
    }
    
    /// Add out-of-line memory
    pub fn add_memory(&mut self, addr: usize, size: usize) {
        match &mut self.body {
            MessageBody::Simple(data) => {
                let inline_data = core::mem::take(data);
                self.body = MessageBody::Complex {
                    port_descriptors: Vec::new(),
                    memory_descriptors: vec![MemoryDescriptor {
                        address: addr,
                        size,
                        deallocate: false,
                    }],
                    inline_data,
                };
                self.header.bits |= MessageHeader::BITS_COMPLEX;
            }
            MessageBody::Complex { memory_descriptors, .. } => {
                memory_descriptors.push(MemoryDescriptor {
                    address: addr,
                    size,
                    deallocate: false,
                });
            }
        }
    }
}

/// Message send options
pub struct SendOptions {
    pub timeout: Option<u32>,
    pub notify: Option<PortName>,
}

/// Message receive options  
pub struct ReceiveOptions {
    pub timeout: Option<u32>,
    pub max_size: usize,
}