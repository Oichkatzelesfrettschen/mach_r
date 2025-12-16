//! ARM64 (AArch64) architecture implementation

use crate::arch::{Architecture, CpuFeatures};

/// ARM64 architecture implementation
pub struct ArchImpl;

// =============================================================================
// Real implementation (kernel mode)
// =============================================================================

#[cfg(not(test))]
impl Architecture for ArchImpl {
    fn init() {
        // Initialize ARM64-specific features
        unsafe {
            // Set up system control register
            core::arch::asm!("msr sctlr_el1, {}", in(reg) 0x30c50838u64);
            // Set up memory attribute indirection register
            core::arch::asm!("msr mair_el1, {}", in(reg) 0xbbff440c0400u64);

            // Check current Exception Level
            let mut el: u64;
            core::arch::asm!("mrs {}, CurrentEL", out(reg) el);
            let el = (el >> 2) & 0b11;

            if el != 1 {
                // We should be in EL1 for kernel
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
        if (flags & (1 << 4)) == 0 {
            // RXFE bit clear means data available
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
        // Convert counter ticks to microseconds (assuming 24MHz counter)
        counter / 24
    }
}

// =============================================================================
// Test mode stubs (no privileged instructions)
// =============================================================================

#[cfg(test)]
impl Architecture for ArchImpl {
    fn init() {
        // No-op in test mode
    }

    fn enable_interrupts() {
        // No-op in test mode
    }

    fn disable_interrupts() {
        // No-op in test mode
    }

    fn interrupts_enabled() -> bool {
        true // Assume enabled in test mode
    }

    fn halt() -> ! {
        loop {
            core::hint::spin_loop();
        }
    }

    fn flush_tlb(_addr: usize) {
        // No-op in test mode
    }

    fn inb(_port: u16) -> u8 {
        0
    }

    fn outb(_port: u16, _value: u8) {
        // No-op
    }

    fn cpu_id() -> usize {
        0 // Always CPU 0 in test mode
    }

    fn keyboard_read() -> u8 {
        0 // No input in test mode
    }

    fn current_timestamp() -> u64 {
        // Use std time in test mode if available, otherwise return 0
        0
    }
}

// =============================================================================
// Feature detection
// =============================================================================

/// Detect CPU features on ARM64
#[cfg(not(test))]
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
        has_msa: false,    // MIPS-specific
        has_vector: false, // RISC-V specific
        cache_line_size: 64,
        physical_address_bits,
        virtual_address_bits: 48, // ARM64 standard
    }
}

/// Detect CPU features (test mode stub)
#[cfg(test)]
pub fn detect_features() -> CpuFeatures {
    CpuFeatures {
        has_fpu: true,
        has_vmx: false,
        has_svm: false,
        has_sve: false,
        has_neon: true,
        has_msa: false,
        has_vector: false,
        cache_line_size: 64,
        physical_address_bits: 48,
        virtual_address_bits: 48,
    }
}

/// Brief halt for yielding CPU
#[cfg(not(test))]
pub fn halt_brief() {
    unsafe {
        core::arch::asm!("yield"); // ARM64 yield instruction
    }
}

/// Brief halt (test mode)
#[cfg(test)]
pub fn halt_brief() {
    core::hint::spin_loop();
}

/// ARM64 exception vector table entry
#[repr(C)]
pub struct ExceptionVector {
    pub handler: extern "C" fn(),
}

// =============================================================================
// User Mode Entry
// =============================================================================

/// Enter user mode and begin execution at the given address
///
/// This sets up the exception return frame and performs ERET to transition
/// from EL1 (kernel) to EL0 (user mode).
///
/// # Safety
/// This function never returns. It transitions execution to user mode.
#[cfg(not(test))]
pub unsafe fn enter_user_mode(entry_point: u64, stack_pointer: u64) -> ! {
    // Set up SPSR_EL1 for return to EL0
    // Bits [4:0] = 0b00000 (EL0t mode)
    // Bit 6 = 0 (FIQ masked - we'll enable in EL0)
    // Bit 7 = 0 (IRQ masked - we'll enable in EL0)
    // Bit 8 = 0 (SError masked)
    // Bit 9 = 0 (Debug masked)
    let spsr_el0: u64 = 0b00000_0_0_0_0; // EL0 with all masks clear

    // Set ELR_EL1 to the entry point (where ERET will jump to)
    core::arch::asm!("msr elr_el1, {}", in(reg) entry_point);

    // Set SPSR_EL1 for EL0 return
    core::arch::asm!("msr spsr_el1, {}", in(reg) spsr_el0);

    // Set up SP_EL0 (user stack pointer)
    core::arch::asm!("msr sp_el0, {}", in(reg) stack_pointer);

    // Clear general purpose registers to prevent kernel data leakage
    core::arch::asm!(
        "mov x0, #0",
        "mov x1, #0",
        "mov x2, #0",
        "mov x3, #0",
        "mov x4, #0",
        "mov x5, #0",
        "mov x6, #0",
        "mov x7, #0",
        "mov x8, #0",
        "mov x9, #0",
        "mov x10, #0",
        "mov x11, #0",
        "mov x12, #0",
        "mov x13, #0",
        "mov x14, #0",
        "mov x15, #0",
        "mov x16, #0",
        "mov x17, #0",
        "mov x18, #0",
        "mov x19, #0",
        "mov x20, #0",
        "mov x21, #0",
        "mov x22, #0",
        "mov x23, #0",
        "mov x24, #0",
        "mov x25, #0",
        "mov x26, #0",
        "mov x27, #0",
        "mov x28, #0",
        "mov x29, #0", // Frame pointer
        "mov x30, #0", // Link register
    );

    // Perform exception return to EL0
    core::arch::asm!("eret", options(noreturn));
}

/// Enter user mode (test stub)
#[cfg(test)]
pub unsafe fn enter_user_mode(_entry_point: u64, _stack_pointer: u64) -> ! {
    // In test mode, we can't actually enter user mode
    // This is a stub that just loops
    loop {
        core::hint::spin_loop();
    }
}

/// ARM64 system register access
pub mod sysreg {
    /// Read current exception level
    #[cfg(not(test))]
    pub fn current_el() -> u8 {
        let el: u64;
        unsafe {
            core::arch::asm!("mrs {}, CurrentEL", out(reg) el);
        }
        ((el >> 2) & 0x3) as u8
    }

    #[cfg(test)]
    pub fn current_el() -> u8 {
        0 // EL0 in test mode
    }

    /// Get exception syndrome register
    #[cfg(not(test))]
    pub fn esr_el1() -> u64 {
        let esr: u64;
        unsafe {
            core::arch::asm!("mrs {}, esr_el1", out(reg) esr);
        }
        esr
    }

    #[cfg(test)]
    pub fn esr_el1() -> u64 {
        0
    }

    /// Get fault address register
    #[cfg(not(test))]
    pub fn far_el1() -> u64 {
        let far: u64;
        unsafe {
            core::arch::asm!("mrs {}, far_el1", out(reg) far);
        }
        far
    }

    #[cfg(test)]
    pub fn far_el1() -> u64 {
        0
    }
}
