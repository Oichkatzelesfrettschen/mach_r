/*
 * Linker script for Mach_R x86_64 kernel
 * Multiboot2-compliant bootable kernel
 */

ENTRY(_start)

SECTIONS
{
    /* Kernel is loaded at 1MB physical address */
    . = 1M;

    /* Multiboot2 header MUST be in first 32KB */
    .multiboot ALIGN(8) : {
        KEEP(*(.multiboot))
    }

    /* Code section */
    .text ALIGN(4K) : {
        *(.text._start)
        *(.text)
        *(.text.*)
    }

    /* Read-only data */
    .rodata ALIGN(4K) : {
        *(.rodata)
        *(.rodata.*)
    }

    /* Initialized data */
    .data ALIGN(4K) : {
        *(.data)
        *(.data.*)
    }

    /* BSS (zero-initialized data) */
    .bss ALIGN(4K) : {
        *(COMMON)
        *(.bss)
        *(.bss.*)
    }

    /* Kernel end marker */
    _kernel_end = .;

    /* Discard sections we don't need */
    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
        *(.note*)
    }
}
