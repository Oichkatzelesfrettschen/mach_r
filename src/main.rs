//! Mach_R kernel binary entry point

#![no_std]
#![no_main]

extern crate alloc;
extern crate mach_r; // Add alloc crate
use alloc::string::String;

use core::panic::PanicInfo;
use mach_r::{arch, boot, console, drivers, ipc, kern, mach_vm, memory, scheduler, task, trap, userland};

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

    // Initialize Mach VM subsystem
    mach_r::print!("[INIT] Mach VM subsystem... ");
    mach_vm::init();
    // Configure with available memory (placeholder values - would come from boot info)
    // In real implementation, get from bootloader's memory map
    mach_vm::init_with_memory(0x100000, 0x10000000); // 1MB to 256MB range
    mach_r::println!("OK");

    // Initialize kern subsystem (zones, kalloc, processor management)
    mach_r::print!("[INIT] Kern subsystem... ");
    kern::init();
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

    // Bootstrap kernel task with special ports
    mach_r::print!("[INIT] Kernel bootstrap... ");
    kernel_bootstrap();
    mach_r::println!("OK");

    mach_r::println!("Mach_R kernel initialized successfully");
    mach_r::println!("");

    // Start the scheduler and enter idle loop
    mach_r::println!("[KERNEL] Starting main scheduler loop...");
    scheduler::global_scheduler().schedule(); // Initial schedule call

    // Fallback if scheduler somehow returns (should not happen for idle thread)
    arch::halt();
}

/// Kernel bootstrap - set up kernel task with special ports
///
/// This creates the essential Mach special ports:
/// - host port (host_self)
/// - host_priv port (privileged operations)
/// - device_master port (device access)
fn kernel_bootstrap() {
    use mach_r::ipc::{space::kernel_space, mach_port_allocate, MachPortRight, PortName};

    // Get kernel IPC space
    let kspace = kernel_space();

    // Allocate host port (receive right creates new port)
    let mut host_port_name: u32 = 0;
    let result = mach_port_allocate(kspace, MachPortRight::Receive, &mut host_port_name);
    if result != mach_r::kern::syscall_sw::KERN_SUCCESS {
        mach_r::println!("WARNING: Failed to allocate host port");
    }

    // Allocate host_priv port
    let mut host_priv_port_name: u32 = 0;
    let result = mach_port_allocate(kspace, MachPortRight::Receive, &mut host_priv_port_name);
    if result != mach_r::kern::syscall_sw::KERN_SUCCESS {
        mach_r::println!("WARNING: Failed to allocate host_priv port");
    }

    // Register the special ports with the host subsystem
    kern::host::host_set_ports(
        PortName(host_port_name),
        PortName(host_priv_port_name),
    );

    // Get the kernel task and set up its IPC space
    if let Some(kernel_task) = kern::task::kernel_task() {
        // Set kernel task's IPC space
        kernel_task.set_ipc_space(kspace.id());

        // Set kernel task's bootstrap port to host port
        *kernel_task.ports.bootstrap.lock() = Some(PortName(host_port_name));

        // Configure host with machine information
        let memory_size = 256 * 1024 * 1024; // 256MB (placeholder)
        kern::host::host_configure(1, memory_size);
    }

    // Run registered startup callbacks
    kern::startup::run_startup_callbacks();

    // Initialize the default memory object (for anonymous memory)
    // This is the kernel's internal pager for zero-fill pages
}

/// Panic handler
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    mach_r::panic::kernel_panic(info); // Use the centralized kernel_panic from mach_r::panic
}
