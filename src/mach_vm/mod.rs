//! Mach Virtual Memory Subsystem
//!
//! Based on Mach4 vm/ subsystem.
//! Provides memory management including:
//! - vm_page: Physical page management
//! - vm_object: Memory objects (backing store abstraction)
//! - vm_map: Address space management
//! - pmap: Physical map (hardware page table management)
//! - memory_object: External memory management interface
//! - vm_pageout: Page daemon for memory reclamation
//!
//! Note: This is separate from the EVM-related vm/ module.

pub mod memory_object;
pub mod pmap;
pub mod vm_external;
pub mod vm_fault;
pub mod vm_kern;
pub mod vm_map;
pub mod vm_object;
pub mod vm_page;
pub mod vm_pageout;
pub mod vm_user;
pub mod xmm;

pub use memory_object::{CopyStrategy, MemoryObject, MemoryObjectId, ReturnPolicy};
pub use pmap::{pmap_create, pmap_enter, pmap_extract, pmap_find, pmap_protect, pmap_remove, Pmap, PmapId};
pub use vm_external::{vm_external_create, vm_external_state_get, VmExternal, VmExternalState};
pub use vm_map::{EntryFlags, VmInherit, VmMap, VmMapEntry, VmMapId, VmProt};
pub use vm_object::{ObjectFlags, VmObject, VmObjectId};
pub use vm_page::{PageFlags, PageQueue, VmPage, PAGE_SIZE};
pub use vm_pageout::{vm_pageout_page, vm_pageout_setup, PageoutDaemon};
pub use xmm::{create_default_object, DefaultMemoryObject, ExistenceMap, XmmMethods, XmmObject};

/// Initialize the Mach VM subsystem
pub fn init() {
    vm_page::init();
    vm_object::init();
    vm_map::init();
    pmap::init();
    memory_object::init();
    vm_pageout::init();
    vm_external::init();
}

/// Initialize VM subsystem with physical memory range
///
/// This should be called after basic init() with the actual physical
/// memory range discovered during boot.
pub fn init_with_memory(start: u64, end: u64) {
    // Initialize page manager with memory range
    vm_page::init_memory(start, end);

    // Calculate total pages and configure pageout daemon
    let total_pages = ((end - start) / vm_page::PAGE_SIZE as u64) as u32;
    vm_pageout::configure(total_pages);

    // Start the pageout daemon
    vm_pageout::start();
}
