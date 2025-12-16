//! ARM64 Syscall Entry/Exit (SVC Handler)
//!
//! Implements the SVC (Supervisor Call) mechanism for user→kernel transitions.
//! Based on Mach4 arm/trap.c and ARMv8 Architecture Reference Manual.
//!
//! ## Mechanism
//!
//! When a user program executes SVC #0:
//! - Exception is taken to EL1
//! - ELR_EL1 ← PC of SVC instruction + 4 (return address)
//! - SPSR_EL1 ← PSTATE at EL0
//! - PC ← VBAR_EL1 + 0x400 (Sync exception from EL0)
//!
//! Arguments are passed in: X0-X7
//! Return value in X0

use core::arch::asm;

use crate::kern::syscall_sw::{mach_trap, KernReturn, TrapArgs};
use crate::mach_vm::vm_fault::{vm_fault, FaultResult, FaultType};

// ============================================================================
// Syscall Frame
// ============================================================================

/// Saved user registers during syscall (exception frame)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct SyscallFrame {
    // General purpose registers
    pub x0: u64,
    pub x1: u64,
    pub x2: u64,
    pub x3: u64,
    pub x4: u64,
    pub x5: u64,
    pub x6: u64,
    pub x7: u64,
    pub x8: u64,   // Syscall number (like Linux)
    pub x9: u64,
    pub x10: u64,
    pub x11: u64,
    pub x12: u64,
    pub x13: u64,
    pub x14: u64,
    pub x15: u64,
    pub x16: u64,
    pub x17: u64,
    pub x18: u64,  // Platform register (usually reserved)
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub x28: u64,
    pub x29: u64,  // Frame pointer (FP)
    pub x30: u64,  // Link register (LR)

    // Stack pointer for EL0
    pub sp_el0: u64,

    // Exception return address
    pub elr_el1: u64,

    // Saved program status
    pub spsr_el1: u64,
}

impl SyscallFrame {
    /// Get syscall arguments as TrapArgs
    pub fn to_trap_args(&self) -> TrapArgs {
        TrapArgs::with_args(&[
            self.x0 as usize,
            self.x1 as usize,
            self.x2 as usize,
            self.x3 as usize,
            self.x4 as usize,
            self.x5 as usize,
            self.x6 as usize,
        ])
    }

    /// Get the syscall number
    /// ARM64 typically uses X8 for syscall number (like Linux ABI)
    /// Mach uses X16 or X0 depending on variant
    pub fn syscall_number(&self) -> i32 {
        // Use X8 as syscall number (Linux ABI compatible)
        self.x8 as i32
    }

    /// Set return value (goes in X0)
    pub fn set_return(&mut self, value: i64) {
        self.x0 = value as u64;
    }
}

// ============================================================================
// Exception Vector Table Entry Points
// ============================================================================

/// Exception vector table
/// Must be aligned to 2KB (2048 bytes) per ARM architecture
#[repr(C, align(2048))]
pub struct ExceptionVectorTable {
    // Current EL with SP_EL0 (should not happen in kernel)
    pub el1_sp0_sync: [u8; 128],
    pub el1_sp0_irq: [u8; 128],
    pub el1_sp0_fiq: [u8; 128],
    pub el1_sp0_serror: [u8; 128],

    // Current EL with SP_ELx (kernel→kernel exceptions)
    pub el1_sp1_sync: [u8; 128],
    pub el1_sp1_irq: [u8; 128],
    pub el1_sp1_fiq: [u8; 128],
    pub el1_sp1_serror: [u8; 128],

    // Lower EL using AArch64 (EL0 → EL1, this is where syscalls go)
    pub el0_aarch64_sync: [u8; 128],
    pub el0_aarch64_irq: [u8; 128],
    pub el0_aarch64_fiq: [u8; 128],
    pub el0_aarch64_serror: [u8; 128],

