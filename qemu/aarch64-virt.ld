/* Linker script for QEMU virt machine (ARM64) */
ENTRY(_start)

SECTIONS
{
    . = 0x40080000; /* QEMU virt machine RAM starts at 0x40000000, kernel at +512K */
    
    .text : ALIGN(4K) {
        KEEP(*(.text.boot))
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