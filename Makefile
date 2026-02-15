# MinimalOS Build System - x86_64 with Limine Bootloader (Rust)
# Supports both BIOS and UEFI boot

# Build configuration
ARCH := x86_64

# Build directories
BUILDDIR := build
DISTDIR := $(BUILDDIR)/dist
ISODIR := $(BUILDDIR)/iso

# Rust target and output
RUST_TARGET := build/target-kernel.json
RUST_KERNEL_BIN := target/target-kernel/debug/minimalos_kernel
ISO := $(DISTDIR)/minimalos.iso

# Limine paths
LIMINE_DIR := limine
LIMINE_BRANCH := v8.x-binary

# Phony targets
.PHONY: all kernel clean iso qemu qemu-bios qemu-uefi run limine help distclean

# Default target
all: kernel

# Build Rust kernel via Cargo
kernel:
	cargo build --package minimalos_kernel --target $(RUST_TARGET)

# Clone/update Limine
limine:
	@if [ ! -d "$(LIMINE_DIR)" ]; then \
		echo "Cloning Limine..."; \
		git clone https://github.com/limine-bootloader/limine.git $(LIMINE_DIR) --branch=$(LIMINE_BRANCH) --depth=1; \
	fi
	@make -C $(LIMINE_DIR)

# Create bootable ISO (BIOS + UEFI)
iso: kernel limine
	@echo "Creating bootable ISO..."
	@mkdir -p $(ISODIR)/boot/limine
	@mkdir -p $(ISODIR)/EFI/BOOT
	@mkdir -p $(DISTDIR)
	
	# Copy kernel
	@cp $(RUST_KERNEL_BIN) $(ISODIR)/boot/kernel
	
	# Copy Limine configuration
	@cp limine.cfg $(ISODIR)/boot/limine/
	
	# Copy Limine boot files
	@cp $(LIMINE_DIR)/limine-bios.sys $(ISODIR)/boot/limine/
	@cp $(LIMINE_DIR)/limine-bios-cd.bin $(ISODIR)/boot/limine/
	@cp $(LIMINE_DIR)/limine-uefi-cd.bin $(ISODIR)/boot/limine/
	
	# Copy EFI executables
	@cp $(LIMINE_DIR)/BOOTX64.EFI $(ISODIR)/EFI/BOOT/
	@cp $(LIMINE_DIR)/BOOTIA32.EFI $(ISODIR)/EFI/BOOT/
	
	# Create ISO with xorriso
	xorriso -as mkisofs -R -r -J \
		-b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		-hfsplus -apm-block-size 2048 \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		$(ISODIR) -o $(ISO)
	
	# Install Limine BIOS stages
	./$(LIMINE_DIR)/limine bios-install $(ISO)
	
	@echo "ISO created: $(ISO)"

# Run in QEMU (BIOS mode)
qemu-bios: iso
	qemu-system-x86_64 -M q35 -m 2G -cdrom $(ISO) -serial stdio

# Run in QEMU (UEFI mode) - requires OVMF
qemu-uefi: iso
	qemu-system-x86_64 -M q35 -m 2G -cdrom $(ISO) -serial stdio \
		-bios /usr/share/OVMF/OVMF_CODE.fd

# Default run target (BIOS)
qemu: qemu-bios
run: qemu

# Debug mode
qemu-debug: iso
	qemu-system-x86_64 -M q35 -m 2G -cdrom $(ISO) -serial stdio \
		-d int,cpu_reset -no-reboot

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	@rm -rf $(DISTDIR) $(ISODIR)
	@echo "Clean complete"

# Full clean (including Limine)
distclean: clean
	@rm -rf $(LIMINE_DIR)

# Help
help:
	@echo "MinimalOS Build System (x86_64 + Limine)"
	@echo "========================================="
	@echo ""
	@echo "Targets:"
	@echo "  make           - Build kernel via Cargo"
	@echo "  make kernel    - Build kernel via Cargo"
	@echo "  make iso       - Create bootable ISO (BIOS + UEFI)"
	@echo "  make run       - Run in QEMU (BIOS mode)"
	@echo "  make qemu-bios - Run in QEMU (BIOS mode)"
	@echo "  make qemu-uefi - Run in QEMU (UEFI mode)"
	@echo "  make clean     - Remove build artifacts"
	@echo "  make distclean - Remove build + Limine"
	@echo ""
	@echo "Output: $(ISO)"
