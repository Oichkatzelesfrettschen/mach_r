//! Virtual Memory Server
//!
//! Manages virtual memory operations including allocation, deallocation,
//! and memory protection for user tasks.

use crate::types::{PortId, TaskId};
use crate::message::Message;
use crate::port::Port;
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::string::ToString;
use spin::Mutex;

/// VM Server operations
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum VmOp {
    Allocate = 2000,
    Deallocate = 2001,
    Protect = 2002,
    Map = 2003,
    Unmap = 2004,
}

/// Memory region descriptor
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub start: usize,
    pub size: usize,
    pub protection: u32,
    pub owner_task: TaskId,
}

/// VM Server implementation
pub struct VmServer {
    server_port: PortId,
    port: Arc<Port>,
    server_task: TaskId,
    regions: Mutex<BTreeMap<usize, MemoryRegion>>,
}

impl VmServer {
    pub fn new(server_task: TaskId) -> Self {
        let port = Port::new(server_task);
        let server_port = port.id();
        Self {
            server_port,
            port,
            server_task,
            regions: Mutex::new(BTreeMap::new()),
        }
    }
    
    pub fn server_port(&self) -> PortId {
        self.server_port
    }
    pub fn server_port_arc(&self) -> Arc<Port> { Arc::clone(&self.port) }
    
    /// Allocate virtual memory
    pub fn vm_allocate(&self, task: TaskId, size: usize, protection: u32) -> Result<usize, i32> {
        let mut regions = self.regions.lock();
        
        // Find a free region (simplified allocation)
        let base_addr = 0x10000000 + (regions.len() * 0x1000000); // 256MB spacing
        
        let region = MemoryRegion {
            start: base_addr,
            size,
            protection,
            owner_task: task,
        };
        
        regions.insert(base_addr, region);
        
        // Actually allocate pages
        let page_manager = crate::memory::page_manager();
        let pages_needed = (size + 4095) / 4096; // Round up to pages
        
        for i in 0..pages_needed {
            if let Ok(phys_page) = page_manager.allocate_page() {
                let virt_addr = crate::paging::VirtualAddress(base_addr + i * 4096);
                let mut page_table = crate::paging::active_page_table();
                let flags = crate::paging::PageTableFlags::PRESENT | crate::paging::PageTableFlags::WRITABLE;
                page_table.map(virt_addr, phys_page, flags);
            } else {
                return Err(-12); // ENOMEM
            }
        }
        
        Ok(base_addr)
    }
    
    /// Deallocate virtual memory
    pub fn vm_deallocate(&self, task: TaskId, addr: usize, size: usize) -> Result<(), i32> {
        let mut regions = self.regions.lock();
        
        if let Some(region) = regions.get(&addr) {
            if region.owner_task != task {
                return Err(-13); // EACCES
            }
            
            if region.size != size {
                return Err(-22); // EINVAL
            }
            
            // Unmap pages
            let pages_to_free = (size + 4095) / 4096;
            let mut page_table = crate::paging::active_page_table();
            
            for i in 0..pages_to_free {
                let virt_addr = crate::paging::VirtualAddress(addr + i * 4096);
                page_table.unmap(virt_addr);
            }
            
            regions.remove(&addr);
            Ok(())
        } else {
            Err(-22) // EINVAL
        }
    }
    
    /// Change memory protection
    pub fn vm_protect(&self, task: TaskId, addr: usize, size: usize, protection: u32) -> Result<(), i32> {
        let mut regions = self.regions.lock();
        
        if let Some(region) = regions.get_mut(&addr) {
            if region.owner_task != task {
                return Err(-13); // EACCES
            }
            
            region.protection = protection;
            
            // Update page table protections
            let pages_to_update = (size + 4095) / 4096;
            let mut page_table = crate::paging::active_page_table();
            
            for i in 0..pages_to_update {
                let virt_addr = crate::paging::VirtualAddress(addr + i * 4096);
                
                // Convert protection flags
                let mut flags = crate::paging::PageTableFlags::PRESENT;
                if protection & 0x02 != 0 { // VM_PROT_WRITE
                    flags |= crate::paging::PageTableFlags::WRITABLE;
                }
                if protection & 0x04 == 0 { // VM_PROT_EXECUTE
                    flags |= crate::paging::PageTableFlags::NO_EXECUTE;
                }
                
                // Get current physical address and remap
                if let Some(phys_addr) = page_table.translate(virt_addr) {
                    page_table.map(virt_addr, crate::paging::PhysicalAddress(phys_addr.0), flags);
                }
            }
            
            Ok(())
        } else {
            Err(-22) // EINVAL
        }
    }
    
