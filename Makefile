# MinimalOS Build System - x86_64 with Limine Bootloader
# Supports both BIOS and UEFI boot

# Build configuration
ARCH := x86_64

# Toolchain
CC := gcc
LD := ld
AS := as
NASM := nasm

# Build directories
BUILDDIR := build
DISTDIR := $(BUILDDIR)/dist
OBJDIR := $(BUILDDIR)/obj
ISODIR := $(BUILDDIR)/iso

# Output files
KERNEL := $(DISTDIR)/minimalos
ISO := $(DISTDIR)/minimalos.iso

# Compiler flags for x86_64 freestanding kernel
CFLAGS := -std=gnu11 -ffreestanding -O2 -Wall -Wextra
CFLAGS += -Ikernel/include
CFLAGS += -m64 -mcmodel=kernel -mno-red-zone
CFLAGS += -mno-80387 -mno-mmx -mno-sse -mno-sse2
CFLAGS += -fno-stack-protector -fno-stack-check -fno-lto -fno-PIC
CFLAGS += -ffunction-sections -fdata-sections

# Preprocessor flags
CPPFLAGS := -MMD -MP

# Assembler flags
ASFLAGS := --64

# Linker flags
LDFLAGS := -m elf_x86_64 -nostdlib -static -z max-page-size=0x1000
LDFLAGS += --gc-sections -T arch/$(ARCH)/linker.ld

# Source files
KERNEL_C_SOURCES := \
	kernel/kernel.c \
	kernel/tty.c \
	kernel/shell.c \
	kernel/arch/x86_64/gdt.c \
	kernel/arch/x86_64/idt.c \
	kernel/arch/x86_64/isr.c \
	kernel/arch/x86_64/irq.c \
	kernel/mm/pmm.c \
	kernel/mm/paging.c \
	kernel/mm/kheap.c \
	kernel/process/process.c \
	kernel/process/scheduler.c \
	kernel/process/syscall.c \
	kernel/commands/utils.c \
	kernel/commands/basic.c \
	kernel/commands/sysinfo.c \
	kernel/commands/memory.c \
	kernel/commands/display.c \
	kernel/commands/tests.c

DRIVER_C_SOURCES := \
	drivers/timer.c \
	drivers/keyboard.c \
	drivers/framebuffer.c \
	drivers/font.c

ASM_SOURCES := \
	kernel/arch/x86_64/gdt_flush.s \
	kernel/arch/x86_64/idt_flush.s \
	kernel/arch/x86_64/isr_stub.s \
	kernel/arch/x86_64/irq_stub.s \
	kernel/arch/x86_64/switch.s \
	kernel/arch/x86_64/usermode.s

# Object files
KERNEL_OBJS := $(patsubst %.c,$(OBJDIR)/%.o,$(KERNEL_C_SOURCES))
DRIVER_OBJS := $(patsubst %.c,$(OBJDIR)/%.o,$(DRIVER_C_SOURCES))
ASM_OBJS := $(patsubst %.s,$(OBJDIR)/%.o,$(ASM_SOURCES))

ALL_OBJS := $(KERNEL_OBJS) $(DRIVER_OBJS) $(ASM_OBJS)
DEPS := $(ALL_OBJS:.o=.d)

# Limine paths
LIMINE_DIR := limine
LIMINE_BRANCH := v8.x-binary

# Phony targets
.PHONY: all clean iso qemu qemu-bios qemu-uefi run dirs limine help
.PHONY: kernel rust-kernel rust-iso rust-run rust-clean

# Default target
all: dirs $(KERNEL)

# Include generated dependencies
-include $(DEPS)

# Create build directories
dirs:
	@mkdir -p $(DISTDIR)
	@mkdir -p $(OBJDIR)/kernel/arch/x86_64
	@mkdir -p $(OBJDIR)/kernel/mm
	@mkdir -p $(OBJDIR)/kernel/process
	@mkdir -p $(OBJDIR)/kernel/commands
	@mkdir -p $(OBJDIR)/drivers

# Link kernel
$(KERNEL): $(ALL_OBJS)
	@echo "Linking kernel..."
	$(LD) $(LDFLAGS) -o $@ $(ALL_OBJS)
	@echo "Kernel built: $@"

# Compile C sources
$(OBJDIR)/%.o: %.c
	@mkdir -p $(dir $@)
	@echo "CC $<"
	@$(CC) $(CFLAGS) $(CPPFLAGS) -c $< -o $@

# Assemble sources (GNU as)
$(OBJDIR)/%.o: %.s
	@mkdir -p $(dir $@)
	@echo "AS $<"
	@$(AS) $(ASFLAGS) $< -o $@

