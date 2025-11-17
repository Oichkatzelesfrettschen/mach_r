//! External pager implementation for Mach_R
//!
//! The external pager is a key Mach concept that allows user-space
//! processes to provide backing store for memory objects. This enables
//! features like memory-mapped files, copy-on-write, and distributed
//! shared memory.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use spin::Mutex;
use crate::types::TaskId;
use crate::port::Port;
use crate::message::Message;
use crate::paging::{VirtualAddress, PhysicalAddress, PageTableFlags, PAGE_SIZE};

/// Memory object - represents a region of pageable memory
pub struct MemoryObject {
    /// Unique identifier
    id: MemoryObjectId,
    /// Size in bytes
    size: usize,
    /// Protection flags
    protection: Protection,
    /// Pager port for this object
    pager_port: Option<Arc<Port>>,
    /// Control port for this object
    control_port: Option<Arc<Port>>,
    /// Shadow object for copy-on-write
    shadow: Option<Arc<MemoryObject>>,
    /// Reference count
    refs: AtomicUsize,
    /// Is this object temporary?
    temporary: bool,
    /// Can this object be copied?
    can_copy: bool,
}

/// Memory object identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemoryObjectId(u64);

impl MemoryObjectId {
    /// Create a new memory object ID
    pub fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(1);
        MemoryObjectId(COUNTER.fetch_add(1, Ordering::Relaxed) as u64)
    }
}

/// Memory protection flags
#[derive(Debug, Clone, Copy)]
pub struct Protection {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Protection {
    /// No access
    pub const NONE: Self = Protection {
        read: false,
        write: false,
        execute: false,
    };
    
    /// Read-only
    pub const READ: Self = Protection {
        read: true,
        write: false,
        execute: false,
    };
    
    /// Read-write
    pub const READ_WRITE: Self = Protection {
        read: true,
        write: true,
        execute: false,
    };
    
    /// Read-execute
    pub const READ_EXECUTE: Self = Protection {
        read: true,
        write: false,
        execute: true,
    };
    
    /// All permissions
    pub const ALL: Self = Protection {
        read: true,
        write: true,
        execute: true,
    };
    
    /// Convert to page table flags
    pub fn to_page_flags(&self) -> PageTableFlags {
        let mut flags = PageTableFlags::PRESENT;
        
        if self.write {
            flags = flags.union(PageTableFlags::WRITABLE);
        }
        
        if !self.execute {
            flags = flags.union(PageTableFlags::NO_EXECUTE);
        }
        
        flags
    }
}

impl MemoryObject {
    /// Create a new memory object
    pub fn new(size: usize, protection: Protection) -> Arc<Self> {
        Arc::new(MemoryObject {
            id: MemoryObjectId::new(),
            size,
            protection,
            pager_port: None,
            control_port: None,
            shadow: None,
            refs: AtomicUsize::new(1),
            temporary: false,
            can_copy: true,
        })
    }
    
    /// Create a memory object backed by an external pager
    pub fn with_pager(size: usize, protection: Protection, pager: Arc<Port>) -> Arc<Self> {
        Arc::new(MemoryObject {
            id: MemoryObjectId::new(),
            size,
            protection,
            pager_port: Some(pager),
            control_port: None,
            shadow: None,
            refs: AtomicUsize::new(1),
            temporary: false,
            can_copy: true,
        })
    }
    
    /// Create a shadow object for copy-on-write
    pub fn create_shadow(&self) -> Arc<Self> {
        Arc::new(MemoryObject {
            id: MemoryObjectId::new(),
            size: self.size,
            protection: self.protection,
            pager_port: None,
            control_port: None,
            shadow: None, // Will be set by caller
            refs: AtomicUsize::new(1),
            temporary: true,
            can_copy: false,
        })
    }
    
    /// Increment reference count
    pub fn reference(&self) {
        self.refs.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Decrement reference count
    pub fn dereference(&self) -> usize {
        self.refs.fetch_sub(1, Ordering::Relaxed) - 1
    }
}

/// External pager interface
pub struct ExternalPager {
    /// Pager port for receiving requests
    port: Arc<Port>,
    /// Task that runs the pager
    task: TaskId,
    /// Memory objects managed by this pager
    objects: Mutex<Vec<Arc<MemoryObject>>>,
    /// Is pager active?
    active: AtomicBool,
}

impl ExternalPager {
    /// Create a new external pager
    pub fn new(task: TaskId) -> Arc<Self> {
        Arc::new(ExternalPager {
            port: Port::new(task),
            task,
            objects: Mutex::new(Vec::new()),
            active: AtomicBool::new(true),
        })
    }

    /// Expose pager's receive port (for object association)
    pub fn pager_port(&self) -> Arc<Port> {
        self.port.clone()
    }
    
