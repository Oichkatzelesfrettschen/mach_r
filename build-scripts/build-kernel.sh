#!/bin/bash
# Mach_R Kernel Build Script (Container)
# Builds the kernel for specified target architecture

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

TARGET="${1:-x86_64-unknown-none}"
BUILD_TYPE="${2:-debug}"

echo -e "${BLUE}🔨 Building Mach_R Kernel${NC}"
echo "================================"
echo -e "Target: ${YELLOW}${TARGET}${NC}"
echo -e "Type: ${YELLOW}${BUILD_TYPE}${NC}"
echo ""

# Build command
if [ "$BUILD_TYPE" = "release" ]; then
    BUILD_CMD="cargo build --release --lib --target ${TARGET}"
else
    BUILD_CMD="cargo build --lib --target ${TARGET}"
fi

echo -e "${BLUE}Running: ${BUILD_CMD}${NC}"
echo ""

if eval "$BUILD_CMD"; then
    echo ""
    echo -e "${GREEN}✅ Build successful!${NC}"

    # Show output file info
    if [ "$BUILD_TYPE" = "release" ]; then
        OUTPUT_DIR="target/${TARGET}/release"
    else
        OUTPUT_DIR="target/${TARGET}/debug"
    fi

    if [ -f "${OUTPUT_DIR}/libmach_r.rlib" ]; then
        echo ""
        echo -e "${BLUE}Build Artifacts:${NC}"
        ls -lh "${OUTPUT_DIR}/libmach_r.rlib"
        echo ""
        echo -e "${GREEN}Output: ${OUTPUT_DIR}/libmach_r.rlib${NC}"
    fi
else
    echo ""
    echo -e "${RED}❌ Build failed!${NC}"
    exit 1
fi
