//! x86_64 Exception and Interrupt Handling
//!
//! Implements IDT setup and exception handlers for x86_64.
//! Based on Mach4 i386/trap.c and Intel SDM interrupt handling.
//!
//! ## Page Faults (Vector 14)
//!
//! When a page fault occurs:
//! - CR2 contains the faulting linear address
//! - Error code contains fault information:
//!   - Bit 0 (P): Page present (0 = not present, 1 = protection violation)
//!   - Bit 1 (W/R): Write access (0 = read, 1 = write)
//!   - Bit 2 (U/S): User mode (0 = supervisor, 1 = user)
//!   - Bit 3 (RSVD): Reserved bit set
//!   - Bit 4 (I/D): Instruction fetch

use core::arch::asm;

use crate::mach_vm::vm_fault::{vm_fault, FaultResult, FaultType};

// ============================================================================
// Exception Frame
// ============================================================================

/// CPU exception frame pushed by interrupt handler
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ExceptionFrame {
    // Pushed by our handler stub
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,

    // Exception vector number (pushed by our stub)
    pub vector: u64,

    // Error code (pushed by CPU for some exceptions, 0 otherwise)
    pub error_code: u64,

    // Pushed by CPU
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

// ============================================================================
// Page Fault Error Code
// ============================================================================

/// Page fault error code bits
pub mod page_fault_error {
    /// Page was present (protection violation vs not-present)
    pub const PRESENT: u64 = 1 << 0;
    /// Write access caused fault
    pub const WRITE: u64 = 1 << 1;
    /// Fault occurred in user mode
    pub const USER: u64 = 1 << 2;
    /// Reserved bit was set in page table
    pub const RESERVED: u64 = 1 << 3;
    /// Instruction fetch caused fault
    pub const INSTRUCTION: u64 = 1 << 4;
    /// Protection key violation
    pub const PK: u64 = 1 << 5;
    /// Shadow stack access fault
    pub const SS: u64 = 1 << 6;
    /// SGX violation
    pub const SGX: u64 = 1 << 15;
}

// ============================================================================
// Exception Handlers
// ============================================================================

/// Page fault handler (vector 14)
///
/// Called when a page fault occurs. This handler:
/// 1. Reads CR2 to get the faulting address
/// 2. Determines the fault type from the error code
/// 3. Calls vm_fault to handle the fault
/// 4. Returns to the faulting instruction on success
#[no_mangle]
pub extern "C" fn page_fault_handler(frame: &mut ExceptionFrame) {
    // Read faulting address from CR2
    let fault_addr: u64;
    unsafe {
        asm!("mov {}, cr2", out(reg) fault_addr);
    }

    let error_code = frame.error_code;
    let is_user = (error_code & page_fault_error::USER) != 0;
    let is_write = (error_code & page_fault_error::WRITE) != 0;
    let is_instruction = (error_code & page_fault_error::INSTRUCTION) != 0;
    let is_present = (error_code & page_fault_error::PRESENT) != 0;

    // Determine fault type
    let fault_type = if is_instruction {
        FaultType::Execute
    } else if is_write {
        FaultType::Write
    } else {
        FaultType::Read
    };

    // Only handle user-mode faults through vm_fault
    // Kernel faults should be handled differently (or panic)
    if !is_user {
        // Kernel page fault - this is a bug
        #[cfg(not(test))]
        {
            crate::println!(
                "KERNEL PAGE FAULT at 0x{:016x}",
                fault_addr
            );
            crate::println!(
                "  RIP: 0x{:016x}, Error: 0x{:x}",
                frame.rip,
                error_code
            );
            crate::println!(
                "  Present: {}, Write: {}, Instruction: {}",
                is_present,
                is_write,
                is_instruction
            );
        }
        // TODO: Panic or handle kernel fault
        return;
    }

    // Handle user page fault via VM subsystem
    let result = handle_user_page_fault(fault_addr, fault_type);

    match result {
        FaultResult::Success => {
            // Fault handled - return and retry the instruction
        }
        FaultResult::ProtectionFailure => {
            #[cfg(not(test))]
            crate::println!(
                "Protection fault: addr=0x{:x}, RIP=0x{:x}, error=0x{:x}",
                fault_addr,
                frame.rip,
                error_code
            );
            // TODO: Send SIGSEGV/SIGBUS to the process
        }
        FaultResult::MemoryError | FaultResult::MemoryFailure => {
            #[cfg(not(test))]
            crate::println!(
                "Segmentation fault: addr=0x{:x}, RIP=0x{:x}, error=0x{:x}",
                fault_addr,
                frame.rip,
                error_code
            );
            // TODO: Send SIGSEGV to the process
        }
        _ => {
            #[cfg(not(test))]
            crate::println!(
                "VM fault failed: addr=0x{:x}, result={:?}",
                fault_addr,
                result
            );
            // TODO: Send signal to the process
        }
    }
}

/// Handle user-mode page fault via VM subsystem
fn handle_user_page_fault(fault_addr: u64, fault_type: FaultType) -> FaultResult {
    // Get current thread
    let thread = match crate::scheduler::current_thread() {
        Some(t) => t,
        None => {
            // No current thread context
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
    task.stats
        .faults
        .fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    // Call vm_fault to handle the page fault
    vm_fault(&map, fault_addr, fault_type, false)
}

/// General protection fault handler (vector 13)
#[no_mangle]
pub extern "C" fn general_protection_handler(frame: &ExceptionFrame) {
    #[cfg(not(test))]
    {
        crate::println!("General Protection Fault!");
        crate::println!("  RIP: 0x{:016x}", frame.rip);
        crate::println!("  Error code: 0x{:x}", frame.error_code);
        crate::println!("  CS: 0x{:x}, SS: 0x{:x}", frame.cs, frame.ss);
    }
    // TODO: Send SIGSEGV to user process or panic for kernel
}

/// Double fault handler (vector 8)
#[no_mangle]
pub extern "C" fn double_fault_handler(frame: &ExceptionFrame) {
    #[cfg(not(test))]
    {
        crate::println!("DOUBLE FAULT!");
        crate::println!("  RIP: 0x{:016x}", frame.rip);
    }
    // Double fault is unrecoverable
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

/// Divide error handler (vector 0)
#[no_mangle]
pub extern "C" fn divide_error_handler(frame: &ExceptionFrame) {
    #[cfg(not(test))]
    crate::println!("Divide Error at RIP: 0x{:016x}", frame.rip);
    // TODO: Send SIGFPE to user process
}

/// Invalid opcode handler (vector 6)
#[no_mangle]
pub extern "C" fn invalid_opcode_handler(frame: &ExceptionFrame) {
    #[cfg(not(test))]
    crate::println!("Invalid Opcode at RIP: 0x{:016x}", frame.rip);
    // TODO: Send SIGILL to user process
}

/// Breakpoint handler (vector 3)
#[no_mangle]
pub extern "C" fn breakpoint_handler(frame: &ExceptionFrame) {
    #[cfg(not(test))]
    crate::println!("Breakpoint at RIP: 0x{:016x}", frame.rip);
    // TODO: Send SIGTRAP to user process for debugging
}

// ============================================================================
// IDT Setup
// ============================================================================

/// IDT entry (Interrupt Descriptor Table)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct IdtEntry {
    /// Low 16 bits of handler address
    pub offset_low: u16,
    /// Code segment selector
    pub selector: u16,
    /// IST index (0 = no IST)
    pub ist: u8,
    /// Type and attributes
    pub type_attr: u8,
    /// Middle 16 bits of handler address
    pub offset_mid: u16,
    /// High 32 bits of handler address
    pub offset_high: u32,
    /// Reserved (must be zero)
    pub reserved: u32,
}

impl IdtEntry {
    /// Create an empty IDT entry
    pub const fn empty() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    /// Create an interrupt gate entry
    pub fn interrupt_gate(handler: u64, selector: u16, ist: u8) -> Self {
        Self {
            offset_low: handler as u16,
            selector,
            ist,
            type_attr: 0x8E, // Present, DPL=0, Interrupt Gate
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }

    /// Create a trap gate entry (doesn't disable interrupts)
    pub fn trap_gate(handler: u64, selector: u16, ist: u8) -> Self {
        Self {
            offset_low: handler as u16,
            selector,
            ist,
            type_attr: 0x8F, // Present, DPL=0, Trap Gate
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

/// IDT pointer for LIDT instruction
#[repr(C, packed)]
pub struct IdtPtr {
    pub limit: u16,
    pub base: u64,
}

/// Number of IDT entries
pub const IDT_ENTRIES: usize = 256;

/// Global IDT
static mut IDT: [IdtEntry; IDT_ENTRIES] = [IdtEntry::empty(); IDT_ENTRIES];

/// Initialize the Interrupt Descriptor Table
pub fn init() {
    // In a full implementation, this would:
    // 1. Set up entries for all CPU exceptions (0-31)
    // 2. Set up entries for hardware interrupts (32+)
    // 3. Load the IDT using LIDT
    //
    // For now, we assume the bootloader has set up basic IDT
    // and we just hook the page fault handler
    //
    // Example setup (would need actual handler stubs):
    // unsafe {
    //     let pf_handler = page_fault_handler as *const () as u64;
    //     IDT[14] = IdtEntry::interrupt_gate(pf_handler, 0x08, 0);
    //
    //     let idt_ptr = IdtPtr {
    //         limit: (core::mem::size_of::<[IdtEntry; IDT_ENTRIES]>() - 1) as u16,
    //         base: IDT.as_ptr() as u64,
    //     };
    //     asm!("lidt [{}]", in(reg) &idt_ptr);
    // }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_fault_error_bits() {
        let error: u64 = page_fault_error::PRESENT | page_fault_error::WRITE | page_fault_error::USER;
        assert!((error & page_fault_error::PRESENT) != 0);
        assert!((error & page_fault_error::WRITE) != 0);
        assert!((error & page_fault_error::USER) != 0);
        assert!((error & page_fault_error::INSTRUCTION) == 0);
    }

    #[test]
    fn test_idt_entry() {
        let entry = IdtEntry::interrupt_gate(0x12345678_9ABCDEF0, 0x08, 1);
        assert_eq!(entry.offset_low, 0xDEF0);
        assert_eq!(entry.offset_mid, 0x9ABC);
        assert_eq!(entry.offset_high, 0x12345678);
        assert_eq!(entry.selector, 0x08);
        assert_eq!(entry.ist, 1);
        assert_eq!(entry.type_attr, 0x8E);
    }
}
