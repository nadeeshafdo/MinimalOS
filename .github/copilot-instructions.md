# MinimalOS AI Development Guide

## System Architecture

This is a minimal educational x86-64 operating system demonstrating OS fundamentals from bootloader to userspace. The system follows a three-phase boot process:

1. **BIOS Bootloader** (`src/boot/boot.asm`): 16-bit real mode → 32-bit protected → 64-bit long mode transition
2. **Kernel** (1MB physical address): 64-bit kernel with hardware abstraction
3. **User Shell** (4MB physical): Simplified user space (currently kernel-space for educational purposes)

## Memory Layout & Build System

- **Image Structure**: 1.44MB floppy format with bootloader (sector 1), kernel (sectors 2-20), user space (21+)
- **Physical Memory**: Bootloader at 0x7C00, kernel at 0x100000 (1MB), user at 0x400000 (4MB)
- **Build Targets**: `make` (build), `make run` (QEMU), `make debug` (GDB), `make clean`
- **Cross-compilation**: Uses freestanding GCC with `-mcmodel=kernel -fno-pie -fno-pic -mno-red-zone`

## Development Patterns

### Assembly Integration

- **Mixed Assembly/C**: Assembly files use NASM ELF64 format, linked with C objects via `kernel.ld`
- **Calling Convention**: System V AMD64 ABI for C functions, inline assembly for hardware access
- **Critical Assembly**: Boot sequence, GDT/IDT setup, TSS management, syscall entry points

### Hardware Abstraction

- **Direct Hardware Access**: VGA at 0xB8000, keyboard via port 0x60, PIC at 0x20/0xA0
- **Driver Pattern**: Each driver (VGA, keyboard, etc.) in `src/kernel/arch/x86_64/` with `.h` interface
- **Interrupt Handling**: IDT setup in `idt.c`, handlers in assembly stubs, C callback functions

### System Calls

- **Fast Syscalls**: Uses SYSCALL/SYSRET instructions, not legacy INT 0x80
- **MSR Configuration**: LSTAR (0xC0000082) points to `syscall_entry`, STAR (0xC0000081) for segments
- **Minimal Interface**: SYS_READ (0) and SYS_WRITE (1) only, extendable pattern in `syscall.c`

## Code Conventions

### File Organization

- **Architecture Separation**: x86_64-specific code isolated in `src/kernel/arch/x86_64/`
- **Standard Headers**: Custom `stdint.h` and `stddef.h` in kernel root (no libc dependency)
- **Build Artifacts**: All outputs to `build/` directory, final image at `build/dist/os.img`

### Memory Management

- **No Dynamic Allocation**: Static memory only, no malloc/free
- **Direct VGA Access**: `volatile char *vga = (volatile char *)0xB8000` pattern
- **Stack Management**: Kernel stack setup in bootloader, user stack in TSS

### Error Handling

- **Boot Errors**: Print message and halt (see `disk_error` in `boot.asm`)
- **Runtime Errors**: Minimal error handling, focus on educational clarity over robustness
- **Debug Support**: GDB integration via `make debug` with remote target setup

## Key Integration Points

### Bootloader ↔ Kernel

- **Kernel Loading**: Bootloader reads 19 sectors from disk to 0x8000, copies to 1MB
- **Mode Transition**: Bootloader sets up initial page tables, enables long mode, jumps to `_kernel_entry`
- **Drive Detection**: Boot drive number preserved and used for loading

### Kernel ↔ Hardware

- **Interrupt Setup**: `setup_idt()` installs handlers, `setup_pic()` configures 8259 PIC
- **Keyboard Driver**: PS/2 keyboard via IRQ1, scancode translation in `keyboard.c`
- **VGA Driver**: Direct framebuffer manipulation with color attribute bytes

### Testing & Deployment

- **QEMU Testing**: Standard floppy disk emulation with serial output
- **Real Hardware**: USB boot compatible, auto-detects boot drive (floppy vs USB/HDD)
- **Debug Workflow**: GDB with QEMU's `-s -S` flags for remote debugging

## Extension Points

When adding features, follow these patterns:

- New drivers: Add to `src/kernel/arch/x86_64/` with header in same directory
- System calls: Add constants to `syscall.c`, extend `syscall_handler()` switch
- User programs: Link with `user.ld`, load at sector 21+ in image creation
- Interrupts: Add ISR stub in assembly, register in `setup_idt()`, implement handler in C

Focus on educational clarity over production complexity. This OS demonstrates core concepts rather than complete implementations.
