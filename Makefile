# =============================================================================
# MinimalOS NextGen — Build System
# =============================================================================
#
# Uses cargo for kernel compilation, make for orchestrating ISO creation
# and QEMU testing.
#
# TARGETS:
#   make              — Build the kernel (debug)
#   make release      — Build the kernel (release, with LTO)
#   make iso          — Build kernel + create bootable ISO (debug)
#   make iso-release  — Build kernel + create bootable ISO (release)
#   make run          — Build + ISO + run in QEMU (debug)
#   make run-release  — Build + ISO + run in QEMU (release)
#   make clean        — Remove build artifacts
#   make distclean    — Remove everything including downloaded Limine
#
# REQUIREMENTS:
#   - Rust nightly toolchain (configured in rust-toolchain.toml)
#   - xorriso:  sudo apt install xorriso
#   - QEMU:     sudo apt install qemu-system-x86
#   - git:      for downloading Limine bootloader
#
# =============================================================================

# -----------------------------------------------------------------------------
# Configuration
# -----------------------------------------------------------------------------

# Cross-compilation target (bare metal x86_64)
TARGET          := x86_64-unknown-none

# Limine bootloader version (binary release branch)
LIMINE_VERSION  := v8.6.0
LIMINE_REPO     := https://github.com/limine-bootloader/limine.git
LIMINE_BRANCH   := $(LIMINE_VERSION)-binary

# Directory layout
BUILD_DIR       := target
LIMINE_DIR      := $(BUILD_DIR)/limine-src
ISO_DIR         := $(BUILD_DIR)/iso

# Kernel binary paths
KERNEL_DEBUG    := $(BUILD_DIR)/$(TARGET)/debug/minimalos-kernel
KERNEL_RELEASE  := $(BUILD_DIR)/$(TARGET)/release/minimalos-kernel

# User binary paths (ELF — loaded from initrd TAR, not flat binary)
SERIAL_DRV_ELF_DEBUG   := $(BUILD_DIR)/$(TARGET)/debug/serial_drv
SERIAL_DRV_ELF_RELEASE := $(BUILD_DIR)/$(TARGET)/release/serial_drv
INIT_ELF_DEBUG         := $(BUILD_DIR)/$(TARGET)/debug/init
INIT_ELF_RELEASE       := $(BUILD_DIR)/$(TARGET)/release/init

# Initrd TAR archive (contains user ELF binaries)
INITRD_DEBUG           := $(BUILD_DIR)/initrd-debug.tar
INITRD_RELEASE         := $(BUILD_DIR)/initrd-release.tar

# Wasm payload paths (compiled separately with wasm32-unknown-unknown target)
WASM_HELLO_RELEASE     := $(BUILD_DIR)/wasm32-unknown-unknown/release/hello_wasm.wasm

# Output ISO paths
ISO_DEBUG       := $(BUILD_DIR)/minimalos-debug.iso
ISO_RELEASE     := $(BUILD_DIR)/minimalos-release.iso

# Limine bootloader files (produced by `make limine`)
LIMINE_CLI      := $(LIMINE_DIR)/limine
LIMINE_BIOS_CD  := $(LIMINE_DIR)/limine-bios-cd.bin
LIMINE_BIOS_SYS := $(LIMINE_DIR)/limine-bios.sys
LIMINE_UEFI_CD  := $(LIMINE_DIR)/limine-uefi-cd.bin
LIMINE_EFI      := $(LIMINE_DIR)/BOOTX64.EFI

# QEMU settings — tuned to approximate N3710 hardware
QEMU            := qemu-system-x86_64
QEMU_MEMORY     := 512M
QEMU_CPUS       := 4
QEMU_FLAGS      := \
	-serial stdio        \
	-no-reboot           \
	-no-shutdown

# objcopy for debugging/analysis (no longer used for flat binary extraction)
OBJCOPY := $(firstword $(shell which llvm-objcopy rust-objcopy objcopy 2>/dev/null))

# Per-crate RUSTFLAGS.
# Since .cargo/config.toml has no rustflags, the RUSTFLAGS env var is respected.
# Kernel: code-model=kernel (top 2GB), static relocation, no SIMD
KERNEL_RUSTFLAGS := -C target-feature=-sse,-sse2,-avx -C code-model=kernel -C relocation-model=static
# User crates: default code-model (small), static relocation, no SIMD
USER_RUSTFLAGS   := -C target-feature=-sse,-sse2,-avx -C relocation-model=static

# Detect OVMF for UEFI boot
OVMF := $(firstword $(wildcard \
	/usr/share/qemu/OVMF.fd           \
	/usr/share/ovmf/OVMF.fd           \
	/usr/share/OVMF/OVMF_CODE_4M.fd   \
	/usr/share/edk2/ovmf/OVMF_CODE.fd \
))

