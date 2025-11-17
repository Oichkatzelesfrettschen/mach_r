//! Architecture-specific code for Mach_R
//!
//! Supports multiple architectures with platform-specific implementations
//! while maintaining a unified Rust codebase.

// Architecture modules
#[cfg(target_arch = "aarch64")]
pub mod aarch64;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

#[cfg(target_arch = "mips64")]
pub mod mips64;

#[cfg(target_arch = "riscv64")]
pub mod riscv64;

// Re-export current architecture
#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[cfg(target_arch = "mips64")]
pub use mips64::*;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

/// Common architecture traits that all platforms must implement
pub trait Architecture {
    /// Initialize the architecture
    fn init();
    
    /// Enable interrupts
    fn enable_interrupts();
    
    /// Disable interrupts
    fn disable_interrupts();
    
    /// Check if interrupts are enabled
    fn interrupts_enabled() -> bool;
    
    /// Halt the processor
    fn halt() -> !;
    
    /// Flush TLB for a specific address
    fn flush_tlb(addr: usize);
    
    /// Read from an I/O port (x86-specific, no-op on others)
    fn inb(port: u16) -> u8;
    
    /// Write to an I/O port (x86-specific, no-op on others)
    fn outb(port: u16, value: u8);
    
    /// Get current CPU ID
    fn cpu_id() -> usize;
    
    /// Read keyboard scancode
    fn keyboard_read() -> u8;
    
    /// Get current timestamp in microseconds
    fn current_timestamp() -> u64;
}

/// Page size for current architecture
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64", target_arch = "riscv64"))]
pub const PAGE_SIZE: usize = 4096;

#[cfg(target_arch = "mips64")]
pub const PAGE_SIZE: usize = 4096; // Can be 4K, 16K, or 64K on MIPS

/// Number of page table levels
#[cfg(target_arch = "aarch64")]
pub const PAGE_LEVELS: usize = 4; // 4-level for 48-bit VA

#[cfg(target_arch = "x86_64")]
pub const PAGE_LEVELS: usize = 4; // 4-level (PML4)

#[cfg(target_arch = "mips64")]
pub const PAGE_LEVELS: usize = 3; // TLB-based

#[cfg(target_arch = "riscv64")]
pub const PAGE_LEVELS: usize = 4; // Sv48

/// Boot information structure
#[repr(C)]
pub struct BootInfo {
    /// Memory map
    pub memory_map: &'static [MemoryRegion],
    /// Kernel start address
    pub kernel_start: usize,
    /// Kernel end address
    pub kernel_end: usize,
    /// Initial stack pointer
    pub stack_top: usize,
    /// Device tree blob (ARM/RISC-V)
    pub dtb: Option<usize>,
    /// UEFI system table (x86_64/ARM64)
    pub uefi_table: Option<usize>,
}

/// Memory region descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub start: usize,
    pub end: usize,
    pub kind: MemoryKind,
}

/// Memory region types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryKind {
    Available = 0,
    Reserved = 1,
    AcpiReclaimable = 2,
    AcpiNvs = 3,
    Kernel = 4,
    FrameBuffer = 5,
    DeviceMemory = 6,
}

/// CPU features detection
pub struct CpuFeatures {
    pub has_fpu: bool,
    pub has_vmx: bool,      // Intel VT-x
    pub has_svm: bool,      // AMD-V
    pub has_sve: bool,      // ARM SVE
    pub has_neon: bool,     // ARM NEON
    pub has_msa: bool,      // MIPS MSA
    pub has_vector: bool,   // RISC-V V extension
    pub cache_line_size: usize,
    pub physical_address_bits: u8,
    pub virtual_address_bits: u8,
}

/// Get CPU features for current architecture
pub fn cpu_features() -> CpuFeatures {
    #[cfg(target_arch = "aarch64")]
    {
        aarch64::detect_features()
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        x86_64::detect_features()
    }
    
    #[cfg(target_arch = "mips64")]
    {
        mips64::detect_features()
    }
    
    #[cfg(target_arch = "riscv64")]
    {
        riscv64::detect_features()
    }
    
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64", target_arch = "mips64", target_arch = "riscv64")))]
    {
        CpuFeatures {
            has_fpu: false,
            has_vmx: false,
            has_svm: false,
            has_sve: false,
            has_neon: false,
            has_msa: false,
            has_vector: false,
            cache_line_size: 64,
            physical_address_bits: 48,
            virtual_address_bits: 48,
        }
    }
}

/// Architecture-specific keyboard reading
pub fn keyboard_read() -> u8 {
    #[cfg(target_arch = "aarch64")]
    {
        aarch64::ArchImpl::keyboard_read()
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        x86_64::ArchImpl::keyboard_read()
    }
    
    #[cfg(target_arch = "mips64")]
    {
        mips64::ArchImpl::keyboard_read()
    }
    
    #[cfg(target_arch = "riscv64")]
    {
        riscv64::ArchImpl::keyboard_read()
    }
    
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64", target_arch = "mips64", target_arch = "riscv64")))]
    {
        0x1c // Return Enter key scancode as default
    }
}

/// Get current timestamp in microseconds
pub fn current_timestamp() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        aarch64::ArchImpl::current_timestamp()
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        x86_64::ArchImpl::current_timestamp()
    }
    
    #[cfg(target_arch = "mips64")]
    {
        mips64::ArchImpl::current_timestamp()
    }
    
    #[cfg(target_arch = "riscv64")]
    {
        riscv64::ArchImpl::current_timestamp()
    }
    
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64", target_arch = "mips64", target_arch = "riscv64")))]
    {
        0 // Default timestamp
    }
}