# Clone/update Limine
limine:
	@if [ ! -d "$(LIMINE_DIR)" ]; then \
		echo "Cloning Limine..."; \
		git clone https://github.com/limine-bootloader/limine.git $(LIMINE_DIR) --branch=$(LIMINE_BRANCH) --depth=1; \
	fi
	@make -C $(LIMINE_DIR)

# Create bootable ISO (BIOS + UEFI)
iso: $(KERNEL) limine
	@echo "Creating bootable ISO..."
	@mkdir -p $(ISODIR)/boot/limine
	@mkdir -p $(ISODIR)/EFI/BOOT
	
	# Copy kernel
	@cp $(KERNEL) $(ISODIR)/boot/
	
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
	qemu-system-x86_64 -cdrom $(ISO) -serial stdio -m 256M

# Run in QEMU (UEFI mode) - requires OVMF
qemu-uefi: iso
	qemu-system-x86_64 -cdrom $(ISO) -serial stdio -m 256M \
		-bios /usr/share/OVMF/OVMF_CODE.fd

# Default run target (BIOS)
qemu: qemu-bios
run: qemu

# Debug mode
qemu-debug: iso
	qemu-system-x86_64 -cdrom $(ISO) -serial stdio -m 256M \
		-d int,cpu_reset -no-reboot

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@rm -rf $(BUILDDIR)
	@echo "Clean complete"

# Full clean (including Limine)
distclean: clean
	@rm -rf $(LIMINE_DIR)

# Help
help:
	@echo "MinimalOS Build System (x86_64 + Limine)"
	@echo "========================================="
	@echo ""
	@echo "C Kernel Targets:"
	@echo "  make           - Build C kernel"
	@echo "  make iso       - Create bootable ISO (BIOS + UEFI)"
	@echo "  make run       - Run in QEMU (BIOS mode)"
	@echo "  make qemu-bios - Run in QEMU (BIOS mode)"
	@echo "  make qemu-uefi - Run in QEMU (UEFI mode)"
	@echo "  make clean     - Remove build artifacts"
	@echo "  make distclean - Remove build + Limine"
	@echo ""
	@echo "Rust Kernel Targets:"
	@echo "  make kernel    - Build Rust kernel via Cargo"
	@echo "  make rust-iso  - Create bootable ISO with Rust kernel"
	@echo "  make rust-run  - Run Rust kernel in QEMU"
	@echo ""
	@echo "Output (C): $(ISO)"

# =============================================================================
# Rust Kernel Build Targets
# =============================================================================

RUST_TARGET := build/target-kernel.json
RUST_KERNEL_BIN := target/target-kernel/debug/minimalos_kernel
RUST_ISO := $(DISTDIR)/minimalos-rust.iso
RUST_ISODIR := $(BUILDDIR)/rust-iso

# Build Rust kernel via Cargo
kernel:
	cargo build --package minimalos_kernel --target $(RUST_TARGET)

rust-kernel: kernel

# Create bootable ISO with Rust kernel
rust-iso: kernel limine
	@echo "Creating bootable ISO (Rust kernel)..."
	@mkdir -p $(RUST_ISODIR)/boot/limine
	@mkdir -p $(RUST_ISODIR)/EFI/BOOT
	@cp $(RUST_KERNEL_BIN) $(RUST_ISODIR)/boot/kernel
	@cp limine.cfg $(RUST_ISODIR)/boot/limine/
	@cp $(LIMINE_DIR)/limine-bios.sys $(RUST_ISODIR)/boot/limine/
	@cp $(LIMINE_DIR)/limine-bios-cd.bin $(RUST_ISODIR)/boot/limine/
	@cp $(LIMINE_DIR)/limine-uefi-cd.bin $(RUST_ISODIR)/boot/limine/
	@cp $(LIMINE_DIR)/BOOTX64.EFI $(RUST_ISODIR)/EFI/BOOT/
	@cp $(LIMINE_DIR)/BOOTIA32.EFI $(RUST_ISODIR)/EFI/BOOT/
	xorriso -as mkisofs -R -r -J \
		-b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		-hfsplus -apm-block-size 2048 \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		$(RUST_ISODIR) -o $(RUST_ISO)
	./$(LIMINE_DIR)/limine bios-install $(RUST_ISO)
	@echo "ISO created: $(RUST_ISO)"

# Run Rust kernel in QEMU
rust-run: rust-iso
	qemu-system-x86_64 -M q35 -m 2G -cdrom $(RUST_ISO) -serial stdio

# Clean Rust build artifacts
rust-clean:
	cargo clean
	@rm -rf $(RUST_ISODIR)
