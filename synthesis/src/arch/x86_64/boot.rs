//! x86_64 boot code with Multiboot2 support

use core::arch::asm;

// NOTE: Multiboot2 header is now in boot32.asm, not here
// The assembly boot stub handles Multiboot2 protocol and transitions to 64-bit mode

/// Boot stack size (64 KB)
const STACK_SIZE: usize = 64 * 1024;

/// Boot stack (currently unused - boot32.asm manages stack)
#[repr(align(16))]
#[allow(dead_code)]
struct Stack([u8; STACK_SIZE]);

#[used]
#[link_section = ".bss"]
static mut BOOT_STACK: Stack = Stack([0; STACK_SIZE]);

/// Kernel main function - called by boot32.asm after 64-bit transition
///
/// On entry (from boot32.asm), parameters are:
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
    crate::vga_println!("  Entry:    _start (boot32.asm)");
    crate::vga_println!("  Magic:    0x{:08x}", magic as u32);
    crate::vga_println!("  MB2 Info: 0x{:016x}", multiboot_info);

    crate::serial_println!("Mach_R: Kernel entry via boot32.asm");
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
                    asm!("hlt");
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

            // Copy packed field values to avoid E0793 unaligned reference errors
            let entry_base = entry.base_addr;
            let entry_length = entry.length;
            let entry_type = entry.mem_type;

            if entry_type == 1 {
                total_available += entry_length;
            }

            // Show first few entries on VGA
            if region_count <= 5 {
                crate::vga_println!(
                    "  [{:2}] 0x{:016x} - 0x{:016x} ({})",
                    region_count,
                    entry_base,
                    entry_base + entry_length - 1,
                    entry.type_name()
                );
            }

            // Log all entries to serial
            crate::serial_println!(
                "Mach_R:   Region {}: 0x{:016x} - 0x{:016x} ({} bytes) - {}",
                region_count,
                entry_base,
                entry_base + entry_length - 1,
                entry_length,
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

        // Copy packed field values to avoid E0793 unaligned reference errors
        let fb_addr = fb.framebuffer_addr;
        let fb_width = fb.framebuffer_width;
        let fb_height = fb.framebuffer_height;
        let fb_pitch = fb.framebuffer_pitch;
        let fb_bpp = fb.framebuffer_bpp;

        crate::vga_println!("  Address:    0x{:016x}", fb_addr);
        crate::vga_println!("  Resolution: {}x{}", fb_width, fb_height);
        crate::vga_println!("  Pitch:      {} bytes", fb_pitch);
        crate::vga_println!("  BPP:        {}", fb_bpp);
        crate::vga_println!("  Type:       {}", fb.type_name());

        crate::serial_println!("Mach_R: Framebuffer at 0x{:016x}", fb_addr);
        crate::serial_println!("Mach_R:   {}x{} @ {} bpp", fb_width, fb_height, fb_bpp);
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

        // Copy packed field values to avoid E0793 unaligned reference errors
        let tag_type_val = tag.tag_type;
        let tag_size_val = tag.size;

        if let Some(tag_type) = TagType::from_u32(tag_type_val) {
            if tag_count <= 8 {
                crate::vga_println!("  [{}] {} ({} bytes)",
                    tag_count, tag_type.name(), tag_size_val);
            }
            crate::serial_println!("Mach_R:   Tag {}: {} (type={}, size={})",
                tag_count, tag_type.name(), tag_type_val, tag_size_val);
        } else {
            if tag_count <= 8 {
                crate::vga_println!("  [{}] Unknown (type={}, {} bytes)",
                    tag_count, tag_type_val, tag_size_val);
            }
            crate::serial_println!("Mach_R:   Tag {}: Unknown (type={}, size={})",
                tag_count, tag_type_val, tag_size_val);
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