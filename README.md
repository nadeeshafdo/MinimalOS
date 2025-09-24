# MinimalOS

A minimal x86-64 operating system that demonstrates basic OS concepts including bootloader, kernel, interrupts, syscalls, and user space.

## Features

- **Custom BIOS Bootloader**: Transitions from 16-bit real mode to 64-bit long mode
- **Kernel Components**:
  - GDT (Global Descriptor Table) setup
  - IDT (Interrupt Descriptor Table) with keyboard interrupt handling
  - VGA text mode output
  - Basic paging support
  - System call interface
  - TSS (Task State Segment) for privilege level transitions
- **User Space**: Simple shell that accepts commands via syscalls
- **Architecture**: Modular design with x86_64-specific code separated

## Project Structure

```
.
├── Makefile              # Build system
├── kernel.ld             # Kernel linker script
├── user.ld               # User space linker script
├── LICENSE
├── README.md
└── src/
    ├── boot/
    │   └── boot.asm      # Custom BIOS bootloader (16-bit to long mode)
    ├── kernel/
    │   ├── arch/
    │   │   └── x86_64/
    │   │       ├── gdt.asm    # GDT setup
    │   │       ├── idt.c      # IDT and ISR setup
    │   │       ├── keyboard.c # Keyboard driver
    │   │       ├── paging.c   # Basic paging
    │   │       ├── tss.asm    # TSS for ring switches
    │   │       └── vga.c      # VGA text output
    │   ├── entry.asm     # Kernel entry point (64-bit)
    │   ├── main.c        # Kernel main, setup
    │   ├── syscall.c     # Syscall handler
    │   ├── stdint.h      # Standard integer types
    │   └── stddef.h      # Standard definitions
    └── user/
        └── shell.c       # User-space shell (uses syscalls)
```

## Building and Running

### Prerequisites

- `nasm` (Netwide Assembler)
- `gcc` (GNU C Compiler)
- `ld` (GNU Linker)
- `qemu-system-x86_64` (for testing)

### Build

```bash
make
```

This creates:
- `build/boot/boot.bin` - Bootloader
- `build/kernel/kernel.bin` - Kernel binary
- `build/user/shell.bin` - User shell binary
- `build/dist/os.img` - Complete bootable floppy disk image

### Run in QEMU

```bash
make run
```

### Debug with GDB

```bash
make debug
```

### Clean

```bash
make clean
```

## Image Layout

The OS image is a raw 1.44MB floppy disk with the following layout:

- **Sector 1**: Bootloader (512 bytes)
- **Sectors 2-20**: Kernel (up to 19 sectors = 9728 bytes)
- **Sectors 21+**: User space shell

## Boot Process

1. **BIOS** loads bootloader from sector 1 to 0x7C00
2. **Bootloader** enables A20 line, loads kernel from disk, sets up paging for long mode, transitions to 64-bit mode
3. **Kernel** initializes VGA, GDT, IDT, keyboard, syscalls, and enters a simple interactive loop
4. **User Shell** (future enhancement) will run in ring 3 using syscalls for I/O

## System Calls

Currently implemented syscalls:
- `SYS_READ (0)`: Read character from keyboard
- `SYS_WRITE (1)`: Write string to VGA display

## Limitations

This is a minimal educational OS with many limitations:
- No file system
- No process management
- No memory management beyond basic paging
- No networking
- Single-tasking
- Limited to x86-64 architecture

## License

MIT License - see LICENSE file for details.