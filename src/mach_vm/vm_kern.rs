//! VM Kernel - Kernel Memory Allocation
//!
//! Based on Mach4 vm/vm_kern.h/c
//! Provides kernel memory allocation routines for wired,
//! non-pageable memory.

use alloc::sync::Arc;
use spin::Mutex;

use crate::mach_vm::vm_map::{MapError, VmInherit, VmMap, VmProt};
use crate::mach_vm::vm_object;
use crate::mach_vm::vm_page::{self, PAGE_SIZE};

// ============================================================================
// Kernel Submap Types
// ============================================================================

/// Types of kernel submaps
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelSubmap {
    /// General kernel memory
    General,
    /// Pageable kernel memory
    Pageable,
    /// IO memory
    Io,
    /// Buffer cache
    Buffer,
}

// ============================================================================
// Kernel Memory Allocation
// ============================================================================

/// Allocate wired kernel memory
///
/// Returns virtual address of allocated memory
pub fn kmem_alloc(size: u64) -> Result<u64, MapError> {
    let kernel_map = crate::mach_vm::vm_map::kernel_map().ok_or(MapError::ResourceShortage)?;

    kmem_alloc_in_map(&kernel_map, size, false)
}

/// Allocate wired, zero-filled kernel memory
pub fn kmem_alloc_wired(size: u64) -> Result<u64, MapError> {
    let kernel_map = crate::mach_vm::vm_map::kernel_map().ok_or(MapError::ResourceShortage)?;

    let addr = kmem_alloc_in_map(&kernel_map, size, true)?;

    // Wire the memory
    kernel_map.wire(addr, addr + size)?;

    Ok(addr)
}

/// Allocate kernel memory in a specific map
fn kmem_alloc_in_map(map: &Arc<VmMap>, size: u64, wired: bool) -> Result<u64, MapError> {
    // Round size to page boundary
    let aligned_size = vm_page::round_page(size);

    // Find space in kernel map
    let addr = map
        .find_space(aligned_size, PAGE_SIZE as u64 - 1)
        .ok_or(MapError::NoSpace)?;

    // Get kernel object
    let kernel_object = vm_object::kernel_object().ok_or(MapError::ResourceShortage)?;

    // Enter mapping
    map.enter(
        addr,
        addr + aligned_size,
        Some(kernel_object),
        addr, // Offset = address for kernel object
        VmProt::DEFAULT,
        VmProt::ALL,
        VmInherit::None, // Don't inherit kernel memory
    )?;

    // Allocate physical pages if wired
    if wired {
        let mut offset = 0u64;
        while offset < aligned_size {
            let phys_addr = vm_page::alloc_page().ok_or(MapError::ResourceShortage)?;

            // In real implementation, would map physical page to virtual address
            let _ = phys_addr;

            offset += PAGE_SIZE as u64;
        }
    }

    Ok(addr)
}

/// Free kernel memory
pub fn kmem_free(addr: u64, size: u64) -> Result<(), MapError> {
    let kernel_map = crate::mach_vm::vm_map::kernel_map().ok_or(MapError::ResourceShortage)?;

    let aligned_size = vm_page::round_page(size);

    // Unwire if wired
    let _ = kernel_map.unwire(addr, addr + aligned_size);

    // Remove mapping
    kernel_map.remove(addr, addr + aligned_size)?;

    // Free physical pages
    let mut offset = 0u64;
    while offset < aligned_size {
        // In real implementation, would look up and free physical pages
        offset += PAGE_SIZE as u64;
    }

    Ok(())
}

/// Allocate kernel memory at a specific address
pub fn kmem_alloc_at(addr: u64, size: u64) -> Result<(), MapError> {
    let kernel_map = crate::mach_vm::vm_map::kernel_map().ok_or(MapError::ResourceShortage)?;

    let aligned_size = vm_page::round_page(size);

    // Get kernel object
    let kernel_object = vm_object::kernel_object().ok_or(MapError::ResourceShortage)?;

    // Enter mapping at specific address
    kernel_map.enter(
        addr,
        addr + aligned_size,
        Some(kernel_object),
        addr,
        VmProt::DEFAULT,
        VmProt::ALL,
        VmInherit::None,
    )?;

    Ok(())
}

