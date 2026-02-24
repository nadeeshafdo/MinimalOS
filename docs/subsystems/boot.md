---
title: Boot & Serial Output
layout: default
parent: Subsystems
nav_order: 1
---

# Sprint 1 — Boot & Serial Output
{: .no_toc }

Get the kernel running on bare metal and prove it with visible output.
{: .fs-6 .fw-300 }

**Status**: ✅ Complete
{: .label .label-green }

<details open markdown="block">
  <summary>Table of contents</summary>
  {: .text-delta }
- TOC
{:toc}
</details>

---

## Overview

Sprint 1 is the foundation — everything needed to boot the kernel and get diagnostic output. After this sprint, the kernel boots via UEFI, prints to the serial port, and renders text on the framebuffer.

---

## Toolchain Configuration

### `rust-toolchain.toml`

Pins the exact Rust nightly version and includes `rust-src` (needed to rebuild `core` and `alloc` for bare metal):

```toml
[toolchain]
channel = "nightly-2026-02-13"
components = ["rust-src", "rustfmt", "clippy"]
targets = ["x86_64-unknown-none"]
```

### `.cargo/config.toml`

Configures the bare-metal compilation target:

- **Target**: `x86_64-unknown-none` — no OS, no libc
- **Code model**: `kernel` — efficient RIP-relative addressing in the higher half
- **Disabled features**: SSE, SSE2, AVX — the kernel doesn't save FPU state on interrupts
- **Soft float**: Avoids SIMD entirely (no `xmm` registers in Ring 0)
- **Build-std**: Rebuilds `core` and `alloc` from source for the bare metal target

---

## Linker Script (`kernel/linker.ld`)

Controls the kernel's memory layout in virtual address space.

### Higher-Half Mapping

The kernel is placed at `0xFFFFFFFF80200000` — the top 2 GB of the 64-bit address space plus a 2 MB offset:

- **Lower half** (`0x0000...7FFF...`) — reserved for userspace
- **Canonical hole** — hardware-enforced gap (invalid addresses)
- **Higher half** (`0xFFFF8000...`) — kernel territory

### Sections

Each section is page-aligned (4 KiB) to enable per-section page table permissions:

| Section | Contents | Permissions |
|:--------|:---------|:-----------|
| `.text` | Executable code (`kmain` first) | R+X |
| `.rodata` | Constants, strings, Limine requests, `.got` entries | R |
| `.data` | Initialized mutable globals | R+W |
| `.bss` | Zero-initialized globals | R+W |

Boundary symbols (`_kernel_start`, `_kernel_end`, `_text_start`, etc.) let the kernel know its own memory layout at runtime.

---

## Boot Protocol (`arch/x86_64/boot.rs`)

