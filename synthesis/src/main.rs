//! Mach_R kernel binary entry point

#![no_std]
#![no_main]

use core::panic::PanicInfo;

// Kernel entry point is in the architecture-specific boot module
// For x86_64: see arch::x86_64::boot::_start
// For ARM64: see arch::aarch64::boot::_start

/// Panic handler
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Try to use serial console for panic output
    #[cfg(target_arch = "x86_64")]
    {
        use core::fmt::Write;
        let mut writer = mach_r::drivers::serial::SerialWriter;
        let _ = writeln!(writer, "\n!!! KERNEL PANIC !!!");

        if let Some(location) = info.location() {
            let _ = writeln!(writer, "Location: {}:{}", location.file(), location.line());
        }

        let _ = writeln!(writer, "Message: {}", info.message());
        let _ = writeln!(writer, "System halted.");
    }

    // Halt forever
    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("hlt");
        }

        #[cfg(not(target_arch = "x86_64"))]
        core::hint::spin_loop();
    }
}