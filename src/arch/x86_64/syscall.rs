//! x86_64 Syscall Entry/Exit
//!
//! Implements the SYSCALL/SYSRET mechanism for user→kernel transitions.
//! Based on Mach4 i386/trap.c and Intel SDM SYSCALL specification.
//!
//! ## Mechanism
//!
//! When a user program executes SYSCALL:
//! - RCX ← RIP (return address)
//! - R11 ← RFLAGS
//! - RIP ← LSTAR MSR (kernel entry)
//! - RFLAGS masked by FMASK MSR
//! - CS:SS set from STAR MSR
//!
//! Arguments are passed in: RDI, RSI, RDX, R10 (not RCX!), R8, R9
//! Return value in RAX

use core::arch::asm;

use super::msr;
use crate::kern::syscall_sw::{mach_trap, TrapArgs, KernReturn};

// ============================================================================
// Syscall Frame
// ============================================================================

/// Saved user registers during syscall
#[repr(C)]
#[derive(Debug, Clone)]
pub struct SyscallFrame {
    // Callee-saved registers (we must preserve these)
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,

    // Syscall arguments (caller-saved, but we need them)
    pub rdi: u64,  // arg0
    pub rsi: u64,  // arg1
    pub rdx: u64,  // arg2
    pub r10: u64,  // arg3 (SYSCALL uses R10 instead of RCX)
    pub r8: u64,   // arg4
    pub r9: u64,   // arg5

    // Saved by SYSCALL instruction
    pub rcx: u64,  // Return RIP
    pub r11: u64,  // Return RFLAGS

    // System call number
    pub rax: u64,

    // User stack pointer (from GS:0 after swapgs)
    pub user_rsp: u64,
}

impl SyscallFrame {
    /// Get syscall arguments as TrapArgs
    pub fn to_trap_args(&self) -> TrapArgs {
        TrapArgs::with_args(&[
            self.rdi as usize,
            self.rsi as usize,
            self.rdx as usize,
            self.r10 as usize,
            self.r8 as usize,
            self.r9 as usize,
        ])
    }

    /// Get the syscall number
    pub fn syscall_number(&self) -> i32 {
        self.rax as i32
    }
}

// ============================================================================
// Kernel Stack for Syscalls
// ============================================================================

/// Per-CPU kernel stack for syscall handling
/// This is pointed to by GS base after swapgs
#[repr(C)]
pub struct CpuSyscallStack {
    /// Kernel stack pointer (top of stack)
    pub kernel_rsp: u64,
    /// User stack pointer (saved on entry)
    pub user_rsp: u64,
    /// Current thread pointer
    pub current_thread: u64,
    /// Scratch space
    pub scratch: [u64; 4],
}

/// Stack size for kernel syscall handling
pub const SYSCALL_STACK_SIZE: usize = 4096;

// ============================================================================
// Syscall Entry Point (Assembly)
// ============================================================================

/// The actual syscall entry point (naked function)
///
/// This is the target of LSTAR MSR. It's called when user space executes SYSCALL.
///
/// On entry:
/// - RCX = user RIP
/// - R11 = user RFLAGS
/// - RAX = syscall number
/// - RDI, RSI, RDX, R10, R8, R9 = arguments
/// - User RSP is still in RSP (no automatic stack switch!)
#[naked]
#[cfg(not(test))]
unsafe extern "C" fn syscall_entry_asm() {
    asm!(
        // Switch to kernel GS base (for per-CPU data)
        "swapgs",

        // Save user RSP to per-CPU area, load kernel RSP
        "mov gs:[8], rsp",      // Save user RSP to cpu_stack.user_rsp
        "mov rsp, gs:[0]",      // Load kernel RSP from cpu_stack.kernel_rsp

        // Build syscall frame on kernel stack
        // Push in reverse order of SyscallFrame struct
        "push qword ptr gs:[8]", // user_rsp
        "push rax",              // syscall number
        "push r11",              // saved RFLAGS
        "push rcx",              // saved RIP (return address)
        "push r9",               // arg5
        "push r8",               // arg4
        "push r10",              // arg3
        "push rdx",              // arg2
        "push rsi",              // arg1
        "push rdi",              // arg0
        "push r15",              // callee-saved
        "push r14",
        "push r13",
        "push r12",
        "push rbp",
        "push rbx",

        // Call Rust handler with pointer to SyscallFrame
        "mov rdi, rsp",          // First arg = frame pointer
        "call {handler}",

        // Return value is in RAX
        // Restore frame (skip callee-saved, we're returning)
        "add rsp, 6*8",          // Skip rbx, rbp, r12-r15
        "pop rdi",               // Restore args (might be modified)
        "pop rsi",
        "pop rdx",
        "pop r10",
        "pop r8",
        "pop r9",
        "pop rcx",               // Return RIP
        "pop r11",               // Return RFLAGS
        "add rsp, 8",            // Skip saved rax (return value already in rax)
        "pop rsp",               // Restore user RSP (from saved user_rsp)

        // Switch back to user GS base
        "swapgs",

        // Return to user space
        "sysretq",
        handler = sym syscall_handler,
        options(noreturn)
    );
}

/// Rust syscall handler
///
/// Called from assembly with pointer to saved frame.
/// Returns the syscall result in RAX.
#[no_mangle]
extern "C" fn syscall_handler(frame: &mut SyscallFrame) -> i64 {
    let syscall_num = frame.syscall_number();

    // Check if it's a Mach trap (we use positive numbers in our trap table)
    // Original Mach used negative numbers, but our implementation uses positive
    let trap_num = if syscall_num < 0 {
        // Convert negative Mach trap to positive table index
        (-syscall_num) as usize
    } else {
        syscall_num as usize
    };

    // Build trap arguments from saved registers
    let args = frame.to_trap_args();

    // Execute the trap
    let result = mach_trap(trap_num, &args);

    result as i64
}

