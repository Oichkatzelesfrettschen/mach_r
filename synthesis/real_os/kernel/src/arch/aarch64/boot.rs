//! ARM64 boot code

/// Boot entry point - called by bootloader
#[no_mangle]
#[link_section = ".text.boot"]
pub extern "C" fn _start() -> ! {
    // Clear BSS
    unsafe {
        extern "C" {
            static mut __bss_start: u8;
            static mut __bss_end: u8;
        }
        
        let bss_start = &raw mut __bss_start as *mut u8;
        let bss_end = &raw mut __bss_end as *mut u8;
        let bss_size = bss_end as usize - bss_start as usize;
        core::ptr::write_bytes(bss_start, 0, bss_size);
    }
    
    // Jump to kernel main
    crate::kernel_main()
}