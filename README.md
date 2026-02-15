# MinimalOS

A 64-bit x86_64 operating system kernel written in **Rust** from scratch, using the [Limine bootloader](https://github.com/limine-bootloader/limine).

> **Note**: This project is in **early development**. Core subsystems are currently stubbed and awaiting implementation. See [QUESTS.md](QUESTS.md) for the development roadmap.

## Current Status

### ✅ Working

- **Limine bootloader** integration (v8.x protocol, BIOS + UEFI support)
- **Higher-half kernel** linked at `0xFFFFFFFF80000000`
- **Framebuffer request** from bootloader (acquired but not yet rendered to)
- **Custom Rust target** with kernel code model and disabled red zone
- **Cargo workspace** structure with modular crates

### 🚧 Planned / In Progress

- **GDT, IDT, TSS** (architecture module stubbed)
- **Physical Memory Manager** (PMM with bitmap allocator)
- **Virtual Memory Manager** (VMM with 4-level paging)
- **Kernel heap allocator** (`GlobalAlloc` trait implementation)
- **Task scheduler** (round-robin, context switching)
- **Interrupt handling** (exceptions, IRQs, PIC/APIC)
- **Drivers** (framebuffer console, PS/2 keyboard, PIT timer, serial)
- **System calls** (syscall/sysret or int 0x80)

## Documentation

Comprehensive documentation is available in the [`docs/`](docs/) directory:

- [**Kernel Architecture**](docs/kernel_architecture.md) — Boot flow, memory layout, modules, custom target
- [**Memory Management**](docs/memory_management.md) — HHDM, PMM, VMM, kernel heap (planned)
- [**Process Management**](docs/process_management.md) — TCB, scheduler, context switching (planned)
- [**Drivers**](docs/drivers.md) — Framebuffer (`kdisplay`), HAL (`khal`), planned driver table
- [**Development Guide**](docs/development_guide.md) — Building, running, adding modules/crates, debugging

## Building

### Prerequisites

| Tool           | Purpose                                      |
|----------------|----------------------------------------------|
| **Rust**       | Nightly toolchain (`nightly-2025-01-01`)     |
| **GNU Make**   | Build orchestration                          |
| **QEMU**       | x86_64 system emulator for testing           |
| **xorriso**    | ISO 9660 image creation                      |
| **Git**        | Cloning Limine bootloader                    |

The Rust toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml) and will be installed automatically by `rustup`.

### Build the Kernel

```bash
make
# or explicitly:
make kernel
```

This compiles the kernel against the custom target [`build/target-kernel.json`](build/target-kernel.json) using Cargo. Output: `target/target-kernel/debug/minimalos_kernel`

### Create a Bootable ISO

```bash
make iso
```

This will:

1. Clone/update the Limine bootloader (v8.x binary branch)
2. Build the kernel
3. Assemble a hybrid BIOS + UEFI bootable ISO

Output: `build/dist/minimalos.iso`

### Run in QEMU

```bash
make run          # BIOS mode (default)
make qemu-bios    # BIOS mode (explicit)
make qemu-uefi    # UEFI mode (requires OVMF firmware)
make qemu-debug   # BIOS mode with interrupt logging
```

## Project Structure

```text
MinimalOS/
├── Cargo.toml                  # Workspace root
├── Makefile                    # Build system (Rust + Limine + ISO creation)
├── limine.cfg                  # Bootloader configuration
├── rust-toolchain.toml         # Pinned nightly Rust toolchain
├── QUESTS.md                   # Quest-based development roadmap
│
├── build/
│   ├── linker.ld               # Higher-half kernel linker script
│   ├── target-kernel.json      # Custom Rust target for kernel
│   └── target-user.json        # Custom Rust target for userspace
│
├── kernel/                     # Kernel binary crate (no_std)
│   ├── Cargo.toml
│   ├── build.rs                # Links the linker script
│   └── src/
│       ├── main.rs             # Entry point (_start)
│       ├── arch/mod.rs         # x86_64 (GDT, IDT, TSS, context switch) [stubbed]
│       ├── memory/mod.rs       # PMM, VMM, heap [stubbed]
│       ├── task/mod.rs         # Scheduler, processes [stubbed]
│       └── traps/mod.rs        # Interrupts, exceptions [stubbed]
│
├── crates/                     # Kernel-space support libraries (no_std)
│   ├── kdisplay/               # Framebuffer graphics and text console
│   ├── khal/                   # Hardware Abstraction Layer (ports, PIC, PIT, etc.)
│   └── klog/                   # Kernel logging subsystem
│
├── sdk/
│   └── sys/                    # Shared types (kernel ↔ userspace)
│
└── docs/                       # Project documentation
    ├── kernel_architecture.md
    ├── memory_management.md
    ├── process_management.md
    ├── drivers.md
    └── development_guide.md
```

## Development Phases

Progress is tracked in [`QUESTS.md`](QUESTS.md):

| Phase | Focus                                    | Status          |
|-------|------------------------------------------|-----------------|
| 1     | Foundation (toolchain, build, boot)      | 🟢 In progress  |
| 2     | Subsystems (klog, kdisplay, khal, sys)   | 🔲 Not started  |
| 3     | Memory management (PMM, VMM, heap)       | 🔲 Not started  |
| 4     | Interrupts & scheduling (IDT, PIT, tasks)| 🔲 Not started  |
| 5     | Userspace (ulib, first userspace app)    | 🔲 Not started  |

## Dependencies

The kernel uses the following external Rust crates:

| Crate      | Version | Purpose                                           |
|------------|---------|---------------------------------------------------|
| `limine`   | 0.5     | Limine boot protocol request/response API         |
| `x86_64`   | 0.15    | CPU structures (GDT, IDT, page tables, port I/O)  |
| `spin`     | 0.9     | Spinlock-based synchronization primitives         |

All kernel-space crates are `#![no_std]`.

## Cleaning

```bash
make clean      # Remove Cargo build artifacts and ISO
make distclean  # Also remove the cloned Limine directory
```

## Boot Modes

MinimalOS supports both legacy and modern boot:

| Mode       | Description         | QEMU Command     |
|------------|---------------------|------------------|
| **BIOS**   | Legacy BIOS boot    | `make qemu-bios` |
| **UEFI**   | Modern UEFI boot    | `make qemu-uefi` |

## License

Educational project. Free to use and modify.

## References

- [OSDev Wiki](https://wiki.osdev.org/)
- [Limine Bootloader](https://github.com/limine-bootloader/limine)
- [Limine Protocol Specification](https://github.com/limine-bootloader/limine/blob/trunk/PROTOCOL.md)
- [The Rust Embedded Book](https://docs.rust-embedded.org/book/)
- [Writing an OS in Rust](https://os.phil-opp.com/)
