//! Simple bootloader for MACH_R kernel
//! 
//! This bootloader:
//! 1. Sets up initial environment
//! 2. Loads the kernel
//! 3. Jumps to kernel entry point

#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::panic::PanicInfo;
use core::arch::asm;

const UART_BASE: usize = 0x0900_0000;

unsafe fn uart_putc(c: u8) {
    let uart = UART_BASE as *mut u8;
    uart.write_volatile(c);
}

unsafe fn uart_puts(s: &str) {
    for byte in s.bytes() {
        if byte == b'\n' {
            uart_putc(b'\r');
        }
        uart_putc(byte);
    }
}

/// Bootloader entry point
#[naked]
#[no_mangle]
#[link_section = ".text.boot"]
pub unsafe extern "C" fn _start() -> ! {
    asm!(
        // Initialize stack
        "ldr x0, =0x80000",
        "mov sp, x0",
        
        // Jump to bootloader main
        "bl bootloader_main",
        
        // Should never return
        "1:",
        "wfi",
        "b 1b",
        options(noreturn)
    );
}

#[no_mangle]
unsafe extern "C" fn bootloader_main() -> ! {
    uart_puts("\n[BOOT] MACH_R Bootloader v0.1.0\n");
    uart_puts("[BOOT] Initializing...\n");
    
    // In a real bootloader, we would:
    // 1. Set up page tables
    // 2. Load kernel from disk
    // 3. Parse ELF headers
    // 4. Relocate kernel
    
    // For now, assume kernel is loaded at 0x80000
    uart_puts("[BOOT] Jumping to kernel at 0x80000...\n\n");
    
    // Jump to kernel
    let kernel_entry: extern "C" fn() -> ! = core::mem::transmute(0x80000usize);
    kernel_entry();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        uart_puts("\n[BOOT] PANIC!\n");
    }
    loop {
        unsafe { asm!("wfi"); }
    }
}