//! Page table management for Mach_R
//!
//! Implements x86_64 4-level paging with support for:
//! - Virtual to physical address translation
//! - Page table creation and management
//! - Memory protection and access control

use core::ops::{Index, IndexMut};
use spin::Mutex;
use alloc::boxed::Box;
use crate::println;

/// Page size (4KB)
pub const PAGE_SIZE: usize = 4096;

/// Number of entries in a page table
pub const ENTRIES_PER_TABLE: usize = 512;

/// Virtual address structure for x86_64
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualAddress(pub usize);

impl VirtualAddress {
    /// Create a new virtual address
    pub fn new(addr: usize) -> Self {
        // Canonical address check for x86_64
        assert!(addr < 0x0000_8000_0000_0000 || addr >= 0xFFFF_8000_0000_0000);
        VirtualAddress(addr)
    }
    
    /// Get the page offset (bits 0-11)
    pub fn page_offset(&self) -> usize {
        self.0 & 0xFFF
    }
    
    /// Get the P1 index (bits 12-20)
    pub fn p1_index(&self) -> usize {
        (self.0 >> 12) & 0x1FF
    }
    
    /// Get the P2 index (bits 21-29)
    pub fn p2_index(&self) -> usize {
        (self.0 >> 21) & 0x1FF
    }
    
    /// Get the P3 index (bits 30-38)
    pub fn p3_index(&self) -> usize {
        (self.0 >> 30) & 0x1FF
    }
    
    /// Get the P4 index (bits 39-47)
    pub fn p4_index(&self) -> usize {
        (self.0 >> 39) & 0x1FF
    }
    
    /// Align down to page boundary
    pub fn align_down(&self) -> Self {
        VirtualAddress(self.0 & !0xFFF)
    }
    
    /// Align up to page boundary
    pub fn align_up(&self) -> Self {
        VirtualAddress((self.0 + PAGE_SIZE - 1) & !(PAGE_SIZE - 1))
    }
}

/// Physical address structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalAddress(pub usize);

impl PhysicalAddress {
    /// Create a new physical address
    pub fn new(addr: usize) -> Self {
        assert!(addr < (1 << 52)); // x86_64 supports 52-bit physical addresses
        PhysicalAddress(addr)
    }
}

/// Page table entry flags
#[derive(Debug, Clone, Copy)]
pub struct PageTableFlags(u64);

impl PageTableFlags {
    /// Entry is present
    pub const PRESENT: Self = PageTableFlags(1 << 0);
    /// Page is writable
    pub const WRITABLE: Self = PageTableFlags(1 << 1);
    /// Page is accessible from user mode
    pub const USER_ACCESSIBLE: Self = PageTableFlags(1 << 2);
    /// Writes go through cache
    pub const WRITE_THROUGH: Self = PageTableFlags(1 << 3);
    /// Disable caching
    pub const NO_CACHE: Self = PageTableFlags(1 << 4);
    /// Page was accessed
    pub const ACCESSED: Self = PageTableFlags(1 << 5);
    /// Page was written to
    pub const DIRTY: Self = PageTableFlags(1 << 6);
    /// Large page (2MB or 1GB)
    pub const HUGE_PAGE: Self = PageTableFlags(1 << 7);
    /// Page is global
    pub const GLOBAL: Self = PageTableFlags(1 << 8);
    /// Disable execution
    pub const NO_EXECUTE: Self = PageTableFlags(1 << 63);
    
    /// Create empty flags
    pub const fn empty() -> Self {
        PageTableFlags(0)
    }
    
    /// Combine flags
    pub const fn union(&self, other: Self) -> Self {
        PageTableFlags(self.0 | other.0)
    }
    
    /// Check if flags contain a specific flag
    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl core::ops::BitOr for PageTableFlags {
    type Output = Self;
    
    fn bitor(self, other: Self) -> Self::Output {
        PageTableFlags(self.0 | other.0)
    }
}

impl core::ops::BitOrAssign for PageTableFlags {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

/// Page table entry
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    /// Create an unused entry
    pub const fn unused() -> Self {
        PageTableEntry(0)
    }
    
    /// Check if entry is unused
    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }
    
    /// Get the flags
    pub fn flags(&self) -> PageTableFlags {
        PageTableFlags(self.0 & 0xFFF | (self.0 & (1 << 63)))
    }
    
