#!/bin/bash
# Run Mach_R x86_64 kernel in QEMU with Multiboot2 support

set -e

# Colors
BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Configuration
KERNEL="${1:-synthesis/target/x86_64-unknown-none/debug/mach_r}"

# Check if kernel exists
if [ ! -f "$KERNEL" ]; then
    echo -e "${YELLOW}Kernel not found at: ${KERNEL}${NC}"
    echo -e "${YELLOW}Building kernel...${NC}"
    cd synthesis
    ~/.cargo/bin/cargo build --bin mach_r --target x86_64-unknown-none
    cd ..
fi

echo -e "${BLUE}==================================${NC}"
echo -e "${BLUE}  Mach_R x86_64 Kernel - QEMU${NC}"
echo -e "${BLUE}==================================${NC}"
echo -e "${GREEN}Kernel: ${KERNEL}${NC}"
echo ""
echo -e "${YELLOW}Press Ctrl+A then X to quit QEMU${NC}"
echo ""

# Run QEMU with Multiboot2 kernel
# -kernel: Boot the kernel directly (QEMU has Multiboot2 support)
# -serial stdio: Redirect serial output to terminal
# -vga std: Use standard VGA for text mode
# -no-reboot: Exit on reboot instead of rebooting
# -no-shutdown: Keep QEMU running after kernel halt
qemu-system-x86_64 \
    -kernel "$KERNEL" \
    -serial stdio \
    -vga std \
    -m 512M \
    -no-reboot \
    -d cpu_reset,guest_errors \
    -D qemu.log
