# MinimalOS

A functional 32-bit x86 operating system built from scratch following OSDev wiki best practices.

## Current Features

### ✅ Core System
- Multiboot-compliant bootloader (GRUB compatible)
- GDT (Global Descriptor Table) with kernel and user segments
- IDT (Interrupt Descriptor Table) with 256 entries
- TSS (Task State Segment) for user mode support
- 32 CPU exception handlers (ISRs)
- 16 hardware interrupt handlers (IRQs) with PIC remapping

### ✅ Memory Management
- Physical Memory Manager (PMM) with bitmap allocator
- Virtual Memory / Paging (32-bit, 4KB pages)
- Kernel Heap (kmalloc/kfree)
- Dynamic framebuffer region mapping

### ✅ Process Management
- Process creation and management
- Round-robin scheduler
- Context switching
- System calls (int 0x80)

### ✅ Drivers
- **VGA Text Mode** (80×25) with scrolling and colors
- **VESA Framebuffer** (1024×768×32) with 8×16 bitmap font
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

## Building

### Prerequisites
- GCC (with 32-bit support)
- GNU Make
- GNU Assembler
- QEMU (for testing)
- GRUB tools (for ISO creation)

### Compile
```bash
cd MinimalOS
make
```

Output: `build/dist/minimalos.bin`

### Run in QEMU
```bash
make run
```

### Create Bootable ISO
```bash
make iso
make qemu-iso
```

## Project Structure

```
MinimalOS/
├── arch/i386/              # Bootloader and linker script
│   ├── boot.s              # Multiboot header (1024×768 framebuffer)
│   └── linker.ld
├── kernel/
│   ├── kernel.c            # Main entry point
│   ├── tty.c               # Dual VGA/framebuffer terminal
│   ├── shell.c             # Command dispatcher
│   ├── arch/i386/          # GDT, IDT, ISR, IRQ, context switch
│   ├── mm/                 # PMM, paging, kernel heap
│   ├── process/            # Process, scheduler, syscalls
│   ├── commands/           # Shell command implementations
│   │   ├── basic.c         # help, clear, echo, reboot, halt, poweroff
│   │   ├── sysinfo.c       # info, mem, uptime, ps, cpuid
│   │   ├── memory.c        # peek, poke, hexdump, alloc
│   │   ├── display.c       # color, banner
│   │   └── tests.c         # test, cpufreq
│   └── include/kernel/     # All kernel headers
├── drivers/
│   ├── keyboard.c          # PS/2 keyboard
│   ├── timer.c             # PIT timer
│   ├── framebuffer.c       # VESA graphics
│   └── font.c              # 8×16 bitmap font
├── build/                  # Build output directory
│   └── dist/               # Final binaries
└── Makefile
```

## Development Status

| Phase | Status | Description |
|-------|--------|-------------|
| 1. Environment Setup | ✅ Complete | Toolchain, QEMU, Makefile |
| 2. Bare Bones Kernel | ✅ Complete | Boot, VGA terminal |
| 3. Core Initialization | ✅ Complete | GDT, IDT, ISR, IRQ, PIC |
| 4. Drivers | ✅ Complete | Timer, keyboard, framebuffer |
| 5. Memory Management | ✅ Complete | PMM, paging, heap |
| 6. Process Management | ✅ Complete | Processes, scheduler, TSS |
| 7. System Calls | ✅ Complete | int 0x80 interface |
| 8. File System | 🔲 Planned | VFS, initrd, FAT32 |
| 9. Shell | ✅ Complete | 18 built-in commands |
| 10. Testing | ✅ Working | QEMU + real hardware tested |

## Known Limitations

- **32-bit only** - 4GB address space limit
- **No disk I/O** - File system not yet implemented  
- **No ACPI** - Poweroff works on VMs only, halts on real hardware
- **Legacy BIOS only** - No UEFI support

## Future Plans

- [ ] ATA/AHCI disk driver
- [ ] File system (FAT32 or custom)
- [ ] ACPI for real hardware power management
- [ ] Consider 64-bit long mode migration
- [ ] User-space program execution

## License

Educational project. Free to use and modify.

## References

- [OSDev Wiki](https://wiki.osdev.org/)
- [OSDev Bare Bones](https://wiki.osdev.org/Bare_Bones)
- [OSDev Meaty Skeleton](https://wiki.osdev.org/Meaty_Skeleton)
