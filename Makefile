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
RAMDISK := $(DISTDIR)/ramdisk.tar

# Limine paths
LIMINE_DIR := limine
LIMINE_BRANCH := v8.x-binary

.PHONY: all kernel clean iso qemu qemu-bios qemu-uefi run limine help distclean ramdisk actor-vfs actor-ui-server actor-shell actor-chaos actor-ps2-keyboard actor-ps2-mouse

# Default target
all: kernel

# Build Rust kernel via Cargo
kernel:
	cargo build --package minimalos_kernel --target $(RUST_TARGET)

actor-vfs:
	RUSTFLAGS="-C link-arg=--no-entry" cargo build --manifest-path actors/vfs/Cargo.toml --target wasm32-unknown-unknown --release
	@mkdir -p ramdisk
	cp target/wasm32-unknown-unknown/release/vfs.wasm ramdisk/vfs.wasm

actor-ui-server:
	RUSTFLAGS="-C link-arg=--no-entry" cargo build --manifest-path actors/ui_server/Cargo.toml --target wasm32-unknown-unknown --release
	@mkdir -p ramdisk
	cp target/wasm32-unknown-unknown/release/ui_server.wasm ramdisk/ui_server.wasm

actor-shell:
	RUSTFLAGS="-C link-arg=--no-entry" cargo build --manifest-path actors/shell/Cargo.toml --target wasm32-unknown-unknown --release
	@mkdir -p ramdisk
	cp target/wasm32-unknown-unknown/release/wasm_shell.wasm ramdisk/shell.wasm

actor-chaos:
	RUSTFLAGS="-C link-arg=--no-entry" cargo build --manifest-path actors/chaos/Cargo.toml --target wasm32-unknown-unknown --release
	@mkdir -p ramdisk
	cp target/wasm32-unknown-unknown/release/chaos.wasm ramdisk/chaos.wasm

actor-ps2-keyboard:
	RUSTFLAGS="-C link-arg=--no-entry" cargo build --manifest-path actors/ps2_keyboard/Cargo.toml --target wasm32-unknown-unknown --release
	@mkdir -p ramdisk
	cp target/wasm32-unknown-unknown/release/ps2_keyboard.wasm ramdisk/ps2_keyboard.wasm

actor-ps2-mouse:
	RUSTFLAGS="-C link-arg=--no-entry" cargo build --manifest-path actors/ps2_mouse/Cargo.toml --target wasm32-unknown-unknown --release
	@mkdir -p ramdisk
	cp target/wasm32-unknown-unknown/release/ps2_mouse.wasm ramdisk/ps2_mouse.wasm

# Build ramdisk tar archive from ramdisk/ directory
ramdisk: actor-vfs actor-ui-server actor-shell actor-chaos actor-ps2-keyboard actor-ps2-mouse
	@mkdir -p $(DISTDIR)
	cp assets/font.psf ramdisk/font.psf
	tar cf $(RAMDISK) -C ramdisk .
	@echo "RAMDisk: $$(wc -c < $(RAMDISK)) bytes"

# Clone/update Limine
limine:
	@if [ ! -d "$(LIMINE_DIR)" ]; then \
		echo "Cloning Limine..."; \
		git clone https://github.com/limine-bootloader/limine.git $(LIMINE_DIR) --branch=$(LIMINE_BRANCH) --depth=1; \
	fi
	@make -C $(LIMINE_DIR)

# Create bootable ISO (BIOS + UEFI)
iso: kernel ramdisk limine
	@echo "Creating bootable ISO..."
	@mkdir -p $(ISODIR)/boot/limine
	@mkdir -p $(ISODIR)/EFI/BOOT
	@mkdir -p $(DISTDIR)
	
	# Copy kernel
	@cp $(RUST_KERNEL_BIN) $(ISODIR)/boot/kernel
	
	# Copy ramdisk
	@cp $(RAMDISK) $(ISODIR)/boot/ramdisk.tar
	
	# Copy Limine configuration
	@cp limine.conf $(ISODIR)/boot/limine/
	
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
	qemu-system-x86_64 -M q35 -m 2G -cdrom $(ISO) -serial stdio -smp 4

# Run in QEMU (UEFI mode) - requires OVMF
qemu-uefi: iso
	qemu-system-x86_64 -M q35 -m 2G -cdrom $(ISO) -serial stdio -smp 4 \
		-drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd

# Default run target (BIOS)
qemu: qemu-bios
run: qemu

# Debug mode
qemu-debug: iso
	qemu-system-x86_64 -M q35 -m 2G -cdrom $(ISO) -serial stdio -smp 4 \
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
	@echo "  make		   - Build kernel via Cargo"
	@echo "  make kernel	- Build kernel via Cargo"
	@echo "  make iso	   - Create bootable ISO (BIOS + UEFI)"
	@echo "  make run	   - Run in QEMU (BIOS mode)"
	@echo "  make qemu-bios - Run in QEMU (BIOS mode)"
	@echo "  make qemu-uefi - Run in QEMU (UEFI mode)"
	@echo "  make clean	 - Remove build artifacts"
	@echo "  make distclean - Remove build + Limine"
	@echo ""
	@echo "Output: $(ISO)"
