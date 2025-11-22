# Use a base image with common development tools
FROM mcr.microsoft.com/devcontainers/base:ubuntu

# Install necessary packages for Rust development and cross-compilation
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends build-essential curl git \
    qemu-system-x86_64 qemu-system-aarch64 \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Install Rust using rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && rustup component add rust-src

# Add Cargo's bin directory to PATH for all users
ENV PATH="/root/.cargo/bin:${PATH}"

# Set the working directory
WORKDIR /workspaces/mach_r
