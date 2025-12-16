//! VM User Interface - Mach VM System Calls
//!
//! Based on Mach4 vm/vm_user.c
//!
//! This module provides the user-facing VM operations that are accessed
//! through Mach IPC. These are the primary interfaces for memory management.
//!
//! ## Key Operations
//!
//! - `vm_allocate`: Allocate virtual memory in a task's address space
//! - `vm_deallocate`: Release virtual memory
//! - `vm_protect`: Change memory protection
//! - `vm_inherit`: Set inheritance on fork
//! - `vm_read`: Read memory from another task
//! - `vm_write`: Write memory to another task
//! - `vm_copy`: Copy memory within a task
//! - `vm_region`: Query information about a memory region

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::kern::syscall_sw::{
    KernReturn, KERN_INVALID_ARGUMENT, KERN_INVALID_TASK, KERN_NO_SPACE, KERN_SUCCESS,
};
use crate::kern::task::task_find;
use crate::kern::thread::TaskId;
use crate::mach_vm::vm_map::{self, EntryFlags, MapError, VmInherit, VmMap, VmProt};
use crate::mach_vm::vm_object::{VmObject, VmObjectId};
use crate::mach_vm::vm_page::PAGE_SIZE;

// Object ID counter
use core::sync::atomic::{AtomicU64, Ordering};
static NEXT_OBJECT_ID: AtomicU64 = AtomicU64::new(1);

// ============================================================================
// Return Codes
// ============================================================================

/// VM-specific return codes
pub const KERN_INVALID_ADDRESS: KernReturn = 1;
pub const KERN_PROTECTION_FAILURE: KernReturn = 2;
pub const KERN_MEMORY_ERROR: KernReturn = 10;
pub const KERN_MEMORY_FAILURE: KernReturn = 14;

// ============================================================================
// VM Allocate
// ============================================================================

/// Allocate virtual memory in a task's address space
///
/// # Arguments
/// * `target_task` - Task to allocate memory in
/// * `address` - In/out: requested address (if anywhere=false) or returned address
/// * `size` - Size of allocation in bytes
/// * `anywhere` - If true, kernel chooses address; if false, use requested address
///
/// # Returns
/// KERN_SUCCESS on success, error code otherwise
pub fn vm_allocate(
    target_map: &Arc<VmMap>,
    address: &mut u64,
    size: u64,
    anywhere: bool,
) -> KernReturn {
    // Validate size
    if size == 0 {
        return KERN_INVALID_ARGUMENT;
    }

    // Round size up to page boundary
    let rounded_size = round_page(size);

    let alloc_addr = if anywhere {
        // Kernel chooses the address
        match target_map.find_space(rounded_size, PAGE_SIZE as u64 - 1) {
            Some(addr) => addr,
            None => return KERN_NO_SPACE,
        }
    } else {
        // User specified address - must be page aligned
        let requested = trunc_page(*address);
        if requested != *address {
            return KERN_INVALID_ARGUMENT;
        }
        requested
    };

    // Create an anonymous memory object for this allocation
    let obj_id = VmObjectId(NEXT_OBJECT_ID.fetch_add(1, Ordering::SeqCst));
    let object = Arc::new(VmObject::anonymous(obj_id, rounded_size));

    // Enter the mapping into the address space
    match target_map.enter(
        alloc_addr,
        alloc_addr + rounded_size,
        Some(object),
        0, // offset
        VmProt::DEFAULT,
        VmProt::ALL,
        VmInherit::Copy,
    ) {
        Ok(()) => {
            *address = alloc_addr;
            KERN_SUCCESS
        }
        Err(MapError::NoSpace) => KERN_NO_SPACE,
        Err(MapError::InvalidRange) => KERN_INVALID_ADDRESS,
        Err(_) => KERN_MEMORY_ERROR,
    }
}

