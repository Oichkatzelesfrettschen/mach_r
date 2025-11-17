.global _start
_start:
    ldr x0, =0x09000000     // UART base
    mov w1, #72             // 'H'
    str w1, [x0]
    mov w1, #105            // 'i'
    str w1, [x0]
    mov w1, #33             // '!'
    str w1, [x0]
    mov w1, #10             // '\n'
    str w1, [x0]
1:  wfe
    b 1b