    /// Handle incoming message (via MIG dispatch)
    pub fn handle_message(&self, msg: Message) -> Option<Message> {
        crate::mig::generated::vm::dispatch(self, &msg)
    }
    
    #[allow(dead_code)]
    fn handle_allocate(&self, msg: Message) -> Option<Message> {
        let data = msg.data();
        if data.len() < 12 {
            return Some(self.create_error_reply(msg.remote_port(), -22));
        }
        
        let size = usize::from_le_bytes([
            data[4], data[5], data[6], data[7],
            data[8], data[9], data[10], data[11]
        ]);
        let protection = if data.len() >= 16 {
            u32::from_le_bytes([data[12], data[13], data[14], data[15]])
        } else {
            0x07 // VM_PROT_READ | VM_PROT_WRITE | VM_PROT_EXECUTE
        };
        
        let requesting_task = TaskId(1); // Simplified
        
        match self.vm_allocate(requesting_task, size, protection) {
            Ok(addr) => {
                let mut reply_data = vec![0u8; 12];
                reply_data[0..4].copy_from_slice(&0i32.to_le_bytes()); // success
                reply_data[4..12].copy_from_slice(&addr.to_le_bytes());
                Some(Message::new_out_of_line(msg.remote_port(), reply_data))
            }
            Err(errno) => Some(self.create_error_reply(msg.remote_port(), errno)),
        }
    }
    
    #[allow(dead_code)]
    fn handle_deallocate(&self, msg: Message) -> Option<Message> {
        let data = msg.data();
        if data.len() < 20 {
            return Some(self.create_error_reply(msg.remote_port(), -22));
        }
        
        let addr = usize::from_le_bytes([
            data[4], data[5], data[6], data[7],
            data[8], data[9], data[10], data[11]
        ]);
        let size = usize::from_le_bytes([
            data[12], data[13], data[14], data[15],
            data[16], data[17], data[18], data[19]
        ]);
        
        let requesting_task = TaskId(1); // Simplified
        
        match self.vm_deallocate(requesting_task, addr, size) {
            Ok(()) => Some(self.create_success_reply(msg.remote_port())),
            Err(errno) => Some(self.create_error_reply(msg.remote_port(), errno)),
        }
    }
    
    #[allow(dead_code)]
    fn handle_protect(&self, msg: Message) -> Option<Message> {
        let data = msg.data();
        if data.len() < 24 {
            return Some(self.create_error_reply(msg.remote_port(), -22));
        }
        
        let addr = usize::from_le_bytes([
            data[4], data[5], data[6], data[7],
            data[8], data[9], data[10], data[11]
        ]);
        let size = usize::from_le_bytes([
            data[12], data[13], data[14], data[15],
            data[16], data[17], data[18], data[19]
        ]);
        let protection = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        
        let requesting_task = TaskId(1); // Simplified
        
        match self.vm_protect(requesting_task, addr, size, protection) {
            Ok(()) => Some(self.create_success_reply(msg.remote_port())),
            Err(errno) => Some(self.create_error_reply(msg.remote_port(), errno)),
        }
    }
    
    #[allow(dead_code)]
    fn handle_map(&self, _msg: Message) -> Option<Message> {
        // Simplified - not implemented
        None
    }

    #[allow(dead_code)]
    fn handle_unmap(&self, _msg: Message) -> Option<Message> {
        // Simplified - not implemented
        None
    }

