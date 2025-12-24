# MinimalOS 64-bit Build System

ARCH := x86_64

# Toolchain
CC := $(shell which x86_64-elf-gcc 2>/dev/null || which x86_64-linux-gnu-gcc 2>/dev/null || echo gcc)
AS := nasm
LD := $(shell which x86_64-elf-ld 2>/dev/null || which x86_64-linux-gnu-ld 2>/dev/null || echo ld)

# Directories
BUILDDIR := build
DISTDIR := $(BUILDDIR)/dist
ARCHDIR := arch/$(ARCH)
KERNELDIR := kernel

# Output
KERNEL := $(DISTDIR)/kernel.bin
ISO := $(DISTDIR)/minimalos.iso

# Compiler flags
CFLAGS := -ffreestanding -mcmodel=large -mno-red-zone -mno-mmx -mno-sse -mno-sse2
CFLAGS += -fno-stack-protector -fno-pic -fno-pie
CFLAGS += -Wall -Wextra -O2 -g
CFLAGS += -I$(KERNELDIR)/include

# Assembler flags
ASFLAGS := -f elf64

# Linker flags
LDFLAGS := -n -nostdlib -T $(ARCHDIR)/linker.ld

# Source files - ASM
ASM_SOURCES := \
	$(ARCHDIR)/boot.asm \
	$(KERNELDIR)/arch/x86_64/isr_stubs.asm \
	$(KERNELDIR)/arch/x86_64/switch.asm \
	$(KERNELDIR)/arch/x86_64/syscall.asm

# Source files - C
C_SOURCES := \
	$(KERNELDIR)/kernel.c \
	$(KERNELDIR)/multiboot2.c \
	$(KERNELDIR)/syscall.c \
	$(KERNELDIR)/arch/x86_64/idt.c \
	$(KERNELDIR)/arch/x86_64/pic.c \
	$(KERNELDIR)/mm/pmm.c \
	$(KERNELDIR)/mm/kheap.c \
	$(KERNELDIR)/process/process.c \
	$(KERNELDIR)/process/scheduler.c \
	$(KERNELDIR)/fs/vfs.c \
	$(KERNELDIR)/fs/initrd.c \
	$(KERNELDIR)/fs/demo_initrd.c \
	drivers/timer.c \
	drivers/keyboard.c \
	drivers/serial.c

# Object files
ASM_OBJS := $(patsubst %.asm,$(BUILDDIR)/%.o,$(ASM_SOURCES))
C_OBJS := $(patsubst %.c,$(BUILDDIR)/%.o,$(C_SOURCES))

ALL_OBJS := $(ASM_OBJS) $(C_OBJS)

.PHONY: all clean run run-serial iso dirs debug help

# Default target
all: dirs $(KERNEL) ## Build the kernel

# Show help
help: ## Show available make targets
	@echo 'MinimalOS Build System'
	@echo ''
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Targets:'
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

dirs:
	@mkdir -p $(DISTDIR)
	@mkdir -p $(BUILDDIR)/$(ARCHDIR)
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)/arch/x86_64
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)/mm
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)/process
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)/fs
	@mkdir -p $(BUILDDIR)/drivers

$(KERNEL): $(ALL_OBJS)
	@echo "Linking kernel..."
	$(LD) $(LDFLAGS) -o $@ $(ALL_OBJS)
	@echo "Kernel built: $@"

$(BUILDDIR)/%.o: %.asm
	@echo "Assembling $<..."
	$(AS) $(ASFLAGS) $< -o $@

$(BUILDDIR)/%.o: %.c
	@echo "Compiling $<..."
	$(CC) $(CFLAGS) -c $< -o $@

iso: $(KERNEL) ## Create bootable ISO image
	@echo "Creating ISO..."
	@mkdir -p $(BUILDDIR)/iso/boot/grub
	@cp $(KERNEL) $(BUILDDIR)/iso/boot/
	@echo 'set timeout=0' > $(BUILDDIR)/iso/boot/grub/grub.cfg
	@echo 'set default=0' >> $(BUILDDIR)/iso/boot/grub/grub.cfg
	@echo 'menuentry "MinimalOS 64-bit" {' >> $(BUILDDIR)/iso/boot/grub/grub.cfg
	@echo '    multiboot2 /boot/kernel.bin' >> $(BUILDDIR)/iso/boot/grub/grub.cfg
	@echo '    boot' >> $(BUILDDIR)/iso/boot/grub/grub.cfg
	@echo '}' >> $(BUILDDIR)/iso/boot/grub/grub.cfg
	grub-mkrescue -o $(ISO) $(BUILDDIR)/iso 2>/dev/null
	@echo "ISO created: $(ISO)"

run: iso ## Run in QEMU
	qemu-system-x86_64 -cdrom $(ISO) -m 256M

run-serial: iso ## Run in QEMU with serial output to terminal
	qemu-system-x86_64 -cdrom $(ISO) -m 256M -serial stdio

debug: iso ## Run in QEMU with debug output and GDB server
	qemu-system-x86_64 -cdrom $(ISO) -m 256M -serial stdio -s -S

clean: ## Remove build artifacts
	@rm -rf $(BUILDDIR)
	@echo "Clean complete"
