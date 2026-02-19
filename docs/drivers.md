---
layout: default
title: Drivers
---

# Drivers

## Overview

MinimalOS includes drivers for the core hardware needed to run an interactive
system: display output, keyboard input, timer interrupts, and serial debugging.
All drivers are implemented across two crates (`kdisplay` and `khal`) plus
the kernel's interrupt handlers.

## Framebuffer Display (`kdisplay`)

**Crate:** `crates/kdisplay/src/lib.rs`

The Limine bootloader provides a linear framebuffer (no legacy VGA text mode).
The `kdisplay` crate renders everything pixel-by-pixel.

### Features

| Feature | Description |
|---------|-------------|
| `fill_screen(fb, color)` | Fill entire framebuffer with a solid colour |
| `draw_char(x, y, ch, fg)` | Render a single character using an 8×16 bitmap font |
| `init_console(fb, fg, bg)` | Initialise a text console with cursor tracking |
| `kprint!` / `kprintln!` | Formatted text output macros (like `println!`) |
| Scrolling | When text reaches the bottom, the framebuffer is shifted up by one row |
| Backspace | Erases the previous character and moves the cursor back |
| Colour support | `Color` struct with constants: `WHITE`, `BLUE`, `BLACK`, `GREEN`, etc. |

### Bitmap Font

Characters are rendered from an embedded 8×16 bitmap font array. Each character
is 16 bytes (one byte per row, 8 pixels wide). Only printable ASCII (32–126) is
supported; unknown characters render as a filled block.

### Console State

A global `Console` is protected by a `spin::Mutex` and stores:

- Current cursor position (column, row)
- Foreground and background colours
- Framebuffer pointer, dimensions, and pitch
- Character grid dimensions (derived from pixel resolution / font size)

## Hardware Abstraction Layer (`khal`)

**Crate:** `crates/khal/src/lib.rs`

The `khal` crate wraps low-level hardware access behind safe(r) Rust interfaces.

### Port I/O

```rust
pub unsafe fn inb(port: u16) -> u8
pub unsafe fn outb(port: u16, value: u8)
```

Thin wrappers around the x86 `in` / `out` instructions, used by all other
hardware drivers.

### PIC (8259 Programmable Interrupt Controller)

```rust
pub fn disable()
```

The legacy dual-PIC is remapped to vectors 32–47 and then fully masked
(all IRQ lines disabled). MinimalOS uses the Local APIC instead.

### APIC (Advanced Programmable Interrupt Controller)

```rust
pub fn init(hhdm_offset: u64) -> u32		// Returns APIC ID
pub fn enable_timer(vector: u8, count: u32, divide: TimerDivide)
pub fn eoi()								 // End-of-interrupt
```

The Local APIC is accessed via MMIO registers mapped through the HHDM.
The APIC timer is configured in **periodic mode** with a divider of 16,
generating interrupts at approximately 100 Hz (count = `0x0020_0000`).

| Register | Offset | Purpose |
|----------|--------|---------|
| Spurious Interrupt Vector | `0xF0` | Enable APIC, set spurious vector |
| Timer LVT | `0x320` | Vector, mode (periodic), mask |
| Timer Initial Count | `0x380` | Countdown value |
| Timer Divide Config | `0x3E0` | Clock divider |
| EOI | `0xB0` | Acknowledge interrupt |

### PS/2 Keyboard

```rust
pub fn read_status() -> u8		  // Read PS/2 status register (port 0x64)
pub fn init()					   // Initialise pc-keyboard decoder
pub fn enable_irq()				 // Unmask IRQ1 in the APIC
pub fn handle_scancode() -> Option<DecodedKey>  // Process one scancode
```

The keyboard driver uses the [`pc-keyboard`](https://crates.io/crates/pc-keyboard)
crate (v0.7) for scancode decoding with full support for:

- Scancode Set 1
- US 104-key layout
- Modifier keys (Shift, Ctrl, Alt, Caps Lock)
- Special keys (arrows, function keys, etc.)

Raw scancodes are read from port `0x60`. The `pc-keyboard` crate translates
them into `DecodedKey::Unicode(char)` or `DecodedKey::RawKey(KeyCode)` events.

### Serial Port (COM1)

```rust
pub fn init()					   // Initialise COM1 at 115200 baud
pub fn write_byte(b: u8)		   // Transmit one byte
pub fn write_str(s: &str)		  // Transmit a string
```

The serial driver is used by the `klog` crate for debug output. It configures
COM1 (I/O base `0x3F8`) with:

| Parameter | Value |
|-----------|-------|
| Baud rate | 115200 |
| Data bits | 8 |
| Stop bits | 1 |
| Parity | None |
| FIFO | Enabled (14-byte trigger) |

Serial output is captured by QEMU's `-serial stdio` or `-serial file:` options.

## Interrupt Handlers

**Files:** `kernel/src/traps/handlers.rs`, `kernel/src/traps/idt.rs`

### IDT Configuration

The IDT contains 256 entries:

| Vector | Source | Handler |
|--------|--------|---------|
| 0 | Divide Error | Panic with register dump |
| 3 | Breakpoint | Log and continue |
| 6 | Invalid Opcode | Panic |
| 8 | Double Fault | Panic (uses IST1 stack) |
| 13 | General Protection | Panic with error code |
| 14 | Page Fault | Panic with CR2 address |
| 32 | APIC Timer (IRQ0) | Call `do_schedule()` for preemption |
| 33 | Keyboard (IRQ1) | Read scancode, push to input buffer |
| 255 | Spurious | Ignored (no EOI) |

### Timer Handler (Vector 32)

The timer interrupt drives preemptive multitasking:

1. Send EOI to the APIC (must be done before scheduling).
2. Call `do_schedule()` to potentially switch to another process.
3. If no context switch occurs, return normally.

### Keyboard Handler (Vector 33)

1. Read the scancode via `khal::keyboard::handle_scancode()`.
2. If a printable character is decoded, push it to the kernel's input ring
   buffer (`task::input::push()`).
3. Echo the character to the framebuffer console.
4. Send EOI.

## Kernel Logging (`klog`)

**Crate:** `crates/klog/src/lib.rs`

The `klog` crate provides formatted logging macros that output to the serial
port:

```rust
klog::info!("Boot complete");
klog::debug!("HHDM offset: {:#x}", offset);
klog::warn!("Unexpected condition");
klog::error!("Fatal: {}", msg);
```

Each message is prefixed with the log level and a newline. Output goes to
COM1, visible in the host terminal via QEMU's serial redirect.
