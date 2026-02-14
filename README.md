# MinimalOS

A functional 64-bit x86_64 operating system built from scratch using the Limine bootloader.

## Current Features

### ✅ Core System
- **Limine bootloader** (BIOS and UEFI support)
- **64-bit Long Mode** kernel
- GDT (Global Descriptor Table) with kernel and user segments
- IDT (Interrupt Descriptor Table) with 256 entries
- TSS (Task State Segment) for user mode support
- 32 CPU exception handlers (ISRs)
- 16 hardware interrupt handlers (IRQs) with PIC remapping

### ✅ Memory Management
- Physical Memory Manager (PMM) with bitmap allocator
- Higher Half Direct Map (HHDM) for physical memory access
- Kernel Heap (kmalloc/kfree)

### ✅ Process Management
- Process creation and management
- Round-robin scheduler
- Context switching (64-bit)
- System calls (int 0x80)

### ✅ Drivers
- **VESA Framebuffer** (graphics mode) with 8×16 bitmap font
- **PS/2 Keyboard** with shift key support
- **PIT Timer** at 100Hz

### ✅ Interactive Shell
18 built-in commands:

| Command | Description |
|---------|-------------|
| `help` | Show available commands |
| `clear` | Clear screen |
| `echo <text>` | Print text |
| `reboot` | Restart system |
| `halt` | Halt CPU |
| `poweroff` | Power off (QEMU/VMs) |
| `info` | System information |
| `mem` | Memory usage |
| `uptime` | System uptime |
| `ps` | List processes |
| `cpuid` | CPU information |
| `cpufreq` | Estimate CPU speed |
| `peek <addr>` | Read memory |
| `poke <addr> <val>` | Write memory |
| `hexdump <addr>` | Dump 64 bytes |
| `alloc <size>` | Allocate memory |
| `color <fg> <bg>` | Set terminal colors |
| `banner` | ASCII art logo |
| `test` | Run diagnostics |

## Documentation

Detailed documentation for each component is available in the `docs/` directory:

- [Kernel Architecture](docs/kernel_architecture.md) - Boot flow, GDT, IDT, Interrupts
- [Memory Management](docs/memory_management.md) - PMM, Virtual Memory, Heap
- [Process Management](docs/process_management.md) - Scheduler, PCB, System Calls
- [Drivers](docs/drivers.md) - Framebuffer, Keyboard, Timer
- [Development Guide](docs/development_guide.md) - Adding commands and syscalls

## Building

### Prerequisites
- GCC (with 64-bit support)
- GNU Make
- GNU Assembler
- QEMU (for testing)
- xorriso (for ISO creation)
- Git (to clone Limine)

### Compile
```bash
cd MinimalOS
make
```

Output: `build/dist/minimalos`

### Create Bootable ISO
```bash
make iso
```

This will:
1. Clone/update Limine bootloader
2. Build the kernel
3. Create a bootable ISO supporting both BIOS and UEFI

### Run in QEMU

**BIOS mode:**
```bash
make qemu-bios
# or just: make run
```

**UEFI mode:** (requires OVMF)
```bash
make qemu-uefi
```

## Project Structure

```
MinimalOS/
├── arch/x86_64/             # Architecture-specific files
│   └── linker.ld            # Linker script for higher-half kernel
├── kernel/
│   ├── kernel.c             # Main entry point (Limine protocol)
│   ├── tty.c                # Framebuffer terminal
│   ├── shell.c              # Command dispatcher
│   ├── arch/x86_64/         # GDT, IDT, ISR, IRQ, context switch
│   ├── mm/                  # PMM, paging, kernel heap
│   ├── process/             # Process, scheduler, syscalls
│   ├── commands/            # Shell command implementations
│   └── include/             # All kernel headers
│       └── limine.h         # Limine boot protocol header
├── drivers/
│   ├── keyboard.c           # PS/2 keyboard
│   ├── timer.c              # PIT timer
│   ├── framebuffer.c        # VESA graphics
│   └── font.c               # 8×16 bitmap font
├── limine.conf              # Limine bootloader configuration
├── build/                   # Build output directory
│   └── dist/                # Final binaries and ISO
└── Makefile
```

## Development Status

| Phase | Status | Description |
|-------|--------|-------------|
| 1. Environment Setup | ✅ Complete | Toolchain, QEMU, Makefile |
| 2. 64-bit Long Mode | ✅ Complete | x86_64 kernel |
| 3. Limine Bootloader | ✅ Complete | BIOS/UEFI support |
| 4. Core Initialization | ✅ Complete | GDT, IDT, ISR, IRQ, PIC |
| 5. Drivers | ✅ Complete | Timer, keyboard, framebuffer |
| 6. Memory Management | ✅ Complete | PMM, HHDM, heap |
| 7. Process Management | ✅ Complete | Processes, scheduler, TSS |
| 8. System Calls | ✅ Complete | int 0x80 interface |
| 9. File System | 🔲 Planned | VFS, initrd, FAT32 |
| 10. Shell | ✅ Complete | 18 built-in commands |

## Known Limitations

- **No disk I/O** - File system not yet implemented  
- **No ACPI** - Poweroff works on VMs only, halts on real hardware
- **No 4-level paging customization** - Uses Limine's page tables

## Future Plans

- [ ] ATA/AHCI disk driver
- [ ] File system (FAT32 or custom)
- [ ] ACPI for real hardware power management
- [ ] Custom page table management
- [ ] User-space program execution

## Boot Modes

This OS supports both boot methods:

| Mode | Description | QEMU Command |
|------|-------------|--------------|
| **BIOS** | Legacy BIOS boot | `make qemu-bios` |
| **UEFI** | Modern UEFI boot | `make qemu-uefi` |

## License

Educational project. Free to use and modify.

## References

- [OSDev Wiki](https://wiki.osdev.org/)
- [Limine Bootloader](https://github.com/limine-bootloader/limine)
- [OSDev Limine Bare Bones](https://wiki.osdev.org/Limine_Bare_Bones)
