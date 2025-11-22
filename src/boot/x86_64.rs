//! x86_64 specific bootloader components for Mach_R
//! Pure Rust implementation for x86_64 architecture

use super::BootInfo;

/// x86_64 page table entry flags
pub const PAGE_PRESENT: u64 = 1 << 0;
pub const PAGE_WRITABLE: u64 = 1 << 1;
pub const PAGE_USER: u64 = 1 << 2;
pub const PAGE_WRITE_THROUGH: u64 = 1 << 3;
pub const PAGE_CACHE_DISABLE: u64 = 1 << 4;
pub const PAGE_ACCESSED: u64 = 1 << 5;
pub const PAGE_DIRTY: u64 = 1 << 6;
pub const PAGE_HUGE: u64 = 1 << 7;
pub const PAGE_NO_EXECUTE: u64 = 1 << 63;

/// x86_64 higher half kernel mapping
pub const KERNEL_VIRTUAL_BASE: u64 = 0xFFFFFFFF80000000;

/// x86_64 page table entry
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
    
    pub const fn is_present(self) -> bool {
        (self.0 & PAGE_PRESENT) != 0
    }
    
    pub const fn address(self) -> u64 {
        self.0 & 0x000FFFFFFFFFF000
    }
    
    pub const fn set_address(mut self, addr: u64) -> Self {
        self.0 = (self.0 & !0x000FFFFFFFFFF000) | (addr & 0x000FFFFFFFFFF000);
        self
    }
    
    pub const fn set_flags(mut self, flags: u64) -> Self {
        self.0 = (self.0 & 0x000FFFFFFFFFF000) | flags;
        self
    }
    
    pub const fn add_flags(mut self, flags: u64) -> Self {
        self.0 |= flags;
        self
    }
}

/// x86_64 page table (512 entries)
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
    
    pub fn physical_address(&self) -> u64 {
        self.entries.as_ptr() as u64
    }
}

/// x86_64 specific memory management
pub struct X86_64MemoryManager {
    pml4: &'static mut PageTable,
}

impl X86_64MemoryManager {
    /// Create new x86_64 memory manager
    pub fn new(pml4: &'static mut PageTable) -> Self {
        pml4.clear();
        Self { pml4 }
    }
    
    /// Map virtual address to physical address
    pub fn map_page(
        &mut self,
        _virtual_addr: u64,
        _physical_addr: u64,
        _flags: u64,
    ) -> Result<(), &'static str> {
        let _indices = [
            ((_virtual_addr >> 39) & 0x1FF) as usize, // PML4 index
            ((_virtual_addr >> 30) & 0x1FF) as usize, // PDPT index
            ((_virtual_addr >> 21) & 0x1FF) as usize, // PD index
            ((_virtual_addr >> 12) & 0x1FF) as usize, // PT index
        ];
        
        // For now, implement identity mapping
        // TODO: Implement full 4-level page table walking
        
        Ok(())
    }
    
    /// Get PML4 physical address
    pub fn pml4_address(&self) -> u64 {
        self.pml4.physical_address()
    }
}

/// x86_64 CPU feature detection
pub mod cpuid {
    /// Check if CPU supports required features
    pub fn check_required_features() -> Result<(), &'static str> {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            // Check for basic x86_64 features
            let mut _eax: u32;
            let _ebx: u32;
            let mut _ecx: u32;
            let mut edx: u32;
            
            // Check CPUID availability - avoid ebx register
            core::arch::asm!(
                "mov eax, 1",
                "cpuid",
                out("eax") _eax,
                out("ecx") _ecx,
                out("edx") edx,
            );
            _ebx = 0; // Skip ebx due to LLVM conflicts
            
            // Check for PAE (Physical Address Extension)
            if (edx & (1 << 6)) == 0 {
                return Err("PAE not supported");
            }
            
            // Check for MSR (Model Specific Registers)
            if (edx & (1 << 5)) == 0 {
                return Err("MSR not supported");
            }
        }
        
        Ok(())
    }
    
    /// Get CPU vendor string
    pub fn get_vendor_string() -> [u8; 12] {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let mut vendor = [0u8; 12];
            let mut ebx: u32;
            let mut ecx: u32;
            let mut edx: u32;
            
            core::arch::asm!(
                "mov eax, 0",
                "cpuid",
                out("ecx") ecx,
                out("edx") edx,
                out("eax") _,
            );
            // Manually get ebx
            core::arch::asm!("mov {0:e}, ebx", out(reg) ebx);
            
            vendor[0..4].copy_from_slice(&ebx.to_le_bytes());
            vendor[4..8].copy_from_slice(&edx.to_le_bytes());
            vendor[8..12].copy_from_slice(&ecx.to_le_bytes());
            
            vendor
        }
        #[cfg(not(target_arch = "x86_64"))]
        *b"Unknown     "
    }
}

