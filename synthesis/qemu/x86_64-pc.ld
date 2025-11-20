/* Linker script for QEMU x86_64 PC */
ENTRY(_start)

SECTIONS
{
    . = 0x100000; /* Load at 1MB (above BIOS/legacy area) */
    
    .boot : {
        KEEP(*(.multiboot_header))
        KEEP(*(.multiboot))
        KEEP(*(.text.boot))
    }
    
    .text : ALIGN(4K) {
        *(.text .text.*)
    }
    
    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    }
    
    .data : ALIGN(4K) {
        *(.data .data.*)
    }
    
    .bss : ALIGN(4K) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }
    
    . = ALIGN(4K);
    __kernel_end = .;
    
    /* Stack grows down from here */
    . = . + 0x100000; /* 1MB stack */
    __stack_top = .;
    
    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
    }
}