ifneq ($(OVMF),)
  QEMU_FLAGS += -bios $(OVMF)
endif

# Disable built-in rules — we only use explicit rules
.SUFFIXES:

# Mark non-file targets
.PHONY: all release iso iso-release run run-release limine clean distclean help

# -----------------------------------------------------------------------------
# Default target
# -----------------------------------------------------------------------------

all: kernel-debug
	@echo ""
	@echo "  Build complete (debug). Kernel: $(KERNEL_DEBUG)"
	@echo "  Run 'make run' to boot in QEMU."

release: kernel-release
	@echo ""
	@echo "  Build complete (release). Kernel: $(KERNEL_RELEASE)"
	@echo "  Run 'make run-release' to boot in QEMU."

# -----------------------------------------------------------------------------
# Kernel build (delegates to cargo)
# -----------------------------------------------------------------------------
#
# Build order is critical:
#   1. Build serial_drv user binary → ELF
#   2. Package user ELF binaries into initrd.tar
#   3. Build kernel (no longer embeds user binaries)
#
# The initrd.tar is loaded by Limine as a boot module and parsed by the
# kernel's TarFS parser at runtime. This replaces the flat binary hack.

.PHONY: kernel-debug kernel-release serial-drv-debug serial-drv-release init-debug init-release initrd-debug initrd-release wasm-hello

# --- Wasm payload (built with standard cargo, NOT workspace — separate target) ---

wasm-hello:
	@echo "[wasm] Building hello_wasm for wasm32-unknown-unknown..."
	cargo build --manifest-path apps/hello_wasm/Cargo.toml --target wasm32-unknown-unknown --target-dir $(BUILD_DIR) --release
	@echo "[wasm] $(WASM_HELLO_RELEASE) ($$(wc -c < $(WASM_HELLO_RELEASE)) bytes)"

# --- User binaries ---

serial-drv-debug:
	RUSTFLAGS="$(USER_RUSTFLAGS)" cargo build -p serial_drv
	@echo "[serial_drv] ELF: $(SERIAL_DRV_ELF_DEBUG) ($$(wc -c < $(SERIAL_DRV_ELF_DEBUG)) bytes)"

serial-drv-release:
	RUSTFLAGS="$(USER_RUSTFLAGS)" cargo build --release -p serial_drv
	@echo "[serial_drv] ELF: $(SERIAL_DRV_ELF_RELEASE) ($$(wc -c < $(SERIAL_DRV_ELF_RELEASE)) bytes)"

init-debug:
	RUSTFLAGS="$(USER_RUSTFLAGS)" cargo build -p init
	@echo "[init] ELF: $(INIT_ELF_DEBUG) ($$(wc -c < $(INIT_ELF_DEBUG)) bytes)"

init-release:
	RUSTFLAGS="$(USER_RUSTFLAGS)" cargo build --release -p init
	@echo "[init] ELF: $(INIT_ELF_RELEASE) ($$(wc -c < $(INIT_ELF_RELEASE)) bytes)"

# --- Initrd TAR archive (contains all userspace ELF binaries) ---

initrd-debug: init-debug serial-drv-debug wasm-hello
	@mkdir -p $(BUILD_DIR)/initrd-staging
	@cp $(INIT_ELF_DEBUG) $(BUILD_DIR)/initrd-staging/init
	@cp $(SERIAL_DRV_ELF_DEBUG) $(BUILD_DIR)/initrd-staging/serial_drv
	@cp $(WASM_HELLO_RELEASE) $(BUILD_DIR)/initrd-staging/hello_wasm.wasm
	@cd $(BUILD_DIR)/initrd-staging && tar cf ../initrd-debug.tar --format=ustar *
	@echo "[initrd] $(INITRD_DEBUG) ($$(wc -c < $(INITRD_DEBUG)) bytes, $$(tar tf $(INITRD_DEBUG) | wc -l) files)"

initrd-release: init-release serial-drv-release wasm-hello
	@mkdir -p $(BUILD_DIR)/initrd-staging
	@cp $(INIT_ELF_RELEASE) $(BUILD_DIR)/initrd-staging/init
	@cp $(SERIAL_DRV_ELF_RELEASE) $(BUILD_DIR)/initrd-staging/serial_drv
	@cp $(WASM_HELLO_RELEASE) $(BUILD_DIR)/initrd-staging/hello_wasm.wasm
	@cd $(BUILD_DIR)/initrd-staging && tar cf ../initrd-release.tar --format=ustar *
	@echo "[initrd] $(INITRD_RELEASE) ($$(wc -c < $(INITRD_RELEASE)) bytes, $$(tar tf $(INITRD_RELEASE) | wc -l) files)"

