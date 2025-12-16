//! x86_64 boot code
//! Multiboot2-compliant bootloader entry point
//!
//! This module provides the initial assembly code that runs when the bootloader
//! (GRUB2) loads our kernel. It performs:
//! 1. Multiboot2 header validation
//! 2. Long mode (64-bit) support detection
//! 3. Page table setup for higher-half kernel
//! 4. Transition from 32-bit protected mode to 64-bit long mode
//! 5. Jump to Rust kernel entry point

use core::arch::global_asm;

/// Stack size for boot process (16 KiB)
const BOOT_STACK_SIZE: usize = 16384;

/// Multiboot2 magic number
const MULTIBOOT2_MAGIC: u32 = 0x36d76289;

/// Page table flags
const PAGE_PRESENT: u64 = 1 << 0;
const PAGE_WRITABLE: u64 = 1 << 1;
const PAGE_HUGE: u64 = 1 << 7;

// Declare the stack in the .bss section
global_asm!(
    ".section .bss",
    ".align 16",
    "boot_stack_bottom:",
    ".skip {stack_size}",
    "boot_stack_top:",
    stack_size = const BOOT_STACK_SIZE,
);

// Multiboot2 header in its own section
global_asm!(
    ".section .multiboot",
    ".align 8",
    // Multiboot2 header start
    "multiboot2_header_start:",
    "    .long 0xe85250d6", // Magic number
    "    .long 0",          // Architecture: i386 (32-bit protected mode)
    "    .long multiboot2_header_end - multiboot2_header_start", // Header length
    "    .long -(0xe85250d6 + 0 + (multiboot2_header_end - multiboot2_header_start))", // Checksum
    // Framebuffer tag (optional)
    "framebuffer_tag_start:",
    "    .short 5",                                          // Type: framebuffer
    "    .short 1",                                          // Flags: optional
    "    .long framebuffer_tag_end - framebuffer_tag_start", // Size
    "    .long 1024",                                        // Width
    "    .long 768",                                         // Height
    "    .long 32",                                          // Depth (bits per pixel)
    "framebuffer_tag_end:",
    // End tag (required)
    "    .short 0", // Type: end
    "    .short 0", // Flags
    "    .long 8",  // Size
    "multiboot2_header_end:",
);

// Page tables in .bss section (will be zero-initialized)
global_asm!(
    ".section .bss",
    ".align 4096",
    // P4 (PML4) - Page Map Level 4
    ".global p4_table",
    "p4_table:",
    "    .skip 4096",
    // P3 (PDPT) - Page Directory Pointer Table
    ".global p3_table",
    "p3_table:",
    "    .skip 4096",
    // P2 (PD) - Page Directory
    ".global p2_table",
    "p2_table:",
    "    .skip 4096",
);

