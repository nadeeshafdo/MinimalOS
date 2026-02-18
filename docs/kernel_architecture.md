---
layout: default
title: Kernel Architecture
---

# Kernel Architecture

## Overview

MinimalOS is a 64-bit x86_64 operating system kernel written in Rust. It boots via the
[Limine bootloader](https://github.com/limine-bootloader/limine) (v8.x protocol) and
runs in the higher half of the virtual address space at `0xFFFF_FFFF_8000_0000`.

The project is structured as a Cargo workspace with a central kernel binary, several
kernel-space library crates, an SDK crate for shared types, and user-space program
crates.

## Boot Flow

```
Limine (BIOS/UEFI)
  │
  ├─ Sets up Long Mode (64-bit)
  ├─ Creates Higher-Half Direct Map page tables
  ├─ Loads kernel ELF at 0xFFFFFFFF80000000
  ├─ Loads ramdisk.tar module
  └─ Jumps to _start
        │
        ├─ klog::init()            — Serial COM1 output
        ├─ PIC disable             — Mask all legacy 8259 IRQs
        ├─ IDT init                — 256 entries, IST for Double Fault
        ├─ syscall::init()         — EFER.SCE, LSTAR, STAR, SFMASK
        ├─ Memory census           — Walk Limine memory map
        ├─ PMM init                — Bitmap frame allocator
        ├─ Paging init             — Read CR3, set HHDM offset
        ├─ APIC MMIO map           — Map Local APIC page
        ├─ Heap init               — Linked-list allocator (64 KiB → 16 MiB)
        ├─ APIC enable             — Local APIC + periodic timer
        ├─ STI                     — Enable interrupts
        ├─ Framebuffer console     — kdisplay init
        ├─ Keyboard init           — PS/2 + IRQ1
        ├─ RAMDisk detect          — Limine module → global storage
        ├─ TAR parse + ELF load    — Load init.elf into user pages
        ├─ Scheduler init          — Create idle + init processes
        └─ do_schedule()           — Context switch to init (Ring 3)
              │
              └─ Idle loop: sti; hlt
```

## Memory Layout

### Virtual Address Space

| Range | Description |
|-------|-------------|
| `0x0000_0000_0040_0000` | User program: `init.elf` (PT_LOAD) |
| `0x0000_0000_0050_0000` | User program: `shell.elf` (PT_LOAD) |
| `0x0000_0000_0080_0000` | User stacks (per-PID: `0x800000 + pid * 0x10000`) |
| `0xFFFF_8000_0000_0000` | Higher-Half Direct Map (HHDM) base |
| `0xFFFF_9000_0000_0000` | Test / dynamic kernel mappings |
| `0xFFFF_A000_0000_0000` | Kernel heap (64 KiB initial, grows to 16 MiB) |
| `0xFFFF_FFFF_8000_0000` | Kernel `.text`, `.rodata`, `.data`, `.bss` |

### Linker Script Sections

The kernel linker script (`build/linker.ld`) places code at `0xFFFF_FFFF_8000_0000`:

| Section | Description |
|---------|-------------|
| `.limine_requests` | Limine protocol request structures |
| `.text` | Executable code |
| `.rodata` | Read-only data (fonts, strings) |
| `.data` / `.bss` | Mutable and zero-initialised data |

Symbols `__kernel_start` and `__kernel_end` delimit the kernel image. All sections are
page-aligned.

A separate linker script (`build/linker-shell.ld`) places the shell binary at
`0x0000_0000_0050_0000` to avoid collision with init at `0x0000_0000_0040_0000`.

## GDT Layout

The Global Descriptor Table contains 7 entries in a specific order required by
`syscall`/`sysret`:

| Index | Selector | Segment | Ring | Notes |
|-------|----------|---------|------|-------|
| 0 | `0x00` | Null | — | Required null descriptor |
| 1 | `0x08` | Kernel Code | 0 | 64-bit code segment |
| 2 | `0x10` | Kernel Data | 0 | Data segment |
| 3 | `0x18` | User Data | 3 | Must precede User Code for STAR |
| 4 | `0x20` | User Code | 3 | 64-bit code segment |
| 5–6 | `0x28` | TSS | 0 | 16-byte TSS descriptor |

The STAR MSR is configured as `0x0010_0008_0000_0000`:
- `sysret` CS = `0x10 | 3` = User Data at `0x18`, User Code at `0x20`
- `syscall` CS = `0x08` → Kernel Code

## TSS (Task State Segment)

The TSS provides:

- **RSP0**: Kernel stack pointer for Ring 3 → Ring 0 transitions. Dynamically
  updated on each context switch via `write_unaligned` (the TSS struct is `#[repr(packed)]`).
- **IST1**: Dedicated stack for Double Fault exceptions, preventing stack overflow
  from cascading into a triple fault.

The TSS pointer is stored in an `AtomicPtr` for safe access from interrupt handlers.

## Kernel Modules

| Module | Path | Contents |
|--------|------|----------|
| `arch` | `kernel/src/arch/` | `gdt.rs`, `idt.rs`, `tss.rs`, `syscall.rs` |
| `memory` | `kernel/src/memory/` | `pmm.rs`, `paging.rs`, `heap.rs`, census, APIC MMIO |
| `task` | `kernel/src/task/` | `process.rs`, `input.rs`, `usermode.rs` |
| `traps` | `kernel/src/traps/` | `idt.rs`, `handlers.rs` |
| `fs` | `kernel/src/fs/` | `tar.rs`, `elf.rs`, `ramdisk.rs` |

## Custom Target

The kernel compiles against `build/target-kernel.json`:

| Property | Value | Reason |
|----------|-------|--------|
| LLVM target | `x86_64-unknown-none-elf` | Freestanding binary |
| Code model | `kernel` | Higher-half addresses |
| Linker | `rust-lld` (GNU-LLD) | Cross-platform linking |
| Panic strategy | `abort` | No unwinding support |
| Red zone | **disabled** | Interrupt safety |
| SIMD/FPU | disabled (`+soft-float`) | No SSE in kernel |

User-space programs use `build/target-user.json` which keeps the red zone enabled
and uses a small code model.

## External Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `limine` | 0.5 | Boot protocol request/response API |
| `x86_64` | 0.15 | CPU structures (GDT, IDT, paging, port I/O) |
| `spin` | 0.9 | Spinlock synchronisation (`Mutex`, `Once`) |
| `pc-keyboard` | 0.7 | PS/2 scancode decoding (layouts, modifiers) |
| `klog` | local | Kernel logging via serial COM1 |
| `kdisplay` | local | Framebuffer graphics and text console |
| `khal` | local | Hardware Abstraction Layer |

## Panic Handler

The panic handler logs the panic message via serial, then enters a tight `cli; hlt`
loop to prevent further execution. There is no unwinding — the kernel is compiled
with `panic = abort`.
