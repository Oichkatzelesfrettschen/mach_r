//! ARM64 page table setup for Mach_R bootloader
//! Pure Rust implementation of AArch64 MMU page table management


/// ARM64 page sizes
pub const PAGE_SIZE: usize = 4096;
pub const LARGE_PAGE_SIZE: usize = 2 * 1024 * 1024; // 2MB
pub const HUGE_PAGE_SIZE: usize = 1024 * 1024 * 1024; // 1GB

/// Virtual address layout
pub const HIGHER_HALF_OFFSET: u64 = 0xFFFF800000000000;
pub const KERNEL_VIRTUAL_BASE: u64 = 0xFFFF800000000000;
pub const USER_SPACE_END: u64 = 0x0000800000000000;

/// Page table entry constants
pub const PTE_VALID: u64 = 1 << 0;
pub const PTE_TABLE: u64 = 1 << 1;
pub const PTE_USER: u64 = 1 << 6;
pub const PTE_READONLY: u64 = 1 << 7;
pub const PTE_SHARED: u64 = 3 << 8;
pub const PTE_AF: u64 = 1 << 10; // Access flag
pub const PTE_NG: u64 = 1 << 11; // Not global
pub const PTE_DBM: u64 = 1 << 51; // Dirty bit modifier
pub const PTE_CONTIGUOUS: u64 = 1 << 52;
pub const PTE_PXN: u64 = 1 << 53; // Privileged execute never
pub const PTE_XN: u64 = 1 << 54;  // Execute never

/// Memory attributes for MAIR_EL1
pub const MAIR_DEVICE_NGNRNE: u64 = 0x00; // Device non-gathering, non-reordering, no early write acknowledgment
pub const MAIR_DEVICE_NGNRE: u64 = 0x04;  // Device non-gathering, non-reordering, early write acknowledgment
pub const MAIR_DEVICE_GRE: u64 = 0x0C;    // Device gathering, reordering, early write acknowledgment
pub const MAIR_NORMAL_NC: u64 = 0x44;     // Normal memory, non-cacheable
pub const MAIR_NORMAL: u64 = 0xFF;        // Normal memory, write-back cacheable

/// Memory attribute indexes (for page table entries)
pub const ATTRIDX_DEVICE: u64 = 0;
pub const ATTRIDX_NORMAL_NC: u64 = 1;
pub const ATTRIDX_NORMAL: u64 = 2;

/// Page table levels
pub const PT_LEVEL_0: usize = 0; // PGD - 512GB per entry
pub const PT_LEVEL_1: usize = 1; // PUD - 1GB per entry  
pub const PT_LEVEL_2: usize = 2; // PMD - 2MB per entry
pub const PT_LEVEL_3: usize = 3; // PTE - 4KB per entry

/// Page table entry
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry(pub u64);

impl PageTableEntry {
    pub const fn new() -> Self {
        Self(0)
    }
    
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }
    
    pub const fn value(self) -> u64 {
        self.0
    }
    
    pub const fn is_valid(self) -> bool {
        (self.0 & PTE_VALID) != 0
    }
    
    pub const fn is_table(self) -> bool {
        (self.0 & PTE_TABLE) != 0
    }
    
    pub const fn address(self) -> u64 {
        self.0 & 0x0000FFFFFFFFF000
    }
    
    pub const fn set_address(mut self, addr: u64) -> Self {
        self.0 = (self.0 & !0x0000FFFFFFFFF000) | (addr & 0x0000FFFFFFFFF000);
        self
    }
    
    pub const fn set_flags(mut self, flags: u64) -> Self {
        self.0 = (self.0 & 0x0000FFFFFFFFF000) | flags;
        self
    }
    
    pub const fn add_flags(mut self, flags: u64) -> Self {
        self.0 |= flags;
        self
    }
    
    pub const fn remove_flags(mut self, flags: u64) -> Self {
        self.0 &= !flags;
        self
    }
}

/// Page table (512 entries per table)
#[repr(C, align(4096))]
#[derive(Copy, Clone)]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::new(); 512],
        }
    }
    
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            entry.0 = 0;
        }
    }
    
    pub fn get_entry(&self, index: usize) -> PageTableEntry {
        self.entries[index]
    }
    
    pub fn set_entry(&mut self, index: usize, entry: PageTableEntry) {
        self.entries[index] = entry;
    }
    
    pub fn as_ptr(&self) -> *const PageTableEntry {
        self.entries.as_ptr()
    }
    
    pub fn as_mut_ptr(&mut self) -> *mut PageTableEntry {
        self.entries.as_mut_ptr()
    }
    
    pub fn physical_address(&self) -> u64 {
        self.as_ptr() as u64
    }
}