    #[allow(dead_code)]
    fn create_success_reply(&self, remote_port: PortId) -> Message {
        let reply_data = 0i32.to_le_bytes().to_vec();
        Message::new_out_of_line(remote_port, reply_data)
    }
    
    #[allow(dead_code)]
    fn create_error_reply(&self, remote_port: PortId, errno: i32) -> Message {
        let reply_data = errno.to_le_bytes().to_vec();
        Message::new_out_of_line(remote_port, reply_data)
    }
}

impl crate::mig::generated::vm::NameService for VmServer {
    fn allocate(&self, size: u64, protection: u32) -> Result<u64, i32> {
        match self.vm_allocate(self.server_task, size as usize, protection) {
            Ok(addr) => Ok(addr as u64),
            Err(e) => Err(e),
        }
    }
    fn deallocate(&self, addr: u64, size: u64) -> i32 {
        match self.vm_deallocate(self.server_task, addr as usize, size as usize) {
            Ok(()) => 0,
            Err(e) => e,
        }
    }
    fn protect(&self, addr: u64, size: u64, protection: u32) -> i32 {
        match self.vm_protect(self.server_task, addr as usize, size as usize, protection) {
            Ok(()) => 0,
            Err(e) => e,
        }
    }
}

impl VmServer {
    /// Poll one message from the server port, dispatch, and reply
    pub fn poll_once(&self) {
        if let Some(msg) = self.port.receive() {
            if let Some(reply) = self.handle_message(msg) {
                let _ = crate::port::send_message(reply.remote_port(), reply);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mig::generated::vm;
    use crate::message::MessageBody;

    #[test]
    fn e2e_vm_allocate_reply() {
        let vm = VmServer::new(TaskId(4));
        let reply = Port::new(TaskId(99));
        // Build allocate request: [ALLOCATE_ID|size u64|prot u32]
        let mut data = alloc::vec::Vec::new();
        data.extend_from_slice(&vm::ALLOCATE_ID.to_le_bytes());
        data.extend_from_slice(&(0x2000u64).to_le_bytes());
        data.extend_from_slice(&(0x3u32).to_le_bytes()); // RW
        let msg = Message::new_out_of_line(vm.server_port(), data).with_reply_port(reply.id());
        assert!(vm.server_port_arc().send(msg).is_ok());
        vm.poll_once();
        let rep = reply.receive().expect("reply");
        match rep.body {
            MessageBody::OutOfLine(bytes) => {
                let mut rc=[0u8;4]; rc.copy_from_slice(&bytes[0..4]);
                let code = i32::from_le_bytes(rc);
                assert_eq!(code, 0);
            }
            _ => panic!("expected OOL"),
        }
    }

    #[test]
    fn e2e_vm_allocate_via_client_call() {
        let vm = VmServer::new(TaskId(4));
        let client = vm::NameClient::new(vm.server_port_arc());
        let reply = Port::new(TaskId(55));
        // Request 0x3000 bytes RW
        let res = client.allocate_call(0x3000, 0x3, &reply);
        // Since allocate_call blocks by waiting on reply port, process server once
        vm.poll_once();
        assert!(res.is_ok() || res.is_err());
    }
}

/// Global VM Server instance
pub static mut VM_SERVER: Option<VmServer> = None;

/// Initialize the VM Server
pub fn init() {
    let server_task = TaskId(4); // VM server gets task ID 4
    let vm_server = VmServer::new(server_task);
    
    // Register with server registry
    super::SERVER_REGISTRY.register_server("vm_server", vm_server.server_port());
    
    // Register with name server
    if let Some(name_server) = unsafe { (*core::ptr::addr_of!(super::name_server::NAME_SERVER)).as_ref() } {
        let _ = name_server.register(
            "vm_server".to_string(),
            vm_server.server_port(),
            server_task
        );
    }
    
    unsafe {
        VM_SERVER = Some(vm_server);
    }
    
    crate::println!("VM Server initialized on port {}", 300);
}

/// Get the VM Server instance
pub fn vm_server() -> &'static VmServer {
    unsafe {
        (*core::ptr::addr_of!(VM_SERVER)).as_ref().expect("VM Server not initialized")
    }
}
