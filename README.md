# MinimalOS

A functional x86 operating system built from scratch following OSDev wiki best practices.

## Features

✅ **Core System**
- Multiboot-compliant bootloader (GRUB compatible)
- GDT (Global Descriptor Table) with kernel and user segments
- IDT (Interrupt Descriptor Table) with 256 entries
- 32 CPU exception handlers (ISRs)
- 16 hardware interrupt handlers (IRQs) with PIC remapping

✅ **Drivers**
- VGA text mode terminal (80x25) with scrolling, colors, newlines
- PS/2 keyboard driver with live input and shift key support
- Programmable Interval Timer (PIT) running at 100Hz

✅ **Build System**
- Makefile with auto-detection of cross-compiler
- QEMU testing targets
- ISO generation support

## Building

### Prerequisites
- GCC (with 32-bit support or cross-compiler)
- GNU Make
- GNU Assembler (as)
- QEMU (for testing)
- GRUB tools (for ISO creation)

### Compilation

```bash
cd /media/nadeeshafdo/shared/repos/MinimalOS
make
```

This will produce `minimalos.bin`, the kernel binary.

## Running

### In QEMU (Recommended)
```bash
make qemu
```

This boots the kernel directly in QEMU.

### Creating Bootable ISO
```bash
make iso
make qemu-iso
```

## Project Structure

```
MinimalOS/
├── arch/i386/          # Architecture-specific code
│   ├── boot.s          # Multiboot header and bootstrap
│   └── linker.ld       # Linker script
├── kernel/
│   ├── kernel.c        # Kernel entry point
│   ├── tty.c           # VGA terminal driver
│   ├── arch/i386/      # i386-specific kernel code
│   │   ├── gdt.c       # Global Descriptor Table
│   │   ├── gdt_flush.s # GDT loading routine
│   │   ├── idt.c       # Interrupt Descriptor Table
│   │   ├── idt_flush.s # IDT loading routine
│   │   ├── isr.c       # Interrupt Service Routines
│   │   ├── isr_stub.s  # ISR assembly stubs
│   │   ├── irq.c       # Hardware interrupt handlers
│   │   └── irq_stub.s  # IRQ assembly stubs
│   └── include/kernel/ # Kernel headers
├── drivers/
│   ├── keyboard.c      # PS/2 keyboard driver
│   └── timer.c         # PIT timer driver
├── Makefile            # Build system
└── README.md           # This file
```

## Features in Detail

### VGA Terminal
- 16 foreground colors, 8 background colors
- Automatic scrolling when screen fills
- Support for newline (`\n`), carriage return (`\r`), backspace (`\b`), and tab (`\t`)
- Screen clearing capability

### Interrupts
- Proper PIC remapping to avoid conflicts with CPU exceptions
- ISRs for all 32 CPU exceptions with descriptive error messages
- IRQs for all 16 hardware interrupts
- EOI (End of Interrupt) handling for master and slave PICs

### Keyboard
- US QWERTY layout
- Scancode to ASCII translation
- Shift key support for uppercase and symbols
- Ring buffer for input storage
- Live echo to terminal

### Timer
- Configurable frequency (currently 100Hz)
- Tick counting for system uptime
- Usesinterrupt IRQ0

## Development Status

**Completed:**
- ✅ Bootloader and kernel setup
- ✅ VGA text mode driver
- ✅ GDT implementation
- ✅ IDT and interrupt handling
- ✅ PIC configuration
- ✅ Timer driver (PIT)
- ✅ Keyboard driver (PS/2)

**In Progress:**
- 🔄 Memory management (physical/virtual)
- 🔄 Process management and scheduling
- 🔄 File system support
- 🔄 User mode and system calls
- 🔄 Shell/command interface

## Testing

The OS boots in QEMU and displays:
1. Welcome banner
2. Memory information from multiboot
3. Initialization of each component
4. Feature list
5. Interactive prompt where you can type

Try typing on the keyboard - all input is echoed to the screen in real-time!

## License

This is an educational project. Feel free to use and modify as needed.

## References

- [OSDev Wiki](https://wiki.osdev.org/)
- [OSDev Bare Bones Tutorial](https://wiki.osdev.org/Bare_Bones)
- [OSDev Meaty Skeleton](https://wiki.osdev.org/Meaty_Skeleton)
