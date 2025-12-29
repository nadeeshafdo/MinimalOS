# MinimalOS Makefile
# Cross-compiler toolchain for x86_64-elf target

# Toolchain (adjust if using different prefix)
AS = nasm
CC = x86_64-elf-gcc
LD = x86_64-elf-ld
OBJCOPY = x86_64-elf-objcopy

# If cross-compiler not found, try native gcc with appropriate flags
ifeq ($(shell which $(CC) 2>/dev/null),)
    CC = gcc
    LD = ld
    OBJCOPY = objcopy
endif

# Directories
SRC_DIR = src
BUILD_DIR = build
ISO_DIR = $(BUILD_DIR)/isofiles

# Compiler flags
CFLAGS = -ffreestanding -fno-stack-protector -fno-pic -mno-red-zone \
         -mcmodel=kernel -mno-mmx -mno-sse -mno-sse2 \
         -Wall -Wextra -Werror -std=c11 -O2 -g \
         -I$(SRC_DIR) -Iinclude

ASFLAGS = -f elf64 -g -F dwarf

LDFLAGS = -n -nostdlib -T linker.ld

# Source files
ASM_SOURCES = $(SRC_DIR)/boot/multiboot2.asm \
              $(SRC_DIR)/boot/gdt.asm \
              $(SRC_DIR)/boot/long_mode.asm \
              $(SRC_DIR)/arch/x86_64/isr_stubs.asm \
              $(SRC_DIR)/arch/x86_64/context.asm \
              $(SRC_DIR)/arch/x86_64/syscall_entry.asm

C_SOURCES = $(SRC_DIR)/kernel/main.c \
            $(SRC_DIR)/kernel/panic.c \
            $(SRC_DIR)/kernel/printk.c \
            $(SRC_DIR)/kernel/multiboot2.c \
            $(SRC_DIR)/kernel/task.c \
            $(SRC_DIR)/arch/x86_64/cpu.c \
            $(SRC_DIR)/arch/x86_64/idt.c \
            $(SRC_DIR)/arch/x86_64/isr.c \
            $(SRC_DIR)/arch/x86_64/syscall.c \
            $(SRC_DIR)/arch/x86_64/apic.c \
            $(SRC_DIR)/arch/x86_64/pic.c \
            $(SRC_DIR)/arch/x86_64/timer.c \
            $(SRC_DIR)/drivers/serial.c \
            $(SRC_DIR)/drivers/vga.c \
            $(SRC_DIR)/drivers/framebuffer.c \
            $(SRC_DIR)/mm/pmm.c \
            $(SRC_DIR)/mm/vmm.c \
            $(SRC_DIR)/mm/heap.c

# Object files
ASM_OBJECTS = $(patsubst $(SRC_DIR)/%.asm,$(BUILD_DIR)/%.o,$(ASM_SOURCES))
C_OBJECTS = $(patsubst $(SRC_DIR)/%.c,$(BUILD_DIR)/%.o,$(C_SOURCES))
OBJECTS = $(ASM_OBJECTS) $(C_OBJECTS)

# Output files
KERNEL = $(BUILD_DIR)/kernel.elf
ISO = $(BUILD_DIR)/minimalos.iso

# Phony targets
.PHONY: all clean run run-debug iso dirs

# Default target
all: dirs $(ISO)

# Create build directories
dirs:
	@mkdir -p $(BUILD_DIR)/boot
	@mkdir -p $(BUILD_DIR)/kernel
	@mkdir -p $(BUILD_DIR)/arch/x86_64
	@mkdir -p $(BUILD_DIR)/drivers
	@mkdir -p $(BUILD_DIR)/mm
	@mkdir -p $(ISO_DIR)/boot/grub

# Compile assembly files
$(BUILD_DIR)/%.o: $(SRC_DIR)/%.asm
	@mkdir -p $(dir $@)
	$(AS) $(ASFLAGS) $< -o $@

# Compile C files
$(BUILD_DIR)/%.o: $(SRC_DIR)/%.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

# Link kernel
$(KERNEL): $(OBJECTS) linker.ld
	$(LD) $(LDFLAGS) -o $@ $(OBJECTS)

# Create ISO
$(ISO): $(KERNEL) grub.cfg
	cp $(KERNEL) $(ISO_DIR)/boot/kernel.elf
	cp grub.cfg $(ISO_DIR)/boot/grub/grub.cfg
	grub-mkrescue -o $@ $(ISO_DIR) 2>/dev/null || \
		grub2-mkrescue -o $@ $(ISO_DIR) 2>/dev/null

# Run in QEMU
run: $(ISO)
	qemu-system-x86_64 -cdrom $(ISO) -serial stdio -m 256M

# Run with GDB debugging
run-debug: $(ISO)
	qemu-system-x86_64 -cdrom $(ISO) -serial stdio -m 256M -s -S &
	@echo "GDB server started on localhost:1234"
	@echo "Run: gdb $(KERNEL) -ex 'target remote :1234'"

# Run with multiple CPUs (SMP)
run-smp: $(ISO)
	qemu-system-x86_64 -cdrom $(ISO) -serial stdio -m 256M -smp 4

# Clean build artifacts
clean:
	rm -rf $(BUILD_DIR)

# Display help
help:
	@echo "MinimalOS Build System"
	@echo ""
	@echo "Targets:"
	@echo "  all        - Build kernel and ISO (default)"
	@echo "  run        - Build and run in QEMU"
	@echo "  run-debug  - Run with GDB server"
	@echo "  run-smp    - Run with 4 CPUs"
	@echo "  clean      - Remove build artifacts"
	@echo "  help       - Show this help"
