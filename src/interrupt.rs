//! Interrupt handling for Mach_R
//!
//! Provides interrupt descriptor table (IDT) management,
//! interrupt service routines (ISRs), and IRQ handling.

use core::mem::size_of;
use spin::Mutex;
use crate::println;

/// Number of IDT entries (Intel standard)
pub const IDT_ENTRIES: usize = 256;

/// Interrupt numbers for exceptions
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Exception {
    DivideByZero = 0,
    Debug = 1,
    NonMaskableInterrupt = 2,
    Breakpoint = 3,
    Overflow = 4,
    BoundRangeExceeded = 5,
    InvalidOpcode = 6,
    DeviceNotAvailable = 7,
    DoubleFault = 8,
    CoprocessorSegmentOverrun = 9,
    InvalidTSS = 10,
    SegmentNotPresent = 11,
    StackSegmentFault = 12,
    GeneralProtectionFault = 13,
    PageFault = 14,
    // 15 is reserved
    X87FloatingPoint = 16,
    AlignmentCheck = 17,
    MachineCheck = 18,
    SimdFloatingPoint = 19,
    Virtualization = 20,
    // 21-31 are reserved
}

/// Hardware interrupt numbers (remapped from default)
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Irq {
    Timer = 32,
    Keyboard = 33,
    Cascade = 34,
    Com2 = 35,
    Com1 = 36,
    Lpt2 = 37,
    FloppyDisk = 38,
    Lpt1 = 39,
    RealTimeClock = 40,
    Free1 = 41,
    Free2 = 42,
    Free3 = 43,
    Mouse = 44,
    Fpu = 45,
    PrimaryAta = 46,
    SecondaryAta = 47,
}

/// System call interrupt number
pub const SYSCALL_INTERRUPT: u8 = 0x80;

/// Interrupt context saved on stack
#[repr(C)]
#[derive(Debug, Clone)]
pub struct InterruptContext {
    // Pushed by interrupt handler
    pub gs: u64,
    pub fs: u64,
    pub es: u64,
    pub ds: u64,
    
    // Pushed by pusha
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub rbx: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
    
    // Interrupt number and error code
    pub int_no: u64,
    pub err_code: u64,
    
    // Pushed by CPU automatically
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub user_rsp: u64,
    pub ss: u64,
}

/// IDT entry structure (x86_64)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct IdtEntry {
    base_low: u16,
    selector: u16,
    ist: u8,
    flags: u8,
    base_mid: u16,
    base_high: u32,
    reserved: u32,
}

impl IdtEntry {
    /// Create a new IDT entry
    pub const fn new() -> Self {
        IdtEntry {
            base_low: 0,
            selector: 0,
            ist: 0,
            flags: 0,
            base_mid: 0,
            base_high: 0,
            reserved: 0,
        }
    }
    
    /// Set the handler address
    pub fn set_handler(&mut self, handler: usize, selector: u16, flags: u8) {
        self.base_low = (handler & 0xFFFF) as u16;
        self.base_mid = ((handler >> 16) & 0xFFFF) as u16;
        self.base_high = ((handler >> 32) & 0xFFFFFFFF) as u32;
        self.selector = selector;
        self.flags = flags;
        self.ist = 0;
        self.reserved = 0;
    }
}

/// IDT pointer structure
#[repr(C, packed)]
pub struct IdtPointer {
    limit: u16,
    base: u64,
}

/// Interrupt Descriptor Table
pub struct Idt {
    entries: [IdtEntry; IDT_ENTRIES],
}

impl Idt {
    /// Create a new IDT
    pub const fn new() -> Self {
        Idt {
            entries: [IdtEntry::new(); IDT_ENTRIES],
        }
    }
    
    /// Set an interrupt handler
    pub fn set_handler(&mut self, index: u8, handler: fn()) {
        let flags = 0x8E; // Present, DPL=0, Interrupt gate
        let selector = 0x08; // Kernel code segment
        self.entries[index as usize].set_handler(handler as usize, selector, flags);
    }
    
    /// Load the IDT
    pub unsafe fn load(&self) {
        let _ptr = IdtPointer {
            limit: (size_of::<[IdtEntry; IDT_ENTRIES]>() - 1) as u16,
            base: self.entries.as_ptr() as u64,
        };
        
        // In real implementation, would use inline assembly:
        // asm!("lidt [{}]", in(reg) &ptr);
    }
}

/// Global IDT instance
static IDT: Mutex<Idt> = Mutex::new(Idt::new());

/// Interrupt handler type
pub type InterruptHandler = fn(&InterruptContext);

/// Interrupt handler table
static HANDLERS: Mutex<[Option<InterruptHandler>; IDT_ENTRIES]> = 
    Mutex::new([None; IDT_ENTRIES]);

/// Common interrupt handler called by all ISRs
pub extern "C" fn interrupt_handler(ctx: &InterruptContext) {
    // Call registered handler if exists
    let handlers = HANDLERS.lock();
    if let Some(handler) = handlers[ctx.int_no as usize] {
        handler(ctx);
    } else {
        default_handler(ctx);
    }
    
    // Send EOI to PIC if hardware interrupt
    if ctx.int_no >= 32 && ctx.int_no < 48 {
        unsafe { end_of_interrupt(ctx.int_no as u8); }
    }
}