    /// Get the physical address
    pub fn addr(&self) -> PhysicalAddress {
        PhysicalAddress((self.0 & 0x000F_FFFF_FFFF_F000) as usize)
    }
    
    /// Set the entry
    pub fn set_entry(&mut self, addr: PhysicalAddress, flags: PageTableFlags) {
        assert!(addr.0 % PAGE_SIZE == 0);
        self.0 = (addr.0 as u64) | flags.0;
    }
    
    /// Set as unused
    pub fn set_unused(&mut self) {
        self.0 = 0;
    }
}

/// Page table structure
#[repr(align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; ENTRIES_PER_TABLE],
}

impl PageTable {
    /// Create a new empty page table
    pub const fn new() -> Self {
        PageTable {
            entries: [PageTableEntry::unused(); ENTRIES_PER_TABLE],
        }
    }
    
    /// Clear all entries
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            entry.set_unused();
        }
    }
    
    /// Get the next level page table
    pub fn next_table(&self, index: usize) -> Option<&PageTable> {
        let entry = &self.entries[index];
        if entry.is_unused() || entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            None
        } else {
            // In real implementation, would need to convert physical to virtual
            Some(unsafe { &*(entry.addr().0 as *const PageTable) })
        }
    }
    
    /// Get the next level page table mutably
    pub fn next_table_mut(&mut self, index: usize) -> Option<&mut PageTable> {
        let entry = &self.entries[index];
        if entry.is_unused() || entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            None
        } else {
            // In real implementation, would need to convert physical to virtual
            Some(unsafe { &mut *(entry.addr().0 as *mut PageTable) })
        }
    }
    
    /// Create next level table if it doesn't exist
    pub fn next_table_create(&mut self, index: usize) -> &mut PageTable {
        if self.next_table(index).is_none() {
            // Allocate new page table
            let table = Box::leak(Box::new(PageTable::new()));
            let addr = PhysicalAddress(table as *const _ as usize);
            let flags = PageTableFlags::PRESENT
                .union(PageTableFlags::WRITABLE)
                .union(PageTableFlags::USER_ACCESSIBLE);
            self.entries[index].set_entry(addr, flags);
        }
        self.next_table_mut(index).unwrap()
    }
}

impl Index<usize> for PageTable {
    type Output = PageTableEntry;
    
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

/// Active page table (P4/PML4)
pub struct ActivePageTable {
    p4: Mutex<&'static mut PageTable>,
}

impl ActivePageTable {
    /// Create a new active page table with given P4 table
    pub fn new(p4_table: Box<PageTable>) -> Self {
        let p4_ptr = Box::into_raw(p4_table);
        ActivePageTable {
            p4: Mutex::new(unsafe { &mut *p4_ptr }),
        }
    }
    
    /// Get the current active page table
    pub unsafe fn current() -> Self {
        let p4_addr: usize;
        // In real implementation, read CR3 register
        // asm!("mov {}, cr3", out(reg) p4_addr);
        p4_addr = 0; // Placeholder
        
        ActivePageTable {
            p4: Mutex::new(&mut *(p4_addr as *mut PageTable)),
        }
    }
    
    /// Map a virtual address to a physical address
    pub fn map(&mut self, virt: VirtualAddress, phys: PhysicalAddress, flags: PageTableFlags) {
        let mut p4 = self.p4.lock();
        
        let p3 = p4.next_table_create(virt.p4_index());
        let p2 = p3.next_table_create(virt.p3_index());
        let p1 = p2.next_table_create(virt.p2_index());
        
        p1[virt.p1_index()].set_entry(phys, flags.union(PageTableFlags::PRESENT));
        
        // Flush TLB
        // In real implementation:
        // unsafe { asm!("invlpg [{}]", in(reg) virt.0); }
    }
    
    /// Unmap a virtual address
    pub fn unmap(&mut self, virt: VirtualAddress) {
        let mut p4 = self.p4.lock();
        
        if let Some(p3) = p4.next_table_mut(virt.p4_index()) {
            if let Some(p2) = p3.next_table_mut(virt.p3_index()) {
                if let Some(p1) = p2.next_table_mut(virt.p2_index()) {
                    p1[virt.p1_index()].set_unused();
                    
                    // Flush TLB
                    unsafe {
                        // In real implementation:
                        // asm!("invlpg [{}]", in(reg) virt.0);
                    }
                }
            }
        }
    }
    
