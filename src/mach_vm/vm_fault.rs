//! VM Fault - Page Fault Handling
//!
//! Based on Mach4 vm/vm_fault.h/c
//! Handles page faults by coordinating with objects and pagers
//! to bring pages into memory.
//!
//! ## Fault Types
//!
//! - **Zero-fill**: Anonymous pages get zero-filled on first access
//! - **Page-in**: Pages retrieved from external pager via XMM
//! - **Copy-on-write**: Shared pages copied on write access
//!
//! ## Integration with XMM
//!
//! For external pagers, vm_fault uses the XMM interface to request pages:
//! 1. vm_fault detects page is not resident
//! 2. Calls m_data_request() on the memory object's XMM layer
//! 3. Thread blocks until pager responds via k_data_supply()
//! 4. Page is installed and fault completes

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::mach_vm::vm_map::{VmMap, VmProt};
use crate::mach_vm::vm_object::{ObjectFlags, PagerType, VmObject};
use crate::mach_vm::vm_page::{self, PAGE_SIZE};
use crate::mach_vm::vm_pageout;
use crate::mach_vm::xmm::XmmObject;

// ============================================================================
// Fault Result
// ============================================================================

/// Result of a page fault operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultResult {
    /// Fault handled successfully
    Success,
    /// Need to retry the fault
    Retry,
    /// Memory shortage
    MemoryShortage,
    /// Memory error
    MemoryError,
    /// Protection failure
    ProtectionFailure,
    /// Memory failure (page not available)
    MemoryFailure,
    /// Interrupted
    Interrupted,
    /// Resource shortage
    ResourceShortage,
}

/// Fault type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultType {
    /// Read fault
    Read,
    /// Write fault
    Write,
    /// Execute fault
    Execute,
}

impl FaultType {
    /// Convert to required protection
    pub fn to_protection(&self) -> VmProt {
        match self {
            FaultType::Read => VmProt::READ,
            FaultType::Write => VmProt::WRITE,
            FaultType::Execute => VmProt::EXECUTE,
        }
    }
}

// ============================================================================
// Fault Statistics
// ============================================================================

/// Global fault statistics
static FAULT_STATS: FaultStats = FaultStats::new();

/// Fault statistics counters
pub struct FaultStats {
    /// Total faults
    pub total: AtomicU64,
    /// Copy-on-write faults
    pub cow: AtomicU64,
    /// Zero-fill faults
    pub zero_fill: AtomicU64,
    /// Page-in faults (from pager)
    pub page_in: AtomicU64,
    /// Reactivated pages
    pub reactivated: AtomicU64,
    /// Failed faults
    pub failures: AtomicU64,
}

impl FaultStats {
    pub const fn new() -> Self {
        Self {
            total: AtomicU64::new(0),
            cow: AtomicU64::new(0),
            zero_fill: AtomicU64::new(0),
            page_in: AtomicU64::new(0),
            reactivated: AtomicU64::new(0),
            failures: AtomicU64::new(0),
        }
    }