    // Lower EL using AArch32 (32-bit user space)
    pub el0_aarch32_sync: [u8; 128],
    pub el0_aarch32_irq: [u8; 128],
    pub el0_aarch32_fiq: [u8; 128],
    pub el0_aarch32_serror: [u8; 128],
}

/// Exception Syndrome Register (ESR_EL1) fields
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionClass {
    Unknown = 0x00,
    WFI_WFE = 0x01,
    MCR_MRC_CP15 = 0x03,
    MCRR_MRRC_CP15 = 0x04,
    MCR_MRC_CP14 = 0x05,
    LDC_STC_CP14 = 0x06,
    SVE_SIMD_FP = 0x07,
    VMRS = 0x08,
    PAUTH = 0x09,
    LD64B_ST64B = 0x0A,
    MRRC_CP14 = 0x0C,
    BTI = 0x0D,
    IllegalExecution = 0x0E,
    SVC_AArch32 = 0x11,
    HVC_AArch32 = 0x12,
    SMC_AArch32 = 0x13,
    SVC_AArch64 = 0x15,
    HVC_AArch64 = 0x16,
    SMC_AArch64 = 0x17,
    MRS_MSR_System = 0x18,
    SVE = 0x19,
    ERET = 0x1A,
    PAC = 0x1C,
    InstructionAbortLowerEL = 0x20,
    InstructionAbortSameEL = 0x21,
    PCAlignment = 0x22,
    DataAbortLowerEL = 0x24,
    DataAbortSameEL = 0x25,
    SPAlignment = 0x26,
    FP_AArch32 = 0x28,
    FP_AArch64 = 0x2C,
    SError = 0x2F,
    BreakpointLowerEL = 0x30,
    BreakpointSameEL = 0x31,
    SoftwareStepLowerEL = 0x32,
    SoftwareStepSameEL = 0x33,
    WatchpointLowerEL = 0x34,
    WatchpointSameEL = 0x35,
    BKPT_AArch32 = 0x38,
    BRK_AArch64 = 0x3C,
}

/// Extract exception class from ESR_EL1
pub fn get_exception_class(esr: u64) -> ExceptionClass {
    let ec = ((esr >> 26) & 0x3F) as u32;
    // Safety: We clamp to known values
    match ec {
        0x15 => ExceptionClass::SVC_AArch64,
        0x20 => ExceptionClass::InstructionAbortLowerEL,
        0x24 => ExceptionClass::DataAbortLowerEL,
        _ => ExceptionClass::Unknown,
    }
}

/// Extract immediate value from SVC instruction (bits 15:0 of ESR)
pub fn get_svc_immediate(esr: u64) -> u16 {
    (esr & 0xFFFF) as u16
}

// ============================================================================
// Syscall Entry Point
// ============================================================================

/// Main synchronous exception handler for EL0 → EL1
///
/// Called from the exception vector when a sync exception occurs from user space.
/// This includes SVC (syscall), page faults, alignment faults, etc.
#[no_mangle]
pub extern "C" fn el0_sync_handler(frame: &mut SyscallFrame) {
    // Read Exception Syndrome Register to determine cause
    let esr: u64;
    unsafe {
        asm!("mrs {}, esr_el1", out(reg) esr);
    }

    let exception_class = get_exception_class(esr);

    match exception_class {
        ExceptionClass::SVC_AArch64 => {
            // System call from user space
            let _svc_imm = get_svc_immediate(esr);
            handle_syscall(frame);
        }
        ExceptionClass::DataAbortLowerEL => {
            // Page fault from user space
            handle_data_abort(frame, esr);
        }
        ExceptionClass::InstructionAbortLowerEL => {
            // Instruction fetch fault from user space
            handle_instruction_abort(frame, esr);
        }
        _ => {
            // Unknown exception - panic or kill process
            handle_unknown_exception(frame, esr);
        }
    }
}

