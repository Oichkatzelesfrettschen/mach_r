//! Physical Map (pmap) - Hardware Page Table Management
//!
//! Based on Mach4 vm/pmap.h/c
//!
//! The pmap module provides an architecture-independent interface for
//! managing hardware page tables. It bridges the high-level vm_map
//! operations with the architecture-specific page table structures.
//!
//! ## Key Operations
//!
//! - `pmap_enter`: Install a mapping from virtual to physical
//! - `pmap_remove`: Remove a mapping
//! - `pmap_protect`: Change protection on existing mappings
//! - `pmap_extract`: Get physical address for virtual address
//!
//! ## Design Notes
//!
//! In Mach, each task has its own pmap. When a page fault is resolved:
//! 1. vm_fault finds/allocates the physical page
//! 2. vm_fault calls pmap_enter to install the mapping
//! 3. The TLB is flushed (implicit or explicit)
//! 4. Control returns to user space to retry the instruction

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::mach_vm::vm_map::VmProt;
use crate::paging::{ActivePageTable, PageTable, PageTableFlags, PhysicalAddress, VirtualAddress};

// ============================================================================
// Pmap ID
// ============================================================================

/// Pmap identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PmapId(pub u64);

impl PmapId {
    pub const KERNEL: Self = Self(1);
    pub const NULL: Self = Self(0);
}

// ============================================================================
// Pmap Statistics
// ============================================================================

/// Statistics for a pmap
#[derive(Debug, Default)]
pub struct PmapStats {
    /// Number of resident pages
    pub resident_count: AtomicU32,
    /// Number of wired pages
    pub wired_count: AtomicU32,
}

impl PmapStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resident(&self) -> u32 {
        self.resident_count.load(Ordering::Relaxed)
    }

    pub fn wired(&self) -> u32 {
        self.wired_count.load(Ordering::Relaxed)
    }
}

// ============================================================================
// Pmap Structure
// ============================================================================

/// Physical map - manages hardware page tables for an address space
pub struct Pmap {
    /// Pmap identifier
    pub id: PmapId,

    /// Reference count
    ref_count: AtomicU32,

    /// Page directory base (CR3 on x86_64, TTBR on ARM)
    pub page_directory_base: AtomicU64,

    /// Statistics
    pub stats: PmapStats,

    /// Active page table wrapper
    page_table: Mutex<Option<Box<ActivePageTable>>>,

    /// Simple mapping cache: virt_page -> phys_page
    /// Used for quick lookups without walking page tables
    mapping_cache: Mutex<BTreeMap<u64, u64>>,
}

impl Pmap {
    /// Create a new pmap
    pub fn new(id: PmapId) -> Self {
        let p4_table = Box::new(PageTable::new());
        let p4_phys = Box::into_raw(p4_table) as u64;

        Self {
            id,
            ref_count: AtomicU32::new(1),
            page_directory_base: AtomicU64::new(p4_phys),
            stats: PmapStats::new(),
            page_table: Mutex::new(Some(Box::new(ActivePageTable::new(unsafe {
                Box::from_raw(p4_phys as *mut PageTable)
            })))),
            mapping_cache: Mutex::new(BTreeMap::new()),
        }
    }

    /// Create the kernel pmap
    pub fn kernel() -> Self {
        let mut pmap = Self::new(PmapId::KERNEL);
        // Kernel pmap gets special treatment - it's always active
        // and maps the kernel address space
        pmap
    }

    /// Increment reference count
    pub fn reference(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement reference count, returns true if deallocated
    pub fn deallocate(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    /// Get reference count
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::Relaxed)
    }