// Main boot code
global_asm!(
    ".section .text",
    ".code32",                                 // Start in 32-bit mode
    ".global _start_s",
    "_start_s:",
    "    # Bootloader (GRUB2) loads us here in 32-bit protected mode",
    "    # EAX contains Multiboot2 magic number",
    "    # EBX contains physical address of Multiboot2 information structure",

    "    # Set up stack",
    "    mov esp, offset boot_stack_top",

    "    # Save Multiboot2 info (we'll pass it to Rust later)",
    "    push ebx",                            // Multiboot info pointer
    "    push eax",                            // Multiboot magic

    "    # Verify Multiboot2 magic number",
    "    cmp eax, {magic}",
    "    jne .Lno_multiboot",

    "    # Check for CPUID support",
    "    call check_cpuid",
    "    test eax, eax",
    "    jz .Lno_cpuid",

    "    # Check for long mode support",
    "    call check_long_mode",
    "    test eax, eax",
    "    jz .Lno_long_mode",

    "    # Set up page tables",
    "    call setup_page_tables",

    "    # Enable PAE (Physical Address Extension)",
    "    mov eax, cr4",
    "    or eax, (1 << 5)",                    // Set PAE bit (bit 5)",
    "    mov cr4, eax",

    "    # Load P4 table address into CR3",
    "    lea eax, [p4_table]",
    "    mov cr3, eax",

    "    # Enable long mode in EFER MSR (Model Specific Register)",
    "    mov ecx, 0xC0000080",                 // EFER MSR number
    "    rdmsr",                                // Read MSR into EDX:EAX
    "    or eax, (1 << 8)",                    // Set LME (Long Mode Enable) bit
    "    wrmsr",                                // Write back to MSR

    "    # Enable paging and protected mode",
    "    mov eax, cr0",
    "    or eax, (1 << 31) | (1 << 0)",        // Set PG (paging) and PE (protected) bits
    "    mov cr0, eax",

    "    # Now we're in compatibility mode (32-bit code in 64-bit mode)",
    "    # Load 64-bit GDT and far jump to 64-bit code",
    "    lgdt [gdt64_pointer]",
    "    # Far jump using push + retf",
    "    push 0x08",                            // Push code segment selector
    "    lea eax, [.Llong_mode_start]",
    "    push eax",                             // Push offset
    "    retf",                                 // Far return (acts as far jump)

    // Error handlers (32-bit)
    ".Lno_multiboot:",
    "    mov al, 'M'",
    "    jmp .Lerror",

    ".Lno_cpuid:",
    "    mov al, 'C'",
    "    jmp .Lerror",

    ".Lno_long_mode:",
    "    mov al, 'L'",
    "    jmp .Lerror",

    ".Lerror:",
    "    # Write error code to VGA text buffer (0xB8000)",
    "    mov dword ptr [0xB8000], 0x4F524F45",  // 'ER' in red
    "    mov dword ptr [0xB8004], 0x4F3A4F52",  // 'R:' in red
    "    mov byte ptr [0xB8008], al",           // Error code
    "    mov byte ptr [0xB8009], 0x4F",         // Red background
    "    hlt",
    ".Lhang:",
    "    jmp .Lhang",

    // Check CPUID support
    "check_cpuid:",
    "    pushfd",
    "    pop eax",
    "    mov ecx, eax",
    "    xor eax, (1 << 21)",                  // Flip ID bit (bit 21)
    "    push eax",
    "    popfd",
    "    pushfd",
    "    pop eax",
    "    push ecx",                             // Restore original FLAGS
    "    popfd",
    "    cmp eax, ecx",
    "    je .Lno_cpuid_support",
    "    mov eax, 1",                           // CPUID supported
    "    ret",
    ".Lno_cpuid_support:",
    "    xor eax, eax",                         // CPUID not supported
    "    ret",

    // Check long mode support
    "check_long_mode:",
    "    # Check for extended CPUID functions",
    "    mov eax, 0x80000000",
    "    cpuid",
    "    cmp eax, 0x80000001",
    "    jb .Lno_long_mode_support",

    "    # Check for long mode bit",
    "    mov eax, 0x80000001",
    "    cpuid",
    "    test edx, (1 << 29)",                 // Long mode bit
    "    jz .Lno_long_mode_support",

    "    mov eax, 1",                           // Long mode supported
    "    ret",
    ".Lno_long_mode_support:",
    "    xor eax, eax",                         // Long mode not supported
    "    ret",

    // Set up page tables
    "setup_page_tables:",
    "    # Identity map first 2 MB (for boot code) + higher half kernel",
    "    # Use 2MB huge pages for simplicity",

    "    # P4[0] -> P3 (for identity mapping)",
    "    lea eax, [p3_table]",
    "    or eax, 0x03",                         // Present + Writable
    "    mov dword ptr [p4_table], eax",

    "    # P4[256] -> P3 (for higher half at 0xFFFF800000000000)",
    "    lea eax, [p3_table]",
    "    or eax, 0x03",                         // Present + Writable
    "    mov dword ptr [p4_table + 256*8], eax",

    "    # P3[0] -> P2",
    "    lea eax, [p2_table]",
    "    or eax, 0x03",                         // Present + Writable
    "    mov dword ptr [p3_table], eax",

    "    # P2[0..512] -> 2MB pages (map 1GB total)",
    "    mov ecx, 0",                           // Counter
    ".Lmap_p2_loop:",
    "    mov eax, ecx",
    "    shl eax, 21",                          // Multiply by 2MB
    "    or eax, 0x83",                         // Present + Writable + Huge",
    "    lea edx, [p2_table]",
    "    mov dword ptr [edx + ecx*8], eax",    // Write low 32 bits
    "    mov dword ptr [edx + ecx*8 + 4], 0",  // Write high 32 bits (0 for < 4GB)
    "    inc ecx",
    "    cmp ecx, 512",                         // Map 512 * 2MB = 1GB
    "    jne .Lmap_p2_loop",

    "    ret",

    // 64-bit code section
    ".code64",
    ".Llong_mode_start:",
    "    # We're now in 64-bit long mode!",

    "    # Set up segment registers (set to null segment except CS)",
    "    mov ax, 0x10",                         // GDT data segment
    "    mov ss, ax",
    "    mov ds, ax",
    "    mov es, ax",
    "    mov fs, ax",
    "    mov gs, ax",

    "    # Reload stack pointer (now 64-bit)",
    "    lea rsp, [rip + boot_stack_top]",

    "    # Pop Multiboot info from stack (now in 64-bit registers)",
    "    pop rdi",                              // Multiboot magic (first argument)",
    "    pop rsi",                              // Multiboot info pointer (second argument)",

    "    # Call Rust kernel entry point",
    "    # extern \"C\" fn kmain(magic: u64, multiboot_info: u64) -> !",
    "    call kmain",

    "    # If kmain returns (it shouldn't), halt",
    ".Lhalt64:",
    "    cli",
    "    hlt",
    "    jmp .Lhalt64",

    magic = const MULTIBOOT2_MAGIC,
);

// Global Descriptor Table (GDT) for 64-bit mode
global_asm!(
    ".section .rodata",
    ".align 16",
    "gdt64:",
    "    .quad 0",                  // Null descriptor
    "    .quad 0x00AF9A000000FFFF", // Code segment (64-bit, executable, readable)
    "    .quad 0x00AF92000000FFFF", // Data segment (64-bit, writable)
    "gdt64_end:",
    "gdt64_pointer:",
    "    .short gdt64_end - gdt64 - 1", // GDT limit
    "    .long gdt64",                  // GDT base (32-bit address for lgdt in 32-bit mode)
);

/// Rust kernel entry point
///
/// This function is called from assembly after the system is in 64-bit mode.
/// Arguments:
/// - magic: Multiboot2 magic number (should be 0x36d76289)
/// - multiboot_info: Physical address of Multiboot2 information structure
#[no_mangle]
pub extern "C" fn kmain(_magic: u64, _multiboot_info: u64) -> ! {
    // Clear screen
    let vga_buffer = 0xB8000 as *mut u8;
    unsafe {
        for i in 0..80 * 25 * 2 {
            *vga_buffer.offset(i) = 0;
        }
    }

    // Print "Mach_R" in green
    let message = b"Mach_R v0.1.0 - x86_64 Boot Successful";
    let color = 0x0A; // Green on black

    unsafe {
        for (i, &byte) in message.iter().enumerate() {
            *vga_buffer.offset((i * 2) as isize) = byte;
            *vga_buffer.offset((i * 2 + 1) as isize) = color;
        }
    }

    // TODO: Parse Multiboot2 information
    // TODO: Call into Rust kernel initialization

    // For now, just halt
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
