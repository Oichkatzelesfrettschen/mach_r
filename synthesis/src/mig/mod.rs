//! MIG (Mach Interface Generator) for Rust
//!
//! Generates client/server stubs for Mach IPC interfaces

use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use crate::message::{Message, MessageBody};
use crate::mach::abi as mach_abi;
use crate::port::Port;
use crate::types::PortId;

/// MIG interface definition
pub struct Interface {
    /// Interface name
    pub name: String,
    /// Subsystem ID
    pub subsystem: u32,
    /// List of routines
    pub routines: Vec<Routine>,
}

/// Routine (RPC call) definition
pub struct Routine {
    /// Routine name
    pub name: String,
    /// Message ID
    pub id: u32,
    /// Input parameters
    pub inputs: Vec<Parameter>,
    /// Output parameters
    pub outputs: Vec<Parameter>,
    /// Is it simpleroutine (no reply)?
    pub simpleroutine: bool,
}

/// Parameter definition
#[derive(Clone)]
pub struct Parameter {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub typ: MigType,
    /// Direction (in/out/inout)
    pub direction: Direction,
}

/// MIG type definitions
#[derive(Clone)]
pub enum MigType {
    /// Scalar types
    Int32,
    Int64,
    UInt32,
    UInt64,
    Boolean,
    /// Port types
    Port,
    PortSendRight,
    PortReceiveRight,
    PortSendOnce,
    /// Array types
    Array(Box<MigType>, usize),
    /// Variable-length array
    VarArray(Box<MigType>),
    /// String type
    String,
    /// Raw bytes
    Bytes(usize),
}

/// Parameter direction
#[derive(Clone)]
pub enum Direction {
    In,
    Out,
    InOut,
}

/// Generated client stub
pub trait MigClient {
    /// Get the server port
    fn server_port(&self) -> &Port;
    
    /// Send a message and wait for reply
    fn call(&self, _msg: Message) -> Result<Message, MigError>;
    
    /// Send a message without waiting for reply
    fn send(&self, _msg: Message) -> Result<(), MigError>;
}

/// Generated server stub
pub trait MigServer {
    /// Dispatch a message to the appropriate handler
    fn dispatch(&mut self, _msg: Message) -> Option<Message>;
}

/// MIG errors
#[derive(Debug, Clone)]
pub enum MigError {
    /// Invalid message ID
    InvalidMessageId,
    /// Parameter marshalling error
    MarshalError,
    /// Port communication error
    PortError,
    /// Server not found
    ServerNotFound,
}

// Re-export generated stubs (written by `xtask mig`)
pub mod generated;

#[cfg(test)]
mod gen_tests {
    use super::generated::name_server::*;
    use crate::types::PortId;
    use crate::port::Port;
    use alloc::sync::Arc;
    use crate::types::TaskId;

    #[test]
    fn generated_constants_exist() {
        assert_eq!(NS_REGISTER_ID, 1000);
        assert_eq!(NS_LOOKUP_ID, 1001);
        assert_eq!(NS_UNREGISTER_ID, 1002);
    }

    #[test]
    fn name_client_constructs() {
        let p = Port::new(TaskId(1));
        let _c = NameClient::new(Arc::clone(&p));
    }
}

/// Helpers for (clean-room) Mach-like marshalling of port descriptors
pub mod marshal {
    use super::*;
    use crate::types::PortId;

    /// Minimal descriptor for a port right in a message
    #[derive(Clone, Debug)]
    pub struct PortDescriptor {
        pub name: PortId,
        pub disposition: mach_abi::MachMsgTypeName,
        pub descriptor_type: mach_abi::MachMsgDescriptorType,
    }

    /// Marshal a port-right MessageBody into a descriptor
    pub fn port_right_to_descriptor(msg: &Message) -> Option<PortDescriptor> {
        match &msg.body {
            MessageBody::PortRight { port, right_type } => Some(PortDescriptor {
                name: *port,
                disposition: right_type.to_mach_name(),
                descriptor_type: mach_abi::MachMsgDescriptorType::Port,
            }),
            _ => None,
        }
    }

    /// Minimal descriptor for out-of-line bytes
    #[derive(Clone, Debug)]
    pub struct OolDescriptor {
        pub data_len: u32,
        pub copy: u32, // mach_abi::MACH_MSG_VIRTUAL_COPY etc.
        pub descriptor_type: mach_abi::MachMsgDescriptorType,
    }

