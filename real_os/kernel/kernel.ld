OUTPUT_FORMAT("elf64-littleaarch64")
OUTPUT_ARCH(aarch64)
ENTRY(kernel_main)

PHDRS
{
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
}

SECTIONS
{
    . = 0x40080000;
    
    .text :
    {
        KEEP(*(.text.boot))
        *(.text .text.*)
    } :text
    
    .rodata :
    {
        *(.rodata .rodata.*)
    } :rodata
    
    .data :
    {
        *(.data .data.*)
    } :data
    
    .bss :
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } :data
    
    __kernel_end = .;
    
    /DISCARD/ :
    {
        *(.comment)
        *(.comment.*)
        *(.note*)
        *(.eh_frame*)
    }
}