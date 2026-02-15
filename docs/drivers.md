# Drivers

## Current Status

Driver support is in the **early stages**. The kernel currently requests a framebuffer
from the Limine bootloader but does not yet render to it. Individual driver modules
have not been implemented.

## Framebuffer Display (`kdisplay` crate)

The `kdisplay` crate (`crates/kdisplay/src/lib.rs`) is a `#![no_std]` library intended
to provide:

- Framebuffer initialisation from the Limine `FramebufferResponse`.
- Pixel plotting and rectangle drawing primitives.
- A text console with bitmap font rendering.
- Colour support and scrolling.

The kernel's `_start` function already obtains the framebuffer response—`kdisplay` will
consume it.

## Hardware Abstraction Layer (`khal` crate)

The `khal` crate (`crates/khal/src/lib.rs`) is a `#![no_std]` library that will wrap
low-level hardware access:

- **Port I/O** — reading and writing x86 I/O ports.
- **PIC (8259)** — Programmable Interrupt Controller initialisation and masking.
- **PIT (8254)** — Programmable Interval Timer configuration for periodic ticks.
- **PS/2 Controller** — Keyboard scan-code reading.

The `x86_64` crate provides the underlying port I/O primitives.

## Planned Drivers

| Driver             | Location / Crate | Description                          |
|--------------------|-------------------|--------------------------------------|
| Framebuffer/Console| `kdisplay`        | VESA framebuffer text rendering      |
| PS/2 Keyboard      | `khal`            | Scan-code decoding, key events       |
| PIT Timer          | `khal`            | 100 Hz system tick                   |
| Serial (COM1)      | `khal`            | Debug output over serial port        |
| PIC                | `khal`            | IRQ routing and acknowledgement      |

## Interrupt Integration

Once the `traps` module implements the IDT and IRQ handlers, drivers will register
their interrupt service routines:

- **IRQ 0** — PIT timer tick → scheduler.
- **IRQ 1** — PS/2 keyboard → input buffer.

## Quest Tracking

Related quests from `QUESTS.md`:

- **[006]** Framebuffer Display (kdisplay)
- **[007]** Hardware Abstraction Layer (khal)
