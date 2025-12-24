# MinimalOS 64-bit Build System

# Architecture
ARCH := x86_64

# Toolchain (try cross-compiler first, fall back to native)
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

# Compiler flags (64-bit, freestanding, no red zone)
CFLAGS := -ffreestanding -mcmodel=large -mno-red-zone -mno-mmx -mno-sse -mno-sse2
CFLAGS += -fno-stack-protector -fno-pic -fno-pie
CFLAGS += -Wall -Wextra -O2 -g

# Assembler flags
ASFLAGS := -f elf64

# Linker flags
LDFLAGS := -n -nostdlib -T $(ARCHDIR)/linker.ld

# Source files
ASM_SOURCES := $(ARCHDIR)/boot.asm
C_SOURCES := $(KERNELDIR)/kernel.c

# Object files
ASM_OBJS := $(patsubst %.asm,$(BUILDDIR)/%.o,$(ASM_SOURCES))
C_OBJS := $(patsubst %.c,$(BUILDDIR)/%.o,$(C_SOURCES))
ALL_OBJS := $(ASM_OBJS) $(C_OBJS)

# Phony targets
.PHONY: all clean run iso dirs

# Default target
all: dirs $(KERNEL)

# Create directories
dirs:
	@mkdir -p $(DISTDIR)
	@mkdir -p $(BUILDDIR)/$(ARCHDIR)
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)

# Link kernel
$(KERNEL): $(ALL_OBJS)
	@echo "Linking kernel..."
	$(LD) $(LDFLAGS) -o $@ $(ALL_OBJS)
	@echo "Kernel built: $@"

# Assemble .asm files
$(BUILDDIR)/%.o: %.asm
	@echo "Assembling $<..."
	$(AS) $(ASFLAGS) $< -o $@

# Compile C files
$(BUILDDIR)/%.o: %.c
	@echo "Compiling $<..."
	$(CC) $(CFLAGS) -c $< -o $@

# Create bootable ISO
iso: $(KERNEL)
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

# Run in QEMU (64-bit)
run: iso
	qemu-system-x86_64 -cdrom $(ISO) -m 256M

# Debug in QEMU
debug: iso
	qemu-system-x86_64 -cdrom $(ISO) -m 256M -d int,cpu_reset -no-reboot

# Clean
clean:
	@rm -rf $(BUILDDIR)
	@echo "Clean complete"

# Help
help:
	@echo "MinimalOS 64-bit Build System"
	@echo ""
	@echo "  make       - Build kernel"
	@echo "  make run   - Run in QEMU"
	@echo "  make iso   - Create bootable ISO"
	@echo "  make clean - Clean build"
	@echo ""
	@echo "Compiler: $(CC)"
	@echo "Assembler: $(AS)"
