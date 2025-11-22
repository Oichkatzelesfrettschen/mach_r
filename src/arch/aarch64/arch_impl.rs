//! ARM64 (AArch64) architecture implementation

use crate::arch::{Architecture, CpuFeatures};

/// ARM64 architecture implementation
pub struct ArchImpl;

impl Architecture for ArchImpl {
    fn init() {
        // Initialize ARM64-specific features
        unsafe {
            // Set up system control register
            core::arch::asm!("msr sctlr_el1, {}", in(reg) 0x30c50838u64);
            // Set up memory attribute indirection register
            core::arch::asm!("msr mair_el1, {}", in(reg) 0xbbff440c0400u64);

            // Check current Exception Level (from real_os/kernel/src/arch/aarch64/mod.rs)
            let mut el: u64;
            core::arch::asm!("mrs {}, CurrentEL", out(reg) el);
            let el = (el >> 2) & 0b11;
            
            if el != 1 {
                // We should be in EL1 for kernel
                // In a real implementation, would transition here or panic
                // For now, print a warning or log.
                // TODO: Handle EL transition or panic if not in EL1
            }
        }
    }
    
    fn enable_interrupts() {
        unsafe {
            core::arch::asm!("msr daif, {}", in(reg) 0x0u64);
        }
    }
    
    fn disable_interrupts() {
        unsafe {
            core::arch::asm!("msr daif, {}", in(reg) 0xfu64);
        }
    }
    
    fn interrupts_enabled() -> bool {
        let daif: u64;
        unsafe {
            core::arch::asm!("mrs {}, daif", out(reg) daif);
        }
        (daif & 0xf) == 0
    }
    
    fn halt() -> ! {
        loop {
            unsafe {
                core::arch::asm!("wfi"); // Wait for interrupt
            }
        }
    }
    
    fn flush_tlb(addr: usize) {
        unsafe {
            core::arch::asm!("tlbi vae1, {}", in(reg) addr >> 12);
            core::arch::asm!("dsb sy");
            core::arch::asm!("isb");
        }
    }
    
    fn inb(_port: u16) -> u8 {
        // ARM64 uses memory-mapped I/O, not port I/O
        0
    }
    
    fn outb(_port: u16, _value: u8) {
        // ARM64 uses memory-mapped I/O, not port I/O
    }
    
    fn cpu_id() -> usize {
        let mpidr: u64;
        unsafe {
            core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr);
        }
        (mpidr & 0xff) as usize
    }
    
    fn keyboard_read() -> u8 {
        // ARM64 QEMU UART keyboard input via MMIO
        const UART_BASE: usize = 0x0900_0000;
        const UART_DR: usize = UART_BASE;
        const UART_FR: usize = UART_BASE + 0x18;
        
        // Check if data is available
        let flags = unsafe { core::ptr::read_volatile(UART_FR as *const u32) };
        if (flags & (1 << 4)) == 0 { // RXFE bit clear means data available
            let data = unsafe { core::ptr::read_volatile(UART_DR as *const u32) };
            (data & 0xff) as u8
        } else {
            0 // No data available
        }
    }
    
    fn current_timestamp() -> u64 {
        // Read the ARM64 system counter (CNTPCT_EL0)
        let counter: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntpct_el0", out(reg) counter);
        }
        
        // Convert counter ticks to microseconds
        // Assuming 24MHz counter (typical for ARM64)
        counter / 24
    }
}

/// Detect CPU features on ARM64
pub fn detect_features() -> CpuFeatures {
    let id_aa64pfr0: u64;
    let id_aa64mmfr0: u64;
    
    unsafe {
        core::arch::asm!("mrs {}, id_aa64pfr0_el1", out(reg) id_aa64pfr0);
        core::arch::asm!("mrs {}, id_aa64mmfr0_el1", out(reg) id_aa64mmfr0);
    }
    
    let has_fpu = ((id_aa64pfr0 >> 16) & 0xf) != 0xf; // FP field
    let has_sve = ((id_aa64pfr0 >> 32) & 0xf) != 0x0; // SVE field
    let has_neon = ((id_aa64pfr0 >> 20) & 0xf) != 0xf; // AdvSIMD field
    
    let pa_range = (id_aa64mmfr0 & 0xf) as u8;
    let physical_address_bits = match pa_range {
        0 => 32,
        1 => 36,
        2 => 40,
        3 => 42,
        4 => 44,
        5 => 48,
        6 => 52,
        _ => 48,
    };
    
    CpuFeatures {
        has_fpu,
        has_vmx: false, // Intel-specific
        has_svm: false, // AMD-specific
        has_sve,
        has_neon,
        has_msa: false, // MIPS-specific
        has_vector: false, // RISC-V specific
        cache_line_size: 64,
        physical_address_bits,
        virtual_address_bits: 48, // ARM64 standard
    }
}

/// Brief halt for yielding CPU (ARM64-specific)
pub fn halt_brief() {
    unsafe {
        core::arch::asm!("yield"); // ARM64 yield instruction
    }
}

/// ARM64 exception vector table entry
#[repr(C)]
pub struct ExceptionVector {
    pub handler: extern "C" fn(),
}

/// ARM64 system register access
pub mod sysreg {
    /// Read current exception level
    pub fn current_el() -> u8 {
        let el: u64;
        unsafe {
            core::arch::asm!("mrs {}, CurrentEL", out(reg) el);
        }
        ((el >> 2) & 0x3) as u8
    }
    
    /// Get exception syndrome register
    pub fn esr_el1() -> u64 {
        let esr: u64;
        unsafe {
            core::arch::asm!("mrs {}, esr_el1", out(reg) esr);
        }
        esr
    }
    
    /// Get fault address register
    pub fn far_el1() -> u64 {
        let far: u64;
        unsafe {
            core::arch::asm!("mrs {}, far_el1", out(reg) far);
        }
        far
    }
}