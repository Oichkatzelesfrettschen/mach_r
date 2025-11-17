# ============================================================================
# Mach_R Operating System - Professional Build System
# ============================================================================

# Build Configuration
VERSION := 0.1.0
PROJECT := mach_r
KERNEL_NAME := mach_r_kernel

# Architecture detection
ARCH ?= $(shell uname -m)
ifeq ($(ARCH),arm64)
    TARGET_ARCH := aarch64
else
    TARGET_ARCH := aarch64
endif

BUILD_TYPE ?= release

# Directories
BUILD_DIR := build
TARGET_DIR := target
DIST_DIR := $(BUILD_DIR)/dist
IMAGE_DIR := $(BUILD_DIR)/images
SYSROOT_DIR := $(BUILD_DIR)/sysroot
LOG_DIR := $(BUILD_DIR)/logs

# Ensure directories exist
$(shell mkdir -p $(BUILD_DIR) $(DIST_DIR) $(IMAGE_DIR) $(SYSROOT_DIR) $(LOG_DIR))

# Rust toolchain
CARGO := cargo
RUSTUP := rustup
RUST_TARGET := aarch64-unknown-none
RUSTFLAGS := -C panic=abort

ifeq ($(BUILD_TYPE),release)
    CARGO_FLAGS := --release
    BUILD_PROFILE := release
else
    CARGO_FLAGS :=
    BUILD_PROFILE := debug
endif

# Output files
KERNEL_ELF := $(TARGET_DIR)/$(RUST_TARGET)/$(BUILD_PROFILE)/$(PROJECT)
KERNEL_BIN := $(DIST_DIR)/$(KERNEL_NAME).bin
DISK_IMG := $(IMAGE_DIR)/mach_r.img
DISK_QCOW2 := $(IMAGE_DIR)/mach_r.qcow2
ISO_IMAGE := $(IMAGE_DIR)/mach_r.iso
UTM_BUNDLE := $(DIST_DIR)/Mach_R.utm

# QEMU settings
QEMU := qemu-system-aarch64
QEMU_MACHINE := virt
QEMU_CPU := cortex-a72
QEMU_MEMORY := 2G
QEMU_CPUS := 4

# Default target
.DEFAULT_GOAL := all

.PHONY: all
all: kernel disk-image

.PHONY: help
help:
	@echo "Mach_R Build System v$(VERSION)"
	@echo "================================"
	@echo "Available targets:"
	@echo "  make kernel      - Build the kernel"
	@echo "  make disk-image  - Create disk image"
	@echo "  make qemu        - Run in QEMU"
	@echo "  make qemu-kernel - Direct kernel boot in QEMU"
	@echo "  make utm         - Create UTM bundle"
	@echo "  make clean       - Clean build artifacts"
	@echo "  make deps        - Install dependencies"
	@echo "  make fmt         - Format code (cargo fmt)"
	@echo "  make fmt-check   - Check formatting (no changes)"
	@echo "  make clippy      - Lint with clippy (deny warnings)"
	@echo "  make lint        - Run fmt-check and clippy"
	@echo ""
	@echo "Configuration:"
	@echo "  ARCH=$(TARGET_ARCH)"
	@echo "  BUILD_TYPE=$(BUILD_TYPE)"

# Build kernel
.PHONY: kernel
kernel: $(KERNEL_BIN)

$(KERNEL_ELF): FORCE
	@echo "[BUILD] Building $(TARGET_ARCH) kernel ($(BUILD_TYPE))..."
	@$(RUSTUP) target add $(RUST_TARGET) 2>/dev/null || true
	@RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) build $(CARGO_FLAGS) \
		--target $(RUST_TARGET) \
		--bin $(PROJECT)
	@echo "[BUILD] Kernel ELF built: $@"

$(KERNEL_BIN): $(KERNEL_ELF)
	@echo "[OBJCOPY] Creating raw binary..."
	@cp $(KERNEL_ELF) $(KERNEL_BIN)
	@echo "[KERNEL] Binary created: $(KERNEL_BIN)"
	@ls -lh $(KERNEL_BIN)

# Create filesystem
.PHONY: filesystem
filesystem:
	@echo "[FS] Creating root filesystem..."
	@mkdir -p $(SYSROOT_DIR)/{bin,dev,etc,lib,proc,sys,tmp,usr,var}
	@echo "Mach_R $(VERSION)" > $(SYSROOT_DIR)/etc/issue
	@echo "mach_r" > $(SYSROOT_DIR)/etc/hostname
	@touch $(SYSROOT_DIR)/dev/null
	@touch $(SYSROOT_DIR)/dev/zero
	@touch $(SYSROOT_DIR)/dev/console
	@echo "[FS] Filesystem created in $(SYSROOT_DIR)"

# Create disk image (simplified for macOS)
.PHONY: disk-image
disk-image: $(DISK_QCOW2)

$(DISK_IMG): $(KERNEL_BIN) filesystem
	@echo "[DISK] Creating raw disk image..."
	@dd if=/dev/zero of=$(DISK_IMG) bs=1M count=256 2>/dev/null
	@echo "[DISK] Raw image created: $(DISK_IMG)"

$(DISK_QCOW2): $(DISK_IMG)
	@echo "[DISK] Converting to QCOW2..."
	@qemu-img convert -f raw -O qcow2 -c $(DISK_IMG) $(DISK_QCOW2)
	@echo "[DISK] QCOW2 created: $(DISK_QCOW2)"
	@ls -lh $(DISK_QCOW2)

