#!/bin/bash
# Create bootable ISO with GRUB2 for Mach_R kernel

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

KERNEL="${1:-synthesis/target/x86_64-unknown-none/debug/mach_r}"
ISO_DIR="build/iso"
ISO_FILE="mach_r.iso"

echo -e "${BLUE}Creating bootable ISO...${NC}"

# Create ISO directory structure
mkdir -p "${ISO_DIR}/boot/grub"

# Copy kernel
echo -e "${YELLOW}Copying kernel...${NC}"
cp "$KERNEL" "${ISO_DIR}/boot/mach_r.bin"

# Create GRUB configuration
echo -e "${YELLOW}Creating GRUB config...${NC}"
cat > "${ISO_DIR}/boot/grub/grub.cfg" << 'EOF'
set timeout=0
set default=0

menuentry "Mach_R x86_64 Kernel" {
    multiboot2 /boot/mach_r.bin
    boot
}
EOF

# Create ISO with GRUB
echo -e "${YELLOW}Building ISO with grub-mkrescue...${NC}"
if command -v grub-mkrescue &> /dev/null; then
    grub-mkrescue -o "$ISO_FILE" "$ISO_DIR"
elif command -v grub2-mkrescue &> /dev/null; then
    grub2-mkrescue -o "$ISO_FILE" "$ISO_DIR"
else
    echo -e "${YELLOW}grub-mkrescue not found, trying xorriso...${NC}"
    # Try using xorriso directly
    xorriso -as mkisofs \
        -o "$ISO_FILE" \
        -b boot/grub/grub.cfg \
        -no-emul-boot \
        -boot-load-size 4 \
        -boot-info-table \
        "$ISO_DIR"
fi

echo -e "${GREEN}ISO created: $ISO_FILE${NC}"
echo -e "${GREEN}Run with: qemu-system-x86_64 -cdrom $ISO_FILE${NC}"