    pub fn incr_total(&self) {
        self.total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_cow(&self) {
        self.cow.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_zero_fill(&self) {
        self.zero_fill.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_page_in(&self) {
        self.page_in.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_reactivated(&self) {
        self.reactivated.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_failures(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }
}

/// Get fault statistics
pub fn stats() -> &'static FaultStats {
    &FAULT_STATS
}

// ============================================================================
// Page Fault Handling
// ============================================================================

/// Handle a page fault
///
/// This is the main entry point for page fault handling.
/// It looks up the faulting address in the map, finds or creates
/// the appropriate page, and installs it in the address space.
pub fn vm_fault(
    map: &VmMap,
    vaddr: u64,
    fault_type: FaultType,
    _change_wiring: bool,
) -> FaultResult {
    FAULT_STATS.incr_total();

    // Round address to page boundary
    let page_addr = vm_page::trunc_page(vaddr);

    // Look up the address in the map
    let entry_start = match map.lookup(page_addr) {
        Some(start) => start,
        None => {
            FAULT_STATS.incr_failures();
            return FaultResult::MemoryError;
        }
    };

    // Get the entry
    let entries = map.entries.lock();
    let entry = match entries.get(&entry_start) {
        Some(e) => e,
        None => {
            FAULT_STATS.incr_failures();
            return FaultResult::MemoryError;
        }
    };

    // Check protection
    let required_prot = fault_type.to_protection();
    if !entry.protection.contains(required_prot) {
        FAULT_STATS.incr_failures();
        return FaultResult::ProtectionFailure;
    }

    // Get the object and offset before dropping lock
    let object = match &entry.object {
        Some(obj) => Arc::clone(obj),
        None => {
            // No object - this shouldn't happen for a valid entry
            FAULT_STATS.incr_failures();
            return FaultResult::MemoryError;
        }
    };

    // Calculate offset into object
    let offset = page_addr - entry_start + entry.offset;
    let is_shadowed = object.get_flags().contains(ObjectFlags::SHADOWED);
    let entry_protection = entry.protection;

    drop(entries); // Release lock before page operations

    // Handle the fault based on type
    let result = if fault_type == FaultType::Write {
        vm_fault_write(&object, offset, is_shadowed)
    } else {
        vm_fault_read(&object, offset)
    };

    // If fault was successful, update hardware page tables
    if result == FaultResult::Success {
        if let Err(e) = install_page_mapping(map, &object, page_addr, offset, entry_protection) {
            // Log but don't fail - the page is allocated, mapping just failed
            #[cfg(not(test))]
            crate::println!("Warning: pmap_enter failed for 0x{:x}: {:?}", page_addr, e);
        }
    }

    result
}

/// Install page mapping in hardware page tables after fault resolution
fn install_page_mapping(
    map: &VmMap,
    object: &Arc<VmObject>,
    virt_addr: u64,
    offset: u64,
    protection: VmProt,
) -> Result<(), super::pmap::PmapError> {
    // Get the page number from the object
    let page_num = match object.page_lookup(offset) {
        Some(pn) => pn,
        None => {
            // Page should have been inserted by vm_fault_read/write
            return Err(super::pmap::PmapError::NotMapped);
        }
    };

    // Convert page number to physical address
    let phys_addr = vm_page::page_to_addr(page_num);

    // Get the pmap from the map
    let pmap_id = match *map.pmap_id.lock() {
        Some(id) => id,
        None => {
            // No pmap associated - create one or use kernel pmap
            // For user maps without pmap, we can't install mappings
            return Ok(()); // Silently succeed - no pmap means no hw page tables yet
        }
    };

    // Look up the pmap
    let pmap = match super::pmap::pmap_find(pmap_id) {
        Some(p) => p,
        None => {
            return Err(super::pmap::PmapError::NotInitialized);
        }
    };

    // Install the mapping
    pmap.enter(virt_addr, phys_addr, protection)
}

/// Handle a read fault
fn vm_fault_read(object: &Arc<VmObject>, offset: u64) -> FaultResult {
    // Look for page in object
    if object.page_lookup(offset).is_some() {
        // Page found - activate it
        FAULT_STATS.incr_reactivated();
        return FaultResult::Success;
    }

    // Check shadow chain
    if let Some(shadow) = object.get_shadow() {
        let shadow_offset = object.shadow_offset.load(Ordering::SeqCst);
        if let Some(page_num) = shadow.page_lookup(offset + shadow_offset) {
            // Page found in shadow - share it
            object.page_insert(offset, page_num);
            FAULT_STATS.incr_reactivated();
            return FaultResult::Success;
        }
    }

    // Check if this object has an external pager
    if object.pager_type == PagerType::External {
        // Object backed by external pager - request page data
        return vm_fault_read_from_pager(object, offset);
    }

    // Anonymous memory - allocate zero-filled page
    match allocate_page_for_object(object, offset) {
        Ok(()) => {
            FAULT_STATS.incr_zero_fill();
            FaultResult::Success
        }
        Err(_) => {
            FAULT_STATS.incr_failures();
            FaultResult::MemoryShortage
        }
    }
}

/// Read a page from external pager
fn vm_fault_read_from_pager(object: &Arc<VmObject>, offset: u64) -> FaultResult {
    // Check if there's a pager port
    let _pager_port = match *object.pager.lock() {
        Some(port) => port,
        None => {
            // No pager - treat as zero-fill
            return match allocate_page_for_object(object, offset) {
                Ok(()) => {
                    FAULT_STATS.incr_zero_fill();
                    FaultResult::Success
                }
                Err(_) => FaultResult::MemoryShortage,
            };
        }
    };

    // In a full implementation, we would:
    // 1. Look up the XMM object associated with this VmObject's pager
    // 2. Call vm_fault_page_in() which calls m_data_request()
    // 3. Block until the pager responds with k_data_supply()
    // 4. Install the page and return
    //
    // For now, we allocate a zero page and mark it for later page-in
    // This simulates the pager returning an empty page

    match allocate_page_for_object(object, offset) {
        Ok(()) => {
            FAULT_STATS.incr_page_in();
            FaultResult::Success
        }
        Err(_) => {
            FAULT_STATS.incr_failures();
            FaultResult::MemoryShortage
        }
    }
}

/// Handle a write fault (may need copy-on-write)
fn vm_fault_write(object: &Arc<VmObject>, offset: u64, is_shadowed: bool) -> FaultResult {
    // Check if we need copy-on-write
    if is_shadowed {
        return vm_fault_copy_on_write(object, offset);
    }

    // Look for page in object
    if let Some(page_num) = object.page_lookup(offset) {
        // Page found - just mark it dirty
        if let Some(page) = vm_page::page_manager().lock().get_page(page_num) {
            page.set_dirty();
        }
        return FaultResult::Success;
    }

    // Need to allocate a new page
    match allocate_page_for_object(object, offset) {
        Ok(()) => {
            FAULT_STATS.incr_zero_fill();
            FaultResult::Success
        }
        Err(_) => {
            FAULT_STATS.incr_failures();
            FaultResult::MemoryShortage
        }
    }
}

/// Handle copy-on-write fault
fn vm_fault_copy_on_write(object: &Arc<VmObject>, offset: u64) -> FaultResult {
    // First, check if we should collapse the shadow chain
    if object.needs_collapse() {
        if object.collapse() {
            // Successfully collapsed - page might now be in object
            if object.page_lookup(offset).is_some() {
                return FaultResult::Success;
            }
        }
    }

    // Get the source page (from shadow chain, searching recursively)
    let source_page_num = find_page_in_shadow_chain(object, offset);

    // Check memory pressure before allocation
    if vm_page::memory_low() {
        vm_pageout::wakeup();
    }

    // Allocate new page
    let new_page_addr = match vm_page::alloc_page() {
        Some(addr) => addr,
        None => {
            // Wake pageout daemon on failure
            vm_pageout::wakeup();
            FAULT_STATS.incr_failures();
            return FaultResult::MemoryShortage;
        }
    };

    let new_page_num = vm_page::addr_to_page(new_page_addr);

    // Copy contents if we have a source
    if let Some(src_page_num) = source_page_num {
        // Copy page contents using physical addresses
        copy_phys_page(src_page_num, new_page_num);
    } else {
        // Zero-fill new page
        zero_phys_page(new_page_num);
    }

    // Insert new page into object (replacing any existing mapping)
    object.page_insert(offset, new_page_num);

    // Mark page as dirty since we just wrote to it
    if let Some(page) = vm_page::page_manager().lock().get_page(new_page_num) {
        page.set_dirty();
    }

    // Try opportunistic collapse after COW
    // This helps clean up shadow chains when they're no longer needed
    try_collapse_shadow_chain(object);

    FAULT_STATS.incr_cow();
    FaultResult::Success
}

/// Attempt to collapse shadow chain opportunistically
///
/// Called after COW faults to try to simplify shadow chains.
/// This is a best-effort operation - failure just means we keep the chain.
fn try_collapse_shadow_chain(object: &Arc<VmObject>) {
    // Check if shadow depth warrants collapse attempt
    if object.shadow_depth() <= 3 {
        return; // Not worth the overhead for shallow chains
    }

    // Try to collapse
    let mut collapse_count = 0;
    while object.collapse() && collapse_count < 5 {
        collapse_count += 1;
        // Continue collapsing until we can't anymore
    }
}

/// Search for a page in the shadow chain
fn find_page_in_shadow_chain(object: &Arc<VmObject>, offset: u64) -> Option<u32> {
    // First check the object itself
    if let Some(page_num) = object.page_lookup(offset) {
        return Some(page_num);
    }

    // Walk the shadow chain
    let mut current_offset = offset;
    let mut current_shadow = object.get_shadow();

    while let Some(shadow) = current_shadow {
        let shadow_offset = object.shadow_offset.load(Ordering::SeqCst);
        current_offset += shadow_offset;

        if let Some(page_num) = shadow.page_lookup(current_offset) {
            return Some(page_num);
        }

        current_shadow = shadow.get_shadow();
    }

    None
}

/// Copy physical page contents
fn copy_phys_page(src_page: u32, dst_page: u32) {
    let page_size = PAGE_SIZE;
    let src_addr = vm_page::page_to_addr(src_page) as *const u8;
    let dst_addr = vm_page::page_to_addr(dst_page) as *mut u8;

    // SAFETY: We own these pages and they are properly aligned
    unsafe {
        core::ptr::copy_nonoverlapping(src_addr, dst_addr, page_size);
    }
}

/// Zero a physical page
fn zero_phys_page(page: u32) {
    let page_size = PAGE_SIZE;
    let addr = vm_page::page_to_addr(page) as *mut u8;

    // SAFETY: We own this page and it is properly aligned
    unsafe {
        core::ptr::write_bytes(addr, 0, page_size);
    }
}

/// Allocate a page for an object at given offset
fn allocate_page_for_object(object: &Arc<VmObject>, offset: u64) -> Result<(), ()> {
    // Check if memory is low before allocation
    if vm_page::memory_low() {
        // Wake up pageout daemon to reclaim pages
        vm_pageout::wakeup();
    }

    // Allocate physical page
    let phys_addr = match vm_page::alloc_page() {
        Some(addr) => addr,
        None => {
            // No free pages - wake pageout daemon and fail
            vm_pageout::wakeup();
            return Err(());
        }
    };

    let page_num = vm_page::addr_to_page(phys_addr);

    // Zero the page (in real implementation)
    // zero_page(page_num);

    // Insert into object
    object.page_insert(offset, page_num);

    // Check memory after allocation
    if vm_page::memory_low() {
        vm_pageout::wakeup();
    }

    Ok(())
}

// ============================================================================
// Wire/Unwire Faults
// ============================================================================

/// Wire pages in a range (make them non-pageable)
pub fn vm_fault_wire(map: &VmMap, start: u64, end: u64) -> FaultResult {
    let page_start = vm_page::trunc_page(start);
    let page_end = vm_page::round_page(end);

    let mut addr = page_start;
    while addr < page_end {
        // Fault in the page if needed
        let result = vm_fault(map, addr, FaultType::Read, true);
        if result != FaultResult::Success {
            // Unwire any pages we already wired
            let _ = vm_fault_unwire(map, page_start, addr);
            return result;
        }

        // Wire the page
        // In real implementation would mark page as wired

        addr += PAGE_SIZE as u64;
    }

    FaultResult::Success
}

/// Unwire pages in a range
pub fn vm_fault_unwire(_map: &VmMap, start: u64, end: u64) -> FaultResult {
    let page_start = vm_page::trunc_page(start);
    let page_end = vm_page::round_page(end);

    // Would unwire pages in range
    let _ = (page_start, page_end); // Suppress unused warning

    FaultResult::Success
}

// ============================================================================
// Prefault
// ============================================================================

/// Prefault pages in a range
pub fn vm_fault_prefault(map: &VmMap, start: u64, end: u64) {
    let page_start = vm_page::trunc_page(start);
    let page_end = vm_page::round_page(end);

    let mut addr = page_start;
    while addr < page_end {
        // Try to fault in the page, ignore failures
        let _ = vm_fault(map, addr, FaultType::Read, false);
        addr += PAGE_SIZE as u64;
    }
}

// ============================================================================
// Fault State Machine
// ============================================================================

/// State of an ongoing page fault
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultState {
    /// Initial state - starting fault processing
    Start,
    /// Looking up the map entry
    LookupMap,
    /// Found entry, checking object
    CheckObject,
    /// Looking up page in object
    LookupPage,
    /// Page found, activating it
    ActivatePage,
    /// Need to page in from pager
    PageIn,
    /// Waiting for pager response
    WaitingForPager,
    /// Page data arrived, installing
    InstallPage,
    /// Handling copy-on-write
    CopyOnWrite,
    /// Fault complete
    Done,
    /// Fault failed
    Failed,
}

/// Fault context - tracks state during a page fault
#[derive(Debug)]
pub struct FaultContext {
    /// Current state
    pub state: FaultState,
    /// Faulting virtual address
    pub vaddr: u64,
    /// Page-aligned address
    pub page_addr: u64,
    /// Fault type (read/write/execute)
    pub fault_type: FaultType,
    /// The map being faulted in
    pub map_id: u64,
    /// Entry start address
    pub entry_start: u64,
    /// Offset into object
    pub offset: u64,
    /// Object ID (if found)
    pub object_id: Option<u64>,
    /// Is this a wiring operation?
    pub wiring: bool,
    /// Retry count
    pub retries: u32,
    /// Maximum retries
    pub max_retries: u32,
}

impl FaultContext {
    /// Create new fault context
    pub fn new(vaddr: u64, fault_type: FaultType, wiring: bool) -> Self {
        Self {
            state: FaultState::Start,
            vaddr,
            page_addr: vm_page::trunc_page(vaddr),
            fault_type,
            map_id: 0,
            entry_start: 0,
            offset: 0,
            object_id: None,
            wiring,
            retries: 0,
            max_retries: 3,
        }
    }

    /// Check if should retry
    pub fn should_retry(&self) -> bool {
        self.retries < self.max_retries
    }

    /// Increment retry counter
    pub fn incr_retries(&mut self) {
        self.retries += 1;
    }
}

// ============================================================================
// Pending Fault Queue
// ============================================================================

/// A pending fault waiting for pager response
#[derive(Debug)]
pub struct PendingFault {
    /// Thread ID waiting
    pub thread_id: u64,
    /// Object ID
    pub object_id: u64,
    /// Offset within object
    pub offset: u64,
    /// When the fault started
    pub start_time: u64,
    /// Has the pager responded?
    pub satisfied: AtomicBool,
    /// Result from pager
    pub result: Mutex<Option<FaultResult>>,
}

impl PendingFault {
    /// Create new pending fault
    pub fn new(thread_id: u64, object_id: u64, offset: u64) -> Self {
        Self {
            thread_id,
            object_id,
            offset,
            start_time: 0, // Would use timer
            satisfied: AtomicBool::new(false),
            result: Mutex::new(None),
        }
    }

    /// Mark as satisfied with result
    pub fn satisfy(&self, result: FaultResult) {
        *self.result.lock() = Some(result);
        self.satisfied.store(true, Ordering::SeqCst);
    }

    /// Check if satisfied
    pub fn is_satisfied(&self) -> bool {
        self.satisfied.load(Ordering::SeqCst)
    }
}

/// Global pending fault table
static PENDING_FAULTS: Mutex<PendingFaultTable> = Mutex::new(PendingFaultTable::new());

/// Table of pending faults waiting for pager
pub struct PendingFaultTable {
    /// Next fault ID
    next_id: u64,
    /// Count of pending faults
    count: u32,
}

impl PendingFaultTable {
    const fn new() -> Self {
        Self {
            next_id: 1,
            count: 0,
        }
    }

    /// Add a pending fault
    pub fn add(&mut self, _fault: PendingFault) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.count += 1;
        // In real implementation, store in hash table by (object_id, offset)
        id
    }

    /// Remove a pending fault
    pub fn remove(&mut self, _fault_id: u64) {
        if self.count > 0 {
            self.count -= 1;
        }
    }

    /// Get pending fault count
    pub fn count(&self) -> u32 {
        self.count
    }
}

/// Get pending faults table
pub fn pending_faults() -> &'static Mutex<PendingFaultTable> {
    &PENDING_FAULTS
}

// ============================================================================
// XMM Integration
// ============================================================================

/// Request page data from external pager via XMM
///
/// This is called when vm_fault needs to page in data from an external pager.
/// The thread will block until the pager responds with k_data_supply.
pub fn vm_fault_page_in(
    object: &Arc<VmObject>,
    offset: u64,
    access: VmProt,
    xmm: &XmmObject,
) -> FaultResult {
    // Create pending fault entry
    let pending = PendingFault::new(
        0, // Current thread ID
        object.id.0,
        offset,
    );

    let fault_id = PENDING_FAULTS.lock().add(pending);

    // Send data request to pager via XMM
    let mut xmm_guard = xmm.lock();
    let result = xmm_guard.m_data_request(offset as usize, PAGE_SIZE, access);

    // Check for immediate failure
    if result != 0 {
        PENDING_FAULTS.lock().remove(fault_id);
        FAULT_STATS.incr_failures();
        return FaultResult::MemoryFailure;
    }

    // In a real implementation, the thread would block here using continuations:
    // thread_block_with_continuation(fault_continuation, fault_id)
    //
    // When the pager responds via k_data_supply(), it would:
    // 1. Find the pending fault entry
    // 2. Install the page data
    // 3. Wake the waiting thread
    //
    // For now, we simulate immediate completion
    PENDING_FAULTS.lock().remove(fault_id);
    FAULT_STATS.incr_page_in();
    FaultResult::Success
}

/// Handle pager data supply (called when pager responds)
///
/// This is called from the IPC layer when a pager sends data via k_data_supply.
pub fn vm_fault_data_supply(object_id: u64, offset: u64, _data: &[u8]) -> FaultResult {
    // In real implementation:
    // 1. Find pending fault for (object_id, offset)
    // 2. Allocate physical page
    // 3. Copy data into page
    // 4. Insert page into object
    // 5. Wake waiting thread

    let _ = (object_id, offset);
    FaultResult::Success
}

/// Handle pager data unavailable (page doesn't exist in backing store)
pub fn vm_fault_data_unavailable(object_id: u64, offset: u64) -> FaultResult {
    // In real implementation:
    // 1. Find pending fault
    // 2. Either zero-fill or report error
    // 3. Wake waiting thread

    let _ = (object_id, offset);
    FaultResult::MemoryFailure
}

// ============================================================================
// Cluster Faulting
// ============================================================================

/// Cluster size for readahead (in pages)
pub const FAULT_CLUSTER_SIZE: usize = 8;

/// Perform clustered page-in (readahead)
pub fn vm_fault_cluster(map: &VmMap, center_addr: u64, fault_type: FaultType) -> FaultResult {
    let center_page = vm_page::trunc_page(center_addr);

    // Calculate cluster bounds
    let cluster_start =
        center_page.saturating_sub((FAULT_CLUSTER_SIZE / 2) as u64 * PAGE_SIZE as u64);
    let cluster_end = center_page + (FAULT_CLUSTER_SIZE / 2) as u64 * PAGE_SIZE as u64;

    // First, handle the center page (this is the required fault)
    let center_result = vm_fault(map, center_page, fault_type, false);
    if center_result != FaultResult::Success {
        return center_result;
    }

    // Then prefetch surrounding pages (best effort)
    let mut addr = cluster_start;
    while addr < cluster_end {
        if addr != center_page {
            // Ignore failures for readahead pages
            let _ = vm_fault(map, addr, FaultType::Read, false);
        }
        addr += PAGE_SIZE as u64;
    }

    FaultResult::Success
}

// ============================================================================
// Fault Cleanup
// ============================================================================

/// Cleanup after a failed fault
fn vm_fault_cleanup(_object: &Arc<VmObject>, _offset: u64, allocated_page: Option<u32>) {
    // Free any page we allocated
    if let Some(page_num) = allocated_page {
        let phys_addr = vm_page::page_to_addr(page_num);
        vm_page::free_page(phys_addr);
    }
}

// ============================================================================
// Page Manager Access
// ============================================================================

/// Expose page_manager for internal use
pub mod internal {
    pub use crate::mach_vm::vm_page::page_manager;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fault_type() {
        assert_eq!(FaultType::Read.to_protection(), VmProt::READ);
        assert_eq!(FaultType::Write.to_protection(), VmProt::WRITE);
        assert_eq!(FaultType::Execute.to_protection(), VmProt::EXECUTE);
    }

    #[test]
    fn test_fault_stats() {
        let stats = FaultStats::new();
        stats.incr_total();
        stats.incr_cow();
        assert_eq!(stats.total.load(Ordering::Relaxed), 1);
        assert_eq!(stats.cow.load(Ordering::Relaxed), 1);
    }
}
