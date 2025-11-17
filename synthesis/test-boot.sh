#!/bin/bash
# Test if our kernel actually boots

echo "Testing MACH_R kernel boot..."

# Create a simple boot test for ARM64
cat > test_boot.S << 'EOF'
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
EOF

echo "Building minimal test kernel..."
aarch64-linux-gnu-as test_boot.S -o test_boot.o 2>/dev/null || as -arch arm64 test_boot.S -o test_boot.o
aarch64-linux-gnu-ld -Ttext=0x40000000 test_boot.o -o test_kernel 2>/dev/null || ld -o test_kernel test_boot.o

echo "Testing minimal kernel..."
timeout 2 qemu-system-aarch64 -M virt -cpu cortex-a57 -kernel test_kernel -nographic -serial mon:stdio

echo ""
echo "Now testing MACH_R kernel..."
timeout 2 qemu-system-aarch64 -M virt -cpu cortex-a57 -kernel target/aarch64-unknown-none/release/mach_r -nographic -serial mon:stdio

echo ""
echo "Test complete."