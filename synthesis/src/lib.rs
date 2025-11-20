//! Mach_R - A modern Rust implementation of the Mach microkernel
//!
//! This crate provides a memory-safe, high-performance implementation
//! of Mach's core concepts including ports, messages, tasks, and threads.

#![no_std]
// #![feature(alloc_error_handler)] // Only on nightly

// Standard library replacement for no_std
extern crate alloc;

// Core types
pub mod types;

// Re-exports
pub mod console;
pub mod memory;
pub mod port;
pub mod message;
pub mod task;
pub mod async_ipc;
pub mod interrupt;
pub mod scheduler;
pub mod syscall;
pub mod paging;
pub mod external_pager;
pub mod arch;
pub mod drivers;
pub mod mig;
pub mod mach;
pub mod trap;
pub mod libc;
pub mod servers;
pub mod init;
pub mod utilities;
pub mod userland;

// Pure Rust stack components
pub mod net;
pub mod fs;
pub mod vm;

// System components
pub mod shell;
pub mod build;
pub mod coreutils;

// Boot components
pub mod boot;

/// Kernel version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Kernel name
pub const NAME: &str = "Mach_R";

// Panic handler for staticlib
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // TODO: Print panic message to serial console once initialization is complete
    // For now, just halt the CPU
    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::asm!("hlt"); }
        #[cfg(target_arch = "aarch64")]
        unsafe { core::arch::asm!("wfe"); }
    }
}

// Custom allocator error handler (nightly only)
// #[cfg(not(test))]
// #[alloc_error_handler]
// fn alloc_error(layout: alloc::alloc::Layout) -> ! {
//     panic!("Allocation error: {:?}", layout);
// }

/// Initialize the kernel library
pub fn init() {
    // Initialize pure Rust stack components
    if let Err(_e) = net::init() {
        // Handle network initialization error
    }
    
    if let Err(_e) = fs::init() {
        // Handle filesystem initialization error
    }
    
    if let Err(_e) = vm::init() {
        // Handle VM initialization error
    }
    
    // Initialize system components
    if let Err(_e) = shell::init() {
        // Handle shell initialization error
    }
    
    if let Err(_e) = build::init() {
        // Handle build system initialization error
    }
    
    if let Err(_e) = coreutils::init() {
        // Handle coreutils initialization error
    }
    
    // Initialize init system
    if let Err(_e) = init::init() {
        // Handle init system initialization error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Initialize allocator for tests
    fn init_test() {
        use core::sync::atomic::{AtomicBool, Ordering};
        static INIT: AtomicBool = AtomicBool::new(false);
        
        if !INIT.swap(true, Ordering::SeqCst) {
            memory::init();
        }
    }
    
    #[test]
    fn test_version() {
        init_test();
        assert_eq!(NAME, "Mach_R");
        assert!(!VERSION.is_empty());
    }
}
