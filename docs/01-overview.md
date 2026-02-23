# MinimalOS — Overview

MinimalOS is a **capability-based microkernel** for x86_64 written in Rust. User-space entities are **WebAssembly actors** interpreted by `tinywasm` inside the kernel's address space (SASOS — Single Address Space OS). It boots via the **Limine** bootloader and runs on QEMU.

## Key Design Principles

- **Capability-based security**: No global identifiers — processes interact only via held capabilities (unforgeable tokens)
- **Wasm actors**: All user-space code runs as WebAssembly modules interpreted by `tinywasm` inside the kernel
- **SASOS**: Single Address Space OS — actors share the kernel's address space, no Ring 3 transition needed
- **Message-passing IPC**: 48-byte cache-line-sized messages with inline capability transfer
- **Microkernel**: Kernel provides only scheduling, memory, IPC, and capability enforcement — filesystem and UI are actors

## Project Structure

```
MinimalOS/
├── Cargo.toml              # Workspace root
├── Makefile                 # Build orchestrator
├── rust-toolchain.toml      # Nightly toolchain config
├── limine.conf              # Bootloader config
├── build/                   # Linker scripts, target JSONs
├── kernel/                  # The microkernel
│   └── src/
│       ├── main.rs          # Entry point & boot sequence
│       ├── cap.rs           # Capability engine
│       ├── ipc.rs           # IPC message & queue
│       ├── wasm.rs          # Wasm runtime & host functions
│       ├── arch/            # x86_64: GDT, IDT, SMP, syscall, TSS
│       ├── memory/          # PMM, paging, heap
│       ├── task/            # Scheduler, process, clock, events, input
│       └── traps/           # Interrupt/exception handlers, IDT setup
├── crates/
│   ├── khal/                # Hardware Abstraction Layer
│   └── klog/                # Kernel logging (serial)
├── actors/
│   ├── sdk/                 # Shared no_std library for Wasm actors
│   ├── vfs/                 # Virtual File System actor
│   ├── ui_server/           # UI compositor / text renderer actor
│   └── shell/               # Shell actor
├── sdk/sys/                 # Legacy FFI declarations (unused)
├── assets/
│   └── font.psf             # PSF v2 font (16×30, 256 glyphs)
├── ramdisk/                 # Files bundled into ramdisk.tar
└── limine/                  # Limine bootloader binaries
```