/// Handle SVC (syscall) exception
fn handle_syscall(frame: &mut SyscallFrame) {
    let syscall_num = frame.syscall_number();

    // Convert negative Mach trap numbers to positive table index
    let trap_num = if syscall_num < 0 {
        (-syscall_num) as usize
    } else {
        syscall_num as usize
    };

    // Build trap arguments from saved registers
    let args = frame.to_trap_args();

    // Execute the trap
    let result = mach_trap(trap_num, &args);

    // Set return value
    frame.set_return(result as i64);
}

/// Handle data abort (page fault) from user space
fn handle_data_abort(frame: &mut SyscallFrame, esr: u64) {
    // Read Fault Address Register
    let fault_addr: u64;
    unsafe {
        asm!("mrs {}, far_el1", out(reg) fault_addr);
    }

    // Determine fault type from ESR
    // WnR (Write not Read): bit 6 - 1 = write fault, 0 = read fault
    let is_write = (esr >> 6) & 1 != 0;
    // DFSC (Data Fault Status Code): bits 5:0
    let dfsc = esr & 0x3F;

    // Determine FaultType from ESR
    let fault_type = if is_write {
        FaultType::Write
    } else {
        FaultType::Read
    };

    // Try to handle the fault via VM subsystem
    let result = handle_page_fault(fault_addr, fault_type);

    match result {
        FaultResult::Success => {
            // Fault handled successfully - return to user space and retry
            // The instruction will be re-executed automatically via ERET
        }
        FaultResult::ProtectionFailure => {
            // Protection violation - send signal to process
            #[cfg(not(test))]
            crate::println!(
                "Protection fault: addr=0x{:x}, write={}, DFSC=0x{:x}",
                fault_addr,
                is_write,
                dfsc
            );
            frame.set_return(-1);
            // TODO: Send SIGSEGV/SIGBUS to the process
        }
        FaultResult::MemoryError | FaultResult::MemoryFailure => {
            // No mapping for address - segmentation fault
            #[cfg(not(test))]
            crate::println!(
                "Segmentation fault: addr=0x{:x}, ELR=0x{:x}, DFSC=0x{:x}",
                fault_addr,
                frame.elr_el1,
                dfsc
            );
            frame.set_return(-1);
            // TODO: Send SIGSEGV to the process
        }
        _ => {
            // Other fault types (memory shortage, etc.)
            #[cfg(not(test))]
            crate::println!(
                "VM fault failed: addr=0x{:x}, result={:?}",
                fault_addr,
                result
            );
            frame.set_return(-1);
        }
    }
}

/// Handle instruction abort from user space
fn handle_instruction_abort(frame: &mut SyscallFrame, esr: u64) {
    let fault_addr: u64;
    unsafe {
        asm!("mrs {}, far_el1", out(reg) fault_addr);
    }

    // IFSC (Instruction Fault Status Code): bits 5:0
    let ifsc = esr & 0x3F;

    // Instruction fetch faults are always execute type
    let result = handle_page_fault(fault_addr, FaultType::Execute);

    match result {
        FaultResult::Success => {
            // Fault handled successfully - retry the instruction fetch
        }
        FaultResult::ProtectionFailure => {
            #[cfg(not(test))]
            crate::println!(
                "Execute protection fault: addr=0x{:x}, IFSC=0x{:x}",
                fault_addr,
                ifsc
            );
            frame.set_return(-1);
            // TODO: Send SIGSEGV to the process
        }
        _ => {
            #[cfg(not(test))]
            crate::println!(
                "Instruction fault: addr=0x{:x}, ELR=0x{:x}, IFSC=0x{:x}",
                fault_addr,
                frame.elr_el1,
                ifsc
            );
            frame.set_return(-1);
            // TODO: Send SIGSEGV to the process
        }
    }
}

