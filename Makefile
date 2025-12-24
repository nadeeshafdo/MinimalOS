# Build configuration
ARCH ?= i386
HOST ?= $(ARCH)-elf

# Try to detect cross-compiler, fall back to system compiler with freestanding flags
CC := $(shell which $(HOST)-gcc 2>/dev/null || echo gcc)
AS := $(shell which $(HOST)-as 2>/dev/null || echo as)
LD := $(shell which $(HOST)-gcc 2>/dev/null || echo gcc)

# Build directories
BUILDDIR := build
DISTDIR := $(BUILDDIR)/dist

# Compiler flags
CFLAGS := -std=gnu99 -ffreestanding -O2 -Wall -Wextra -Werror=implicit-function-declaration
CFLAGS += -Ikernel/include -m32 -nostdlib -fno-pie -fno-stack-protector

# Assembler flags
ASFLAGS := --32

# Linker flags
LDFLAGS := -m32 -ffreestanding -nostdlib -T arch/$(ARCH)/linker.ld

# Directories
ARCHDIR := arch/$(ARCH)
KERNELDIR := kernel
DRIVERDIR := drivers
LIBCDIR := libc

# Output files
KERNEL := $(DISTDIR)/minimalos.bin
ISO := $(DISTDIR)/minimalos.iso

# Source files
KERNEL_SOURCES := \
	$(KERNELDIR)/kernel.c \
	$(KERNELDIR)/tty.c \
	$(KERNELDIR)/arch/i386/gdt.c \
	$(KERNELDIR)/arch/i386/idt.c \
	$(KERNELDIR)/arch/i386/isr.c \
	$(KERNELDIR)/arch/i386/irq.c \
	$(KERNELDIR)/mm/pmm.c \
	$(KERNELDIR)/mm/paging.c \
	$(KERNELDIR)/mm/kheap.c \
	$(KERNELDIR)/process/process.c \
	$(KERNELDIR)/process/scheduler.c \
	$(KERNELDIR)/process/syscall.c \
	$(KERNELDIR)/shell.c \
	$(KERNELDIR)/commands/utils.c \
	$(KERNELDIR)/commands/basic.c \
	$(KERNELDIR)/commands/sysinfo.c \
	$(KERNELDIR)/commands/memory.c \
	$(KERNELDIR)/commands/display.c \
	$(KERNELDIR)/commands/tests.c

DRIVER_SOURCES := \
	$(DRIVERDIR)/timer.c \
	$(DRIVERDIR)/keyboard.c

ARCH_ASM_SOURCES := \
	$(ARCHDIR)/boot.s \
	$(KERNELDIR)/arch/i386/gdt_flush.s \
	$(KERNELDIR)/arch/i386/idt_flush.s \
	$(KERNELDIR)/arch/i386/isr_stub.s \
	$(KERNELDIR)/arch/i386/irq_stub.s \
	$(KERNELDIR)/arch/i386/switch.s \
	$(KERNELDIR)/arch/i386/usermode.s

# Object files (in build directory)
KERNEL_OBJS := $(patsubst %.c,$(BUILDDIR)/%.o,$(KERNEL_SOURCES))
DRIVER_OBJS := $(patsubst %.c,$(BUILDDIR)/%.o,$(DRIVER_SOURCES))
ARCH_OBJS := $(patsubst %.s,$(BUILDDIR)/%.o,$(ARCH_ASM_SOURCES))

ALL_OBJS := $(ARCH_OBJS) $(KERNEL_OBJS) $(DRIVER_OBJS)

# Phony targets
.PHONY: all clean iso qemu qemu-iso qemu-debug run help dirs

# Default target
all: dirs $(KERNEL)

# Create build directories
dirs:
	@mkdir -p $(DISTDIR)
	@mkdir -p $(BUILDDIR)/$(ARCHDIR)
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)/arch/i386
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)/mm
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)/process
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)/commands
	@mkdir -p $(BUILDDIR)/$(DRIVERDIR)

# Link kernel
$(KERNEL): $(ALL_OBJS)
	@echo "Linking kernel..."
	$(LD) $(LDFLAGS) -o $@ $(ALL_OBJS)
	@echo "Kernel built successfully: $@"

# Compile C sources
$(BUILDDIR)/%.o: %.c
	@echo "Compiling $<..."
	$(CC) $(CFLAGS) -c $< -o $@

# Assemble sources
$(BUILDDIR)/%.o: %.s
	@echo "Assembling $<..."
	$(AS) $(ASFLAGS) $< -o $@

# Create bootable ISO
iso: $(KERNEL)
	@echo "Creating bootable ISO..."
	@mkdir -p $(BUILDDIR)/iso/boot/grub
	@cp $(KERNEL) $(BUILDDIR)/iso/boot/
	@cp iso/boot/grub/grub.cfg $(BUILDDIR)/iso/boot/grub/ 2>/dev/null || \
		echo "Note: No grub.cfg found, create iso/boot/grub/grub.cfg first"
	@grub-mkrescue -o $(ISO) $(BUILDDIR)/iso 2>/dev/null || \
		(echo "Error: grub-mkrescue not found. Install grub-pc-bin or grub-common"; exit 1)
	@echo "ISO created: $(ISO)"

# Run in QEMU (kernel only)
qemu: $(KERNEL)
	qemu-system-i386 -kernel $(KERNEL) -serial stdio

# Run ISO in QEMU
qemu-iso: iso
	qemu-system-i386 -cdrom $(ISO) -serial stdio

# Run in QEMU with debugging
qemu-debug: $(KERNEL)
	qemu-system-i386 -kernel $(KERNEL) -serial stdio -d int,cpu_reset -no-reboot

# Convenient alias for running the OS
run: qemu

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@rm -rf $(BUILDDIR)
	@echo "Clean complete"

# Help target
help:
	@echo "MinimalOS Build System"
	@echo "====================="
	@echo ""
	@echo "Targets:"
	@echo "  make          - Build kernel binary"
	@echo "  make run      - Run kernel in QEMU (alias for 'make qemu')"
	@echo "  make iso      - Create bootable ISO image"
	@echo "  make qemu     - Run kernel in QEMU"
	@echo "  make qemu-iso - Run ISO in QEMU"
	@echo "  make clean    - Remove build artifacts"
	@echo ""
	@echo "Output directory: $(DISTDIR)/"
	@echo "Compiler: $(CC)"
	@echo "Assembler: $(AS)"
