//! x86_64 architecture support for Mach_R
//!
//! Intel/AMD 64-bit architecture implementation

use crate::arch::{Architecture, CpuFeatures};
use core::arch::asm;

pub mod boot;
pub mod exceptions;
pub mod gdt;
pub mod mmu;
pub mod syscall;

/// x86_64 implementation of Architecture trait  
pub struct X86_64;

/// Type alias for x86_64 architecture implementation
pub type ArchImpl = X86_64;

impl X86_64 {
    /// Direct port input - available as static method
    pub fn inb(port: u16) -> u8 {
        let value: u8;
        unsafe {
            asm!("in al, dx", out("al") value, in("dx") port);
        }
        value
    }

    /// Direct port output - available as static method  
    pub fn outb(port: u16, value: u8) {
        unsafe {
            asm!("out dx, al", in("dx") port, in("al") value);
        }
    }
}

impl Architecture for X86_64 {
    fn init() {
        // Initialize GDT
        gdt::init();

        // Initialize IDT
        exceptions::init();

        // Initialize paging
        mmu::init();

        // Enable SSE/AVX if available
        enable_simd();
    }

    fn enable_interrupts() {
        unsafe {
            asm!("sti");
        }
    }

    fn disable_interrupts() {
        unsafe {
            asm!("cli");
        }
    }

    fn interrupts_enabled() -> bool {
        let flags: u64;
        unsafe {
            asm!("pushfq; pop {}", out(reg) flags);
        }
        (flags & 0x200) != 0 // IF flag
    }

    fn halt() -> ! {
        loop {
            unsafe {
                asm!("hlt");
            }
        }
    }

    fn flush_tlb(addr: usize) {
        unsafe {
            asm!("invlpg [{}]", in(reg) addr);
        }
    }

    fn inb(port: u16) -> u8 {
        let value: u8;
        unsafe {
            asm!("in al, dx", out("al") value, in("dx") port);
        }
        value
    }

    fn outb(port: u16, value: u8) {
        unsafe {
            asm!("out dx, al", in("dx") port, in("al") value);
        }
    }

    fn cpu_id() -> usize {
        // Simple CPU ID - just return 0 for now
        // Real implementation would read APIC ID but ebx conflicts with LLVM
        0
    }

    fn keyboard_read() -> u8 {
        Self::inb(0x60) // Read from keyboard data port
    }

    fn current_timestamp() -> u64 {
        // Read Time Stamp Counter
        let low: u32;
        let high: u32;
        unsafe {
            asm!(
                "rdtsc",
                out("eax") low,
                out("edx") high,
            );
        }
        ((high as u64) << 32) | (low as u64)
    }
}

/// Enable SIMD extensions (SSE, AVX)
fn enable_simd() {
    unsafe {
        // Enable SSE
        let mut cr0: u64;
        asm!("mov {}, cr0", out(reg) cr0);
        cr0 &= !(1 << 2); // Clear EM
        cr0 |= 1 << 1; // Set MP
        asm!("mov cr0, {}", in(reg) cr0);

        // Enable SSE/SSE2
        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4);
        cr4 |= 3 << 9; // Set OSFXSR and OSXMMEXCPT
        asm!("mov cr4, {}", in(reg) cr4);

        // Check for AVX support and enable if available
        if has_avx() {
            // Enable XSAVE
            cr4 |= 1 << 18; // OSXSAVE
            asm!("mov cr4, {}", in(reg) cr4);

            // Enable AVX
            let xcr0: u64 = 0x7; // x87, SSE, AVX
            asm!(
                "xsetbv",
                in("ecx") 0u32,
                in("eax") xcr0 as u32,
                in("edx") (xcr0 >> 32) as u32,
            );
        }
    }
}

/// Check if AVX is supported
fn has_avx() -> bool {
    // Simplified - assume no AVX to avoid ebx register conflicts
    false
}

/// Detect CPU features
pub fn detect_features() -> CpuFeatures {
    let mut features = CpuFeatures {
        has_fpu: true, // Always present on x86_64
        has_vmx: false,
        has_svm: false,
        has_sve: false,    // ARM-specific
        has_neon: false,   // ARM-specific
        has_msa: false,    // MIPS-specific
        has_vector: false, // RISC-V specific
        cache_line_size: 64,
        physical_address_bits: 48,
        virtual_address_bits: 48,
    };

    // Simplified feature detection - avoid ebx register conflicts
    features.has_vmx = false; // Would require CPUID with ebx
    features.has_svm = false; // Would require CPUID with ebx

    // Use default address sizes for now (would need CPUID without ebx conflicts)
    features.physical_address_bits = 48;
    features.virtual_address_bits = 48;

    features
}

/// CPU control registers
pub mod cr {
    use core::arch::asm;

    /// Read CR0
    pub fn cr0() -> u64 {
        let cr0: u64;
        unsafe {
            asm!("mov {}, cr0", out(reg) cr0);
        }
        cr0
    }

