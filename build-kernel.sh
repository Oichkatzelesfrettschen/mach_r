#!/bin/bash
# Build minimal kernel binaries for MACH_R
set -e

export PATH="/opt/homebrew/opt/rustup/bin:$PATH"

echo "🦀 Building MACH_R Kernel Binaries"
echo "=================================="

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Build for ARM64
echo -e "${YELLOW}Building ARM64 kernel...${NC}"
if RUSTFLAGS="-A warnings" cargo build --bin mach_r --target aarch64-unknown-none --release; then
    if [[ -f "target/aarch64-unknown-none/release/mach_r" ]]; then
        size=$(du -h "target/aarch64-unknown-none/release/mach_r" | cut -f1)
        echo -e "${GREEN}✅ ARM64 kernel: $size${NC}"
    fi
else
    echo "❌ ARM64 build failed"
fi

# Build for x86_64
echo ""
echo -e "${YELLOW}Building x86_64 kernel...${NC}"
if RUSTFLAGS="-A warnings" cargo build --bin mach_r --target x86_64-unknown-none --release; then
    if [[ -f "target/x86_64-unknown-none/release/mach_r" ]]; then
        size=$(du -h "target/x86_64-unknown-none/release/mach_r" | cut -f1)
        echo -e "${GREEN}✅ x86_64 kernel: $size${NC}"
    fi
else
    echo "❌ x86_64 build failed"
fi

echo ""
echo -e "${BLUE}Kernel build artifacts:${NC}"
find target -name "mach_r" -type f -exec ls -lh {} \;