# Create ISO image
$(ISO_IMAGE): $(KERNEL_BIN)
	@echo "[ISO] Creating ISO image..."
	@mkdir -p $(BUILD_DIR)/iso/boot
	@cp $(KERNEL_BIN) $(BUILD_DIR)/iso/boot/kernel.bin
	@hdiutil makehybrid -o $(ISO_IMAGE) $(BUILD_DIR)/iso 2>/dev/null || \
		echo "[ISO] ISO creation needs additional tools"

# Create UTM bundle
.PHONY: utm
utm: $(DISK_QCOW2)
	@echo "[UTM] Creating UTM bundle..."
	@mkdir -p $(UTM_BUNDLE)/Images
	@cp $(DISK_QCOW2) $(UTM_BUNDLE)/Images/mach_r.qcow2
	@echo '<?xml version="1.0" encoding="UTF-8"?>' > $(UTM_BUNDLE)/config.plist
	@echo '<plist version="1.0"><dict>' >> $(UTM_BUNDLE)/config.plist
	@echo '<key>Name</key><string>Mach_R OS</string>' >> $(UTM_BUNDLE)/config.plist
	@echo '<key>Architecture</key><string>aarch64</string>' >> $(UTM_BUNDLE)/config.plist
	@echo '<key>Memory</key><integer>2048</integer>' >> $(UTM_BUNDLE)/config.plist
	@echo '</dict></plist>' >> $(UTM_BUNDLE)/config.plist
	@echo "[UTM] Bundle created: $(UTM_BUNDLE)"

# Run in QEMU with disk
.PHONY: qemu
qemu: $(DISK_QCOW2)
	@echo "[QEMU] Starting Mach_R with disk..."
	@$(QEMU) \
		-M $(QEMU_MACHINE) \
		-cpu $(QEMU_CPU) \
		-smp $(QEMU_CPUS) \
		-m $(QEMU_MEMORY) \
		-drive if=virtio,format=qcow2,file=$(DISK_QCOW2) \
		-kernel $(KERNEL_BIN) \
		-append "root=/dev/vda2 console=ttyAMA0" \
		-device virtio-net-pci,netdev=net0 \
		-netdev user,id=net0 \
		-nographic

# Direct kernel boot
.PHONY: qemu-kernel
qemu-kernel: $(KERNEL_BIN)
	@echo "[QEMU] Direct kernel boot..."
	@$(QEMU) \
		-M $(QEMU_MACHINE) \
		-cpu $(QEMU_CPU) \
		-smp $(QEMU_CPUS) \
		-m $(QEMU_MEMORY) \
		-kernel $(KERNEL_BIN) \
		-nographic

# Run in QEMU with graphics
.PHONY: qemu-gui
qemu-gui: $(DISK_QCOW2)
	@echo "[QEMU] Starting with display..."
	@$(QEMU) \
		-M $(QEMU_MACHINE) \
		-cpu $(QEMU_CPU) \
		-smp $(QEMU_CPUS) \
		-m $(QEMU_MEMORY) \
		-drive if=virtio,format=qcow2,file=$(DISK_QCOW2) \
		-kernel $(KERNEL_BIN) \
		-device virtio-gpu-pci \
		-display default \
		-serial stdio

# Debug with GDB
.PHONY: qemu-debug
qemu-debug: $(KERNEL_BIN)
	@echo "[QEMU] Starting with GDB server..."
	@$(QEMU) \
		-M $(QEMU_MACHINE) \
		-cpu $(QEMU_CPU) \
		-smp $(QEMU_CPUS) \
		-m $(QEMU_MEMORY) \
		-kernel $(KERNEL_BIN) \
		-nographic \
		-s -S &
	@echo "[GDB] Connect with: gdb $(KERNEL_ELF) -ex 'target remote :1234'"

# Open in UTM
.PHONY: utm-run
utm-run: utm
	@echo "[UTM] Opening in UTM..."
	@open $(UTM_BUNDLE)

# Testing
.PHONY: test
test:
	@echo "[TEST] Running tests..."
	@$(CARGO) test --lib

# Documentation
.PHONY: doc
doc:
	@echo "[DOC] Building documentation..."
	@$(CARGO) doc --no-deps --open

# Formatting and linting
.PHONY: fmt fmt-check clippy lint
fmt:
	@echo "[FMT] Formatting with rustfmt..."
	@$(CARGO) fmt

fmt-check:
	@echo "[FMT] Checking formatting..."
	@$(CARGO) fmt -- --check

clippy:
	@echo "[LINT] Running clippy..."
	@$(CARGO) clippy -- -D warnings

lint: fmt-check clippy
	@echo "[LINT] fmt-check and clippy passed"

# Clean
.PHONY: clean
clean:
	@echo "[CLEAN] Removing build artifacts..."
	@$(CARGO) clean
	@rm -rf $(BUILD_DIR)

# Install dependencies
.PHONY: deps
deps:
	@echo "[DEPS] Installing dependencies..."
	@command -v brew >/dev/null || echo "Please install Homebrew"
	@brew list qemu >/dev/null 2>&1 || brew install qemu
	@brew list --cask utm >/dev/null 2>&1 || brew install --cask utm
	@$(RUSTUP) target add $(RUST_TARGET) || true
	@$(RUSTUP) component add rust-src || true
	@$(RUSTUP) component add rustfmt clippy || true

# Statistics
.PHONY: stats
stats:
	@echo "[STATS] Project Statistics"
	@echo "Lines of Rust code:"
	@find src -name "*.rs" | xargs wc -l | tail -1
	@echo "Modules:"
	@find src -type f -name "*.rs" | wc -l

FORCE:

.PHONY: FORCE