    /// Write CR0
    pub fn set_cr0(cr0: u64) {
        unsafe {
            asm!("mov cr0, {}", in(reg) cr0);
        }
    }

    /// Read CR2 (page fault address)
    pub fn cr2() -> u64 {
        let cr2: u64;
        unsafe {
            asm!("mov {}, cr2", out(reg) cr2);
        }
        cr2
    }

    /// Read CR3 (page table base)
    pub fn cr3() -> u64 {
        let cr3: u64;
        unsafe {
            asm!("mov {}, cr3", out(reg) cr3);
        }
        cr3
    }

    /// Write CR3
    pub fn set_cr3(cr3: u64) {
        unsafe {
            asm!("mov cr3, {}", in(reg) cr3);
        }
    }

    /// Read CR4
    pub fn cr4() -> u64 {
        let cr4: u64;
        unsafe {
            asm!("mov {}, cr4", out(reg) cr4);
        }
        cr4
    }

    /// Write CR4
    pub fn set_cr4(cr4: u64) {
        unsafe {
            asm!("mov cr4, {}", in(reg) cr4);
        }
    }
}

// =============================================================================
// User Mode Entry
// =============================================================================

/// GDT segment selectors for user mode
pub mod selectors {
    /// User code segment selector (ring 3)
    pub const USER_CODE_64: u16 = 0x23; // GDT index 4, RPL 3
    /// User data segment selector (ring 3)
    pub const USER_DATA: u16 = 0x2B; // GDT index 5, RPL 3
}

/// Enter user mode and begin execution at the given address
///
/// This sets up the IRETQ stack frame and performs IRETQ to transition
/// from ring 0 (kernel) to ring 3 (user mode).
///
/// # Safety
/// This function never returns. It transitions execution to user mode.
pub unsafe fn enter_user_mode(entry_point: u64, stack_pointer: u64) -> ! {
    // Set up the IRETQ frame on the stack:
    // [RSP + 32] = SS (user data selector, ring 3)
    // [RSP + 24] = RSP (user stack pointer)
    // [RSP + 16] = RFLAGS (with interrupts enabled)
    // [RSP + 8]  = CS (user code selector, ring 3)
    // [RSP + 0]  = RIP (entry point)

    // RFLAGS: Enable interrupts (IF=1), and reserved bit 1 must be set
    let rflags: u64 = 0x202; // IF=1, reserved bit 1=1

    asm!(
        // Clear all general-purpose registers to prevent kernel data leakage
        "xor rax, rax",
        "xor rbx, rbx",
        "xor rcx, rcx",
        "xor rdx, rdx",
        "xor rsi, rsi",
        "xor rdi, rdi",
        "xor r8, r8",
        "xor r9, r9",
        "xor r10, r10",
        "xor r11, r11",
        "xor r12, r12",
        "xor r13, r13",
        "xor r14, r14",
        "xor r15, r15",
        "xor rbp, rbp",

        // Push IRETQ frame
        "push {ss}",      // SS
        "push {rsp_user}", // RSP
        "push {rflags}",   // RFLAGS
        "push {cs}",       // CS
        "push {rip}",      // RIP

        // Return to user mode
        "iretq",

        ss = in(reg) selectors::USER_DATA as u64,
        rsp_user = in(reg) stack_pointer,
        rflags = in(reg) rflags,
        cs = in(reg) selectors::USER_CODE_64 as u64,
        rip = in(reg) entry_point,
        options(noreturn),
    );
}

/// Model-specific registers
pub mod msr {
    use core::arch::asm;

    /// IA32_EFER MSR
    pub const EFER: u32 = 0xC0000080;
    /// IA32_STAR MSR (syscall target)
    pub const STAR: u32 = 0xC0000081;
    /// IA32_LSTAR MSR (long mode syscall target)
    pub const LSTAR: u32 = 0xC0000082;
    /// IA32_FMASK MSR (syscall flag mask)
    pub const FMASK: u32 = 0xC0000084;
    /// FS base
    pub const FS_BASE: u32 = 0xC0000100;
    /// GS base
    pub const GS_BASE: u32 = 0xC0000101;
    /// Kernel GS base
    pub const KERNEL_GS_BASE: u32 = 0xC0000102;

    /// Read MSR
    pub fn rdmsr(msr: u32) -> u64 {
        let low: u32;
        let high: u32;
        unsafe {
            asm!(
                "rdmsr",
                in("ecx") msr,
                out("eax") low,
                out("edx") high,
            );
        }
        ((high as u64) << 32) | (low as u64)
    }

    /// Write MSR
    pub fn wrmsr(msr: u32, value: u64) {
        let low = value as u32;
        let high = (value >> 32) as u32;
        unsafe {
            asm!(
                "wrmsr",
                in("ecx") msr,
                in("eax") low,
                in("edx") high,
            );
        }
    }
}
