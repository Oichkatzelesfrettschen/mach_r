//! Mach_R kernel binary entry point

#![no_std]
#![no_main]

extern crate mach_r;

use core::panic::PanicInfo;
use mach_r::{console, memory, port, task, trap, drivers};

/// Kernel entry point for ARM64
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Initialize stack pointer and other CPU state
    // This would normally be done in assembly
    
    // Jump to kernel main
    kernel_main()
}

/// Kernel entry point for x86_64
#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Initialize stack pointer and other CPU state
    // This would normally be done in assembly
    
    // Jump to kernel main
    kernel_main()
}

/// Main kernel initialization
fn kernel_main() -> ! {
    // Initialize console for output
    console::init();
    
    mach_r::println!("{} Microkernel v{}", mach_r::NAME, mach_r::VERSION);
    mach_r::println!("Initializing core subsystems...");
    
    // Initialize memory management
    memory::init();
    mach_r::println!("[OK] Memory management");
    
    // Initialize drivers
    drivers::init();
    mach_r::println!("[OK] Device drivers");
    
    // Initialize port subsystem
    port::init();
    mach_r::println!("[OK] Port subsystem");
    
    // Initialize task management
    task::init();
    mach_r::println!("[OK] Task management");
    
    // Initialize trap interface
    trap::init();
    mach_r::println!("[OK] Trap interface");
    
    // Initialize userland
    mach_r::userland::init();
    mach_r::println!("[OK] Userland subsystem");
    
    mach_r::println!("Mach_R kernel initialized successfully");
    mach_r::println!("");
    
    // Start init process
    mach_r::println!("Starting init process...");
    mach_r::init::start_init()
}

/// Panic handler
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    mach_r::println!("\n!!! KERNEL PANIC !!!");
    
    if let Some(location) = info.location() {
        mach_r::println!("Location: {}:{}", location.file(), location.line());
    }
    
    // info.message() returns a PanicMessage in newer Rust versions
    mach_r::println!("Message: {}", info.message());
    
    mach_r::println!("System halted.");
    
    loop {
        core::hint::spin_loop();
    }
}