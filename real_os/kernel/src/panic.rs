//! Panic handler

use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    crate::println!("\n!!! KERNEL PANIC !!!");
    
    if let Some(location) = info.location() {
        crate::println!("Location: {}:{}", location.file(), location.line());
    }
    
    // PanicInfo::message() returns PanicMessage directly in newer versions
    let msg = info.message();
    crate::println!("Message: {}", msg);
    
    crate::println!("System halted.");
    
    loop {
        crate::arch::wait_for_interrupt();
    }
}