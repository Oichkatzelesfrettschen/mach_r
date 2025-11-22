//! Mach_R kernel binary entry point

#![no_std]
#![no_main]

extern crate mach_r;
extern crate alloc; // Add alloc crate
use alloc::string::String;

use core::panic::PanicInfo;
use mach_r::{
    arch,
    console,
    memory,
    ipc,
    task,
    scheduler,
    trap,
    drivers,
    boot,
    userland,
};




/// Kernel entry point for ARM64
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // This calls the boot.s from real_os which jumps to kernel_main
    kernel_main()
}

/// Kernel entry point for x86_64
#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // This calls the boot.s from real_os which jumps to kernel_main
    kernel_main()
}

/// Main kernel initialization
fn kernel_main() -> ! {
    // Initialize console first so we can see output
    console::init();
    mach_r::println!("\n\n=== MACH_R Microkernel v0.1.0 ===");
    mach_r::println!("Pure Rust, REAL implementation");
    mach_r::println!("");
    
    // Initialize architecture-specific features
    mach_r::print!("[INIT] Architecture... ");
    boot::arch_init(); // This now includes EL1 check for AArch64
    mach_r::println!("OK");
    
    // Initialize physical memory management (using consolidated memory module)
    mach_r::print!("[INIT] Memory management... ");
    memory::init(); // This will initialize heap and page manager
    mach_r::println!("OK");
    
    // Initialize synchronization primitives
    mach_r::print!("[INIT] Synchronization... ");

    mach_r::println!("OK");

    // Initialize IPC system
    mach_r::print!("[INIT] IPC subsystem... ");
    ipc::init();
    mach_r::println!("OK");
    
    // Initialize task management
    mach_r::print!("[INIT] Task management... ");
    task::init(String::from("kernel_task")); // Pass kernel task name
    mach_r::println!("OK");
    
    // Initialize scheduler
    mach_r::print!("[INIT] Scheduler... ");
    scheduler::init(scheduler::idle_thread_entry as unsafe extern "C" fn() -> !); // Pass idle thread entry point
    mach_r::println!("OK");
    
    // Initialize trap interface
    mach_r::print!("[INIT] Trap interface... ");
    trap::init();
    mach_r::println!("OK");

    // Initialize drivers
    mach_r::print!("[INIT] Device drivers... ");
    drivers::init();
    mach_r::println!("OK");
    
    // Initialize userland (if applicable)
    mach_r::print!("[INIT] Userland subsystem... ");
    userland::init();
    mach_r::println!("OK");

    mach_r::println!("Mach_R kernel initialized successfully");
    mach_r::println!("");
    
    // Start the scheduler and enter idle loop
    mach_r::println!("[KERNEL] Starting main scheduler loop...");
    scheduler::global_scheduler().schedule(); // Initial schedule call

    // Fallback if scheduler somehow returns (should not happen for idle thread)
    loop {
        arch::halt();
    }
}

/// Panic handler
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    mach_r::panic::kernel_panic(info); // Use the centralized kernel_panic from mach_r::panic
}