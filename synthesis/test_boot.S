.section .text
.global _start
_start:
    // Set up UART for output
    mov x0, #0x09000000     // UART0 base for QEMU virt machine
    mov w1, #0x48           // 'H'
    str w1, [x0]
    mov w1, #0x69           // 'i'
    str w1, [x0]
    mov w1, #0x21           // '!'
    str w1, [x0]
    mov w1, #0x0A           // '\n'
    str w1, [x0]
    
    // Infinite loop
1:  wfe
    b 1b
