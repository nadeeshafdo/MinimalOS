# MinimalOS v2.0 - Production Shell OS

A minimal 32-bit operating system with Multiboot support, demonstrating core OS concepts including bootloading, protected mode, VGA text output, and a foundation for building an interactive shell.

## 🎯 Overview

MinimalOS is a lightweight, educational operating system designed to teach fundamental OS development concepts. It uses the industry-standard Multiboot specification, allowing it to boot via QEMU's built-in Multiboot loader or GRUB.

## ✨ Features

-  **Multiboot Compliant**: Standard bootloader interface (GRUB/QEMU compatible)
- **32-bit Protected Mode**: Runs in i386 protected mode
- **VGA Text Mode**: Direct VGA text buffer manipulation at 0xB8000
- **Minimal Footprint**: ~9.5KB kernel binary
- **Clean Architecture**: Separated boot stub and kernel code
- **Production Ready**: Optimized build system with multiple run modes

## 🏗️ Architecture

```
MinimalOS Structure:
├── Bootloader (Multiboot Stub)
│   └── Sets up stack and transfers control to kernel
└── Kernel (32-bit C)
    └── VGA text mode driver
    └── Kernel initialization
    └── Ready for shell integration
```

### Memory Layout

| Address    | Component          |
|------------|--------------------|
| 0x100000   | Kernel Load Address (1MB) |
| 0xB8000    | VGA Text Buffer   |
| Stack      | 16KB stack space  |

## 🚀 Quick Start

### Prerequisites

```bash
sudo apt-get install nasm gcc make qemu-system-x86
```

### Build

```bash
make
```

### Run

**GUI Mode** (recommended):
```bash
make run
```

**Terminal Mode** (ncurses):
```bash
make run-term
```

## 📁 Project Structure

```
MinimalOS/
├── src/
│   ├── boot/
│   │   ├── multiboot.asm       # Multiboot header & boot stub
│   │   └── legacy/
│   │       └── boot.asm        # Legacy custom bootloader (archived)
│   └── kernel/
│       └── main.c              # Kernel entry point & VGA driver
├── kernel.ld                   # Linker script for 32-bit kernel
├── kernel.ld.old              # Legacy 64-bit linker script (archived)
├── Makefile                    # Production build system
├── Makefile.custom            # Legacy custom bootloader build (archived)
└── README.md                   # This file
```

## 🛠️ Build System

### Targets

| Target       | Description                          |
|------------- |--------------------------------------|
| `make`       | Build the kernel (default)           |
| `make run`   | Run in QEMU with GUI                 |
| `make run-term` | Run in QEMU terminal mode         |
| `make clean` | Remove build artifacts               |
| `make info`  | Display build information            |

### Build Output

```
[ASM] src/boot/multiboot.asm
[CC]  src/kernel/main.c
[LD]  build/minimalos.bin

Binary: build/minimalos.bin (9.5K)
Architecture: i386 (32-bit Protected Mode)
Bootloader: Multiboot (QEMU/GRUB compatible)
```

## 📚 Technical Details

### Multiboot Specification

MinimalOS implements the Multiboot specification, which provides a standardized interface between bootloaders and operating systems. This allows the kernel to:

- Be loaded by any Multiboot-compliant bootloader (GRUB, QEMU, etc.)
- Receive boot information from the bootloader
- Skip complex bootloader development
- Focus on kernel features

### VGA Text Mode

The kernel writes directly to VGA memory at `0xB8000`:
- Each character is 2 bytes: 1 byte for ASCII, 1 byte for color
- 80x25 character grid (2000 characters total)
- Color format: `(background << 4) | foreground`

Example:
```c
volatile unsigned short* vga = (volatile unsigned short*)0xB8000;
vga[0] = 0x0F00 | 'H';  // White 'H' on black background
```

## 🎓 Educational Value

This OS demonstrates:

1. **Multiboot Protocol**: Industry-standard bootloader interface
2. **Protected Mode**: 32-bit x86 protected mode setup
3. **Memory-Mapped I/O**: Direct hardware access via VGA buffer
4. **Freestanding Environment**: OS development without standard library
5. **Low-Level I/O**: VGA text mode manipulation
6. **Build Systems**: Cross-compilation and linking for bare metal

## 🔧 Development

### Compiling

The kernel is compiled as a freestanding 32-bit binary:
```bash
gcc -m32 -ffreestanding -O2 -Wall -Wextra -nostdlib -c main.c
```

### Linking

Custom linker script places kernel at 1MB:
```ld
SECTIONS {
    . = 1M;
    .text : { *(.multiboot) *(.text) }
    ...
}
```

### Testing

QEMU provides Multiboot support via `-kernel` flag:
```bash
qemu-system-i386 -kernel build/minimalos.bin
```

## 📈 Current Status

**Working**:
- ✅ Multiboot compliance
- ✅ Kernel boots successfully
- ✅ VGA text output
- ✅ Clean build system
- ✅ QEMU compatibility

**In Development**:
- 🔄 PS/2 keyboard driver integration
- 🔄 Interactive shell implementation
- 🔄 Command parsing and execution
- 🔄 Advanced VGA features (scrolling, colors)

## 🗺️ Roadmap

### Phase 1: Foundation (Complete)
- [x] Multiboot bootloader
- [x] 32-bit protected mode
- [x] VGA text output
- [x] Build system

### Phase 2: I/O (In Progress)
- [ ] Keyboard input driver
- [ ] Interrupt handlers
- [ ] Serial port output

### Phase 3: Shell (Planned)
- [ ] Command parser
- [ ] Built-in commands
- [ ] Command history
- [ ] Tab completion

### Phase 4: Advanced (Future)
- [ ] Memory management
- [ ] Process/task switching
- [ ] File system basics
- [ ] Network stack

## 🤝 Contributing

This is an educational project. Feel free to:
- Study the code
- Experiment with modifications
- Add new features
- Improve documentation

## 📖 Learning Resources

- [OSDev Wiki](https://wiki.osdev.org/) - Comprehensive OS development guide
- [Multiboot Specification](https://www.gnu.org/software/grub/manual/multiboot/multiboot.html)
- [Intel x86 Manual](https://software.intel.com/content/www/us/en/develop/articles/intel-sdm.html)
- [VGA Text Mode](https://wiki.osdev.org/Text_mode)

## 📝 License

MIT License - See LICENSE file for details

## 🙏 Acknowledgments

- OSDev community for extensive documentation
- QEMU project for excellent emulation
- GNU toolchain for cross-compilation support

---

**MinimalOS v2.0** - A minimal yet production-ready foundation for OS development