#!/bin/bash
# Mach_R Build Environment Setup
# Prepares build environment on ARM Mac

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

cat << "EOF"
╔═══════════════════════════════════════════╗
║   Mach_R Build Environment Setup          ║
║   ARM Mac → x86_64 Cross-Compilation      ║
╚═══════════════════════════════════════════╝
EOF

echo ""

# Check system
echo -e "${BLUE}Checking system...${NC}"
ARCH=$(uname -m)
OS=$(uname -s)

if [ "$OS" != "Darwin" ]; then
    echo -e "${YELLOW}⚠️  This script is optimized for macOS${NC}"
fi

echo -e "Architecture: ${YELLOW}${ARCH}${NC}"
echo -e "OS: ${YELLOW}${OS}${NC}"
echo ""

# Create directory structure
echo -e "${BLUE}Creating directory structure...${NC}"
mkdir -p build/dist
mkdir -p build/iso
mkdir -p build/logs
mkdir -p ~/.docker/mach_r/cargo/registry
mkdir -p ~/.docker/mach_r/cargo/git
mkdir -p ~/.docker/mach_r/target

echo -e "${GREEN}✅ Directories created${NC}"

# Check Docker
echo ""
echo -e "${BLUE}Checking Docker...${NC}"

if ! command -v docker &> /dev/null; then
    echo -e "${RED}❌ Docker not installed${NC}"
    echo ""
    echo -e "${YELLOW}Install Docker Desktop for Mac:${NC}"
    echo "  1. Download from: https://www.docker.com/products/docker-desktop"
    echo "  2. Install and start Docker Desktop"
    echo "  3. Run this script again"
    exit 1
fi

if ! docker info &> /dev/null 2>&1; then
    echo -e "${YELLOW}⚠️  Docker is installed but not running${NC}"
    echo "  Please start Docker Desktop and run this script again"
    exit 1
fi

echo -e "${GREEN}✅ Docker is running${NC}"

# Check Docker Compose
if ! docker-compose version &> /dev/null 2>&1; then
    echo -e "${RED}❌ Docker Compose not found${NC}"
    echo "  Install: brew install docker-compose"
    exit 1
fi

echo -e "${GREEN}✅ Docker Compose available${NC}"

# Enable Rosetta for x86_64 emulation
echo ""
echo -e "${BLUE}Checking Rosetta 2 (for x86_64 emulation)...${NC}"

if [ "$ARCH" = "arm64" ]; then
    if ! pgrep -q oahd; then
        echo -e "${YELLOW}Installing Rosetta 2...${NC}"
        softwareupdate --install-rosetta --agree-to-license
    fi
    echo -e "${GREEN}✅ Rosetta 2 enabled${NC}"
else
    echo -e "${BLUE}ℹ️  Running on x86_64, Rosetta not needed${NC}"
fi

# Build Docker image
echo ""
echo -e "${BLUE}Building Docker image...${NC}"
echo -e "${YELLOW}This may take 5-10 minutes the first time...${NC}"

if docker-compose -f docker-compose.build.yml build builder; then
    echo -e "${GREEN}✅ Docker image built successfully${NC}"
else
    echo -e "${RED}❌ Docker image build failed${NC}"
    exit 1
fi

# Test build environment
echo ""
echo -e "${BLUE}Testing build environment...${NC}"

if docker-compose -f docker-compose.build.yml run --rm builder rustc --version; then
    echo -e "${GREEN}✅ Build environment is working${NC}"
else
    echo -e "${RED}❌ Build environment test failed${NC}"
    exit 1
fi

# Summary
echo ""
echo -e "${GREEN}╔═══════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║   Setup Complete!                         ║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════╝${NC}"
echo ""
echo -e "${BLUE}Build Environment:${NC}"
echo "  • Docker image: mach_r_builder:latest"
echo "  • Platform: linux/amd64 (x86_64)"
echo "  • Rust targets: x86_64-unknown-none, aarch64-unknown-none"
echo "  • Tools: NASM, GRUB, QEMU, GDB"
echo ""
echo -e "${BLUE}Quick Start:${NC}"
echo "  1. Build kernel:  ./scripts/build-in-container.sh build"
echo "  2. Run tests:     ./scripts/build-in-container.sh test"
echo "  3. Dev shell:     ./scripts/build-in-container.sh shell"
echo "  4. QEMU test:     ./scripts/build-in-container.sh qemu"
echo ""
echo -e "${BLUE}Next Steps:${NC}"
echo "  • Start implementing Phase 1: Boot Infrastructure"
echo "  • See BOOTABLE_ROADMAP.md for detailed plan"
echo ""
echo -e "${GREEN}Happy Hacking! 🦀${NC}"