/// x86_64 control registers
pub mod control_regs {
    /// Read CR0 register
    pub fn read_cr0() -> u64 {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let value: u64;
            core::arch::asm!("mov {}, cr0", out(reg) value);
            value
        }
        #[cfg(not(target_arch = "x86_64"))]
        0
    }
    
    /// Write CR0 register
    pub unsafe fn write_cr0(value: u64) {
        #[cfg(target_arch = "x86_64")]
        core::arch::asm!("mov cr0, {}", in(reg) value);
    }
    
    /// Read CR3 register (page table base)
    pub fn read_cr3() -> u64 {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let value: u64;
            core::arch::asm!("mov {}, cr3", out(reg) value);
            value
        }
        #[cfg(not(target_arch = "x86_64"))]
        0
    }
    
    /// Write CR3 register (set page table base)
    pub unsafe fn write_cr3(value: u64) {
        #[cfg(target_arch = "x86_64")]
        core::arch::asm!("mov cr3, {}", in(reg) value);
    }
    
    /// Read CR4 register
    pub fn read_cr4() -> u64 {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let value: u64;
            core::arch::asm!("mov {}, cr4", out(reg) value);
            value
        }
        #[cfg(not(target_arch = "x86_64"))]
        0
    }
    
    /// Write CR4 register
    pub unsafe fn write_cr4(value: u64) {
        #[cfg(target_arch = "x86_64")]
        core::arch::asm!("mov cr4, {}", in(reg) value);
    }
}

/// x86_64 segment management
pub mod segments {
    /// Global Descriptor Table entry
    #[repr(C, packed)]
    #[derive(Debug, Clone, Copy)]
    pub struct GdtEntry {
        pub limit_low: u16,
        pub base_low: u16,
        pub base_middle: u8,
        pub access: u8,
        pub granularity: u8,
        pub base_high: u8,
    }
    
    impl GdtEntry {
        pub const fn new() -> Self {
            Self {
                limit_low: 0,
                base_low: 0,
                base_middle: 0,
                access: 0,
                granularity: 0,
                base_high: 0,
            }
        }
        
        pub const fn code_segment() -> Self {
            Self {
                limit_low: 0xFFFF,
                base_low: 0,
                base_middle: 0,
                access: 0x9A, // Present, Ring 0, Executable, Readable
                granularity: 0xAF, // 64-bit, 4KB granularity
                base_high: 0,
            }
        }
        
        pub const fn data_segment() -> Self {
            Self {
                limit_low: 0xFFFF,
                base_low: 0,
                base_middle: 0,
                access: 0x92, // Present, Ring 0, Writable
                granularity: 0xCF, // 32-bit, 4KB granularity
                base_high: 0,
            }
        }
    }
    
    /// Global Descriptor Table
    #[repr(C, packed)]
    pub struct Gdt {
        pub entries: [GdtEntry; 8],
    }
    
    impl Gdt {
        pub const fn new() -> Self {
            Self {
                entries: [
                    GdtEntry::new(),              // Null segment
                    GdtEntry::code_segment(),     // Kernel code segment
                    GdtEntry::data_segment(),     // Kernel data segment
                    GdtEntry::new(),              // User code segment (placeholder)
                    GdtEntry::new(),              // User data segment (placeholder)
                    GdtEntry::new(),              // TSS (placeholder)
                    GdtEntry::new(),              // Reserved
                    GdtEntry::new(),              // Reserved
                ],
            }
        }
    }
    
    /// GDT descriptor for LGDT instruction
    #[repr(C, packed)]
    pub struct GdtDescriptor {
        pub limit: u16,
        pub base: u64,
    }
    
    /// Load GDT
    pub unsafe fn load_gdt(gdt: &Gdt) {
        let descriptor = GdtDescriptor {
            limit: (core::mem::size_of::<Gdt>() - 1) as u16,
            base: gdt as *const Gdt as u64,
        };
        
        #[cfg(target_arch = "x86_64")]
        core::arch::asm!(
            "lgdt [{}]",
            in(reg) &descriptor,
        );
    }
}

/// x86_64 interrupt management
pub mod interrupts {
    /// Disable interrupts
    pub fn disable() {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("cli");
        }
    }
    
    /// Enable interrupts
    pub fn enable() {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("sti");
        }
    }
    
    /// Halt CPU
    pub fn halt() -> ! {
        #[cfg(target_arch = "x86_64")]
        loop {
            unsafe {
                core::arch::asm!("hlt");
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        loop {
            core::hint::spin_loop();
        }
    }
}

/// x86_64 trampoline for kernel entry
pub fn execute_trampoline_x86_64(
    kernel_entry: u64,
    stack_top: u64,
    boot_info: &'static BootInfo,
) -> ! {
    unsafe {
        // Set up stack
        core::arch::asm!(
            "mov rsp, {}",
            in(reg) stack_top,
        );
        
        // Call kernel entry point
        let kernel_fn: extern "C" fn(&'static BootInfo) -> ! = 
            core::mem::transmute(kernel_entry);
        kernel_fn(boot_info);
    }
}

/// Initialize x86_64 specific features
pub fn init_x86_64() -> Result<(), &'static str> {
    // Check CPU features
    cpuid::check_required_features()?;
    
    // Disable interrupts during initialization
    interrupts::disable();
    
    // Set up basic GDT
    static mut GDT: segments::Gdt = segments::Gdt::new();
    unsafe {
        segments::load_gdt(&*core::ptr::addr_of!(GDT));
    }
    
    Ok(())
}