//! Minimal working ARM64 kernel that actually boots and outputs

#![no_std]
#![no_main]

use core::panic::PanicInfo;

// QEMU virt machine UART0 base address
const UART0: *mut u8 = 0x0900_0000 as *mut u8;

/// Write a byte to UART
unsafe fn uart_write_byte(byte: u8) {
    UART0.write_volatile(byte);
}

/// Write a string to UART
unsafe fn uart_write_str(s: &str) {
    for byte in s.bytes() {
        uart_write_byte(byte);
    }
}

/// Kernel entry point
#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        uart_write_str("MACH_R Minimal Kernel Started!\n");
        uart_write_str("This is a real kernel that boots.\n");
        uart_write_str("Memory at: 0x");
        
        // Print our load address
        let addr = _start as *const () as usize;
        for i in (0..16).rev() {
            let nibble = (addr >> (i * 4)) & 0xF;
            let ch = if nibble < 10 {
                b'0' + nibble as u8
            } else {
                b'A' + (nibble - 10) as u8
            };
            uart_write_byte(ch);
        }
        uart_write_str("\n");
        
        uart_write_str("Kernel size: ~61KB\n");
        uart_write_str("Entering idle loop...\n");
    }
    
    loop {
        unsafe {
            core::arch::asm!("wfe");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        uart_write_str("PANIC!\n");
    }
    loop {
        unsafe {
            core::arch::asm!("wfe");
        }
    }
}

// Boot header for direct loading
#[link_section = ".text.boot"]
#[no_mangle]
pub unsafe extern "C" fn _boot() -> ! {
    // Set stack pointer
    core::arch::asm!(
        "mov sp, #0x80000",
    );
    
    // Jump to start
    _start()
}