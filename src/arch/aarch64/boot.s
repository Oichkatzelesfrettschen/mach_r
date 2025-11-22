.section .text.boot
.global _start_s

_start_s:
    // Set stack pointer
    ldr x0, =0x40100000
    mov sp, x0
    
    // Clear BSS
    ldr x0, =__bss_start
    ldr x1, =__bss_end
1:  
    cmp x0, x1
    b.ge 2f
    str xzr, [x0], #8
    b 1b
2:
    
    // Jump to kernel main
    bl kernel_main
    
    // Hang if we return
3:  wfe
    b 3b