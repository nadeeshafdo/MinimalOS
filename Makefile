# Tools
ASM = nasm
CC = gcc
LD = ld

# Directories
SRC_DIR = src
BUILD_DIR = build
BOOT_BUILD_DIR = $(BUILD_DIR)/boot
KERNEL_BUILD_DIR = $(BUILD_DIR)/kernel
USER_BUILD_DIR = $(BUILD_DIR)/user
DIST_DIR = $(BUILD_DIR)/dist

# Files
BOOTLOADER_SRC = $(SRC_DIR)/boot/boot.asm
BOOTLOADER_BIN = $(BOOT_BUILD_DIR)/boot.bin

KERNEL_SOURCES = $(SRC_DIR)/kernel/entry.asm $(SRC_DIR)/kernel/main.c \
                 $(SRC_DIR)/kernel/arch/x86_64/gdt.asm $(SRC_DIR)/kernel/arch/x86_64/idt.c \
                 $(SRC_DIR)/kernel/arch/x86_64/keyboard.c $(SRC_DIR)/kernel/arch/x86_64/paging.c \
                 $(SRC_DIR)/kernel/arch/x86_64/tss.asm $(SRC_DIR)/kernel/arch/x86_64/vga.c \
                 $(SRC_DIR)/kernel/syscall.c $(SRC_DIR)/user/shell.c

KERNEL_ASM_SOURCES = $(filter %.asm,$(KERNEL_SOURCES))
KERNEL_C_SOURCES = $(filter %.c,$(KERNEL_SOURCES))
KERNEL_ASM_OBJS = $(KERNEL_ASM_SOURCES:$(SRC_DIR)/%.asm=$(KERNEL_BUILD_DIR)/%.o)
KERNEL_C_OBJS = $(KERNEL_C_SOURCES:$(SRC_DIR)/%.c=$(KERNEL_BUILD_DIR)/%.o)
KERNEL_OBJS = $(KERNEL_ASM_OBJS) $(KERNEL_C_OBJS)
KERNEL_ELF = $(KERNEL_BUILD_DIR)/kernel.elf
KERNEL_BIN = $(KERNEL_BUILD_DIR)/kernel.bin

USER_SOURCES = $(SRC_DIR)/user/shell.c
USER_OBJS = $(USER_SOURCES:$(SRC_DIR)/%.c=$(USER_BUILD_DIR)/%.o)
USER_BIN = $(USER_BUILD_DIR)/shell.bin

OS_IMAGE = $(DIST_DIR)/os.img

# Flags
ASMFLAGS = -f bin
ASM_ELF_FLAGS = -f elf64
CFLAGS = -m64 -ffreestanding -nostdlib -mcmodel=kernel -fno-pie -fno-pic -fno-stack-protector -O2 -mno-red-zone -I$(SRC_DIR)/kernel
LDFLAGS = -T kernel.ld

# Default
all: $(OS_IMAGE)

# Directories
$(BUILD_DIR) $(BOOT_BUILD_DIR) $(KERNEL_BUILD_DIR) $(USER_BUILD_DIR) $(DIST_DIR):
	mkdir -p $@

# Bootloader
$(BOOTLOADER_BIN): $(BOOTLOADER_SRC) | $(BOOT_BUILD_DIR)
	$(ASM) $(ASMFLAGS) $< -o $@

# Kernel ASM objects
$(KERNEL_BUILD_DIR)/%.o: $(SRC_DIR)/%.asm | $(KERNEL_BUILD_DIR)
	mkdir -p $(dir $@)
	$(ASM) $(ASM_ELF_FLAGS) $< -o $@

# Kernel C objects
$(KERNEL_BUILD_DIR)/%.o: $(SRC_DIR)/%.c | $(KERNEL_BUILD_DIR)
	mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

# Link kernel
$(KERNEL_ELF): $(KERNEL_OBJS) kernel.ld | $(KERNEL_BUILD_DIR)
	$(LD) $(LDFLAGS) $(KERNEL_OBJS) -o $@

$(KERNEL_BIN): $(KERNEL_ELF)
	objcopy -O binary $< $@

# User objects
$(USER_BUILD_DIR)/%.o: $(SRC_DIR)/%.c | $(USER_BUILD_DIR)
	mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

# Link user shell as flat binary
$(USER_BIN): $(USER_OBJS) user.ld
	$(LD) -T user.ld $(USER_OBJS) -o $@ --oformat binary

# Create image (allocate 1.44MB, bootloader sector 1, kernel sectors 2-20)
$(OS_IMAGE): $(BOOTLOADER_BIN) $(KERNEL_BIN) | $(DIST_DIR)
	dd if=/dev/zero of=$@ bs=512 count=2880
	dd if=$(BOOTLOADER_BIN) of=$@ bs=512 count=1 conv=notrunc
	dd if=$(KERNEL_BIN) of=$@ bs=512 seek=1 count=19 conv=notrunc

# Run in QEMU
run: $(OS_IMAGE)
	qemu-system-x86_64 -drive file=$(OS_IMAGE),format=raw,if=floppy -serial stdio

# Debug
debug: $(OS_IMAGE)
	qemu-system-x86_64 -drive file=$(OS_IMAGE),format=raw,if=floppy -serial stdio -s -S &
	gdb -ex "target remote localhost:1234" -ex "set architecture i386:x86-64"

# Clean
clean:
	rm -rf $(BUILD_DIR)