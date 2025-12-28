# Makefile for MinimalOS
# Targets: clean, all, kernel, userspace, img, disk

# Tools
CC := gcc
ASM := nasm
LD := ld
GRUB_MKRESCUE := grub-mkrescue

# Directories
BUILD_DIR := build
DIST_DIR := dist
SRC_DIR := src

# Output
KERNEL_BIN := $(BUILD_DIR)/kernel.elf
ISO_IMAGE := $(DIST_DIR)/minimalos.iso

# Compiler Flags
# -mno-red-zone is CRITICAL for specific x86_64 kernel interrupt handling
CFLAGS := -ffreestanding -fno-stack-protector -fno-pic -fno-pie \
          -mno-red-zone -mcmodel=kernel -m64 -O2 -Wall -Wextra \
          -I$(SRC_DIR)/kernel/include

# Linker Flags
LDFLAGS := -n -nostdlib -T linker.ld -z max-page-size=0x1000

# Assembler Flags
ASMFLAGS := -f elf64

# Sources
KERNEL_C_SOURCES := $(shell find $(SRC_DIR)/kernel -name '*.c')
KERNEL_ASM_SOURCES := $(shell find $(SRC_DIR)/boot $(SRC_DIR)/kernel -name '*.S')
KERNEL_OBJECTS := $(patsubst $(SRC_DIR)/%.c, $(BUILD_DIR)/%.o, $(KERNEL_C_SOURCES)) \
                  $(patsubst $(SRC_DIR)/%.S, $(BUILD_DIR)/%.o, $(KERNEL_ASM_SOURCES))

# Targets

.PHONY: all clean kernel userspace img disk dirs

all: dirs kernel userspace

dirs:
	@mkdir -p $(BUILD_DIR)
	@mkdir -p $(DIST_DIR)
	@mkdir -p $(BUILD_DIR)/boot
	@mkdir -p $(BUILD_DIR)/kernel
	@mkdir -p $(BUILD_DIR)/userspace

# Kernel Build
kernel: dirs $(KERNEL_BIN)

$(KERNEL_BIN): $(KERNEL_OBJECTS)
	$(LD) $(LDFLAGS) -o $@ $(KERNEL_OBJECTS)

# C Compilation
$(BUILD_DIR)/%.o: $(SRC_DIR)/%.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

# ASM Compilation
$(BUILD_DIR)/%.o: $(SRC_DIR)/%.S
	@mkdir -p $(dir $@)
	$(ASM) $(ASMFLAGS) $< -o $@

# Userspace Build (Placeholder)
userspace: dirs
	@echo "Building userspace..."
	# TODO: Implement userspace build rules

# ISO Generation
img: kernel
	@mkdir -p $(BUILD_DIR)/isodir/boot/grub
	cp $(KERNEL_BIN) $(BUILD_DIR)/isodir/boot/
	
	# Generate grub.cfg
	@echo 'set timeout=0' > $(BUILD_DIR)/isodir/boot/grub/grub.cfg
	@echo 'set default=0' >> $(BUILD_DIR)/isodir/boot/grub/grub.cfg
	@echo '' >> $(BUILD_DIR)/isodir/boot/grub/grub.cfg
	@echo 'menuentry "MinimalOS" {' >> $(BUILD_DIR)/isodir/boot/grub/grub.cfg
	@echo '    multiboot2 /boot/kernel.elf' >> $(BUILD_DIR)/isodir/boot/grub/grub.cfg
	@echo '    boot' >> $(BUILD_DIR)/isodir/boot/grub/grub.cfg
	@echo '}' >> $(BUILD_DIR)/isodir/boot/grub/grub.cfg
	
	$(GRUB_MKRESCUE) -o $(ISO_IMAGE) $(BUILD_DIR)/isodir

# Disk Image (Placeholder)
disk: img
	@echo "ISO generated at $(ISO_IMAGE). Write this to USB to boot."

clean:
	rm -rf $(BUILD_DIR) $(DIST_DIR)