# --- Kernel (independent of user binaries — reads ELF from initrd at runtime) ---

kernel-debug: initrd-debug
	RUSTFLAGS="$(KERNEL_RUSTFLAGS)" cargo build -p minimalos-kernel

kernel-release: initrd-release
	RUSTFLAGS="$(KERNEL_RUSTFLAGS)" cargo build --release -p minimalos-kernel

# -----------------------------------------------------------------------------
# Limine bootloader setup
# -----------------------------------------------------------------------------
#
# Downloads the binary release of Limine (pre-built EFI binaries and
# BIOS boot sectors) and compiles the `limine` CLI utility.
#
# The CLI utility is needed to install the BIOS bootloader into the ISO.
# You only need to run this once — it's cached in target/limine-src/.

$(LIMINE_CLI): | $(BUILD_DIR)
	@echo "[limine] Cloning Limine $(LIMINE_VERSION) binary release..."
	@if [ ! -d "$(LIMINE_DIR)/.git" ]; then \
		git clone --depth 1 --branch $(LIMINE_BRANCH) $(LIMINE_REPO) $(LIMINE_DIR); \
	fi
	@echo "[limine] Building limine CLI..."
	@$(MAKE) -C $(LIMINE_DIR)
	@echo "[limine] Ready."

limine: $(LIMINE_CLI)

$(BUILD_DIR):
	@mkdir -p $(BUILD_DIR)

# -----------------------------------------------------------------------------
# ISO image creation
# -----------------------------------------------------------------------------
#
# Creates a bootable ISO with both BIOS and UEFI support:
#   1. Build the kernel
#   2. Ensure Limine is available
#   3. Assemble the ISO directory structure
#   4. Run xorriso to create the ISO
#   5. Install Limine BIOS boot sector
#
# ISO layout:
#   /boot/minimalos-kernel     — our kernel ELF binary
#   /boot/limine.conf          — Limine bootloader configuration
#   /boot/limine-bios-cd.bin   — BIOS El Torito boot image
#   /boot/limine-bios.sys      — BIOS stage 2
#   /boot/limine-uefi-cd.bin   — UEFI El Torito boot image
#   /EFI/BOOT/BOOTX64.EFI     — UEFI fallback bootloader

iso: kernel-debug $(LIMINE_CLI)
	$(call make-iso,$(KERNEL_DEBUG),$(INITRD_DEBUG),$(ISO_DEBUG))

iso-release: kernel-release $(LIMINE_CLI)
	$(call make-iso,$(KERNEL_RELEASE),$(INITRD_RELEASE),$(ISO_RELEASE))

# Reusable function: $(call make-iso,<kernel-elf>,<initrd-tar>,<output-iso>)
define make-iso
	@echo "[iso] Assembling ISO directory..."
	@rm -rf $(ISO_DIR)
	@mkdir -p $(ISO_DIR)/boot $(ISO_DIR)/EFI/BOOT
	@cp $(1) $(ISO_DIR)/boot/minimalos-kernel
	@cp $(2) $(ISO_DIR)/boot/initrd.tar
	@cp boot/limine.conf $(ISO_DIR)/boot/limine.conf
	@cp $(LIMINE_BIOS_CD)  $(ISO_DIR)/boot/
	@cp $(LIMINE_BIOS_SYS) $(ISO_DIR)/boot/
	@cp $(LIMINE_UEFI_CD)  $(ISO_DIR)/boot/
	@cp $(LIMINE_EFI)      $(ISO_DIR)/EFI/BOOT/BOOTX64.EFI
	@echo "[iso] Creating ISO image: $(3)"
	@xorriso -as mkisofs                           \
		-R -J                                      \
		-b boot/limine-bios-cd.bin                 \
		-no-emul-boot                              \
		-boot-load-size 4                          \
		-boot-info-table                           \
		--efi-boot boot/limine-uefi-cd.bin         \
		-efi-boot-part --efi-boot-image            \
		-o $(3) $(ISO_DIR) 2>/dev/null
	@$(LIMINE_CLI) bios-install $(3) 2>/dev/null
	@echo "[iso] Done: $(3) ($$(du -h $(3) | cut -f1))"
endef

# -----------------------------------------------------------------------------
# Run in QEMU
# -----------------------------------------------------------------------------
#
# Boots the ISO in QEMU with settings approximating the N3710:
#   -smp 4       :  4 cores (like N3710)
#   -m 512M      :  enough to test (use 8G for full emulation)
#   -serial stdio:  kernel serial output goes to your terminal
#   -no-reboot   :  halt on crash instead of rebooting (easier debug)
#
# If OVMF is detected, UEFI boot is used. Otherwise falls back to BIOS.
#
# Press Ctrl+A, X to exit QEMU.