Uses the [Limine boot protocol](https://limine-bootloader.org/) to communicate with the bootloader. The kernel defines static request structures; Limine fills in the responses before jumping to `kmain()`.

### Requests

| Request | Data Provided |
|:--------|:-------------|
| `HhdmRequest` | HHDM offset — where all physical RAM is mapped in virtual space |
| `MemoryMapRequest` | Physical memory map — usable, reserved, ACPI, framebuffer regions |
| `FramebufferRequest` | Framebuffer address, dimensions, pixel format |
| `RsdpRequest` | ACPI RSDP table pointer (for hardware discovery) |
| `ExecutableAddressRequest` | Kernel physical/virtual load addresses |

### Bootloader Configuration (`boot/limine.conf`)

```
timeout: 0
serial: yes

/MinimalOS NextGen
    protocol: limine
    kernel_path: boot():/boot/minimalos-kernel
```

---

## Serial UART (`arch/x86_64/serial.rs`)

A 16550-compatible UART driver for COM1 (`0x3F8`), the first output device initialized during boot.

### Configuration

- **Baud rate**: 115200
- **Format**: 8 data bits, no parity, 1 stop bit (8N1)
- **FIFO**: Enabled, 14-byte trigger level

### Key Details

- Uses direct x86 port I/O (`outb`/`inb`)
- Protected by a ticket spinlock for thread safety
- Implements `core::fmt::Write` for `write!()` macro support
- Polls the transmit holding register before each byte

---

## CPU Primitives (`arch/x86_64/cpu.rs`)

Low-level CPU operations used throughout the kernel:

| Function | Instruction | Purpose |
|:---------|:-----------|:--------|
| `halt()` | `hlt` | Halt CPU until next interrupt |
| `halt_forever()` | `cli; hlt` loop | Permanent halt (used after boot) |
| `read_cr2()` | `mov rax, cr2` | Faulting address on page fault |
| `read_cr3()` | `mov rax, cr3` | Current page table base |
| `write_cr3()` | `mov cr3, rax` | Switch page tables |
| `invlpg()` | `invlpg [addr]` | Flush single TLB entry |
| `flush_tlb_all()` | Write CR3 | Flush entire TLB |
| `rdtsc()` | `rdtsc` | Read timestamp counter |
| `rdmsr()` / `wrmsr()` | `rdmsr` / `wrmsr` | Read/write model-specific registers |

---

## Logging (`util/logger.rs`)

Provides `kprint!()` and `kprintln!()` macros that write to the serial port. These are the kernel's primary debugging tool.

```rust
kprintln!("[boot] HHDM offset: {:#018X}", hhdm_offset);
kprintln!("[pmm] {} free frames", stats.free_frames);
```

Uses Rust's `core::fmt` formatting infrastructure — supports `{}`, `{:#X}`, `{:?}`, and all standard format specifiers.

---

## Framebuffer Console (`drivers/framebuffer.rs`)

A text-mode console rendered on the UEFI framebuffer using a built-in 8×16 bitmap font.

### Features

- Renders ASCII characters using a 256-glyph bitmap font
- White text on black background
- Automatic line wrapping and scrolling
- Supports newline, carriage return, and tab characters
- Direct pixel manipulation via the linear framebuffer

### How It Works

1. Limine provides a framebuffer with address, dimensions, and pixel format
2. The console tracks a cursor position (row, column) on a character grid
3. Each character write renders 8×16 pixels from the font bitmap
4. When the cursor reaches the bottom, the screen scrolls up by one row (memcopy)

---

## Synchronization (`sync/spinlock.rs`)

An IRQ-safe **ticket spinlock** used to protect shared kernel state.

### How Ticket Locks Work

Unlike test-and-set spinlocks that cause thundering herd, ticket locks provide FIFO fairness:

1. Arriving thread atomically increments `next_ticket` and saves its ticket number
2. Thread spins until `now_serving` equals its ticket
3. On unlock, `now_serving` is incremented, waking the next waiter

### Properties

- **IRQ-safe**: Disables interrupts while the lock is held
- **FIFO ordering**: Prevents starvation
- **Bounded spin**: Each waiter spins O(1) times per advancement
- Uses `AtomicU64` with `Ordering::Acquire` / `Ordering::Release` for correctness

---

## Address Types (`memory/address.rs`)

Type-safe wrappers for physical and virtual addresses:

| Type | Range | Use |
|:-----|:------|:----|
| `PhysAddr` | Physical memory (0 → ~8 GB) | PMM, DMA, page table entries |
| `VirtAddr` | Virtual memory (full 64-bit) | Pointers, kernel addresses |

### HHDM Translation

The Higher Half Direct Map (HHDM) provides a simple, total mapping of all physical memory into the kernel's virtual address space:

```rust
// Convert physical address to virtual via HHDM
let virt = phys_addr.to_virt();  // phys + HHDM_OFFSET

// Convert virtual HHDM address back to physical
let phys = VirtAddr::to_phys(virt);  // virt - HHDM_OFFSET
```

The HHDM offset is provided by Limine at boot and stored in a global.

---

## Panic Handler (`util/panic.rs`)

On panic, the kernel:

1. Prints the panic message and source location to serial
2. Halts all CPU cores indefinitely

In a microkernel, panics should be extremely rare — they indicate a bug in the trusted computing base.

---

## Entry Point (`main.rs`)

The `kmain()` function orchestrates the entire boot sequence through 5 phases. Sprint 1 implements Phases 1–2:

```rust
#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    // Phase 1: Init serial → kprintln!() works
    // Phase 2: Parse boot info, init framebuffer
    // Phase 3: Memory management (Sprint 2)
    // Phase 4: Scheduler + processes (Sprint 3-4)
    // Phase 5: Userspace entry (Sprint 5-7)

    arch::cpu::halt_forever()
}
```

---

## Files Implemented

| File | Lines | Purpose |
|:-----|:------|:--------|
| `rust-toolchain.toml` | ~15 | Nightly toolchain pin |
| `.cargo/config.toml` | ~30 | Bare-metal build config |
| `Cargo.toml` | ~50 | Workspace configuration |
| `kernel/Cargo.toml` | ~60 | Kernel dependencies |
| `kernel/linker.ld` | ~180 | Memory layout |
| `boot/limine.conf` | ~5 | Bootloader config |
| `kernel/src/main.rs` | ~420 | Entry point |
| `kernel/src/arch/x86_64/boot.rs` | ~200 | Limine protocol |
| `kernel/src/arch/x86_64/serial.rs` | ~200 | Serial UART |
| `kernel/src/arch/x86_64/cpu.rs` | ~180 | CPU primitives |
| `kernel/src/memory/address.rs` | ~120 | Address types |
| `kernel/src/sync/spinlock.rs` | ~150 | Ticket spinlock |
| `kernel/src/drivers/framebuffer.rs` | ~350 | Framebuffer console |
| `kernel/src/util/logger.rs` | ~50 | kprint! macros |
| `kernel/src/util/panic.rs` | ~20 | Panic handler |
| `Makefile` | ~290 | Build system |