/// Page table manager
pub struct PageTableManager {
    root_table: *mut PageTable,
    allocated_count: usize,
}

impl PageTableManager {
    /// Create new page table manager with root table
    pub fn new(root_table: &'static mut PageTable) -> Self {
        root_table.clear();
        Self {
            root_table: root_table as *mut PageTable,
            allocated_count: 0,
        }
    }
    
    /// Get root page table physical address
    pub fn root_address(&self) -> u64 {
        self.root_table as u64
    }
    
    /// Allocate a new page table
    pub fn allocate_table(&mut self, allocate_fn: impl FnOnce() -> Option<&'static mut PageTable>) -> Option<*mut PageTable> {
        if let Some(table) = allocate_fn() {
            table.clear();
            self.allocated_count += 1;
            Some(table as *mut PageTable)
        } else {
            None
        }
    }
    
    /// Map a virtual address to physical address
    pub fn map_page(
        &mut self,
        virtual_addr: u64,
        physical_addr: u64,
        flags: u64,
        allocate_fn: impl Fn() -> Option<&'static mut PageTable>,
    ) -> Result<(), &'static str> {
        let vpn = [
            ((virtual_addr >> 39) & 0x1FF) as usize, // L0 index
            ((virtual_addr >> 30) & 0x1FF) as usize, // L1 index  
            ((virtual_addr >> 21) & 0x1FF) as usize, // L2 index
            ((virtual_addr >> 12) & 0x1FF) as usize, // L3 index
        ];
        
        let mut current_table = self.root_table;
        
        // Walk through levels 0-2, creating tables as needed
        for level in 0..3 {
            let entry = unsafe { (*current_table).get_entry(vpn[level]) };
            
            if !entry.is_valid() {
                // Need to create a new table
                let new_table_ptr = self.allocate_table(&allocate_fn)
                    .ok_or("Failed to allocate page table")?;
                
                let new_entry = PageTableEntry::new()
                    .set_address(new_table_ptr as u64)
                    .set_flags(PTE_VALID | PTE_TABLE);
                
                unsafe {
                    (*current_table).set_entry(vpn[level], new_entry);
                }
                current_table = new_table_ptr;
            } else if entry.is_table() {
                // Follow existing table
                current_table = entry.address() as *mut PageTable;
            } else {
                return Err("Invalid page table entry");
            }
        }
        
        // Set the final page entry
        let page_entry = PageTableEntry::new()
            .set_address(physical_addr & !0xFFF)
            .set_flags(flags | PTE_VALID);
        
        unsafe {
            (*current_table).set_entry(vpn[3], page_entry);
        }
        
        Ok(())
    }
    
    /// Map a large range of memory
    pub fn map_range(
        &mut self,
        virtual_start: u64,
        physical_start: u64,
        size: u64,
        flags: u64,
        allocate_fn: impl Fn() -> Option<&'static mut PageTable>,
    ) -> Result<(), &'static str> {
        let mut virtual_addr = virtual_start;
        let mut physical_addr = physical_start;
        let end_addr = virtual_start + size;
        
        while virtual_addr < end_addr {
            self.map_page(virtual_addr, physical_addr, flags, &allocate_fn)?;
            virtual_addr += PAGE_SIZE as u64;
            physical_addr += PAGE_SIZE as u64;
        }
        
        Ok(())
    }
    
    /// Create identity mapping for low memory
    pub fn identity_map_low_memory(
        &mut self,
        size: u64,
        allocate_fn: impl Fn() -> Option<&'static mut PageTable>,
    ) -> Result<(), &'static str> {
        let flags = PTE_AF | (ATTRIDX_NORMAL << 2);
        self.map_range(0, 0, size, flags, allocate_fn)
    }
    
    /// Create higher half mapping for kernel
    pub fn map_kernel_higher_half(
        &mut self,
        physical_start: u64,
        size: u64,
        allocate_fn: impl Fn() -> Option<&'static mut PageTable>,
    ) -> Result<(), &'static str> {
        let flags = PTE_AF | (ATTRIDX_NORMAL << 2);
        self.map_range(KERNEL_VIRTUAL_BASE, physical_start, size, flags, allocate_fn)
    }
    
    /// Map device memory
    pub fn map_device(
        &mut self,
        virtual_addr: u64,
        physical_addr: u64,
        size: u64,
        allocate_fn: impl Fn() -> Option<&'static mut PageTable>,
    ) -> Result<(), &'static str> {
        let flags = PTE_AF | PTE_XN | (ATTRIDX_DEVICE << 2);
        self.map_range(virtual_addr, physical_addr, size, flags, allocate_fn)
    }
}

