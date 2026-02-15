# Kernel Architecture

## Overview

MinimalOS is a 64-bit x86_64 operating system kernel written in Rust. It boots via the
[Limine bootloader](https://github.com/limine-bootloader/limine) (v8.x protocol) and
runs in the higher-half of virtual address space at `0xFFFFFFFF80000000`.

The project is structured as a Cargo workspace with a central kernel binary and several
supporting crates.

## Boot Flow

1. **Limine bootloader** loads the kernel ELF from the ISO (`boot:///kernel`).
2. The bootloader sets up long mode (64-bit), page tables with a higher-half direct
   map, and passes control to the kernel entry point `_start`.
3. `_start` (in `kernel/src/main.rs`) verifies that the Limine base revision is
   supported.
4. The kernel requests a framebuffer from the bootloader via `FramebufferRequest`.
5. Currently the kernel halts after framebuffer acquisition (`hlt` loop).

## Memory Layout

The linker script (`build/linker.ld`) places the kernel in the higher half:

| Section              | Description                            |
|----------------------|----------------------------------------|
| `.limine_requests`   | Limine protocol request structures     |
| `.text`              | Executable code                        |
| `.rodata`            | Read-only data                         |
| `.data` / `.bss`     | Mutable and zero-initialised data      |

Symbols `__kernel_start` and `__kernel_end` delimit the kernel image.

All sections are page-aligned (`MAXPAGESIZE`).

## Custom Target

The kernel is compiled against a custom target specification
(`build/target-kernel.json`):

- **LLVM target**: `x86_64-unknown-none-elf`
- **Code model**: `kernel` (suitable for higher-half addresses)
- **Linker**: `rust-lld` (GNU-LLD flavour)
- **Panic strategy**: `abort` (no unwinding)
- **Red zone**: disabled (required for interrupt safety)
- **SIMD/FPU**: disabled (`-mmx,-sse,-sse2,…,+soft-float`)

A separate user-space target (`build/target-user.json`) exists for future user-mode
binaries. It differs in that the red zone is **not** disabled and no restricted feature
flags are set.

## Kernel Modules

The kernel source is organised into four submodules under `kernel/src/`:

| Module     | File                          | Purpose                              |
|------------|-------------------------------|--------------------------------------|
| `arch`     | `kernel/src/arch/mod.rs`      | x86_64-specific code (CPU, GDT, etc)|
| `memory`   | `kernel/src/memory/mod.rs`    | Physical and virtual memory managers |
| `task`     | `kernel/src/task/mod.rs`      | Task scheduler and process control   |
| `traps`    | `kernel/src/traps/mod.rs`     | Interrupt and exception handling     |

All four modules are currently stubs awaiting implementation.

## External Dependencies

| Crate    | Version | Purpose                                   |
|----------|---------|-------------------------------------------|
| `limine` | 0.5     | Limine boot protocol request/response API |
| `x86_64` | 0.15    | CPU structures (GDT, IDT, paging, I/O)    |
| `spin`   | 0.9     | Spinlock-based synchronisation primitives  |

## Build Script

`kernel/build.rs` tells Cargo to:

1. Add `build/` to the native library search path.
2. Pass `-Tlinker.ld` to the linker so the custom linker script is used.
3. Rebuild when `build/linker.ld` changes.

## Panic Handler

A minimal panic handler is provided that halts the CPU in an infinite loop. There is
no unwinding support.
