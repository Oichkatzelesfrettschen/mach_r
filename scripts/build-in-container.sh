#!/bin/bash
# Mach_R Container Build Wrapper
# Easy-to-use wrapper for building in Docker container on ARM Mac

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
COMPOSE_FILE="docker-compose.build.yml"
SERVICE="builder"

# Print banner
cat << "EOF"
╔═══════════════════════════════════════════╗
║   Mach_R Build Container (ARM Mac)        ║
║   x86_64 Cross-Compilation Environment    ║
╚═══════════════════════════════════════════╝
EOF

echo ""

# Check Docker
if ! command -v docker &> /dev/null; then
    echo -e "${RED}❌ Docker not found!${NC}"
    echo -e "${YELLOW}Install Docker Desktop for Mac:${NC}"
    echo "  https://www.docker.com/products/docker-desktop"
    exit 1
fi

if ! docker info &> /dev/null 2>&1; then
    echo -e "${RED}❌ Docker is not running!${NC}"
    echo -e "${YELLOW}Start Docker Desktop and try again.${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Docker is running${NC}"

# Check if image exists, build if not
if ! docker images | grep -q "mach_r_builder"; then
    echo -e "${YELLOW}Building container image (this may take a few minutes)...${NC}"
    docker-compose -f "${COMPOSE_FILE}" build builder
fi

# Parse command
COMMAND="${1:-build}"

case "$COMMAND" in
    build)
        TARGET="${2:-x86_64-unknown-none}"
        BUILD_TYPE="${3:-debug}"

        echo -e "${BLUE}Building for ${TARGET} (${BUILD_TYPE})...${NC}"

        if [ "$BUILD_TYPE" = "release" ]; then
            docker-compose -f "${COMPOSE_FILE}" run --rm ${SERVICE} \
                cargo build --release --lib --target "${TARGET}"
        else
            docker-compose -f "${COMPOSE_FILE}" run --rm ${SERVICE} \
                cargo build --lib --target "${TARGET}"
        fi
        ;;

    test)
        echo -e "${BLUE}Running tests...${NC}"
        docker-compose -f "${COMPOSE_FILE}" run --rm ${SERVICE} \
            cargo test --lib
        ;;

    shell)
        echo -e "${BLUE}Starting interactive shell...${NC}"
        docker-compose -f "${COMPOSE_FILE}" run --rm ${SERVICE} /bin/bash
        ;;

    clean)
        echo -e "${YELLOW}Cleaning build artifacts...${NC}"
        docker-compose -f "${COMPOSE_FILE}" run --rm ${SERVICE} \
            cargo clean
        ;;

    qemu)
        echo -e "${BLUE}Building and launching QEMU...${NC}"
        docker-compose -f "${COMPOSE_FILE}" run --rm qemu
        ;;

    image)
        echo -e "${BLUE}Creating bootable disk image...${NC}"
        docker-compose -f "${COMPOSE_FILE}" run --rm ${SERVICE} \
            bash /usr/local/bin/create-bootable.sh
        ;;

    rebuild)
        echo -e "${YELLOW}Rebuilding container image...${NC}"
        docker-compose -f "${COMPOSE_FILE}" build --no-cache builder
        ;;

    help|--help|-h)
        cat <<HELP

Usage: $0 [COMMAND] [OPTIONS]

Commands:
  build [target] [type]   Build the kernel
                          target: x86_64-unknown-none (default), aarch64-unknown-none
                          type: debug (default), release

  test                    Run tests

  shell                   Start interactive shell in container

  clean                   Clean build artifacts

  qemu                    Build and launch in QEMU

  image                   Create bootable disk image (.img and .qcow2)

  rebuild                 Rebuild container image from scratch

  help                    Show this help

Examples:
  $0 build                       # Build for x86_64 (debug)
  $0 build x86_64-unknown-none release
  $0 test                        # Run tests
  $0 shell                       # Interactive development
  $0 qemu                        # Boot in QEMU

HELP
        ;;

    *)
        echo -e "${RED}Unknown command: $COMMAND${NC}"
        echo "Run '$0 help' for usage information"
        exit 1
        ;;
esac

echo ""
echo -e "${GREEN}✅ Done!${NC}"
