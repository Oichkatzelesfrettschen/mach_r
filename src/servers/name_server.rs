//! Mach Name Server Implementation
//!
//! The Name Server provides a distributed naming service for Mach ports.
//! It allows tasks to register and look up ports by name, providing the
//! foundation for service discovery in the Mach_R system.

use crate::types::{PortId, TaskId};
use crate::port::{PortRights, Port};
use crate::message::Message;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::Mutex;

// Implement clean-room NameService trait for this server
impl crate::mig::generated::name_server::NameService for NameServer {
    fn register(&self, name: &str, port: PortId) -> i32 {
        match NameServer::register(self, name.to_string(), port, self.server_task) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    }

    fn lookup(&self, name: &str) -> Result<PortId, i32> {
        NameServer::lookup(self, name).ok_or(-2)
    }

    fn unregister(&self, name: &str) -> i32 {
        match NameServer::unregister(self, name, self.server_task) {
            Ok(()) => 0,
            Err(_) => -13,
        }
    }
}

/// Name Server port operations
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum NameServerOp {
    Register = 1000,
    Lookup = 1001,
    Unregister = 1002,
    List = 1003,
    CheckIn = 1004,
    CheckOut = 1005,
}

/// Name Server request message
#[repr(C)]
pub struct NameServerRequest {
    pub op: NameServerOp,
    pub name_len: u32,
    // Variable length name follows
}

/// Name Server reply message
#[repr(C)]
pub struct NameServerReply {
    pub result: i32,
    pub port_id: PortId,
    pub count: u32,
    // Variable length data follows for list operations
}

/// Name binding entry
#[derive(Debug, Clone)]
pub struct NameBinding {
    pub name: String,
    pub port_id: PortId,
    pub owner_task: TaskId,
    pub rights: PortRights,
    pub timestamp: u64,
}

/// The Name Server state
pub struct NameServer {
    /// Port bindings by name
    bindings: Mutex<BTreeMap<String, NameBinding>>,
    /// Reverse lookup: port to names
    port_to_names: Mutex<BTreeMap<PortId, Vec<String>>>,
    /// Server port for receiving requests
    server_port: PortId,
    /// Server port object
    port: Arc<Port>,
    /// Task ID of the name server
    server_task: TaskId,
}

impl NameServer {
    /// Create a new Name Server
    pub fn new(server_task: TaskId) -> Self {
        // Allocate a well-known port for the name server
        let port = Port::new(server_task);
        let server_port = port.id();
        
        Self {
            bindings: Mutex::new(BTreeMap::new()),
            port_to_names: Mutex::new(BTreeMap::new()),
            server_port,
            port,
            server_task,
        }
    }
    
    /// Get the server port ID
    pub fn server_port(&self) -> PortId {
        self.server_port
    }

    /// Get a clone of the server's port Arc
    pub fn server_port_arc(&self) -> Arc<Port> {
        Arc::clone(&self.port)
    }
    
    /// Register a name binding
    pub fn register(&self, name: String, port_id: PortId, owner_task: TaskId) -> Result<(), &'static str> {
        let mut bindings = self.bindings.lock();
        let mut port_names = self.port_to_names.lock();
        
        // Check if name already exists
        if bindings.contains_key(&name) {
            return Err("Name already registered");
        }
        
        let binding = NameBinding {
            name: name.clone(),
            port_id,
            owner_task,
            rights: PortRights::default(),
            timestamp: crate::arch::current_timestamp(),
        };
        
        // Add to primary index
        bindings.insert(name.clone(), binding);
        
        // Add to reverse index
        port_names.entry(port_id)
            .or_insert_with(Vec::new)
            .push(name);
        
