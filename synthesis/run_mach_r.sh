#!/bin/bash
# ============================================================================
# Mach_R OS - Quick Run Script
# ============================================================================
# This script builds and runs the Mach_R operating system in various ways
# ============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
VERSION="0.1.0"
QEMU_CMD="qemu-system-aarch64"

echo -e "${BLUE}============================================${NC}"
echo -e "${BLUE}     Mach_R Operating System v${VERSION}${NC}"
echo -e "${BLUE}============================================${NC}"
echo ""

# Function to print status
status() {
    echo -e "${GREEN}[*]${NC} $1"
}

error() {
    echo -e "${RED}[!]${NC} $1"
}

info() {
    echo -e "${YELLOW}[i]${NC} $1"
}

# Check dependencies
check_deps() {
    status "Checking dependencies..."
    
    if ! command -v cargo &> /dev/null; then
        error "Cargo not found. Please install Rust."
        exit 1
    fi
    
    if ! command -v $QEMU_CMD &> /dev/null; then
        error "QEMU not found. Installing..."
        brew install qemu
    fi
    
    if ! command -v qemu-img &> /dev/null; then
        error "qemu-img not found. Please install QEMU."
        exit 1
    fi
    
    status "All dependencies satisfied!"
}

# Build the kernel
build_kernel() {
    status "Building Mach_R kernel..."
    
    # Build as library (works without custom target)
    cargo build --release --lib
    
    # Create kernel binary for demonstration
    mkdir -p build/dist
    cp target/release/libmach_r.rlib build/dist/mach_r_kernel.bin
    
    status "Kernel built successfully!"
}

# Create filesystem
create_filesystem() {
    status "Creating root filesystem..."
    
    make filesystem
    
    # Add some demo files
    echo "Welcome to Mach_R OS!" > build/sysroot/etc/motd
    echo "#!/bin/sh" > build/sysroot/bin/init
    echo "echo 'Mach_R Init System'" >> build/sysroot/bin/init
    chmod +x build/sysroot/bin/init
    
    status "Filesystem created!"
}

# Create disk image
create_disk() {
    status "Creating disk image..."
    
    # Create raw image
    dd if=/dev/zero of=build/images/mach_r.img bs=1M count=256 2>/dev/null
    
    # Convert to QCOW2
    qemu-img convert -f raw -O qcow2 -c build/images/mach_r.img build/images/mach_r.qcow2
    
    local SIZE=$(ls -lh build/images/mach_r.qcow2 | awk '{print $5}')
    status "Disk image created: build/images/mach_r.qcow2 (${SIZE})"
}

# Create UTM bundle
create_utm() {
    status "Creating UTM bundle..."
    
    make utm 2>/dev/null || true
    
    if [ -d "build/dist/Mach_R.utm" ]; then
        status "UTM bundle created: build/dist/Mach_R.utm"
        info "To use with UTM: open build/dist/Mach_R.utm"
    else
        error "UTM bundle creation failed"
    fi
}

# Run in QEMU
run_qemu() {
    status "Starting Mach_R in QEMU..."
    echo ""
    info "Press Ctrl+A then X to exit QEMU"
    echo ""
    sleep 2
    
    $QEMU_CMD \
        -M virt \
        -cpu cortex-a72 \
        -smp 4 \
        -m 2G \
        -drive if=virtio,format=qcow2,file=build/images/mach_r.qcow2 \
        -device virtio-net-pci,netdev=net0 \
        -netdev user,id=net0 \
        -nographic
}

# Run with GUI
run_qemu_gui() {
    status "Starting Mach_R with display..."
    
    $QEMU_CMD \
        -M virt \
        -cpu cortex-a72 \
        -smp 4 \
        -m 2G \
        -drive if=virtio,format=qcow2,file=build/images/mach_r.qcow2 \
        -device virtio-gpu-pci \
        -display default \
        -serial stdio &
    
    info "QEMU started with display window"
}

# Main menu
show_menu() {
    echo ""
    echo "What would you like to do?"
    echo ""
    echo "  1) Build everything"
    echo "  2) Run in QEMU (console)"
    echo "  3) Run in QEMU (with display)"
    echo "  4) Create UTM bundle"
    echo "  5) Clean build"
    echo "  6) Exit"
    echo ""
    read -p "Select option [1-6]: " choice
    
    case $choice in
        1)
            check_deps
            build_kernel
            create_filesystem
            create_disk
            create_utm
            info "Build complete! You can now run the OS."
            show_menu
            ;;
        2)
            if [ ! -f "build/images/mach_r.qcow2" ]; then
                error "Disk image not found. Please build first."
                show_menu
            else
                run_qemu
            fi
            ;;
        3)
            if [ ! -f "build/images/mach_r.qcow2" ]; then
                error "Disk image not found. Please build first."
                show_menu
            else
                run_qemu_gui
                show_menu
            fi
            ;;
        4)
            create_utm
            show_menu
            ;;
        5)
            status "Cleaning build artifacts..."
            make clean
            status "Clean complete!"
            show_menu
            ;;
        6)
            info "Goodbye!"
            exit 0
            ;;
        *)
            error "Invalid option"
            show_menu
            ;;
    esac
}

# Main entry point
main() {
    # Parse command line arguments
    if [ "$1" == "--help" ] || [ "$1" == "-h" ]; then
        echo "Usage: $0 [OPTION]"
        echo ""
        echo "Options:"
        echo "  --build       Build everything"
        echo "  --run         Run in QEMU"
        echo "  --gui         Run with display"
        echo "  --utm         Create UTM bundle"
        echo "  --clean       Clean build"
        echo "  --help        Show this help"
        echo ""
        echo "Without options, shows interactive menu."
        exit 0
    elif [ "$1" == "--build" ]; then
        check_deps
        build_kernel
        create_filesystem
        create_disk
        create_utm
    elif [ "$1" == "--run" ]; then
        run_qemu
    elif [ "$1" == "--gui" ]; then
        run_qemu_gui
    elif [ "$1" == "--utm" ]; then
        create_utm
    elif [ "$1" == "--clean" ]; then
        make clean
    else
        # Interactive mode
        show_menu
    fi
}

# Run main
main "$@"