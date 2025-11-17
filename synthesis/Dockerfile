FROM rust:1.89

# Install cross-compilation tools
RUN apt-get update && apt-get install -y \
    gcc-aarch64-linux-gnu \
    gcc-x86-64-linux-gnu \
    qemu-user-static \
    && rm -rf /var/lib/apt/lists/*

# Add bare-metal targets
RUN rustup target add aarch64-unknown-none x86_64-unknown-none

# Set working directory
WORKDIR /workspace

# Copy source code
COPY . .

# Default command
CMD ["cargo", "build", "--lib"]
