# MinimalOS - 64-bit Operating System

A minimal 64-bit operating system written from scratch in C and x86_64 assembly.
Built for educational purposes to learn OS development concepts.

## Features

### Core System
- **64-bit Long Mode** - Full x86_64 support with 4-level paging
- **Multiboot2 Boot** - GRUB2 compatible bootloader
- **GDT/IDT** - Global Descriptor Table and Interrupt Descriptor Table
- **PIC** - Programmable Interrupt Controller with IRQ remapping
- **TSS** - Task State Segment for privilege level transitions

### Memory Management
- **Physical Memory Manager** - Bitmap-based page frame allocator
- **Kernel Heap** - Dynamic memory allocation (kmalloc/kfree)
- **Multiboot2 Memory Map** - Automatic memory detection

### Process Management
- **Process Control Blocks** - Full process state management
- **Round-robin Scheduler** - Preemptive multitasking support
- **Context Switching** - 64-bit register save/restore

### System Calls
- **SYSCALL/SYSRET** - Fast system call interface via MSR
- **6 System Calls** - read, write, exit, getpid, yield, sleep

### File System
- **Virtual File System (VFS)** - Unified file interface
- **Initrd** - Initial RAM disk with demo files
- **Path Resolution** - Full path lookup support

### Drivers
- **PIT Timer** - Programmable Interval Timer (100Hz)
- **PS/2 Keyboard** - Full scancode translation with modifiers
- **Serial Port** - COM1 debug output at 115200 baud
- **VGA Text Mode** - 80x25 text with colors and hardware cursor

### Shell Commands
| Command | Description |
|---------|-------------|
| `help` | Show available commands |
| `clear` | Clear the screen |
| `echo TEXT` | Print text to screen |
| `uptime` | Show system uptime |
| `date` | Show formatted time |
| `info` | System information |
| `mem` | Memory information |
| `ps` | List all processes |
| `ls` | List files in initrd |
| `cat FILE` | Display file contents |
| `syscall` | Test syscall interface |
| `usermode` | Show userspace status |
| `reboot` | Reboot the system |
| `halt` | Halt the CPU |

## Building

### Requirements
- `nasm` - Netwide Assembler
- `gcc` (x86_64-elf or x86_64-linux-gnu)
- `ld` - GNU Linker
- `grub-mkrescue` - GRUB2 ISO creator
- `qemu-system-x86_64` - Emulator for testing

### Build Commands
```bash
make help       # Show all available targets
make            # Build the kernel
make iso        # Create bootable ISO
make run        # Run in QEMU
make run-serial # Run with serial debug output
make debug      # Run with GDB server
make clean      # Remove build artifacts
```

## Project Structure
```
MinimalOS/
├── arch/x86_64/
│   ├── boot.asm        # Multiboot2 header, long mode setup
│   └── linker.ld       # Linker script
├── kernel/
│   ├── kernel.c        # Main kernel and shell
│   ├── syscall.c       # System call handlers
│   ├── multiboot2.c    # Multiboot2 parsing
│   ├── arch/x86_64/
│   │   ├── idt.c       # Interrupt Descriptor Table
│   │   ├── pic.c       # PIC initialization
│   │   ├── tss_setup.c # Task State Segment
│   │   ├── isr_stubs.asm
│   │   ├── switch.asm  # Context switching
│   │   ├── syscall.asm # SYSCALL entry
│   │   ├── tss.asm     # GDT with Ring 3
│   │   └── user.asm    # User mode entry
│   ├── mm/
│   │   ├── pmm.c       # Physical memory manager
│   │   └── kheap.c     # Kernel heap
│   ├── process/
│   │   ├── process.c   # Process management
│   │   └── scheduler.c # Round-robin scheduler
│   ├── fs/
│   │   ├── vfs.c       # Virtual file system
│   │   ├── initrd.c    # RAM disk driver
│   │   └── demo_initrd.c
│   ├── user/
│   │   └── user.c      # Userspace support
│   └── include/        # Header files
├── drivers/
│   ├── timer.c         # PIT timer
│   ├── keyboard.c      # PS/2 keyboard
│   └── serial.c        # Serial port
├── Makefile
└── README.md
```

## Development Status

- [x] Phase 1: Boot to Long Mode
- [x] Phase 2: Core Infrastructure (IDT, PIC, Serial)
- [x] Phase 3: Device Drivers (Timer, Keyboard)
- [x] Phase 4: Memory Management (PMM, Heap)
- [x] Phase 5: Process Management
- [x] Phase 6: System Calls
- [x] Phase 7: File System (VFS, Initrd)
- [x] Phase 8: Userspace Infrastructure
- [x] Phase 9: Shell (echo, date, info)

## License

This project is for educational purposes.

## Acknowledgments

- [OSDev Wiki](https://wiki.osdev.org/) - Essential OS development resource
- [AMD64 Architecture Manual](https://developer.amd.com/)
