# MinimalOS

A multitasking operating system for x86_64 architecture with microservices architecture.

**Current Status:** ~60% Complete ([View Detailed Status](IMPLEMENTATION_STATUS.md) | [Quick Reference](STATUS_SUMMARY.md))

## Features
- ✅ GRUB Multiboot2 bootloader
- ✅ 64-bit long mode with proper page tables
- ✅ GDT/IDT with exception handling
- ✅ Serial and VGA drivers
- ✅ Physical and virtual memory management
- ✅ Process management with round-robin scheduler
- ✅ ELF64 program loader
- ✅ IPC message passing
- ✅ System calls with user mode (Ring 3)
- ⚠️ Modular microservices architecture (in progress)

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

## Implementation Status

### ✅ Completed
- [x] Bootloader (Multiboot2)
- [x] 64-bit mode transition
- [x] GDT/IDT setup with TSS
- [x] Serial/VGA/Timer drivers
- [x] Memory management (PMM, VMM, Heap)
- [x] Process management with context switching
- [x] Round-robin scheduler
- [x] ELF64 loader
- [x] IPC message passing (blocking receive)
- [x] System calls (write, exit, ipc_send, ipc_recv)
- [x] User mode support (Ring 0→3 transitions)

### 🚧 In Progress / Not Started
- [ ] Virtual Filesystem (VFS) layer
- [ ] Initial ramdisk (initrd) support
- [ ] File-related syscalls (open, read, close)
- [ ] Process creation syscalls (fork, exec, wait)
- [ ] Keyboard driver
- [ ] Terminal/TTY service
- [ ] Shell program
- [ ] Ramdisk build system

**For detailed status, see:**
- [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) - Comprehensive analysis with evidence
- [STATUS_SUMMARY.md](STATUS_SUMMARY.md) - Quick reference guide

## License
MIT
