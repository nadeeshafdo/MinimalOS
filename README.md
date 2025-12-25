# MinimalOS

A multitasking operating system for x86_64 architecture with microservices architecture.

## Features
- GRUB Multiboot2 bootloader
- 64-bit long mode
- GDT/IDT with exception handling
- Serial and VGA drivers
- Modular microservices architecture

## Build Requirements
- x86_64-elf-gcc cross-compiler
- x86_64-elf-binutils
- GRUB
- xorriso (for ISO creation)
- QEMU (for testing)

## Building
```bash
make          # Build ISO image
make run      # Build and run in QEMU
make clean    # Clean build artifacts
```

## Project Structure
```
MinimalOS/
├── src/
│   ├── boot/          # Bootloader code
│   └── kernel/        # Kernel code
├── build/             # Build outputs
├── dist/              # Distribution ISO
└── Makefile           # Build system
```

## Status
- [x] Bootloader (Multiboot2)
- [x] 64-bit mode transition
- [x] GDT/IDT setup
- [x] Serial/VGA drivers
- [x] Basic kernel initialization
- [ ] Memory management
- [ ] Process management
- [ ] Filesystem
- [ ] Shell

## License
MIT