// ============================================================================
// Syscall Initialization
// ============================================================================

/// STAR MSR layout:
/// - Bits 63:48: SYSRET CS (user CS) and SS (user SS = user CS + 8)
/// - Bits 47:32: SYSCALL CS (kernel CS) and SS (kernel SS = kernel CS + 8)
/// - Bits 31:0: Reserved (must be 0)
///
/// For typical GDT layout:
/// - Kernel CS = 0x08, Kernel SS = 0x10
/// - User CS = 0x18 (actually 0x1B with RPL=3), User SS = 0x20 (0x23 with RPL=3)
const KERNEL_CS: u64 = 0x08;
const USER_CS: u64 = 0x1B;  // 0x18 | RPL 3

/// Initialize x86_64 syscall mechanism
///
/// Sets up MSRs for SYSCALL/SYSRET:
/// - STAR: Segment selectors for kernel/user mode
/// - LSTAR: Syscall entry point address
/// - FMASK: RFLAGS mask (clear IF to disable interrupts on entry)
#[cfg(not(test))]
pub fn init() {
    // Enable SYSCALL/SYSRET in EFER
    let efer = msr::rdmsr(msr::EFER);
    msr::wrmsr(msr::EFER, efer | 1); // Set SCE (Syscall Enable) bit

    // Set up STAR MSR
    // SYSRET: user_cs = bits[63:48], user_ss = user_cs + 8
    // SYSCALL: kernel_cs = bits[47:32], kernel_ss = kernel_cs + 8
    // The actual CS loaded is STAR[47:32] for SYSCALL, STAR[63:48]+16 for SYSRET
    let star = ((USER_CS - 16) << 48) | (KERNEL_CS << 32);
    msr::wrmsr(msr::STAR, star);

    // Set LSTAR to our syscall entry point
    let entry = syscall_entry_asm as *const () as u64;
    msr::wrmsr(msr::LSTAR, entry);

    // Set FMASK to clear IF (interrupt flag) on syscall entry
    // Also clear DF (direction flag) and TF (trap flag)
    let fmask: u64 = 0x200 | 0x400 | 0x100; // IF | DF | TF
    msr::wrmsr(msr::FMASK, fmask);

    // Initialize per-CPU syscall stack
    // This would normally be done per-CPU during CPU bringup
    init_per_cpu_stack();
}

/// Initialize the per-CPU syscall stack
///
/// Sets up GS base to point to per-CPU data structure containing kernel stack.
#[cfg(not(test))]
fn init_per_cpu_stack() {
    // In a full implementation, this would:
    // 1. Allocate a kernel stack for this CPU
    // 2. Set up CpuSyscallStack structure
    // 3. Set KERNEL_GS_BASE MSR to point to it
    //
    // For now, we just set up a placeholder
    // Real implementation would allocate from heap

    static mut CPU0_STACK: CpuSyscallStack = CpuSyscallStack {
        kernel_rsp: 0,
        user_rsp: 0,
        current_thread: 0,
        scratch: [0; 4],
    };

    static mut CPU0_KERNEL_STACK: [u8; SYSCALL_STACK_SIZE] = [0; SYSCALL_STACK_SIZE];

    unsafe {
        // Set kernel RSP to top of stack (grows down)
        CPU0_STACK.kernel_rsp = CPU0_KERNEL_STACK.as_ptr() as u64
            + SYSCALL_STACK_SIZE as u64
            - 8; // 8-byte aligned

        // Set KERNEL_GS_BASE to our per-CPU structure
        // After swapgs, GS will point here
        msr::wrmsr(msr::KERNEL_GS_BASE, &CPU0_STACK as *const _ as u64);
    }
}

/// Test mode initialization (no-op)
#[cfg(test)]
pub fn init() {
    // No MSR access in test mode
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if currently in kernel mode
pub fn is_kernel_mode() -> bool {
    let cs: u16;
    unsafe {
        asm!("mov {:x}, cs", out(reg) cs);
    }
    (cs & 3) == 0  // RPL bits = 0 means kernel mode
}

/// Get the current user stack pointer (if in syscall context)
pub fn get_user_rsp() -> Option<u64> {
    if is_kernel_mode() {
        // Read from per-CPU structure
        let user_rsp: u64;
        unsafe {
            asm!("mov {}, gs:[8]", out(reg) user_rsp);
        }
        if user_rsp != 0 {
            Some(user_rsp)
        } else {
            None
        }
    } else {
        None
    }
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
            rbx: 0, rbp: 0, r12: 0, r13: 0, r14: 0, r15: 0,
            rdi: 100, rsi: 200, rdx: 300, r10: 400, r8: 500, r9: 600,
            rcx: 0, r11: 0, rax: 25, user_rsp: 0,
        };

        let args = frame.to_trap_args();
        assert_eq!(args.arg(0), 100);
        assert_eq!(args.arg(1), 200);
        assert_eq!(args.arg(2), 300);
        assert_eq!(args.arg(3), 400);
        assert_eq!(args.arg(4), 500);
        assert_eq!(args.arg(5), 600);
    }

    #[test]
    fn test_syscall_number() {
        let frame = SyscallFrame {
            rbx: 0, rbp: 0, r12: 0, r13: 0, r14: 0, r15: 0,
            rdi: 0, rsi: 0, rdx: 0, r10: 0, r8: 0, r9: 0,
            rcx: 0, r11: 0, rax: 25, user_rsp: 0,
        };

        assert_eq!(frame.syscall_number(), 25);
    }
}
