//! Mach_R - A modern Rust implementation of the Mach microkernel
//!
//! This crate provides a memory-safe, high-performance implementation
//! of Mach's core concepts including ports, messages, tasks, and threads.

#![no_std]
#![cfg_attr(not(test), no_main)]
#![allow(dead_code)]
// TODO: Remove once all features are implemented

// Kernel-appropriate clippy configuration
// Many kernel types have specialized initialization that doesn't fit Default
#![allow(clippy::new_without_default)]
// Hardware register code often uses explicit bit shifts for documentation
#![allow(clippy::identity_op)]
// Kernel code often needs explicit casts for memory-mapped I/O
#![allow(clippy::unnecessary_cast)]
// Manual ceiling division is clearer in memory allocation contexts
#![allow(clippy::manual_div_ceil)]
// Kernel IPC returns () errors for simple failure indication
#![allow(clippy::result_unit_err)]
// Large Message types in Result errors are intentional for IPC recovery
#![allow(clippy::result_large_err)]
// Large enum variants are expected for IPC message body types
#![allow(clippy::large_enum_variant)]

// #![feature(alloc_error_handler)] // Only on nightly

// Standard library replacement for no_std
extern crate alloc;

// Core types
pub mod types;

// Re-exports
pub mod arch;
pub mod async_ipc;
pub mod console;
pub mod drivers;
pub mod external_pager;
pub mod init;
pub mod interrupt;
pub mod ipc;
pub mod kern;
pub mod libc;
pub mod mach;
pub mod memory;
pub mod message;
pub mod mig;
pub mod paging;
pub mod panic;
pub mod port;
pub mod scheduler;
pub mod servers;
pub mod sync;
pub mod syscall;
pub mod task;
pub mod trap;
pub mod userland;
pub mod utilities;

// Pure Rust stack components
pub mod fs;
pub mod net;
pub mod vm;

// Mach VM subsystem (separate from EVM vm module)
pub mod mach_vm;

// Mach Device subsystem
pub mod device;

// System components
pub mod shell;
// pub mod build;
pub mod coreutils;

// Boot components
pub mod boot;

/// Kernel version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Kernel name
pub const NAME: &str = "Mach_R";

// Panic handler is in main.rs for the binary

// Custom allocator error handler (nightly only)
// #[cfg(not(test))]
// #[alloc_error_handler]
// fn alloc_error(layout: alloc::alloc::Layout) -> ! {
//     panic!("Allocation error: {:?}", layout);
// }

/// Initialize the kernel library
pub fn init() {
    // Initialize kern subsystem (processors, timers, scheduling primitives)
    kern::init();

    // Initialize Mach VM subsystem
    mach_vm::init();

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

    // if let Err(_e) = build::init() {
    // Handle build system initialization error
    // }

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
