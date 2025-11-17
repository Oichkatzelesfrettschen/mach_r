//! ARM64 architecture support

use core::arch::asm;

pub mod boot;
pub mod mm;

/// Initialize ARM64-specific features
pub fn init() {
    // Set up exception vectors
    unsafe {
        // For now, just ensure we're in EL1
        let mut el: u64;
        asm!("mrs {}, CurrentEL", out(reg) el);
        let el = (el >> 2) & 0b11;
        
        if el != 1 {
            // We should be in EL1 for kernel
            // In real implementation, would transition here
        }
    }
}

/// Wait for interrupt (low power wait)
pub fn wait_for_interrupt() {
    unsafe {
        asm!("wfi");
    }
}

/// Disable interrupts
pub fn disable_interrupts() {
    unsafe {
        asm!("msr daifset, #2");
    }
}

/// Enable interrupts
pub fn enable_interrupts() {
    unsafe {
        asm!("msr daifclr, #2");
    }
}