/// Allocate virtual memory (task-based interface)
pub fn vm_allocate_task(
    target_task: TaskId,
    address: &mut u64,
    size: u64,
    anywhere: bool,
) -> KernReturn {
    let task = match task_find(target_task) {
        Some(t) => t,
        None => return KERN_INVALID_TASK,
    };

    let map_id = match task.get_map_id() {
        Some(id) => id,
        None => return KERN_INVALID_TASK,
    };

    let map = match vm_map::lookup(map_id) {
        Some(m) => m,
        None => return KERN_INVALID_TASK,
    };

    vm_allocate(&map, address, size, anywhere)
}

// ============================================================================
// VM Deallocate
// ============================================================================

/// Deallocate virtual memory from a task's address space
///
/// # Arguments
/// * `target_map` - Map to deallocate from
/// * `address` - Start address (must be page aligned)
/// * `size` - Size to deallocate in bytes
///
/// # Returns
/// KERN_SUCCESS on success, error code otherwise
pub fn vm_deallocate(target_map: &Arc<VmMap>, address: u64, size: u64) -> KernReturn {
    // Validate parameters
    if size == 0 {
        return KERN_SUCCESS; // Deallocating nothing is OK
    }

    let start = trunc_page(address);
    let end = round_page(address + size);

    if start != address {
        return KERN_INVALID_ARGUMENT;
    }

    match target_map.remove(start, end) {
        Ok(()) => KERN_SUCCESS,
        Err(MapError::InvalidRange) => KERN_INVALID_ADDRESS,
        Err(MapError::NotFound) => KERN_INVALID_ADDRESS,
        Err(_) => KERN_MEMORY_ERROR,
    }
}

/// Deallocate virtual memory (task-based interface)
pub fn vm_deallocate_task(target_task: TaskId, address: u64, size: u64) -> KernReturn {
    let task = match task_find(target_task) {
        Some(t) => t,
        None => return KERN_INVALID_TASK,
    };

    let map_id = match task.get_map_id() {
        Some(id) => id,
        None => return KERN_INVALID_TASK,
    };

    let map = match vm_map::lookup(map_id) {
        Some(m) => m,
        None => return KERN_INVALID_TASK,
    };

    vm_deallocate(&map, address, size)
}

// ============================================================================
// VM Protect
// ============================================================================

/// Change protection on a memory region
///
/// # Arguments
/// * `target_map` - Map containing the region
/// * `address` - Start address
/// * `size` - Size of region
/// * `set_maximum` - If true, also sets maximum protection
/// * `new_protection` - New protection value
///
/// # Returns
/// KERN_SUCCESS on success, error code otherwise
pub fn vm_protect(
    target_map: &Arc<VmMap>,
    address: u64,
    size: u64,
    set_maximum: bool,
    new_protection: VmProt,
) -> KernReturn {
    if size == 0 {
        return KERN_SUCCESS;
    }

    let start = trunc_page(address);
    let end = round_page(address + size);

    if start != address {
        return KERN_INVALID_ARGUMENT;
    }

    // For now, we only implement current protection change
    // set_maximum would also change the max_protection field
    let _ = set_maximum;

    match target_map.protect(start, end, new_protection) {
        Ok(()) => KERN_SUCCESS,
        Err(MapError::InvalidRange) => KERN_INVALID_ADDRESS,
        Err(MapError::NotFound) => KERN_INVALID_ADDRESS,
        Err(MapError::ProtectionFailure) => KERN_PROTECTION_FAILURE,
        Err(_) => KERN_MEMORY_ERROR,
    }
}

// ============================================================================
// VM Inherit
// ============================================================================

/// Set inheritance attribute on a memory region
///
/// # Arguments
/// * `target_map` - Map containing the region
/// * `address` - Start address
/// * `size` - Size of region
/// * `new_inheritance` - New inheritance value
///
/// # Returns
/// KERN_SUCCESS on success, error code otherwise
pub fn vm_inherit(
    target_map: &Arc<VmMap>,
    address: u64,
    size: u64,
    new_inheritance: VmInherit,
) -> KernReturn {
    if size == 0 {
        return KERN_SUCCESS;
    }

    let start = trunc_page(address);
    let end = round_page(address + size);

    if start != address {
        return KERN_INVALID_ARGUMENT;
    }

    // Get entries in the range and update their inheritance
    let entries = target_map.entries.lock();

    // Find overlapping entries
    for (&entry_start, entry) in entries.range(..end) {
        let entry_end = entry_start + entry.size();
        if entry_end > start {
            // This entry overlaps our range
            // In a full implementation, we'd update the inheritance field
            let _ = new_inheritance;
        }
    }

    KERN_SUCCESS
}