/// Initialize ARM64 MMU
pub fn init_mmu(page_table_addr: u64) {
    unsafe {
        // Set up MAIR_EL1 (Memory Attribute Indirection Register)
        let mair_value = 
            (MAIR_DEVICE_NGNRNE << 0) |   // Index 0: Device memory
            (MAIR_NORMAL_NC << 8) |       // Index 1: Normal non-cacheable
            (MAIR_NORMAL << 16);          // Index 2: Normal cacheable
        
        core::arch::asm!(
            "msr mair_el1, {}",
            in(reg) mair_value
        );
        
        // Set up TCR_EL1 (Translation Control Register)
        let tcr_value = 
            (16u64 << 0) |     // T0SZ: 48-bit virtual addresses for TTBR0_EL1
            (0u64 << 6) |      // EPD0: Enable TTBR0_EL1 translation
            (0u64 << 7) |      // IRGN0: Normal memory, Inner Write-Back cacheable
            (0u64 << 8) |      // ORGN0: Normal memory, Outer Write-Back cacheable  
            (0u64 << 10) |     // SH0: Non-shareable
            (0u64 << 12) |     // TG0: 4KB granule
            (16u64 << 16) |    // T1SZ: 48-bit virtual addresses for TTBR1_EL1
            (0u64 << 22) |     // A1: ASID defined by TTBR0_EL1
            (0u64 << 23) |     // EPD1: Enable TTBR1_EL1 translation
            (0u64 << 24) |     // IRGN1: Normal memory, Inner Write-Back cacheable
            (0u64 << 26) |     // ORGN1: Normal memory, Outer Write-Back cacheable
            (0u64 << 28) |     // SH1: Non-shareable
            (0u64 << 30) |     // TG1: 4KB granule
            (0u64 << 32) |     // IPS: 40-bit physical address space
            (0u64 << 35) |     // AS: 8-bit ASID
            (1u64 << 36) |     // TBI0: Top Byte Ignore for TTBR0_EL1
            (1u64 << 37);      // TBI1: Top Byte Ignore for TTBR1_EL1
        
        core::arch::asm!(
            "msr tcr_el1, {}",
            in(reg) tcr_value
        );
        
        // Set TTBR1_EL1 (Translation Table Base Register 1) for higher half
        core::arch::asm!(
            "msr ttbr1_el1, {}",
            in(reg) page_table_addr
        );
        
        // Set TTBR0_EL1 for user space (initially same as kernel)
        core::arch::asm!(
            "msr ttbr0_el1, {}",
            in(reg) page_table_addr
        );
        
        // Invalidate TLB
        core::arch::asm!("tlbi vmalle1is");
        core::arch::asm!("dsb ish");
        core::arch::asm!("isb");
        
        // Enable MMU
        let mut sctlr: u64;
        core::arch::asm!("mrs {}, sctlr_el1", out(reg) sctlr);
        sctlr |= 1; // Set M bit to enable MMU
        core::arch::asm!(
            "msr sctlr_el1, {}",
            in(reg) sctlr
        );
        
        core::arch::asm!("isb");
    }
}

/// Convert physical address to higher half virtual address  
pub const fn phys_to_virt(physical_addr: u64) -> u64 {
    physical_addr + HIGHER_HALF_OFFSET
}

/// Convert higher half virtual address to physical address
pub const fn virt_to_phys(virtual_addr: u64) -> u64 {
    virtual_addr - HIGHER_HALF_OFFSET
}

/// Get virtual address index for page table level
pub const fn vaddr_index(vaddr: u64, level: usize) -> usize {
    ((vaddr >> (12 + (3 - level) * 9)) & 0x1FF) as usize
}

/// Align address down to page boundary
pub const fn page_align_down(addr: u64) -> u64 {
    addr & !(PAGE_SIZE as u64 - 1)
}

/// Align address up to page boundary  
pub const fn page_align_up(addr: u64) -> u64 {
    (addr + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1)
}

/// Calculate number of pages needed for size
pub const fn pages_needed(size: u64) -> u64 {
    (size + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64
}