    /// Enter a mapping from virtual to physical address
    ///
    /// This is the key function called by vm_fault to install page mappings.
    pub fn enter(&self, virt_addr: u64, phys_addr: u64, prot: VmProt) -> Result<(), PmapError> {
        // Convert VmProt to PageTableFlags
        let mut flags = PageTableFlags::PRESENT;

        if prot.contains(VmProt::WRITE) {
            flags |= PageTableFlags::WRITABLE;
        }

        if !prot.contains(VmProt::EXECUTE) {
            flags |= PageTableFlags::NO_EXECUTE;
        }

        // User space mappings need USER_ACCESSIBLE
        // Check if address is in user space range
        if virt_addr < 0x0000_8000_0000_0000 {
            flags |= PageTableFlags::USER_ACCESSIBLE;
        }

        // Update the page table
        let mut pt_guard = self.page_table.lock();
        if let Some(ref mut pt) = *pt_guard {
            let virt = VirtualAddress::new(virt_addr as usize);
            let phys = PhysicalAddress::new(phys_addr as usize);
            pt.map(virt, phys, flags);

            // Update cache
            let virt_page = virt_addr & !0xFFF;
            let phys_page = phys_addr & !0xFFF;
            self.mapping_cache.lock().insert(virt_page, phys_page);

            // Update statistics
            self.stats.resident_count.fetch_add(1, Ordering::Relaxed);

            // Flush TLB for this address
            self.flush_tlb(virt_addr);

            Ok(())
        } else {
            Err(PmapError::NotInitialized)
        }
    }

    /// Remove a mapping
    pub fn remove(&self, virt_addr: u64) -> Result<(), PmapError> {
        let mut pt_guard = self.page_table.lock();
        if let Some(ref mut pt) = *pt_guard {
            let virt = VirtualAddress::new(virt_addr as usize);
            pt.unmap(virt);

            // Update cache
            let virt_page = virt_addr & !0xFFF;
            self.mapping_cache.lock().remove(&virt_page);

            // Update statistics
            let old = self.stats.resident_count.fetch_sub(1, Ordering::Relaxed);
            if old == 0 {
                // Prevent underflow
                self.stats.resident_count.store(0, Ordering::Relaxed);
            }

            // Flush TLB
            self.flush_tlb(virt_addr);

            Ok(())
        } else {
            Err(PmapError::NotInitialized)
        }
    }

    /// Remove mappings in a range
    pub fn remove_range(&self, start: u64, end: u64) -> Result<(), PmapError> {
        let page_size = 4096u64;
        let mut addr = start & !0xFFF;

        while addr < end {
            let _ = self.remove(addr);
            addr += page_size;
        }

        Ok(())
    }

    /// Change protection on an existing mapping
    pub fn protect(&self, virt_addr: u64, prot: VmProt) -> Result<(), PmapError> {
        // Get current physical mapping
        let phys = match self.extract(virt_addr) {
            Some(p) => p,
            None => return Err(PmapError::NotMapped),
        };

        // Remove and re-enter with new protection
        self.remove(virt_addr)?;
        self.enter(virt_addr, phys, prot)
    }

    /// Extract physical address from virtual address
    pub fn extract(&self, virt_addr: u64) -> Option<u64> {
        // First check cache
        let virt_page = virt_addr & !0xFFF;
        let offset = virt_addr & 0xFFF;

        if let Some(&phys_page) = self.mapping_cache.lock().get(&virt_page) {
            return Some(phys_page | offset);
        }

        // Walk page tables
        let pt_guard = self.page_table.lock();
        if let Some(ref pt) = *pt_guard {
            let virt = VirtualAddress::new(virt_addr as usize);
            pt.translate(virt).map(|p| p.0 as u64)
        } else {
            None
        }
    }

    /// Check if a virtual address is mapped
    pub fn is_mapped(&self, virt_addr: u64) -> bool {
        self.extract(virt_addr).is_some()
    }

    /// Flush TLB for a specific address
    fn flush_tlb(&self, virt_addr: u64) {
        // Architecture-specific TLB flush
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) virt_addr);
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // TLBI VAE1IS - TLB Invalidate by VA, EL1, Inner Shareable
            core::arch::asm!(
                "dsb ishst",
                "tlbi vaae1is, {}",
                "dsb ish",
                "isb",
                in(reg) virt_addr >> 12,
            );
        }
    }

    /// Flush entire TLB
    pub fn flush_tlb_all(&self) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            // Reload CR3 to flush TLB
            let cr3: u64;
            core::arch::asm!("mov {}, cr3", out(reg) cr3);
            core::arch::asm!("mov cr3, {}", in(reg) cr3);
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!(
                "dsb ishst",
                "tlbi vmalle1is",
                "dsb ish",
                "isb",
            );
        }
    }

    /// Activate this pmap (load its page directory)
    pub fn activate(&self) {
        let pdb = self.page_directory_base.load(Ordering::SeqCst);

        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) pdb);
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Set TTBR0_EL1 for user space
            core::arch::asm!(
                "msr ttbr0_el1, {}",
                "isb",
                in(reg) pdb,
            );
        }
    }
}