/// Common page fault handler - looks up VM map and calls vm_fault
fn handle_page_fault(fault_addr: u64, fault_type: FaultType) -> FaultResult {
    // Get current thread
    let thread = match crate::scheduler::current_thread() {
        Some(t) => t,
        None => {
            // No current thread - this is a kernel fault without thread context
            return FaultResult::MemoryError;
        }
    };

    // Get task from thread
    // Note: scheduler uses crate::types::TaskId, but task_find needs kern::thread::TaskId
    // Both are u64 wrappers, so we convert via the inner value
    let task_id = crate::kern::thread::TaskId(thread.task_id.0);
    let task = match crate::kern::task::task_find(task_id) {
        Some(t) => t,
        None => {
            return FaultResult::MemoryError;
        }
    };

    // Get VM map from task
    let map_id = match task.get_map_id() {
        Some(id) => id,
        None => {
            // Task has no VM map (kernel task without map?)
            return FaultResult::MemoryError;
        }
    };

    // Look up the map
    let map = match crate::mach_vm::vm_map::lookup(map_id) {
        Some(m) => m,
        None => {
            return FaultResult::MemoryError;
        }
    };

    // Update fault statistics
    task.stats.faults.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    // Call vm_fault to handle the page fault
    let result = vm_fault(&map, fault_addr, fault_type, false);

    // Track COW faults
    if fault_type == FaultType::Write && result == FaultResult::Success {
        // Note: This is a heuristic - vm_fault increments cow stats internally
        // but we track at task level too for task_info
    }

    result
}

/// Handle unknown exception
fn handle_unknown_exception(frame: &mut SyscallFrame, esr: u64) {
    frame.set_return(-1);

    #[cfg(not(test))]
    crate::println!("Unknown exception: ESR=0x{:x}, ELR=0x{:x}",
        esr, frame.elr_el1);
}

// ============================================================================
// Exception Vector Setup
// ============================================================================

/// Install our exception vector table
#[cfg(not(test))]
pub fn init() {
    // In a real implementation, this would:
    // 1. Build or link to an exception vector table
    // 2. Set VBAR_EL1 to point to it
    // 3. Ensure the table is properly aligned (2KB)

    // The actual vector table would contain stubs that:
    // 1. Save all registers to the stack
    // 2. Call the appropriate Rust handler
    // 3. Restore registers
    // 4. Execute ERET

    // For now, we assume the bootloader or start.s has set up VBAR_EL1
    // We just need to ensure our handlers are properly linked

    // Example of setting VBAR_EL1 (commented out - needs actual vector table):
    // unsafe {
    //     let vbar = &EXCEPTION_VECTORS as *const _ as u64;
    //     asm!("msr vbar_el1, {}", in(reg) vbar);
    // }
}

#[cfg(test)]
pub fn init() {
    // No-op in test mode
}

// ============================================================================
// Assembly Stubs for Exception Vectors
// ============================================================================

