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
	$(KERNELDIR)/arch/x86_64/isr_stubs.asm

# Source files - C
C_SOURCES := \
	$(KERNELDIR)/kernel.c \
	$(KERNELDIR)/multiboot2.c \
	$(KERNELDIR)/arch/x86_64/idt.c \
	$(KERNELDIR)/arch/x86_64/pic.c \
	$(KERNELDIR)/mm/pmm.c \
	$(KERNELDIR)/mm/kheap.c \
	drivers/timer.c \
	drivers/keyboard.c

# Object files
ASM_OBJS := $(patsubst %.asm,$(BUILDDIR)/%.o,$(ASM_SOURCES))
C_OBJS := $(patsubst %.c,$(BUILDDIR)/%.o,$(C_SOURCES))

ALL_OBJS := $(ASM_OBJS) $(C_OBJS)

.PHONY: all clean run iso dirs debug

all: dirs $(KERNEL)

dirs:
	@mkdir -p $(DISTDIR)
	@mkdir -p $(BUILDDIR)/$(ARCHDIR)
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)/arch/x86_64
	@mkdir -p $(BUILDDIR)/$(KERNELDIR)/mm
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

run: iso
	qemu-system-x86_64 -cdrom $(ISO) -m 256M

debug: iso
	qemu-system-x86_64 -cdrom $(ISO) -m 256M -d int -no-reboot

clean:
	@rm -rf $(BUILDDIR)
	@echo "Clean complete"
