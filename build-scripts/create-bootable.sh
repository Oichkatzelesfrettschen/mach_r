#!/bin/bash
# Create bootable disk image for QEMU
# Combines kernel, bootloader (GRUB), and creates .img and .qcow2

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}💿 Creating Bootable Disk Image${NC}"
echo "=================================="

# Configuration
KERNEL_BIN="${1:-target/x86_64-unknown-none/release/mach_r}"
OUTPUT_IMG="${2:-mach_r.img}"
OUTPUT_QCOW2="${3:-mach_r.qcow2}"
IMG_SIZE_MB="${4:-128}"

# Check if kernel exists
if [ ! -f "$KERNEL_BIN" ]; then
    echo -e "${RED}❌ Kernel binary not found: ${KERNEL_BIN}${NC}"
    echo -e "${YELLOW}Build the kernel first:${NC}"
    echo "  cargo build --release --target x86_64-unknown-none"
    exit 1
fi

echo -e "${BLUE}Kernel: ${KERNEL_BIN}${NC}"
echo -e "${BLUE}Output: ${OUTPUT_IMG} → ${OUTPUT_QCOW2}${NC}"
echo ""

# Create build directory
BUILD_DIR="build/dist"
mkdir -p "${BUILD_DIR}/boot/grub"
mkdir -p "${BUILD_DIR}/iso"

# Step 1: Create raw disk image
echo -e "${YELLOW}Step 1: Creating raw disk image (${IMG_SIZE_MB}MB)...${NC}"
dd if=/dev/zero of="${OUTPUT_IMG}" bs=1M count="${IMG_SIZE_MB}" status=progress

# Step 2: Create partition table
echo -e "${YELLOW}Step 2: Creating partition table...${NC}"
parted "${OUTPUT_IMG}" -s mklabel msdos
parted "${OUTPUT_IMG}" -s mkpart primary ext2 1MiB 100%
parted "${OUTPUT_IMG}" -s set 1 boot on

# Step 3: Format partition
echo -e "${YELLOW}Step 3: Formatting partition...${NC}"
# Create loop device
LOOP_DEVICE=$(losetup -f)
if [ -z "$LOOP_DEVICE" ]; then
    echo -e "${RED}❌ No free loop device available${NC}"
    exit 1
fi

sudo losetup -P "${LOOP_DEVICE}" "${OUTPUT_IMG}"
sudo mkfs.ext2 "${LOOP_DEVICE}p1"

# Step 4: Mount and install GRUB
echo -e "${YELLOW}Step 4: Installing GRUB...${NC}"
MOUNT_POINT="/tmp/mach_r_mount_$$"
mkdir -p "${MOUNT_POINT}"
sudo mount "${LOOP_DEVICE}p1" "${MOUNT_POINT}"

# Create directory structure
sudo mkdir -p "${MOUNT_POINT}/boot/grub"

# Copy kernel
echo -e "${BLUE}Copying kernel...${NC}"
sudo cp "${KERNEL_BIN}" "${MOUNT_POINT}/boot/kernel.bin"

# Create GRUB configuration
echo -e "${BLUE}Creating GRUB config...${NC}"
sudo tee "${MOUNT_POINT}/boot/grub/grub.cfg" > /dev/null <<EOF
set timeout=0
set default=0

menuentry "Mach_R Microkernel" {
    multiboot2 /boot/kernel.bin
    boot
}
EOF

# Install GRUB bootloader
echo -e "${BLUE}Installing GRUB to MBR...${NC}"
sudo grub-install \
    --target=i386-pc \
    --boot-directory="${MOUNT_POINT}/boot" \
    --modules="part_msdos ext2 multiboot2" \
    "${LOOP_DEVICE}"

# Cleanup
echo -e "${YELLOW}Step 5: Cleaning up...${NC}"
sudo umount "${MOUNT_POINT}"
sudo losetup -d "${LOOP_DEVICE}"
rmdir "${MOUNT_POINT}"

# Step 6: Convert to QCOW2
echo -e "${YELLOW}Step 6: Converting to QCOW2...${NC}"
qemu-img convert -f raw -O qcow2 "${OUTPUT_IMG}" "${OUTPUT_QCOW2}"

# Show results
echo ""
echo -e "${GREEN}✅ Bootable disk images created!${NC}"
echo ""
echo -e "${BLUE}Files created:${NC}"
ls -lh "${OUTPUT_IMG}" "${OUTPUT_QCOW2}"

echo ""
echo -e "${GREEN}To test in QEMU:${NC}"
echo -e "  qemu-system-x86_64 -drive file=${OUTPUT_QCOW2},format=qcow2 -m 256M -serial stdio"

echo ""
echo -e "${YELLOW}Note: This script requires sudo for loop device and mounting${NC}"