/// Assembly macro for saving all registers on exception entry
/// Would be defined in a separate .s file in a full implementation
#[cfg(not(test))]
macro_rules! exception_entry {
    () => {
        r#"
        // Save x0-x29 (frame pointer)
        stp x0, x1, [sp, #-16]!
        stp x2, x3, [sp, #-16]!
        stp x4, x5, [sp, #-16]!
        stp x6, x7, [sp, #-16]!
        stp x8, x9, [sp, #-16]!
        stp x10, x11, [sp, #-16]!
        stp x12, x13, [sp, #-16]!
        stp x14, x15, [sp, #-16]!
        stp x16, x17, [sp, #-16]!
        stp x18, x19, [sp, #-16]!
        stp x20, x21, [sp, #-16]!
        stp x22, x23, [sp, #-16]!
        stp x24, x25, [sp, #-16]!
        stp x26, x27, [sp, #-16]!
        stp x28, x29, [sp, #-16]!

        // Save x30 (link register), sp_el0
        mrs x0, sp_el0
        stp x30, x0, [sp, #-16]!

        // Save elr_el1, spsr_el1
        mrs x0, elr_el1
        mrs x1, spsr_el1
        stp x0, x1, [sp, #-16]!
        "#
    };
}

/// Assembly macro for restoring all registers on exception return
#[cfg(not(test))]
macro_rules! exception_exit {
    () => {
        r#"
        // Restore elr_el1, spsr_el1
        ldp x0, x1, [sp], #16
        msr elr_el1, x0
        msr spsr_el1, x1

        // Restore x30, sp_el0
        ldp x30, x0, [sp], #16
        msr sp_el0, x0

        // Restore x0-x29
        ldp x28, x29, [sp], #16
        ldp x26, x27, [sp], #16
        ldp x24, x25, [sp], #16
        ldp x22, x23, [sp], #16
        ldp x20, x21, [sp], #16
        ldp x18, x19, [sp], #16
        ldp x16, x17, [sp], #16
        ldp x14, x15, [sp], #16
        ldp x12, x13, [sp], #16
        ldp x10, x11, [sp], #16
        ldp x8, x9, [sp], #16
        ldp x6, x7, [sp], #16
        ldp x4, x5, [sp], #16
        ldp x2, x3, [sp], #16
        ldp x0, x1, [sp], #16

        // Return from exception
        eret
        "#
    };
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if currently at EL1 (kernel mode)
pub fn is_kernel_mode() -> bool {
    let current_el: u64;
    unsafe {
        asm!("mrs {}, CurrentEL", out(reg) current_el);
    }
    ((current_el >> 2) & 0x3) == 1  // EL1
}

/// Read the Exception Link Register (return address)
pub fn get_elr_el1() -> u64 {
    let elr: u64;
    unsafe {
        asm!("mrs {}, elr_el1", out(reg) elr);
    }
    elr
}

/// Read the Fault Address Register
pub fn get_far_el1() -> u64 {
    let far: u64;
    unsafe {
        asm!("mrs {}, far_el1", out(reg) far);
    }
    far
}

/// Read the Exception Syndrome Register
pub fn get_esr_el1() -> u64 {
    let esr: u64;
    unsafe {
        asm!("mrs {}, esr_el1", out(reg) esr);
    }
    esr
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_frame_args() {
        let frame = SyscallFrame {
            x0: 100, x1: 200, x2: 300, x3: 400, x4: 500, x5: 600, x6: 700,
            x7: 800, x8: 25, x9: 0, x10: 0, x11: 0, x12: 0, x13: 0, x14: 0,
            x15: 0, x16: 0, x17: 0, x18: 0, x19: 0, x20: 0, x21: 0, x22: 0,
            x23: 0, x24: 0, x25: 0, x26: 0, x27: 0, x28: 0, x29: 0, x30: 0,
            sp_el0: 0, elr_el1: 0, spsr_el1: 0,
        };

        let args = frame.to_trap_args();
        assert_eq!(args.arg(0), 100);
        assert_eq!(args.arg(1), 200);
        assert_eq!(args.arg(2), 300);
    }

    #[test]
    fn test_syscall_number() {
        let frame = SyscallFrame {
            x0: 0, x1: 0, x2: 0, x3: 0, x4: 0, x5: 0, x6: 0, x7: 0,
            x8: 25, // Syscall number in x8
            x9: 0, x10: 0, x11: 0, x12: 0, x13: 0, x14: 0, x15: 0,
            x16: 0, x17: 0, x18: 0, x19: 0, x20: 0, x21: 0, x22: 0,
            x23: 0, x24: 0, x25: 0, x26: 0, x27: 0, x28: 0, x29: 0, x30: 0,
            sp_el0: 0, elr_el1: 0, spsr_el1: 0,
        };

        assert_eq!(frame.syscall_number(), 25);
    }

    #[test]
    fn test_exception_class() {
        // SVC exception: EC = 0x15
        let esr = 0x15u64 << 26;
        assert_eq!(get_exception_class(esr), ExceptionClass::SVC_AArch64);

        // Data abort: EC = 0x24
        let esr = 0x24u64 << 26;
        assert_eq!(get_exception_class(esr), ExceptionClass::DataAbortLowerEL);
    }

    #[test]
    fn test_svc_immediate() {
        // SVC #0x1234
        let esr = (0x15u64 << 26) | 0x1234;
        assert_eq!(get_svc_immediate(esr), 0x1234);
    }
}
