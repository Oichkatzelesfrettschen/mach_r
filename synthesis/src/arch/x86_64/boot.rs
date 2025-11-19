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
///
/// On entry, registers contain:
/// - EAX/RAX: Multiboot2 magic number (0x36d76289)
/// - EBX/RBX: Physical address of Multiboot2 info structure
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Capture multiboot parameters before setting up stack
    let (magic, multiboot_info): (u64, u64);
    unsafe {
        asm!(
            "mov {0}, rax",
            "mov {1}, rbx",
            out(reg) magic,
            out(reg) multiboot_info,
            options(nomem, nostack, preserves_flags)
        );

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

        // Call kernel main with multiboot parameters
        kmain(magic, multiboot_info);
    }
}

/// Kernel main function
///
/// Called by _start with:
/// - magic: Multiboot2 magic number (should be 0x36d76289)
/// - multiboot_info: Physical address of Multiboot2 info structure
#[no_mangle]
pub extern "C" fn kmain(magic: u64, multiboot_info: u64) -> ! {
    use crate::boot::multiboot2::{Multiboot2InfoParser, verify_magic, format_memory_size, TagType};

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

    // Display boot entry information
    crate::vga_println!("");
    crate::vga_println!("Boot Information:");
    crate::vga_println!("  Entry:    _start");
    crate::vga_println!("  Magic:    0x{:08x}", magic as u32);
    crate::vga_println!("  MB2 Info: 0x{:016x}", multiboot_info);

    crate::serial_println!("Mach_R: Kernel entry at _start");
    crate::serial_println!("Mach_R: Magic number: 0x{:08x}", magic as u32);
    crate::serial_println!("Mach_R: Multiboot2 info at: 0x{:016x}", multiboot_info);

    // Verify Multiboot2 magic number
    if !verify_magic(magic as u32) {
        crate::vga_println!("");
        crate::vga_println!("[ERROR] Invalid Multiboot2 magic!");
        crate::vga_println!("  Expected: 0x36d76289");
        crate::vga_println!("  Got:      0x{:08x}", magic as u32);
        crate::serial_println!("Mach_R: ERROR - Invalid Multiboot2 magic number!");
        loop {
            unsafe { asm!("hlt"); }
        }
    }

    crate::vga_println!("  [OK] Valid Multiboot2 magic");
    crate::serial_println!("Mach_R: Multiboot2 magic verified");

    // Parse Multiboot2 information structure
    let mb2_info = unsafe {
        match Multiboot2InfoParser::new(multiboot_info) {
            Some(info) => info,
            None => {
                crate::vga_println!("");
                crate::vga_println!("[ERROR] Invalid Multiboot2 info pointer!");
                crate::serial_println!("Mach_R: ERROR - Invalid Multiboot2 info structure");
                loop {
                    unsafe { asm!("hlt"); }
                }
            }
        }
    };

    crate::vga_println!("  [OK] Multiboot2 info parsed");
    crate::vga_println!("       Total size: {} bytes", mb2_info.total_size());
    crate::serial_println!("Mach_R: Multiboot2 info size: {} bytes", mb2_info.total_size());

    // Display bootloader name
    if let Some(name) = mb2_info.bootloader_name() {
        crate::vga_println!("");
        crate::vga_println!("Bootloader: {}", name);
        crate::serial_println!("Mach_R: Bootloader: {}", name);
    }

    // Display command line
    if let Some(cmdline) = mb2_info.command_line() {
        crate::vga_println!("Command:    {}", cmdline);
        crate::serial_println!("Mach_R: Command line: {}", cmdline);
    }

    // Display memory information
    crate::vga_println!("");
    crate::vga_println!("Memory Information:");

    if let Some((mem_lower, mem_upper)) = mb2_info.basic_memory_info() {
        let (lower_val, lower_unit) = format_memory_size(mem_lower);
        let (upper_val, upper_unit) = format_memory_size(mem_upper);
        crate::vga_println!("  Lower:  {} {}", lower_val, lower_unit);
        crate::vga_println!("  Upper:  {} {}", upper_val, upper_unit);
        crate::serial_println!("Mach_R: Lower memory: {} KB", mem_lower);
        crate::serial_println!("Mach_R: Upper memory: {} KB", mem_upper);
    }

    // Display memory map
    if let Some(mmap) = mb2_info.memory_map() {
        crate::vga_println!("");
        crate::vga_println!("Memory Map:");
        crate::serial_println!("Mach_R: Memory map:");

        let mut region_count = 0;
        let mut total_available = 0u64;

        for entry in mmap {
            region_count += 1;

            if entry.mem_type == 1 {
                total_available += entry.length;
            }

            // Show first few entries on VGA
            if region_count <= 5 {
                crate::vga_println!(
                    "  [{:2}] 0x{:016x} - 0x{:016x} ({})",
                    region_count,
                    entry.base_addr,
                    entry.base_addr + entry.length - 1,
                    entry.type_name()
                );
            }

            // Log all entries to serial
            crate::serial_println!(
                "Mach_R:   Region {}: 0x{:016x} - 0x{:016x} ({} bytes) - {}",
                region_count,
                entry.base_addr,
                entry.base_addr + entry.length - 1,
                entry.length,
                entry.type_name()
            );
        }

        if region_count > 5 {
            crate::vga_println!("  ... and {} more regions", region_count - 5);
        }

        let (total_val, total_unit) = format_memory_size((total_available / 1024) as u32);
        crate::vga_println!("");
        crate::vga_println!("Total Available: {} {}", total_val, total_unit);
        crate::serial_println!("Mach_R: Total available memory: {} bytes", total_available);
    }

    // Display framebuffer info if present
    if let Some(fb) = mb2_info.framebuffer() {
        crate::vga_println!("");
        crate::vga_println!("Framebuffer:");
        crate::vga_println!("  Address:    0x{:016x}", fb.framebuffer_addr);
        crate::vga_println!("  Resolution: {}x{}", fb.framebuffer_width, fb.framebuffer_height);
        crate::vga_println!("  Pitch:      {} bytes", fb.framebuffer_pitch);
        crate::vga_println!("  BPP:        {}", fb.framebuffer_bpp);
        crate::vga_println!("  Type:       {}", fb.type_name());

        crate::serial_println!("Mach_R: Framebuffer at 0x{:016x}", fb.framebuffer_addr);
        crate::serial_println!("Mach_R:   {}x{} @ {} bpp",
            fb.framebuffer_width, fb.framebuffer_height, fb.framebuffer_bpp);
    }

    // Display load base address if present
    if let Some(load_addr) = mb2_info.load_base_addr() {
        crate::vga_println!("");
        crate::vga_println!("Kernel Load: 0x{:08x}", load_addr);
        crate::serial_println!("Mach_R: Load base address: 0x{:08x}", load_addr);
    }

    // List all tags found
    crate::vga_println!("");
    crate::vga_println!("Multiboot2 Tags Found:");
    crate::serial_println!("Mach_R: Multiboot2 tags:");

    let mut tag_count = 0;
    for tag in mb2_info.tags() {
        tag_count += 1;

        if let Some(tag_type) = TagType::from_u32(tag.tag_type) {
            if tag_count <= 8 {
                crate::vga_println!("  [{}] {} ({} bytes)",
                    tag_count, tag_type.name(), tag.size);
            }
            crate::serial_println!("Mach_R:   Tag {}: {} (type={}, size={})",
                tag_count, tag_type.name(), tag.tag_type, tag.size);
        } else {
            if tag_count <= 8 {
                crate::vga_println!("  [{}] Unknown (type={}, {} bytes)",
                    tag_count, tag.tag_type, tag.size);
            }
            crate::serial_println!("Mach_R:   Tag {}: Unknown (type={}, size={})",
                tag_count, tag.tag_type, tag.size);
        }
    }

    if tag_count > 8 {
        crate::vga_println!("  ... and {} more tags", tag_count - 8);
    }

    crate::serial_println!("Mach_R: Total tags found: {}", tag_count);

    // Initialize GDT
    crate::vga_println!("");
    crate::serial_println!("Mach_R: Initializing GDT...");
    super::gdt::init();
    crate::serial_println!("Mach_R: GDT initialized");

    #[cfg(target_arch = "x86_64")]
    crate::vga_println!("[OK] GDT initialized");

    // Display final boot message
    #[cfg(target_arch = "x86_64")]
    {
        crate::vga_println!("");
        crate::vga_println!("==================================");
        crate::vga_println!("Mach_R Microkernel Boot Complete!");
        crate::vga_println!("==================================");
        crate::vga_println!("");
        crate::vga_println!("System ready. Entering idle loop.");
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