        Ok(())
    }
    
    /// Look up a name binding
    pub fn lookup(&self, name: &str) -> Option<PortId> {
        let bindings = self.bindings.lock();
        bindings.get(name).map(|binding| binding.port_id)
    }
    
    /// Unregister a name binding
    pub fn unregister(&self, name: &str, requesting_task: TaskId) -> Result<(), &'static str> {
        let mut bindings = self.bindings.lock();
        let mut port_names = self.port_to_names.lock();
        
        // Find the binding
        let binding = bindings.get(name).ok_or("Name not found")?;
        
        // Check permissions - only owner can unregister
        if binding.owner_task != requesting_task {
            return Err("Permission denied");
        }
        
        let port_id = binding.port_id;
        
        // Remove from primary index
        bindings.remove(name);
        
        // Remove from reverse index
        if let Some(names) = port_names.get_mut(&port_id) {
            names.retain(|n| n != name);
            if names.is_empty() {
                port_names.remove(&port_id);
            }
        }
        
        Ok(())
    }
    
    /// List all registered names (optionally filtered by prefix)
    pub fn list(&self, prefix: Option<&str>) -> Vec<String> {
        let bindings = self.bindings.lock();
        
        if let Some(prefix) = prefix {
            bindings.keys()
                .filter(|name| name.starts_with(prefix))
                .cloned()
                .collect()
        } else {
            bindings.keys().cloned().collect()
        }
    }
    
    /// Handle incoming message
    pub fn handle_message(&self, msg: Message) -> Option<Message> {
        // Use MIG-generated dispatch to handle message
        crate::mig::generated::name_server::dispatch(self, &msg)
    }
    
    /// Handle register request
    fn handle_register(&self, msg: Message, req: &NameServerRequest) -> Option<Message> {
        let data = msg.data();
        let req_size = core::mem::size_of::<NameServerRequest>();
        
        if data.len() < req_size + req.name_len as usize {
            let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
            return Some(self.create_error_reply(reply_to, -1));
        }
        
        // Extract name
        let name_bytes = &data[req_size..req_size + req.name_len as usize];
        let name = match core::str::from_utf8(name_bytes) {
            Ok(s) => String::from(s),
            Err(_) => {
                let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
                return Some(self.create_error_reply(reply_to, -1))
            },
        };
        
        // Get the port to register from payload after the name
        let port_id;
        let port_pos = req_size + req.name_len as usize;
        if data.len() >= port_pos + core::mem::size_of::<u64>() {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&data[port_pos..port_pos+8]);
            port_id = PortId(u64::from_le_bytes(buf));
        } else {
            let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
            return Some(self.create_error_reply(reply_to, -1));
        }
        let owner_task = TaskId(1); // Simplified - would get from message context
        
        let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
        match self.register(name, port_id, owner_task) {
            Ok(()) => Some(self.create_success_reply(reply_to, port_id)),
            Err(_) => Some(self.create_error_reply(reply_to, -17)), // EEXIST
        }
    }
    
    /// Handle lookup request
    fn handle_lookup(&self, msg: Message, req: &NameServerRequest) -> Option<Message> {
        let data = msg.data();
        let req_size = core::mem::size_of::<NameServerRequest>();
        
        if data.len() < req_size + req.name_len as usize {
            let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
            return Some(self.create_error_reply(reply_to, -1));
        }
        
        // Extract name
        let name_bytes = &data[req_size..req_size + req.name_len as usize];
        let name = match core::str::from_utf8(name_bytes) {
            Ok(s) => s,
            Err(_) => {
                let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
                return Some(self.create_error_reply(reply_to, -1))
            },
        };
        
        let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
        match self.lookup(name) {
            Some(port_id) => Some(self.create_success_reply(reply_to, port_id)),
            None => Some(self.create_error_reply(reply_to, -2)), // ENOENT
        }
    }
    
    /// Handle unregister request
    fn handle_unregister(&self, msg: Message, req: &NameServerRequest) -> Option<Message> {
        let data = msg.data();
        let req_size = core::mem::size_of::<NameServerRequest>();
        
        if data.len() < req_size + req.name_len as usize {
            let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
            return Some(self.create_error_reply(reply_to, -1));
        }
        
        // Extract name
        let name_bytes = &data[req_size..req_size + req.name_len as usize];
        let name = match core::str::from_utf8(name_bytes) {
            Ok(s) => s,
            Err(_) => {
                let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
                return Some(self.create_error_reply(reply_to, -1))
            },
        };
        
        let requesting_task = TaskId(1); // Simplified - would get from message context
        
        let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
        match self.unregister(name, requesting_task) {
            Ok(()) => Some(self.create_success_reply(reply_to, PortId(0))),
            Err(_) => Some(self.create_error_reply(reply_to, -13)), // EACCES
        }
    }
    
    /// Handle list request
    fn handle_list(&self, msg: Message, _req: &NameServerRequest) -> Option<Message> {
        let names = self.list(None);
        let mut reply_data = Vec::new();
        
        // Create reply header
        let reply = NameServerReply {
            result: 0,
            port_id: PortId(0),
            count: names.len() as u32,
        };
        
        reply_data.extend_from_slice(unsafe {
            core::slice::from_raw_parts(
                &reply as *const _ as *const u8,
                core::mem::size_of::<NameServerReply>()
            )
        });
        
        // Add names
        for name in names {
            let name_bytes = name.as_bytes();
            reply_data.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
            reply_data.extend_from_slice(name_bytes);
        }
        
        let reply_to = msg.header.local_port.unwrap_or(msg.remote_port());
        Some(Message::new_out_of_line(reply_to, reply_data))
    }
    
    /// Handle check-in request (Mach-style service registration)
    fn handle_checkin(&self, msg: Message, req: &NameServerRequest) -> Option<Message> {
        // Check-in is like register but with bootstrap semantics
        self.handle_register(msg, req)
    }
    
    /// Handle check-out request (Mach-style service lookup)
    fn handle_checkout(&self, msg: Message, req: &NameServerRequest) -> Option<Message> {
        // Check-out is like lookup but with bootstrap semantics
        self.handle_lookup(msg, req)
    }
    
    /// Create success reply
    fn create_success_reply(&self, remote_port: PortId, result_port: PortId) -> Message {
        let reply = NameServerReply {
            result: 0,
            port_id: result_port,
            count: 0,
        };
        
        let reply_data = unsafe {
            core::slice::from_raw_parts(
                &reply as *const _ as *const u8,
                core::mem::size_of::<NameServerReply>()
            )
        }.to_vec();
        
        Message::new_out_of_line(remote_port, reply_data)
    }
    
    /// Create error reply
    fn create_error_reply(&self, remote_port: PortId, error_code: i32) -> Message {
        let reply = NameServerReply {
            result: error_code,
            port_id: PortId(0),
            count: 0,
        };
        
        let reply_data = unsafe {
            core::slice::from_raw_parts(
                &reply as *const _ as *const u8,
                core::mem::size_of::<NameServerReply>()
            )
        }.to_vec();
        
        Message::new_out_of_line(remote_port, reply_data)
    }

    /// Poll one message from the server port and process it
    pub fn poll_once(&self) {
        if let Some(msg) = self.port.receive() {
            if let Some(reply) = self.handle_message(msg) {
                let _ = crate::port::send_message(reply.remote_port(), reply);
            }
        }
    }
}