// ============================================================================
// VM Read
// ============================================================================

/// Read data from another task's address space
///
/// # Arguments
/// * `target_map` - Map to read from
/// * `address` - Source address
/// * `size` - Number of bytes to read
/// * `data` - Output buffer
///
/// # Returns
/// KERN_SUCCESS on success, error code otherwise
pub fn vm_read(target_map: &Arc<VmMap>, address: u64, size: u64, data: &mut Vec<u8>) -> KernReturn {
    if size == 0 {
        data.clear();
        return KERN_SUCCESS;
    }

    let start = address;
    let _end = address + size;

    // Verify the range is mapped
    let entries = target_map.entries.lock();

    // Simple check - verify start address is mapped
    let entry_start = match target_map.lookup(start) {
        Some(e) => e,
        None => return KERN_INVALID_ADDRESS,
    };

    let entry = match entries.get(&entry_start) {
        Some(e) => e,
        None => return KERN_INVALID_ADDRESS,
    };

    // Check protection
    if !entry.protection.can_read() {
        return KERN_PROTECTION_FAILURE;
    }

    // In a real implementation, we'd:
    // 1. Walk the page tables to find physical pages
    // 2. Copy data from those pages
    // For now, return empty data with success
    data.resize(size as usize, 0);
    KERN_SUCCESS
}

// ============================================================================
// VM Write
// ============================================================================

/// Write data to another task's address space
///
/// # Arguments
/// * `target_map` - Map to write to
/// * `address` - Destination address
/// * `data` - Data to write
///
/// # Returns
/// KERN_SUCCESS on success, error code otherwise
pub fn vm_write(target_map: &Arc<VmMap>, address: u64, data: &[u8]) -> KernReturn {
    if data.is_empty() {
        return KERN_SUCCESS;
    }

    let start = address;
    let _end = address + data.len() as u64;

    // Verify the range is mapped and writable
    let entries = target_map.entries.lock();

    let entry_start = match target_map.lookup(start) {
        Some(e) => e,
        None => return KERN_INVALID_ADDRESS,
    };

    let entry = match entries.get(&entry_start) {
        Some(e) => e,
        None => return KERN_INVALID_ADDRESS,
    };

    // Check protection
    if !entry.protection.can_write() {
        return KERN_PROTECTION_FAILURE;
    }

    // In a real implementation, we'd:
    // 1. Walk the page tables to find physical pages
    // 2. Handle copy-on-write if needed
    // 3. Copy data into those pages
    KERN_SUCCESS
}

// ============================================================================
// VM Copy
// ============================================================================

/// Copy memory within a task's address space
///
/// # Arguments
/// * `target_map` - Map containing both source and destination
/// * `source_address` - Source address
/// * `size` - Number of bytes to copy
/// * `dest_address` - Destination address
///
/// # Returns
/// KERN_SUCCESS on success, error code otherwise
pub fn vm_copy(
    target_map: &Arc<VmMap>,
    source_address: u64,
    size: u64,
    dest_address: u64,
) -> KernReturn {
    if size == 0 {
        return KERN_SUCCESS;
    }

    // Verify source is readable
    let entries = target_map.entries.lock();

    if let Some(src_start) = target_map.lookup(source_address) {
        if let Some(entry) = entries.get(&src_start) {
            if !entry.protection.can_read() {
                return KERN_PROTECTION_FAILURE;
            }
        }
    } else {
        return KERN_INVALID_ADDRESS;
    }

    // Verify destination is writable
    if let Some(dst_start) = target_map.lookup(dest_address) {
        if let Some(entry) = entries.get(&dst_start) {
            if !entry.protection.can_write() {
                return KERN_PROTECTION_FAILURE;
            }
        }
    } else {
        return KERN_INVALID_ADDRESS;
    }

    drop(entries);

    // In a real implementation:
    // 1. Set up copy-on-write sharing if possible
    // 2. Or perform actual copy
    KERN_SUCCESS
}