run: iso
	@echo ""
	@echo "  Booting MinimalOS NextGen (debug) in QEMU..."
	@echo "  Press Ctrl+A, X to exit."
	@echo ""
	@test -f $(BUILD_DIR)/virtio-test.img || dd if=/dev/zero of=$(BUILD_DIR)/virtio-test.img bs=1M count=1 2>/dev/null
	$(QEMU) -cdrom $(ISO_DEBUG) -smp $(QEMU_CPUS) -m $(QEMU_MEMORY) $(QEMU_FLAGS) \
		-drive file=$(BUILD_DIR)/virtio-test.img,format=raw,if=virtio

run-release: iso-release
	@echo ""
	@echo "  Booting MinimalOS NextGen (release) in QEMU..."
	@echo "  Press Ctrl+A, X to exit."
	@echo ""
	$(QEMU) -cdrom $(ISO_RELEASE) -smp $(QEMU_CPUS) -m $(QEMU_MEMORY) $(QEMU_FLAGS)

# Headless run — serial output to file, exits after timeout
# Usage: make run-headless [TIMEOUT=10]
TIMEOUT ?= 10

.PHONY: run-headless
run-headless: iso
	@echo "[qemu] Booting headless (timeout=$(TIMEOUT)s)..."
	@rm -f $(BUILD_DIR)/serial.log
	@test -f $(BUILD_DIR)/virtio-test.img || dd if=/dev/zero of=$(BUILD_DIR)/virtio-test.img bs=1M count=1 2>/dev/null
	@$(QEMU) -cdrom $(ISO_DEBUG) -smp $(QEMU_CPUS) -m $(QEMU_MEMORY) \
		-drive file=$(BUILD_DIR)/virtio-test.img,format=raw,if=virtio \
		-serial file:$(BUILD_DIR)/serial.log \
		-display none \
		-no-reboot -no-shutdown \
		$(if $(OVMF),-bios $(OVMF)) &                                       \
	QEMU_PID=$$!;                                                            \
	sleep $(TIMEOUT);                                                        \
	kill $$QEMU_PID 2>/dev/null; wait $$QEMU_PID 2>/dev/null;               \
	echo "";                                                                 \
	echo "[qemu] Serial output:";                                           \
	echo "-------------------------------------------------------";         \
	cat $(BUILD_DIR)/serial.log 2>/dev/null;                                 \
	echo "";                                                                 \
	echo "-------------------------------------------------------"

# -----------------------------------------------------------------------------
# Clean
# -----------------------------------------------------------------------------

clean:
	cargo clean
	@rm -rf $(ISO_DIR)
	@rm -f $(ISO_DEBUG) $(ISO_RELEASE)
	@rm -f $(BUILD_DIR)/serial.log $(BUILD_DIR)/qemu-debug.log
	@rm -f $(INITRD_DEBUG) $(INITRD_RELEASE)
	@rm -rf $(BUILD_DIR)/initrd-staging
	@echo "Clean."

distclean: clean
	@rm -rf $(LIMINE_DIR)
	@echo "Distclean (Limine removed)."

# -----------------------------------------------------------------------------
# Help
# -----------------------------------------------------------------------------

help:
	@echo ""
	@echo "  MinimalOS NextGen — Build System"
	@echo ""
	@echo "  USAGE:"
	@echo "    make              Build the kernel (debug)"
	@echo "    make release      Build the kernel (release, with LTO)"
	@echo "    make iso          Build + create bootable ISO (debug)"
	@echo "    make iso-release  Build + create bootable ISO (release)"
	@echo "    make run          Build + ISO + boot in QEMU (debug)"
	@echo "    make run-release  Build + ISO + boot in QEMU (release)"
	@echo "    make run-headless Boot headless, serial to file (TIMEOUT=10)"
	@echo "    make limine       Download/build Limine bootloader"
	@echo "    make clean        Remove build artifacts"
	@echo "    make distclean    Remove everything incl. Limine"
	@echo "    make help         Show this message"
	@echo ""
	@echo "  VARIABLES:"
	@echo "    QEMU_MEMORY=8G    Set QEMU RAM (default: $(QEMU_MEMORY))"
	@echo "    QEMU_CPUS=2       Set QEMU CPU count (default: $(QEMU_CPUS))"
	@echo "    TIMEOUT=15        Headless timeout seconds (default: $(TIMEOUT))"
	@echo ""
