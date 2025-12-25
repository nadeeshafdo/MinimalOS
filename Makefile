# MinimalOS Makefile

# Toolchain
AS := as
CC := gcc
LD := ld

# Directories
SRC_DIR := src
BUILD_DIR := build
DIST_DIR := dist
ISO_DIR := iso

# Compiler flags
ASFLAGS := --64
CFLAGS := -ffreestanding -fno-stack-protector -fno-pic -mno-red-zone \
          -mno-mmx -mno-sse -mno-sse2 -mcmodel=kernel \
          -Wall -Wextra -Werror -O2 -g -I$(SRC_DIR)/kernel

LDFLAGS := -nostdlib -static -z max-page-size=0x1000 -T linker.ld

# Source files
BOOT_ASM := $(wildcard $(SRC_DIR)/boot/*.S)
KERNEL_ASM := $(wildcard $(SRC_DIR)/kernel/arch/x86_64/*.S)
KERNEL_C := $(wildcard $(SRC_DIR)/kernel/*.c) \
            $(wildcard $(SRC_DIR)/kernel/arch/x86_64/*.c) \
            $(wildcard $(SRC_DIR)/kernel/drivers/*.c) \
            $(wildcard $(SRC_DIR)/kernel/lib/*.c)

# Object files
BOOT_OBJ := $(patsubst $(SRC_DIR)/%.S,$(BUILD_DIR)/%.o,$(BOOT_ASM))
KERNEL_ASM_OBJ := $(patsubst $(SRC_DIR)/%.S,$(BUILD_DIR)/%.o,$(KERNEL_ASM))
KERNEL_C_OBJ := $(patsubst $(SRC_DIR)/%.c,$(BUILD_DIR)/%.o,$(KERNEL_C))

ALL_OBJ := $(BOOT_OBJ) $(KERNEL_ASM_OBJ) $(KERNEL_C_OBJ)

# Kernel binary
KERNEL := $(BUILD_DIR)/kernel.elf

# ISO image
ISO := $(DIST_DIR)/minimalos.iso

.PHONY: all clean kernel iso run

all: iso

# Build kernel
kernel: $(KERNEL)

$(KERNEL): $(ALL_OBJ)
	@echo "Linking kernel..."
	@mkdir -p $(dir $@)
	$(LD) $(LDFLAGS) -o $@ $^

# Compile assembly files (boot)
$(BUILD_DIR)/boot/%.o: $(SRC_DIR)/boot/%.S
	@echo "Assembling $<..."
	@mkdir -p $(dir $@)
	$(AS) $(ASFLAGS) -o $@ $<

# Compile assembly files (kernel)
$(BUILD_DIR)/kernel/%.o: $(SRC_DIR)/kernel/%.S
	@echo "Assembling $<..."
	@mkdir -p $(dir $@)
	$(AS) $(ASFLAGS) -o $@ $<

# Compile C files
$(BUILD_DIR)/kernel/%.o: $(SRC_DIR)/kernel/%.c
	@echo "Compiling $<..."
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c -o $@ $<

# Create ISO image
iso: $(ISO)

$(ISO): $(KERNEL)
	@echo "Creating ISO image..."
	@mkdir -p $(ISO_DIR)/boot/grub
	@cp $(KERNEL) $(ISO_DIR)/boot/
	@echo 'set timeout=0' > $(ISO_DIR)/boot/grub/grub.cfg
	@echo 'set default=0' >> $(ISO_DIR)/boot/grub/grub.cfg
	@echo '' >> $(ISO_DIR)/boot/grub/grub.cfg
	@echo 'menuentry "MinimalOS" {' >> $(ISO_DIR)/boot/grub/grub.cfg
	@echo '    multiboot2 /boot/kernel.elf' >> $(ISO_DIR)/boot/grub/grub.cfg
	@echo '    boot' >> $(ISO_DIR)/boot/grub/grub.cfg
	@echo '}' >> $(ISO_DIR)/boot/grub/grub.cfg
	@mkdir -p $(DIST_DIR)
	grub-mkrescue -o $(ISO) $(ISO_DIR) 2>/dev/null

# Run in QEMU
run: $(ISO)
	@echo "Starting QEMU..."
	qemu-system-x86_64 -cdrom $(ISO) -serial stdio

# Clean build artifacts
clean:
	@echo "Cleaning..."
	@rm -rf $(BUILD_DIR) $(ISO_DIR) $(DIST_DIR)

# Help
help:
	@echo "MinimalOS Build System"
	@echo ""
	@echo "Targets:"
	@echo "  all     - Build ISO image (default)"
	@echo "  kernel  - Build kernel binary only"
	@echo "  iso     - Create bootable ISO image"
	@echo "  run     - Build and run in QEMU"
	@echo "  clean   - Remove all build artifacts"
	@echo "  help    - Show this help message"
