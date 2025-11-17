#!/bin/bash
echo "🐳 Building Mach_R in container..."

# Build ARM64 target
echo "Building for ARM64..."
docker-compose run --rm mach_r_builder cargo build --lib --target aarch64-unknown-none

# Build x86_64 target  
echo "Building for x86_64..."
docker-compose run --rm mach_r_builder cargo build --lib --target x86_64-unknown-none

echo "🎉 Container builds complete!"