    /// Register a memory object with this pager
    pub fn register_object(&self, obj: Arc<MemoryObject>) {
        let mut objects = self.objects.lock();
        objects.push(obj);
    }
    
    /// Handle a page fault for a memory object
    pub fn handle_fault(&self, obj_id: MemoryObjectId, offset: usize) -> Result<PhysicalAddress, PagerError> {
        // Find the memory object
        let objects = self.objects.lock();
        let obj = objects.iter()
            .find(|o| o.id == obj_id)
            .ok_or(PagerError::ObjectNotFound)?;
        
        // Check bounds
        if offset >= obj.size {
            return Err(PagerError::InvalidOffset);
        }
        
        // Send page request to external pager
        if let Some(ref pager_port) = obj.pager_port {
            let request = PageRequest {
                object_id: obj_id,
                offset,
                size: PAGE_SIZE,
                protection: obj.protection,
            };
            
            // Create request message
            let msg = self.create_page_request_message(request)?;
            pager_port.send(msg).map_err(|_| PagerError::PagerDead)?;
            
            // Wait for response (simplified - should be async)
            if let Some(reply) = pager_port.receive() {
                // Parse reply and get physical page
                return self.parse_page_reply(reply);
            }
        }
        
        // No pager, allocate zero page
        Ok(self.allocate_zero_page())
    }
    
    /// Create a page request message
    fn create_page_request_message(&self, request: PageRequest) -> Result<Message, PagerError> {
        // Serialize request into message
        // In real implementation, would properly encode the request
        let data = alloc::format!("PAGE_REQUEST:{}:{}", request.object_id.0, request.offset);
        Message::new_inline(self.port.id(), data.as_bytes())
            .map_err(|_| PagerError::MessageCreationFailed)
    }
    
    /// Parse a page reply message
    fn parse_page_reply(&self, msg: Message) -> Result<PhysicalAddress, PagerError> {
        // In real implementation, would extract physical address from message
        // For now, return a dummy address
        Ok(PhysicalAddress::new(0x10000))
    }
    
    /// Allocate a zero-filled page
    fn allocate_zero_page(&self) -> PhysicalAddress {
        // In real implementation, would allocate from physical memory manager
        PhysicalAddress::new(0x20000)
    }
}

/// Page request from kernel to pager
#[derive(Debug, Clone)]
pub struct PageRequest {
    /// Memory object ID
    pub object_id: MemoryObjectId,
    /// Offset within object
    pub offset: usize,
    /// Size of request (usually PAGE_SIZE)
    pub size: usize,
    /// Required protection
    pub protection: Protection,
}

/// Pager errors
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PagerError {
    /// Memory object not found
    ObjectNotFound,
    /// Invalid offset in object
    InvalidOffset,
    /// Pager port is dead
    PagerDead,
    /// Failed to create message
    MessageCreationFailed,
    /// Pager refused request
    RequestDenied,
}

/// VM map entry - represents a mapping in a task's address space
pub struct VmMapEntry {
    /// Start address
    start: VirtualAddress,
    /// End address
    end: VirtualAddress,
    /// Memory object backing this region
    object: Arc<MemoryObject>,
    /// Offset into memory object
    offset: usize,
    /// Protection for this mapping
    protection: Protection,
    /// Maximum protection allowed
    max_protection: Protection,
    /// Inheritance behavior
    inheritance: Inheritance,
    /// Is this region wired?
    wired: bool,
    /// Copy-on-write?
    copy_on_write: bool,
}

/// Inheritance behavior for VM regions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Inheritance {
    /// Share the region with child
    Share,
    /// Copy the region to child
    Copy,
    /// Don't inherit
    None,
}

impl VmMapEntry {
    /// Create a new VM map entry
    pub fn new(
        start: VirtualAddress,
        end: VirtualAddress,
        object: Arc<MemoryObject>,
        offset: usize,
        protection: Protection,
    ) -> Self {
        VmMapEntry {
            start,
            end,
            object,
            offset,
            protection,
            max_protection: Protection::ALL,
            inheritance: Inheritance::Copy,
            wired: false,
            copy_on_write: false,
        }
    }
    
    /// Get the size of this entry
    pub fn size(&self) -> usize {
        self.end.0 - self.start.0
    }
    
    /// Check if an address is within this entry
    pub fn contains(&self, addr: VirtualAddress) -> bool {
        addr.0 >= self.start.0 && addr.0 < self.end.0
    }
    
    /// Handle a fault in this region
    pub fn handle_fault(&self, addr: VirtualAddress, write: bool) -> Result<PhysicalAddress, PagerError> {
        // Check protection
        if write && !self.protection.write {
            if self.copy_on_write {
                // Handle copy-on-write
                return self.handle_cow_fault(addr);
            }
            return Err(PagerError::RequestDenied);
        }
        
        // Calculate offset into memory object
        let region_offset = addr.0 - self.start.0;
        let object_offset = self.offset + region_offset;
        
        // Request page from memory object's pager
        // In real implementation, would go through the pager
        Ok(PhysicalAddress::new(0x30000))
    }
    
