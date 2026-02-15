# Development Guide

## Prerequisites

| Tool        | Purpose                                    |
|-------------|--------------------------------------------|
| **Rust**    | Nightly toolchain (`nightly-2025-01-01`)   |
| **QEMU**    | x86_64 system emulator for testing         |
| **xorriso** | ISO 9660 image creation                    |
| **Git**     | Cloning Limine bootloader                  |
| **GNU Make**| Build orchestration                        |

The exact Rust toolchain is pinned in `rust-toolchain.toml` and includes the
`rust-src` and `llvm-tools-preview` components.

## Building

### Build the Kernel

```bash
make kernel
# or simply:
make
```

This compiles the `minimalos_kernel` package against the custom target
`build/target-kernel.json`. The resulting ELF binary is placed at
`target/target-kernel/debug/minimalos_kernel`.

### Create a Bootable ISO

```bash
make iso
```

This will:

1. Build the kernel.
2. Clone/update the Limine bootloader (v8.x branch).
3. Assemble a hybrid BIOS+UEFI bootable ISO at `build/dist/minimalos.iso`.

### Run in QEMU

```bash
make run          # BIOS mode (default)
make qemu-bios    # BIOS mode (explicit)
make qemu-uefi    # UEFI mode (requires OVMF)
make qemu-debug   # BIOS mode with interrupt logging, no reboot on triple-fault
```

QEMU is configured with a Q35 machine type and 2 GiB RAM. Serial output is
directed to `stdio`.

## Project Layout

```
MinimalOS/
├── Cargo.toml                  # Workspace root
├── Makefile                    # Build orchestration
├── limine.cfg                  # Bootloader configuration
├── rust-toolchain.toml         # Pinned nightly Rust toolchain
├── QUESTS.md                   # Development quest tracker
│
├── build/
│   ├── linker.ld               # Higher-half kernel linker script
│   ├── target-kernel.json      # Custom Rust target for the kernel
│   └── target-user.json        # Custom Rust target for user-space
│
├── kernel/                     # Kernel binary crate
│   ├── Cargo.toml
│   ├── build.rs                # Linker script integration
│   └── src/
│       ├── main.rs             # Entry point (_start)
│       ├── arch/mod.rs         # x86_64-specific code
│       ├── memory/mod.rs       # PMM / VMM
│       ├── task/mod.rs         # Scheduler / processes
│       └── traps/mod.rs        # IDT / exceptions / IRQs
│
├── crates/                     # Kernel-space libraries
│   ├── kdisplay/               # Framebuffer graphics
│   ├── khal/                   # Hardware Abstraction Layer
│   └── klog/                   # Kernel logging
│
├── sdk/
│   └── sys/                    # Shared types (kernel ↔ userspace)
│
└── docs/                       # Project documentation
```

## Workspace Crates

| Crate        | Path               | Description                                |
|--------------|--------------------|--------------------------------------------|
| `minimalos_kernel` | `kernel/`    | Kernel entry point and core subsystems     |
| `klog`       | `crates/klog/`     | Kernel logging subsystem                   |
| `kdisplay`   | `crates/kdisplay/` | Framebuffer display and text console       |
| `khal`       | `crates/khal/`     | Hardware abstraction (ports, PIC, PIT)     |
| `sys`        | `sdk/sys/`         | Shared types between kernel and userspace  |

All crates are `#![no_std]`.

## Adding a New Kernel Module

1. Create a new directory under `kernel/src/` (e.g., `kernel/src/mymod/mod.rs`).
2. Declare it in `kernel/src/main.rs` with `mod mymod;`.
3. Implement your module. Use `klog` for debug output once it is available.

## Adding a New Crate

1. Create a new directory under `crates/` with a `Cargo.toml` and `src/lib.rs`.
2. Mark it `#![no_std]`.
3. The workspace `Cargo.toml` uses `members = ["crates/*"]` so it will be picked up
   automatically.
4. Add it as a dependency in `kernel/Cargo.toml` if the kernel needs it:
   ```toml
   mylib = { path = "../crates/mylib" }
   ```

## Cleaning

```bash
make clean      # Remove Cargo build artifacts and ISO output
make distclean  # Also remove the cloned Limine directory
```

## Development Roadmap

The project follows the quest phases defined in `QUESTS.md`:

| Phase | Focus                          | Status      |
|-------|--------------------------------|-------------|
| 1     | Foundation (toolchain, build)  | In progress |
| 2     | Subsystems (klog, kdisplay, khal, sys) | Not started |
| 3     | Memory management (PMM, VMM)  | Not started |
| 4     | Interrupts & scheduling        | Not started |
| 5     | Userspace                      | Not started |

## Debugging Tips

- Use `make qemu-debug` to get interrupt/exception logs and prevent automatic reboot
  on triple-faults.
- Serial output goes to the terminal (`-serial stdio`), so once `klog` + serial
  driver are implemented, `println!`-style debugging will work.
- The `x86_64` crate provides utilities for reading CR2, CR3, and other control
  registers useful during page-fault debugging.
