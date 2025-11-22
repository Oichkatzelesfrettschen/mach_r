# Mach_R Build Container
# Comprehensive cross-compilation environment for ARM Mac → x86_64 kernel development
#
# This container provides all tools needed to build a bootable x86_64 kernel on ARM Mac:
# - Rust with bare-metal targets
# - NASM assembler
# - GCC cross-compilers
# - GRUB bootloader tools
# - QEMU for testing
# - Image creation tools

# Use multi-stage build for smaller final image
FROM --platform=linux/amd64 ubuntu:22.04 AS base

# Prevent interactive prompts during build
ENV DEBIAN_FRONTEND=noninteractive
ENV TZ=UTC

# Install system dependencies in layers for better caching
RUN apt-get update && apt-get install -y \
    # Build essentials
    build-essential \
    curl \
    wget \
    git \
    # Cross-compilation toolchain
    gcc-x86-64-linux-gnu \
    gcc-aarch64-linux-gnu \
    binutils-x86-64-linux-gnu \
    binutils-aarch64-linux-gnu \
    && rm -rf /var/lib/apt/lists/*

# Install assemblers and bootloader tools (separate layer for caching)
RUN apt-get update && apt-get install -y \
    # Assemblers
    nasm \
    yasm \
    # Bootloader tools
    grub-pc-bin \
    grub-common \
    xorriso \
    mtools \
    dosfstools \
    && rm -rf /var/lib/apt/lists/*

# Install QEMU and utilities (separate layer)
RUN apt-get update && apt-get install -y \
    # QEMU for testing
    qemu-system-x86 \
    qemu-system-arm \
    qemu-system-aarch64 \
    qemu-utils \
    # Disk image tools
    parted \
    kpartx \
    # Debugging tools
    gdb \
    gdb-multiarch \
    && rm -rf /var/lib/apt/lists/*

# Install development utilities (separate layer)
RUN apt-get update && apt-get install -y \
    # Development tools
    vim \
    nano \
    less \
    htop \
    tree \
    # Version control
    git \
    # Archive tools
    zip \
    unzip \
    tar \
    && rm -rf /var/lib/apt/lists/*

# Install Rust (rustup) - separate layer as it changes with Rust versions
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    --default-toolchain stable \
    --profile minimal \
    --no-modify-path

ENV PATH="/root/.cargo/bin:${PATH}"

# Add Rust bare-metal targets
RUN /root/.cargo/bin/rustup target add \
    x86_64-unknown-none \
    aarch64-unknown-none

# Install Rust components (separate layer)
RUN /root/.cargo/bin/rustup component add \
    rust-src \
    rustfmt \
    clippy \
    llvm-tools-preview

# Install cargo tools (separate layer - these change less frequently)
RUN /root/.cargo/bin/cargo install \
    cargo-binutils \
    cargo-make \
    && rm -rf /root/.cargo/registry

# Set up working directory
WORKDIR /workspace

# Create mount points for volumes
RUN mkdir -p /workspace/target \
    && mkdir -p /root/.cargo/registry \
    && mkdir -p /workspace/build/dist

# Copy build scripts and configuration (do this last as it changes most often)
# COPY build-scripts/ /usr/local/bin/
# RUN chmod +x /usr/local/bin/*.sh 2>/dev/null || true

# Environment variables for cross-compilation
ENV CARGO_TARGET_X86_64_UNKNOWN_NONE_LINKER=x86_64-linux-gnu-gcc
ENV CARGO_TARGET_AARCH64_UNKNOWN_NONE_LINKER=aarch64-linux-gnu-gcc
ENV CC_x86_64_unknown_none=x86_64-linux-gnu-gcc
ENV AR_x86_64_unknown_none=x86_64-linux-gnu-ar
ENV CC_aarch64_unknown_none=aarch64-linux-gnu-gcc
ENV AR_aarch64_unknown_none=aarch64-linux-gnu-ar

# Default build command
CMD ["cargo", "build", "--lib", "--target", "x86_64-unknown-none"]

# Healthcheck to ensure container is working
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD cargo --version && rustc --version && nasm --version || exit 1

# Labels for metadata
LABEL maintainer="Mach_R Project"
LABEL description="Complete build environment for Mach_R microkernel"
LABEL version="0.1.0"
LABEL architecture="multi-arch (supports ARM64 host → x86_64 target)"