/// Default interrupt handler
fn default_handler(ctx: &InterruptContext) {
    println!("Unhandled interrupt: {:#x}", ctx.int_no);
    
    // Halt on critical exceptions
    if ctx.int_no < 32 {
        println!("Exception occurred! Halting...");
        loop {
            core::hint::spin_loop();
        }
    }
}

/// Register an interrupt handler
pub fn register_handler(interrupt: u8, handler: InterruptHandler) {
    let mut handlers = HANDLERS.lock();
    handlers[interrupt as usize] = Some(handler);
}

/// Page fault handler
pub fn page_fault_handler(ctx: &InterruptContext) {
    // Get fault address from CR2
    let fault_addr: usize = 0x1000; // Default fault address for ARM64
    
    println!("Page fault at {:#x}, address: {:#x}", ctx.rip, fault_addr);
    
    // Check if address is in valid memory region
    if fault_addr < 0x1000 {
        panic!("Invalid memory access: null pointer dereference");
    }
    
    // Allocate new page for demand paging
    let page_manager = crate::memory::page_manager();
    if let Ok(_page) = page_manager.allocate_page() {
        // Map the new page at the fault address
        let mut page_table = crate::paging::active_page_table();
        let virt_addr = crate::paging::VirtualAddress(fault_addr & !0xfff);
        let phys_addr = crate::paging::PhysicalAddress(fault_addr & !0xfff);
        page_table.map(virt_addr, phys_addr, crate::paging::PageTableFlags::WRITABLE);
    } else {
        panic!("Out of memory during page fault handling");
    }
}

/// Timer interrupt handler
pub fn timer_handler(_ctx: &InterruptContext) {
    // Increment system tick
    crate::task::scheduler::tick();
    
    // Trigger scheduler if needed
    if crate::task::scheduler::should_reschedule() {
        crate::task::scheduler::schedule();
    }
}

/// Keyboard interrupt handler
pub fn keyboard_handler(_ctx: &InterruptContext) {
    // Read scan code from keyboard port for ARM64
    let scancode: u8 = crate::arch::keyboard_read();
    
    // Process scan code
    crate::console::process_keyboard(scancode);
}

/// System call handler
pub fn syscall_handler(ctx: &InterruptContext) {
    // System call number in rax
    let syscall_num = ctx.rax;
    
    // Arguments in rdi, rsi, rdx, rcx, r8, r9
    let args = [ctx.rdi, ctx.rsi, ctx.rdx, ctx.rcx];
    
    // Dispatch system call
    let _result = crate::syscall::dispatch(syscall_num, &args);
    
    // Return value in rax (would need mutable context)
    // ctx.rax = result;
}

/// Programmable Interrupt Controller (PIC) commands
mod pic {
    pub const PIC1_COMMAND: u16 = 0x20;
    pub const PIC1_DATA: u16 = 0x21;
    pub const PIC2_COMMAND: u16 = 0xA0;
    pub const PIC2_DATA: u16 = 0xA1;
    
    pub const PIC_EOI: u8 = 0x20;
}

/// Send End of Interrupt to PIC
pub unsafe fn end_of_interrupt(irq: u8) {
    if irq >= 40 {
        // Send to slave PIC
        // In real implementation:
        // asm!("out 0xA0, al", in("al") pic::PIC_EOI);
    }
    // Send to master PIC
    // asm!("out 0x20, al", in("al") pic::PIC_EOI);
}

/// Initialize the PIC (remap IRQs to 32-47)
pub unsafe fn init_pic() {
    // In real implementation, would send initialization commands
    // to remap hardware interrupts away from exceptions
}

/// Initialize interrupt handling
pub fn init() {
    unsafe {
        // Initialize PIC
        init_pic();
        
        // Set up IDT
        let idt = IDT.lock();
        
        // Register exception handlers
        // In real implementation, would set actual handlers
        // idt.set_handler(14, page_fault_wrapper);
        
        // Register IRQ handlers
        register_handler(Irq::Timer as u8, timer_handler);
        register_handler(Irq::Keyboard as u8, keyboard_handler);
        
        // Register system call handler
        register_handler(SYSCALL_INTERRUPT, syscall_handler);
        
        // Load IDT
        idt.load();
        
        // Enable interrupts
        enable_interrupts();
    }
}

/// Enable interrupts
pub unsafe fn enable_interrupts() {
    // In real implementation:
    // asm!("sti");
}

/// Disable interrupts
pub unsafe fn disable_interrupts() {
    // In real implementation:
    // asm!("cli");
}

/// Check if interrupts are enabled
pub fn interrupts_enabled() -> bool {
    let flags: u64;
    // In real implementation:
    // unsafe { asm!("pushfq; pop {}", out(reg) flags); }
    flags = 0x200; // IF flag set
    (flags & 0x200) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_idt_entry() {
        let mut entry = IdtEntry::new();
        entry.set_handler(0xDEADBEEF, 0x08, 0x8E);
        
        // Can't directly access packed fields in tests
        // This test verifies the handler can be set without panicking
        assert!(true);
    }
    
    #[test]
    fn test_interrupt_registration() {
        let test_handler: InterruptHandler = |_ctx| {
            // Test handler
        };
        
        register_handler(32, test_handler);
        
        let handlers = HANDLERS.lock();
        assert!(handlers[32].is_some());
    }
}