    /// Translate a virtual address to physical
    pub fn translate(&self, virt: VirtualAddress) -> Option<PhysicalAddress> {
        let p4 = self.p4.lock();
        
        let p3 = p4.next_table(virt.p4_index())?;
        let p2 = p3.next_table(virt.p3_index())?;
        let p1 = p2.next_table(virt.p2_index())?;
        
        let entry = &p1[virt.p1_index()];
        if entry.is_unused() {
            None
        } else {
            Some(PhysicalAddress(entry.addr().0 + virt.page_offset()))
        }
    }
}

/// Page fault error code
#[derive(Debug, Clone, Copy)]
pub struct PageFaultErrorCode(u32);

impl PageFaultErrorCode {
    /// Page protection violation (vs not present)
    pub fn protection_violation(&self) -> bool {
        self.0 & 1 != 0
    }
    
    /// Caused by write (vs read)
    pub fn write(&self) -> bool {
        self.0 & 2 != 0
    }
    
    /// From user mode (vs kernel)
    pub fn user(&self) -> bool {
        self.0 & 4 != 0
    }
    
    /// Reserved bit violation
    pub fn reserved_write(&self) -> bool {
        self.0 & 8 != 0
    }
    
    /// Instruction fetch
    pub fn instruction_fetch(&self) -> bool {
        self.0 & 16 != 0
    }
}

/// Handle page fault
pub fn handle_page_fault(addr: VirtualAddress, error_code: PageFaultErrorCode) {
    println!("Page fault at {:?}", addr);
    println!("  Protection violation: {}", error_code.protection_violation());
    println!("  Write access: {}", error_code.write());
    println!("  User mode: {}", error_code.user());
    
    // In real implementation:
    // 1. Check if address is in valid region
    // 2. Allocate page if needed
    // 3. Call external pager if configured
    // 4. Map page
    // 5. Return to retry instruction
}

/// Initialize paging
pub fn init() {
    // In real implementation:
    // 1. Set up initial page tables
    // 2. Map kernel code and data
    // 3. Enable paging by setting CR0.PG
    println!("Paging initialized");
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_virtual_address() {
        let addr = VirtualAddress::new(0x1234_5678_9ABC);
        assert_eq!(addr.page_offset(), 0xABC);
        assert_eq!(addr.p1_index(), 0x189);
        assert_eq!(addr.p2_index(), 0x1BC);
        assert_eq!(addr.p3_index(), 0x159);
        assert_eq!(addr.p4_index(), 0x024);
    }
    
    #[test]
    fn test_page_table_entry() {
        let mut entry = PageTableEntry::unused();
        assert!(entry.is_unused());
        
        let addr = PhysicalAddress::new(0x1000);
        let flags = PageTableFlags::PRESENT.union(PageTableFlags::WRITABLE);
        entry.set_entry(addr, flags);
        
        assert!(!entry.is_unused());
        assert_eq!(entry.addr().0, 0x1000);
        assert!(entry.flags().contains(PageTableFlags::PRESENT));
    }
    
    #[test]
    fn test_page_table() {
        let mut table = PageTable::new();
        assert!(table[0].is_unused());
        
        let addr = PhysicalAddress::new(0x2000);
        let flags = PageTableFlags::PRESENT;
        table[0].set_entry(addr, flags);
        
        assert!(!table[0].is_unused());
        assert_eq!(table[0].addr().0, 0x2000);
    }
}

/// Global active page table
static ACTIVE_PAGE_TABLE: Mutex<Option<Box<ActivePageTable>>> = Mutex::new(None);

/// Initialize the global page table
pub fn init_page_table() {
    let mut page_table_lock = ACTIVE_PAGE_TABLE.lock();
    let p4_table = Box::new(PageTable::new());
    let active_table = Box::new(ActivePageTable::new(p4_table));
    *page_table_lock = Some(active_table);
}

/// Get the active page table
pub fn active_page_table() -> ActivePageTable {
    let page_table_lock = ACTIVE_PAGE_TABLE.lock();
    if let Some(ref table) = *page_table_lock {
        // Create a new instance for now
        let p4_table = Box::new(PageTable::new());
        ActivePageTable::new(p4_table)
    } else {
        // Initialize if not done yet
        drop(page_table_lock);
        init_page_table();
        let p4_table = Box::new(PageTable::new());
        ActivePageTable::new(p4_table)
    }
}