    /// Handle copy-on-write fault
    fn handle_cow_fault(&self, _addr: VirtualAddress) -> Result<PhysicalAddress, PagerError> {
        // In real implementation:
        // 1. Allocate new physical page
        // 2. Copy contents from original page
        // 3. Update mapping to new page
        // 4. Remove COW flag
        Ok(PhysicalAddress::new(0x40000))
    }
}

/// VM map - manages address space for a task
pub struct VmMap {
    /// Map entries
    entries: Mutex<Vec<VmMapEntry>>,
    /// Minimum address
    min_address: VirtualAddress,
    /// Maximum address
    max_address: VirtualAddress,
    /// Next available address for allocation
    next_address: Mutex<VirtualAddress>,
}

impl VmMap {
    /// Create a new VM map
    pub fn new() -> Self {
        VmMap {
            entries: Mutex::new(Vec::new()),
            min_address: VirtualAddress::new(0x1000),
            max_address: VirtualAddress::new(0x7FFF_FFFF_F000),
            next_address: Mutex::new(VirtualAddress::new(0x1000)),
        }
    }
    
    /// Allocate a region in the address space
    pub fn allocate(
        &self,
        size: usize,
        object: Arc<MemoryObject>,
        protection: Protection,
    ) -> Result<VirtualAddress, PagerError> {
        let mut next = self.next_address.lock();
        let mut entries = self.entries.lock();
        
        // Align size to page boundary
        let aligned_size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        
        // Find free space
        let start = *next;
        let end = VirtualAddress::new(start.0 + aligned_size);
        
        // Check bounds
        if end.0 > self.max_address.0 {
            return Err(PagerError::InvalidOffset);
        }
        
        // Create entry
        let entry = VmMapEntry::new(start, end, object, 0, protection);
        entries.push(entry);
        
        // Update next address
        *next = end;
        
        Ok(start)
    }
    
    /// Find entry containing an address
    pub fn find_entry(&self, addr: VirtualAddress) -> Option<VmMapEntry> {
        let entries = self.entries.lock();
        entries.iter()
            .find(|e| e.contains(addr))
            .cloned()
    }
    
    /// Handle a page fault
    pub fn handle_fault(&self, addr: VirtualAddress, write: bool) -> Result<PhysicalAddress, PagerError> {
        let entries = self.entries.lock();
        let entry = entries.iter()
            .find(|e| e.contains(addr))
            .ok_or(PagerError::ObjectNotFound)?;
        
        entry.handle_fault(addr, write)
    }
}

// Implement Clone for VmMapEntry to fix the compilation error
impl Clone for VmMapEntry {
    fn clone(&self) -> Self {
        VmMapEntry {
            start: self.start,
            end: self.end,
            object: self.object.clone(),
            offset: self.offset,
            protection: self.protection,
            max_protection: self.max_protection,
            inheritance: self.inheritance,
            wired: self.wired,
            copy_on_write: self.copy_on_write,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_object_creation() {
        let obj = MemoryObject::new(4096, Protection::READ_WRITE);
        assert_eq!(obj.size, 4096);
        assert!(obj.protection.read);
        assert!(obj.protection.write);
    }
    
    #[test]
    fn test_protection_flags() {
        let prot = Protection::READ_EXECUTE;
        let flags = prot.to_page_flags();
        assert!(flags.contains(PageTableFlags::PRESENT));
        assert!(flags.contains(PageTableFlags::NO_EXECUTE));
    }
    
    #[test]
    fn test_vm_map_entry() {
        let obj = MemoryObject::new(8192, Protection::READ);
        let entry = VmMapEntry::new(
            VirtualAddress::new(0x1000),
            VirtualAddress::new(0x3000),
            obj,
            0,
            Protection::READ,
        );
        
        assert_eq!(entry.size(), 0x2000);
        assert!(entry.contains(VirtualAddress::new(0x1500)));
        assert!(!entry.contains(VirtualAddress::new(0x3000)));
    }
    
    #[test]
    fn test_vm_map_allocation() {
        let map = VmMap::new();
        let obj = MemoryObject::new(4096, Protection::READ_WRITE);
        
        let addr = map.allocate(4096, obj, Protection::READ_WRITE);
        assert!(addr.is_ok());
        
        let start = addr.unwrap();
        assert_eq!(start.0, 0x1000);
    }

    #[test]
    fn test_pager_page_in_mock() {
        let pager = ExternalPager::new(TaskId(10));
        let obj = MemoryObject::with_pager(4096, Protection::READ, pager.pager_port());
        pager.register_object(obj.clone());
        // Attempt to handle a fault on first page
        let pa = pager.handle_fault(obj.id, 0);
        assert!(pa.is_ok());
    }
}
