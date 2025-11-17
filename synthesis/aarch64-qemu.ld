/* Linker script for ARM64 QEMU virt machine */
ENTRY(_boot)

SECTIONS
{
    . = 0x40000000;  /* QEMU loads kernel here */
    
    .text.boot : {
        *(.text.boot)
    }
    
    .text : {
        *(.text .text.*)
    }
    
    .rodata : {
        *(.rodata .rodata.*)
    }
    
    .data : {
        *(.data .data.*)
    }
    
    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        __bss_end = .;
    }
    
    /DISCARD/ : {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
    }
}