/// Global Name Server instance
pub static mut NAME_SERVER: Option<NameServer> = None;

/// Initialize the Name Server
pub fn init() {
    let server_task = TaskId(2); // Name server gets task ID 2
    let name_server = NameServer::new(server_task);
    
    // Register the name server with itself
    let _ = name_server.register(
        "name_server".to_string(),
        name_server.server_port(),
        server_task
    );
    
    // Register with global server registry
    super::SERVER_REGISTRY.register_server("name_server", name_server.server_port());
    
    unsafe {
        NAME_SERVER = Some(name_server);
    }
    
    crate::println!("Name Server initialized on port {}", 100);
}

/// Get the Name Server instance
pub fn name_server() -> &'static NameServer {
    unsafe {
        (*core::ptr::addr_of!(NAME_SERVER)).as_ref().expect("Name Server not initialized")
    }
}

/// Process messages for the Name Server (called by scheduler)
pub fn process_messages() {
    let _ns = name_server();

    // In a real implementation, this would:
    // 1. Check for incoming messages on the server port
    // 2. Process each message
    // 3. Send replies
    //
    // For now, this is a placeholder that would be called by the scheduler
}

#[cfg(test)]
    mod tests {
        use super::*;
        use crate::mig::generated::name_server as mig_ns;
        use crate::message::MessageBody;

        #[repr(C)]
        struct Req {
            op: NameServerOp,
            name_len: u32,
    }

    #[test]
        fn mig_style_register_lookup_roundtrip() {
        let ns = NameServer::new(TaskId(10));
        let caller_port = PortId(123);
        let name = "svc";
        let req = Req{ op: NameServerOp::Register, name_len: name.len() as u32 };
        // Build OutOfLine message data
        let mut data = alloc::vec::Vec::new();
        let req_bytes = unsafe { core::slice::from_raw_parts(&req as *const _ as *const u8, core::mem::size_of::<Req>()) };
        data.extend_from_slice(req_bytes);
        data.extend_from_slice(name.as_bytes());
        // append port id to register
        data.extend_from_slice(&(caller_port.0 as u64).to_le_bytes());
        let msg = Message::new_out_of_line(caller_port, data);
        let reply = ns.handle_message(msg).expect("reply");
        let rep = unsafe { &*(reply.data().as_ptr() as *const NameServerReply) };
        assert_eq!(rep.result, 0);

        // Lookup
        let req = Req{ op: NameServerOp::Lookup, name_len: name.len() as u32 };
        let mut data = alloc::vec::Vec::new();
        let req_bytes = unsafe { core::slice::from_raw_parts(&req as *const _ as *const u8, core::mem::size_of::<Req>()) };
        data.extend_from_slice(req_bytes);
        data.extend_from_slice(name.as_bytes());
        let msg = Message::new_out_of_line(caller_port, data);
        let reply = ns.handle_message(msg).expect("reply");
        let rep = unsafe { &*(reply.data().as_ptr() as *const NameServerReply) };
        assert_eq!(rep.result, 0);
        }

        #[test]
        fn e2e_register_via_port_and_reply() {
            // Set up name server with real port
        let ns = NameServer::new(TaskId(10));
        // Client reply port
        let reply = Port::new(TaskId(99));
        // Build a register request message with local (reply) port set
        let name = "svc2";
        let target_port = PortId(500);
        let req = Req { op: NameServerOp::Register, name_len: name.len() as u32 };
        let mut data = alloc::vec::Vec::new();
        let req_bytes = unsafe { core::slice::from_raw_parts(&req as *const _ as *const u8, core::mem::size_of::<Req>()) };
        data.extend_from_slice(req_bytes);
        data.extend_from_slice(name.as_bytes());
        data.extend_from_slice(&(target_port.0 as u64).to_le_bytes());
        let msg = Message::new_out_of_line(ns.server_port(), data).with_reply_port(reply.id());
        // Send to server port and process one message
        assert!(ns.server_port_arc().send(msg).is_ok());
        ns.poll_once();
        // Receive reply on client reply port
        let rep = reply.receive().expect("reply");
            match rep.body {
                MessageBody::OutOfLine(bytes) => {
                    // First 4 bytes: result code (i32 LE)
                    let mut rc = [0u8;4];
                    rc.copy_from_slice(&bytes[0..4]);
                    let result = i32::from_le_bytes(rc);
                    assert_eq!(result, 0);
                }
                _ => panic!("expected OOL reply"),
            }
        }
    }