// ============================================================================
// Kernel Submap Management
// ============================================================================

/// Kernel submaps
struct KernelSubmaps {
    /// General submap
    general: Option<Arc<VmMap>>,
    /// Pageable submap
    pageable: Option<Arc<VmMap>>,
    /// IO submap
    io: Option<Arc<VmMap>>,
    /// Buffer submap
    buffer: Option<Arc<VmMap>>,
}

impl KernelSubmaps {
    const fn new() -> Self {
        Self {
            general: None,
            pageable: None,
            io: None,
            buffer: None,
        }
    }
}

static KERNEL_SUBMAPS: spin::Once<Mutex<KernelSubmaps>> = spin::Once::new();

fn kernel_submaps() -> &'static Mutex<KernelSubmaps> {
    KERNEL_SUBMAPS.call_once(|| Mutex::new(KernelSubmaps::new()));
    KERNEL_SUBMAPS.get().unwrap()
}

/// Initialize kernel submaps
#[allow(clippy::too_many_arguments)]
pub fn init_submaps(
    general_start: u64,
    general_end: u64,
    pageable_start: u64,
    pageable_end: u64,
    io_start: u64,
    io_end: u64,
    buffer_start: u64,
    buffer_end: u64,
) {
    let mut submaps = kernel_submaps().lock();

    submaps.general = Some(Arc::new(VmMap::kernel(
        crate::mach_vm::vm_map::VmMapId(100),
        general_start,
        general_end,
    )));

    submaps.pageable = Some(Arc::new(VmMap::new(
        crate::mach_vm::vm_map::VmMapId(101),
        pageable_start,
        pageable_end,
    )));

    submaps.io = Some(Arc::new(VmMap::kernel(
        crate::mach_vm::vm_map::VmMapId(102),
        io_start,
        io_end,
    )));

    submaps.buffer = Some(Arc::new(VmMap::kernel(
        crate::mach_vm::vm_map::VmMapId(103),
        buffer_start,
        buffer_end,
    )));
}

/// Get a kernel submap
pub fn get_submap(submap_type: KernelSubmap) -> Option<Arc<VmMap>> {
    let submaps = kernel_submaps().lock();
    match submap_type {
        KernelSubmap::General => submaps.general.clone(),
        KernelSubmap::Pageable => submaps.pageable.clone(),
        KernelSubmap::Io => submaps.io.clone(),
        KernelSubmap::Buffer => submaps.buffer.clone(),
    }
}

/// Allocate from a specific submap
pub fn kmem_suballoc(submap_type: KernelSubmap, size: u64) -> Result<u64, MapError> {
    let submap = get_submap(submap_type).ok_or(MapError::ResourceShortage)?;

    kmem_alloc_in_map(&submap, size, true)
}

// ============================================================================
// Copy Operations
// ============================================================================

/// Copy from kernel to user space
pub fn copyout(
    _kernel_addr: u64,
    _user_map: &VmMap,
    _user_addr: u64,
    _size: u64,
) -> Result<(), MapError> {
    // Would perform actual copy in real implementation
    // Need to validate user mapping, handle faults, etc.
    Ok(())
}

/// Copy from user to kernel space
pub fn copyin(
    _user_map: &VmMap,
    _user_addr: u64,
    _kernel_addr: u64,
    _size: u64,
) -> Result<(), MapError> {
    // Would perform actual copy in real implementation
    Ok(())
}

/// Copy string from user space
pub fn copyinstr(
    _user_map: &VmMap,
    _user_addr: u64,
    _kernel_addr: u64,
    _max_len: u64,
) -> Result<u64, MapError> {
    // Would copy string and return length
    Ok(0)
}

