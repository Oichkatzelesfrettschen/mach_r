//! x86_64 boot code with Multiboot2 support

use core::arch::asm;

/// Multiboot2 magic number
const MULTIBOOT2_MAGIC: u32 = 0xe85250d6;
const MULTIBOOT_ARCHITECTURE_I386: u32 = 0;

/// Multiboot2 header
#[repr(C, align(8))]
struct Multiboot2Header {
    magic: u32,
    architecture: u32,
    header_length: u32,
    checksum: u32,
    // End tag
    end_tag_type: u16,
    end_tag_flags: u16,
    end_tag_size: u32,
}

/// Multiboot2 header placed in .multiboot section
#[used]
#[link_section = ".multiboot"]
static MULTIBOOT_HEADER: Multiboot2Header = {
    const HEADER_LENGTH: u32 = core::mem::size_of::<Multiboot2Header>() as u32;
    Multiboot2Header {
        magic: MULTIBOOT2_MAGIC,
        architecture: MULTIBOOT_ARCHITECTURE_I386,
        header_length: HEADER_LENGTH,
        checksum: 0u32.wrapping_sub(MULTIBOOT2_MAGIC)
            .wrapping_sub(MULTIBOOT_ARCHITECTURE_I386)
            .wrapping_sub(HEADER_LENGTH),
        end_tag_type: 0,
        end_tag_flags: 0,
        end_tag_size: 8,
    }
};

/// Boot stack size (64 KB)
const STACK_SIZE: usize = 64 * 1024;

/// Boot stack
#[repr(align(16))]
struct Stack([u8; STACK_SIZE]);

#[used]
#[link_section = ".bss"]
static mut BOOT_STACK: Stack = Stack([0; STACK_SIZE]);

/// Entry point called by bootloader
#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        // Set up stack pointer
        let stack_top = BOOT_STACK.0.as_ptr().add(STACK_SIZE) as u64;
        asm!(
            "mov rsp, {}",
            "mov rbp, {}",
            in(reg) stack_top,
            in(reg) stack_top,
        );

        // Clear direction flag (required by System V ABI)
        asm!("cld");

        // Call kernel main
        kmain();
    }
}

/// Kernel main function
#[no_mangle]
pub extern "C" fn kmain() -> ! {
    // Initialize serial console first for early debug output
    crate::drivers::serial::init();
    crate::serial_println!("Mach_R: Serial console initialized");

    // Initialize VGA console
    #[cfg(target_arch = "x86_64")]
    {
        crate::drivers::vga::init();
        crate::vga_println!("Mach_R v{} - x86_64 Boot", crate::VERSION);
        crate::vga_println!("==================================");
    }

    crate::serial_println!("Mach_R: VGA console initialized");

    // Initialize GDT
    crate::serial_println!("Mach_R: Initializing GDT...");
    super::gdt::init();
    crate::serial_println!("Mach_R: GDT initialized");

    #[cfg(target_arch = "x86_64")]
    crate::vga_println!("[OK] GDT");

    // Display boot message
    #[cfg(target_arch = "x86_64")]
    {
        crate::vga_println!("");
        crate::vga_println!("Mach_R Microkernel booted successfully!");
        crate::vga_println!("Architecture: x86_64");
        crate::vga_println!("Boot protocol: Multiboot2");
        crate::vga_println!("");
    }

    crate::serial_println!("Mach_R: Boot complete!");
    crate::serial_println!("Mach_R: Entering idle loop...");

    // Halt loop
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}