# MinimalOS - Production Shell OS with Multiboot/GRUB Support
# Architecture: i386 (32-bit)
# Bootloader: QEMU Multiboot (GRUB-compatible)

# Tools
ASM = nasm
CC = gcc
LD = ld

# Directories
SRC_DIR = src
BUILD_DIR = build

# Source files
MULTIBOOT_ASM = $(SRC_DIR)/boot/multiboot.asm
KERNEL_MAIN = $(SRC_DIR)/kernel/main.c

# Output files
KERNEL_OBJS = $(BUILD_DIR)/multiboot.o $(BUILD_DIR)/main.o
KERNEL_BIN = $(BUILD_DIR)/minimalos.bin

# Compiler flags for optimized 32-bit freestanding kernel
CFLAGS = -m32 -ffreestanding -O2 -Wall -Wextra -fno-exceptions -nostdlib

# Linker script
LDSCRIPT = kernel.ld

.PHONY: all clean run run-term info

all: $(KERNEL_BIN)
	@echo ""
	@echo "======================================================================"
	@echo "           MinimalOS - Production Shell OS"
	@echo "======================================================================"
	@echo "Build Status: ✅ SUCCESS"
	@echo ""
	@echo "Binary: $(KERNEL_BIN) ($(shell ls -lh $(KERNEL_BIN) | awk '{print $$5}'))"
	@echo "Architecture: i386 (32-bit Protected Mode)"
	@echo "Bootloader: Multiboot (QEMU/GRUB compatible)"
	@echo ""
	@echo "Run with:"
	@echo "  make run       - Launch in QEMU GUI"
	@echo "  make run-term  - Launch in terminal (ncurses)"
	@echo "======================================================================"
	@echo ""

# Create build directory
$(BUILD_DIR):
	@mkdir -p $(BUILD_DIR)

# Assemble multiboot stub
$(BUILD_DIR)/multiboot.o: $(MULTIBOOT_ASM) | $(BUILD_DIR)
	@echo "[ASM] $<"
	@$(ASM) -f elf32 $< -o $@

# Compile kernel
$(BUILD_DIR)/main.o: $(KERNEL_MAIN) | $(BUILD_DIR)
	@echo "[CC]  $<"
	@$(CC) $(CFLAGS) -c $< -o $@

# Link kernel
$(KERNEL_BIN): $(KERNEL_OBJS) $(LDSCRIPT)
	@echo "[LD]  $@"
	@$(LD) -m elf_i386 -T $(LDSCRIPT) -o $@ $(KERNEL_OBJS)

# Run with QEMU GUI
run: $(KERNEL_BIN)
	@echo "Starting MinimalOS in QEMU..."
	@qemu-system-i386 -kernel $(KERNEL_BIN)

# Run with terminal output
run-term: $(KERNEL_BIN)
	@echo "Starting MinimalOS in QEMU (terminal mode)..."
	@qemu-system-i386 -kernel $(KERNEL_BIN) -display curses

# Show build info
info:
	@echo "MinimalOS - Build Information"
	@echo "Kernel Binary: $(KERNEL_BIN)"
	@echo "Source Files:"
	@echo "  - $(MULTIBOOT_ASM)"
	@echo "  - $(KERNEL_MAIN)"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@rm -rf $(BUILD_DIR)
	@echo "Clean complete."