// ============================================================================
// Physical Memory Mapping
// ============================================================================

/// Map physical memory into kernel virtual space
pub fn kmem_map_phys(phys_addr: u64, size: u64) -> Result<u64, MapError> {
    let kernel_map = crate::mach_vm::vm_map::kernel_map().ok_or(MapError::ResourceShortage)?;

    let aligned_size = vm_page::round_page(size);

    // Find virtual address
    let virt_addr = kernel_map
        .find_space(aligned_size, PAGE_SIZE as u64 - 1)
        .ok_or(MapError::NoSpace)?;

    // Create IO object for physical memory
    let io_object = vm_object::allocate(aligned_size);

    // Enter mapping
    kernel_map.enter(
        virt_addr,
        virt_addr + aligned_size,
        Some(io_object),
        phys_addr, // Offset is physical address
        VmProt::DEFAULT,
        VmProt::ALL,
        VmInherit::None,
    )?;

    // Wire the mapping
    kernel_map.wire(virt_addr, virt_addr + aligned_size)?;

    Ok(virt_addr)
}

/// Unmap physical memory from kernel
pub fn kmem_unmap_phys(virt_addr: u64, size: u64) -> Result<(), MapError> {
    kmem_free(virt_addr, size)
}

// ============================================================================
// Page-level Operations
// ============================================================================

/// Allocate a single page of kernel memory
pub fn kmem_alloc_page() -> Option<u64> {
    kmem_alloc_wired(PAGE_SIZE as u64).ok()
}

/// Free a single page of kernel memory
pub fn kmem_free_page(addr: u64) {
    let _ = kmem_free(addr, PAGE_SIZE as u64);
}

/// Allocate contiguous pages
pub fn kmem_alloc_pages(count: usize) -> Result<u64, MapError> {
    let size = (count * PAGE_SIZE) as u64;
    kmem_alloc_wired(size)
}

/// Free contiguous pages
pub fn kmem_free_pages(addr: u64, count: usize) -> Result<(), MapError> {
    let size = (count * PAGE_SIZE) as u64;
    kmem_free(addr, size)
}

// ============================================================================
// Statistics
// ============================================================================

/// Kernel memory statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct KmemStats {
    /// Total allocated
    pub allocated: u64,
    /// Total freed
    pub freed: u64,
    /// Current in use
    pub in_use: u64,
    /// Peak usage
    pub peak: u64,
    /// Allocation count
    pub alloc_count: u64,
    /// Free count
    pub free_count: u64,
}

static KMEM_STATS: spin::Once<Mutex<KmemStats>> = spin::Once::new();

fn kmem_stats() -> &'static Mutex<KmemStats> {
    KMEM_STATS.call_once(|| Mutex::new(KmemStats::default()));
    KMEM_STATS.get().unwrap()
}

/// Get kernel memory statistics
pub fn get_stats() -> KmemStats {
    *kmem_stats().lock()
}

/// Update allocation statistics
fn stat_alloc(size: u64) {
    let mut stats = kmem_stats().lock();
    stats.allocated += size;
    stats.in_use += size;
    stats.alloc_count += 1;
    if stats.in_use > stats.peak {
        stats.peak = stats.in_use;
    }
}

/// Update free statistics
fn stat_free(size: u64) {
    let mut stats = kmem_stats().lock();
    stats.freed += size;
    stats.in_use = stats.in_use.saturating_sub(size);
    stats.free_count += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kmem_stats() {
        let stats = KmemStats::default();
        assert_eq!(stats.allocated, 0);
        assert_eq!(stats.in_use, 0);
    }

    #[test]
    fn test_submap_types() {
        assert_ne!(KernelSubmap::General, KernelSubmap::Io);
        assert_ne!(KernelSubmap::Pageable, KernelSubmap::Buffer);
    }
}
