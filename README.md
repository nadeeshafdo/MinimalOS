# MinimalOS - 64-bit Long Mode

A modern 64-bit x86_64 operating system built from scratch.

## Target Architecture

- **64-bit Long Mode** (x86_64)
- **Multiboot2** bootloader protocol
- **Higher-half kernel** (virtual address 0xFFFFFFFF80000000+)
- **4-level paging** (PML4)
- **Modern syscall/sysret** interface

## Development Phases

| Phase | Status | Description |
|-------|--------|-------------|
| 1. Boot to Long Mode | 🔲 | Multiboot2, 32→64 transition |
| 2. Core Init | 🔲 | GDT64, IDT64, basic paging |
| 3. Interrupts | 🔲 | PIC/APIC, timer, keyboard |
| 4. Memory | 🔲 | PMM, higher-half paging |
| 5. Processes | 🔲 | Scheduler, TSS, Ring 0→3 |
| 6. Syscalls | 🔲 | syscall/sysret interface |
| 7. File System | 🔲 | initrd, VFS, FAT32 |
| 8. Userspace | 🔲 | ELF64 loader, init process |
| 9. Shell | 🔲 | Userspace shell |

## Building

```bash
make        # Build kernel
make run    # Run in QEMU
make clean  # Clean build
```

## Requirements

- x86_64-elf cross-compiler (or x86_64-linux-gnu-gcc)
- NASM or GNU as
- QEMU with x86_64 support
- grub-mkrescue (for ISO)

## License

Educational project.
