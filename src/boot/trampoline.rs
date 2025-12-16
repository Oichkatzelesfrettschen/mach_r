//! ARM64 assembly trampoline for kernel entry
//! Pure Rust implementation with inline assembly

#![cfg(target_arch = "aarch64")]

use super::{paging::KERNEL_VIRTUAL_BASE, BootInfo};

/// Kernel entry point signature
pub type KernelEntryPoint = extern "C" fn(boot_info: &'static BootInfo) -> !;

/// Boot trampoline parameters
#[repr(C)]
pub struct TrampolineParams {
    pub kernel_entry: u64,
    pub stack_top: u64,
    pub boot_info: u64,
    pub page_table: u64,
}

// Simplified ARM64 boot trampoline assembly (avoid complex global_asm for now)
// TODO: Implement proper ARM64 assembly once target toolchain is configured

/// Simple boot trampoline implementation
fn boot_trampoline_impl(params: *const TrampolineParams) -> ! {
    unsafe {
        let params = &*params;

        // Setup basic MMU and jump to kernel
        // This is a simplified version - full implementation would setup page tables

        // For now, just jump directly to kernel entry point
        let kernel_entry: extern "C" fn(&'static BootInfo) -> ! =
            core::mem::transmute(params.kernel_entry);

        let boot_info = &*(params.boot_info as *const BootInfo);
        kernel_entry(boot_info);
    }
}

/// Execute boot trampoline to jump to kernel
pub fn execute_trampoline(
    kernel_entry: u64,
    stack_top: u64,
    boot_info: &'static BootInfo,
    page_table: u64,
) -> ! {
    let params = TrampolineParams {
        kernel_entry,
        stack_top,
        boot_info: boot_info as *const _ as u64,
        page_table,
    };

    boot_trampoline_impl(&params);
}

/// Setup basic identity mapping for trampoline
pub fn setup_identity_mapping() {
    // TODO: Implement proper identity mapping
    // For now, assume identity mapping is already established
}

/// Flush translation lookaside buffer
pub fn flush_translation_buffer() {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("tlbi vmalle1is; dsb ish; isb");
    }
}

/// Get current ARM64 exception level
pub fn current_exception_level() -> u32 {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        let mut el: u64;
        core::arch::asm!("mrs {}, CurrentEL", out(reg) el);
        (el >> 2) as u32
    }
    #[cfg(not(target_arch = "aarch64"))]
    1 // Default to EL1 equivalent for other architectures
}

/// Switch to EL1 from EL2 if necessary
pub fn ensure_el1() {
    #[cfg(target_arch = "aarch64")]
    {
        let current_el = current_exception_level();
        if current_el == 2 {
            // TODO: Implement EL2 to EL1 transition
            // For now, assume we're already in the correct level
        }
    }
}

/// Prepare for kernel handoff
pub fn prepare_kernel_handoff() {
    // Ensure we're in EL1
    ensure_el1();

    // Set up basic identity mapping
    setup_identity_mapping();

    // Flush any pending operations
    unsafe {
        core::arch::asm!(
            "dsb sy", // Data synchronization barrier
            "isb"     // Instruction synchronization barrier
        );
    }
}

/// Calculate higher half virtual address for kernel entry
pub fn kernel_virtual_address(physical_entry: u64) -> u64 {
    KERNEL_VIRTUAL_BASE + physical_entry
}

/// ARM64 cache maintenance operations
pub mod cache {
    /// Clean and invalidate data cache
    pub fn clean_and_invalidate_dcache() {
        unsafe {
            core::arch::asm!(
                "ic ialluis",   // Invalidate instruction cache
                "dc cisw, xzr", // Clean and invalidate data cache
                "dsb sy",       // Data synchronization barrier
                "isb"           // Instruction synchronization barrier
            );
        }
    }

    /// Invalidate instruction cache
    pub fn invalidate_icache() {
        unsafe {
            core::arch::asm!(
                "ic ialluis", // Invalidate all instruction cache
                "dsb sy",
                "isb"
            );
        }
    }

    /// Data memory barrier
    pub fn data_memory_barrier() {
        unsafe {
            core::arch::asm!("dmb sy");
        }
    }

    /// Data synchronization barrier
    pub fn data_sync_barrier() {
        unsafe {
            core::arch::asm!("dsb sy");
        }
    }

    /// Instruction synchronization barrier
    pub fn instruction_sync_barrier() {
        unsafe {
            core::arch::asm!("isb");
        }
    }
}

/// ARM64 system register access
pub mod sysreg {
    /// Read MIDR_EL1 (Main ID Register)
    pub fn read_midr() -> u64 {
        let mut value: u64;
        unsafe {
            core::arch::asm!("mrs {}, midr_el1", out(reg) value);
        }
        value
    }

    /// Read MPIDR_EL1 (Multiprocessor Affinity Register)  
    pub fn read_mpidr() -> u64 {
        let mut value: u64;
        unsafe {
            core::arch::asm!("mrs {}, mpidr_el1", out(reg) value);
        }
        value
    }

    /// Read ID_AA64MMFR0_EL1 (Memory Model Feature Register 0)
    pub fn read_id_aa64mmfr0() -> u64 {
        let mut value: u64;
        unsafe {
            core::arch::asm!("mrs {}, id_aa64mmfr0_el1", out(reg) value);
        }
        value
    }

    /// Set vector table base address
    ///
    /// # Safety
    ///
    /// - `addr` must point to a valid AArch64 exception vector table
    /// - The vector table must be properly aligned (2KB boundary)
    /// - Must be called from EL1 or higher
    pub unsafe fn set_vbar_el1(addr: u64) {
        core::arch::asm!("msr vbar_el1, {}", in(reg) addr);
    }

    /// Get stack pointer
    pub fn read_sp() -> u64 {
        let mut sp: u64;
        unsafe {
            core::arch::asm!("mov {}, sp", out(reg) sp);
        }
        sp
    }
}