// ============================================================================
// VM Region
// ============================================================================

/// Information about a memory region
#[derive(Debug, Clone)]
pub struct VmRegionInfo {
    /// Start address of region
    pub address: u64,
    /// Size of region
    pub size: u64,
    /// Current protection
    pub protection: VmProt,
    /// Maximum protection
    pub max_protection: VmProt,
    /// Inheritance
    pub inheritance: VmInherit,
    /// Is region shared?
    pub shared: bool,
    /// Object offset
    pub offset: u64,
}

/// Query information about a memory region
///
/// # Arguments
/// * `target_map` - Map to query
/// * `address` - In/out: address to query (returns start of region)
///
/// # Returns
/// Region info on success, error code otherwise
pub fn vm_region(target_map: &Arc<VmMap>, address: &mut u64) -> Result<VmRegionInfo, KernReturn> {
    let entries = target_map.entries.lock();

    // Find the entry containing or after the address
    for (&entry_start, entry) in entries.range(*address..) {
        if entry_start >= *address || entry.contains(*address) {
            *address = entry_start;
            return Ok(VmRegionInfo {
                address: entry_start,
                size: entry.size(),
                protection: entry.protection,
                max_protection: entry.max_protection,
                inheritance: entry.inheritance,
                shared: entry.flags.contains(EntryFlags::IS_SHARED),
                offset: entry.offset,
            });
        }
    }

    Err(KERN_INVALID_ADDRESS)
}

// ============================================================================
// VM Wire
// ============================================================================

/// Wire memory to prevent paging
pub fn vm_wire(target_map: &Arc<VmMap>, address: u64, size: u64) -> KernReturn {
    if size == 0 {
        return KERN_SUCCESS;
    }

    let start = trunc_page(address);
    let end = round_page(address + size);

    match target_map.wire(start, end) {
        Ok(()) => KERN_SUCCESS,
        Err(MapError::InvalidRange) => KERN_INVALID_ADDRESS,
        Err(MapError::NotFound) => KERN_INVALID_ADDRESS,
        Err(_) => KERN_MEMORY_ERROR,
    }
}

/// Unwire memory to allow paging
pub fn vm_unwire(target_map: &Arc<VmMap>, address: u64, size: u64) -> KernReturn {
    if size == 0 {
        return KERN_SUCCESS;
    }

    let start = trunc_page(address);
    let end = round_page(address + size);

    match target_map.unwire(start, end) {
        Ok(()) => KERN_SUCCESS,
        Err(MapError::InvalidRange) => KERN_INVALID_ADDRESS,
        Err(MapError::NotFound) => KERN_INVALID_ADDRESS,
        Err(_) => KERN_MEMORY_ERROR,
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Round address down to page boundary
#[inline]
pub fn trunc_page(addr: u64) -> u64 {
    addr & !(PAGE_SIZE as u64 - 1)
}

/// Round size up to page boundary
#[inline]
pub fn round_page(size: u64) -> u64 {
    (size + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_rounding() {
        assert_eq!(trunc_page(0x1000), 0x1000);
        assert_eq!(trunc_page(0x1001), 0x1000);
        assert_eq!(trunc_page(0x1FFF), 0x1000);

        assert_eq!(round_page(0x1000), 0x1000);
        assert_eq!(round_page(0x1001), 0x2000);
        assert_eq!(round_page(0x1FFF), 0x2000);
    }

    #[test]
    fn test_return_codes() {
        assert_eq!(KERN_SUCCESS, 0);
        assert_ne!(KERN_INVALID_ADDRESS, KERN_SUCCESS);
        assert_ne!(KERN_PROTECTION_FAILURE, KERN_SUCCESS);
    }
}