    /// Marshal an OOL message body into a descriptor
    pub fn ool_to_descriptor(msg: &Message) -> Option<OolDescriptor> {
        match &msg.body {
            MessageBody::OutOfLine(bytes) => Some(OolDescriptor {
                data_len: bytes.len() as u32,
                copy: mach_abi::MACH_MSG_VIRTUAL_COPY,
                descriptor_type: mach_abi::MachMsgDescriptorType::Ool,
            }),
            _ => None,
        }
    }

    /// Minimal descriptor for OOL ports
    #[derive(Clone, Debug)]
    pub struct OolPortsDescriptor {
        pub names: alloc::vec::Vec<PortId>,
        pub disposition: mach_abi::MachMsgTypeName,
        pub descriptor_type: mach_abi::MachMsgDescriptorType,
    }

    /// Marshal a MessageBody::PortArray into OOL-Ports descriptor
    pub fn port_array_to_descriptor(msg: &Message) -> Option<OolPortsDescriptor> {
        match &msg.body {
            MessageBody::PortArray { ports, right_type } => Some(OolPortsDescriptor {
                names: ports.clone(),
                disposition: right_type.to_mach_name(),
                descriptor_type: mach_abi::MachMsgDescriptorType::OolPorts,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_descriptor_marshalling() {
        let m = crate::message::Message {
            header: crate::message::MessageHeader {
                size: 0,
                msg_type: crate::message::MessageType::Normal,
                remote_port: Some(PortId(1)),
                local_port: None,
                sequence: 0,
                id: 0,
            },
            body: crate::message::MessageBody::PortRight { port: PortId(42), right_type: crate::message::PortRightType::SendOnce }
        };
        let d = marshal::port_right_to_descriptor(&m).expect("descriptor");
        assert_eq!(d.name, PortId(42));
        assert_eq!(d.descriptor_type, mach_abi::MachMsgDescriptorType::Port);
        assert_eq!(d.disposition as u32, 18);
    }

    #[test]
    fn ool_and_oolports_marshalling() {
        // OOL bytes
        let m = crate::message::Message::new_out_of_line(PortId(1), alloc::vec![1,2,3,4]);
        let d = marshal::ool_to_descriptor(&m).expect("ool");
        assert_eq!(d.data_len, 4);
        assert_eq!(d.descriptor_type, mach_abi::MachMsgDescriptorType::Ool);

        // OOL ports
        let body = crate::message::MessageBody::PortArray { ports: alloc::vec![PortId(7), PortId(8)], right_type: crate::message::PortRightType::Send };
        let m2 = crate::message::Message { header: crate::message::MessageHeader{ size:0, msg_type: crate::message::MessageType::Normal, remote_port: Some(PortId(2)), local_port: None, sequence:0, id: 0 }, body };
        let d2 = marshal::port_array_to_descriptor(&m2).expect("oolports");
        assert_eq!(d2.names.len(), 2);
        assert_eq!(d2.descriptor_type, mach_abi::MachMsgDescriptorType::OolPorts);
        assert_eq!(d2.disposition as u32, 17);
    }
}

/// Example: VM subsystem interface (like Mach's vm_map)
pub mod vm_interface {
    use super::*;
    use crate::types::TaskId;
    
    /// VM subsystem ID
    pub const VM_SUBSYSTEM: u32 = 2400;
    
    /// Message IDs
    pub const VM_ALLOCATE_ID: u32 = VM_SUBSYSTEM + 1;
    pub const VM_DEALLOCATE_ID: u32 = VM_SUBSYSTEM + 2;
    pub const VM_PROTECT_ID: u32 = VM_SUBSYSTEM + 3;
    pub const VM_MAP_ID: u32 = VM_SUBSYSTEM + 4;
    
    /// VM allocate request
    #[repr(C)]
    pub struct VmAllocateRequest {
        pub header: MessageHeader,
        pub target_task: TaskId,
        pub address: u64,
        pub size: u64,
        pub anywhere: bool,
    }
    
    /// VM allocate reply
    #[repr(C)]
    pub struct VmAllocateReply {
        pub header: MessageHeader,
        pub return_code: i32,
        pub address: u64,
    }
    
    /// Message header (like mach_msg_header_t)
    #[repr(C)]
    pub struct MessageHeader {
        pub bits: u32,
        pub size: u32,
        pub remote_port: PortId,
        pub local_port: PortId,
        pub id: u32,
    }
    
    /// VM client implementation
    pub struct VmClient {
        server_port: Port,
    }
    
    impl VmClient {
        /// Create a new VM client
        pub fn new(server_port: Port) -> Self {
            VmClient { server_port }
        }
        
        /// Allocate virtual memory
        pub fn vm_allocate(
            &self,
            target_task: TaskId,
            address: u64,
            size: u64,
            anywhere: bool,
        ) -> Result<u64, MigError> {
            // Build request message
            let request = VmAllocateRequest {
                header: MessageHeader {
                    bits: 0,
                    size: core::mem::size_of::<VmAllocateRequest>() as u32,
                    remote_port: self.server_port.id(),
                    local_port: PortId::new(),
                    id: VM_ALLOCATE_ID,
                },
                target_task,
                address,
                size,
                anywhere,
            };
            
            // Marshal request
            let msg = marshal_vm_allocate_request(&request)?;
            
            // Send and wait for reply
            let reply_msg = self.call(msg)?;
            
            // Unmarshal reply
            let reply = unmarshal_vm_allocate_reply(&reply_msg)?;
            
            if reply.return_code != 0 {
                return Err(MigError::ServerNotFound);
            }
            
            Ok(reply.address)
        }
    }
    
    impl MigClient for VmClient {
        fn server_port(&self) -> &Port {
            &self.server_port
        }
        
        fn call(&self, msg: Message) -> Result<Message, MigError> {
            // Send message
            self.server_port.send(msg)
                .map_err(|_| MigError::PortError)?;
            
            // Wait for reply
            self.server_port.receive()
                .ok_or(MigError::PortError)
        }
        
        fn send(&self, msg: Message) -> Result<(), MigError> {
            self.server_port.send(msg)
                .map_err(|_| MigError::PortError)
        }
    }
    
    /// Marshal VM allocate request
    fn marshal_vm_allocate_request(req: &VmAllocateRequest) -> Result<Message, MigError> {
        // In real implementation, would properly serialize
        let data = unsafe {
            core::slice::from_raw_parts(
                req as *const _ as *const u8,
                core::mem::size_of::<VmAllocateRequest>(),
            )
        };
        
        Message::new_inline(req.header.remote_port, data)
            .map_err(|_| MigError::MarshalError)
    }
    
    /// Unmarshal VM allocate reply
    fn unmarshal_vm_allocate_reply(_msg: &Message) -> Result<VmAllocateReply, MigError> {
        // In real implementation, would properly deserialize
        // For now, return dummy
        Ok(VmAllocateReply {
            header: MessageHeader {
                bits: 0,
                size: 0,
                remote_port: PortId::new(),
                local_port: PortId::new(),
                id: VM_ALLOCATE_ID,
            },
            return_code: 0,
            address: 0x1000,
        })
    }
}

/// Macro to generate MIG interfaces (simplified)
#[macro_export]
macro_rules! mig_interface {
    (
        subsystem $name:ident = $id:expr;
        $(
            routine $routine_name:ident($msg_id:expr) {
                inputs: { $($in_name:ident: $in_type:ty),* }
                outputs: { $($out_name:ident: $out_type:ty),* }
            }
        )*
    ) => {
        pub mod $name {
            
            
            pub const SUBSYSTEM_ID: u32 = $id;
            
            $(
                #[allow(non_upper_case_globals)]
                pub const $routine_name: u32 = $msg_id;
            )*
            
            // Generate client and server stubs...
        }
    };
}

mig_interface! {
    subsystem file_server = 3000;
    
    routine file_open(3001) {
        inputs: { path: String, flags: u32 }
        outputs: { handle: Port, error: i32 }
    }
    
    routine file_read(3002) {
        inputs: { handle: Port, offset: u64, length: u32 }
        outputs: { data: VarArray, error: i32 }
    }
    
    routine file_write(3003) {
        inputs: { handle: Port, offset: u64, data: VarArray }
        outputs: { bytes_written: u32, error: i32 }
    }
}