// ============================================================================
// Pmap Errors
// ============================================================================

/// Pmap operation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmapError {
    /// Pmap not initialized
    NotInitialized,
    /// Address not mapped
    NotMapped,
    /// Protection violation
    Protection,
    /// Out of memory
    OutOfMemory,
}

// ============================================================================
// Global Pmap Management
// ============================================================================

/// Global pmap table
struct PmapManager {
    pmaps: BTreeMap<PmapId, Arc<Pmap>>,
    next_id: u64,
    kernel_pmap: Option<Arc<Pmap>>,
}

impl PmapManager {
    fn new() -> Self {
        Self {
            pmaps: BTreeMap::new(),
            next_id: 2, // 1 is reserved for kernel
            kernel_pmap: None,
        }
    }

    fn bootstrap(&mut self) -> Arc<Pmap> {
        let kernel = Arc::new(Pmap::kernel());
        self.pmaps.insert(PmapId::KERNEL, Arc::clone(&kernel));
        self.kernel_pmap = Some(Arc::clone(&kernel));
        kernel
    }

    fn create(&mut self) -> Arc<Pmap> {
        let id = PmapId(self.next_id);
        self.next_id += 1;

        let pmap = Arc::new(Pmap::new(id));
        self.pmaps.insert(id, Arc::clone(&pmap));
        pmap
    }

    fn find(&self, id: PmapId) -> Option<Arc<Pmap>> {
        self.pmaps.get(&id).cloned()
    }

    fn kernel(&self) -> Option<Arc<Pmap>> {
        self.kernel_pmap.clone()
    }

    fn destroy(&mut self, id: PmapId) {
        if id != PmapId::KERNEL {
            self.pmaps.remove(&id);
        }
    }
}

static PMAP_MANAGER: Mutex<Option<PmapManager>> = Mutex::new(None);

fn pmap_manager() -> &'static Mutex<Option<PmapManager>> {
    let mut guard = PMAP_MANAGER.lock();
    if guard.is_none() {
        let mut mgr = PmapManager::new();
        mgr.bootstrap();
        *guard = Some(mgr);
    }
    drop(guard);
    &PMAP_MANAGER
}

/// Initialize pmap subsystem
pub fn init() {
    let _ = pmap_manager();
}

/// Create a new pmap
pub fn pmap_create() -> Arc<Pmap> {
    pmap_manager()
        .lock()
        .as_mut()
        .expect("pmap not initialized")
        .create()
}

/// Find pmap by ID
pub fn pmap_find(id: PmapId) -> Option<Arc<Pmap>> {
    pmap_manager()
        .lock()
        .as_ref()
        .and_then(|m| m.find(id))
}

/// Get kernel pmap
pub fn kernel_pmap() -> Option<Arc<Pmap>> {
    pmap_manager().lock().as_ref().and_then(|m| m.kernel())
}

/// Destroy a pmap
pub fn pmap_destroy(id: PmapId) {
    if let Some(ref mut mgr) = *pmap_manager().lock() {
        mgr.destroy(id);
    }
}

// ============================================================================
// Convenience Functions for vm_fault
// ============================================================================

/// Enter a page mapping (convenience wrapper for vm_fault)
///
/// This is the main function called by vm_fault after resolving a page fault.
pub fn pmap_enter(pmap: &Pmap, virt: u64, phys: u64, prot: VmProt) -> Result<(), PmapError> {
    pmap.enter(virt, phys, prot)
}

/// Remove a page mapping
pub fn pmap_remove(pmap: &Pmap, virt: u64) -> Result<(), PmapError> {
    pmap.remove(virt)
}

/// Change protection on a mapping
pub fn pmap_protect(pmap: &Pmap, virt: u64, prot: VmProt) -> Result<(), PmapError> {
    pmap.protect(virt, prot)
}

/// Extract physical address from virtual
pub fn pmap_extract(pmap: &Pmap, virt: u64) -> Option<u64> {
    pmap.extract(virt)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pmap_create() {
        let pmap = Pmap::new(PmapId(1));
        assert_eq!(pmap.id, PmapId(1));
        assert_eq!(pmap.ref_count(), 1);
    }

    #[test]
    fn test_pmap_stats() {
        let stats = PmapStats::new();
        assert_eq!(stats.resident(), 0);
        assert_eq!(stats.wired(), 0);
    }
}
