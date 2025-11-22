.section .text.boot
.code32
.global _start_s

_start_s:
    // Disable interrupts
    cli

    // Setup a temporary stack (e.g., at 0x80000 - 4KB for initial stack)
    // The bootloader should have already set up a basic environment,
    // but we'll ensure a stack is available for initial Rust code.
    mov $0x80000, %esp

    // Clear BSS (optional for early boot, but good practice if Rust uses it)
    // The Rust entry point usually handles this if needed, but in assembly, it's safer.
    // extern __bss_start, __bss_end
    // mov $__bss_start, %edi
    // mov $__bss_end, %ecx
    // sub %edi, %ecx
    // shr $2, %ecx // Divide by 4 for dword count
    // xor %eax, %eax
    // rep stosl // Fill with zeros

    // Jump to the 64-bit long mode Rust kernel entry point.
    // The linker script will define kernel_main as the 64-bit entry.
    call kernel_main

    // If kernel_main returns (it shouldn't), halt the system.
    cli
    hlt

.size _start, . - _start