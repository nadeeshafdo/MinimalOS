//! PS/2 keyboard driver — [038]-[042].
//!
//! Reads scancodes from port 0x60 via IRQ1 (vector 33) and converts
//! Scancode Set 1 key-down events to ASCII characters.

use crate::port::{inb, outb};
use spin::Mutex;

// ── PS/2 controller ports ─────────────────────────────────────────

/// Data port — read scancodes, write commands to device.
const PS2_DATA: u16 = 0x60;
/// Status / command port.
const PS2_STATUS: u16 = 0x64;

// Status register bits
const STATUS_OUTPUT_FULL: u8 = 1 << 0;

// ── PIC constants (IRQ1 = keyboard, remapped to vector 33) ────────

const PIC1_DATA: u16 = 0x21;
const PIC1_COMMAND: u16 = 0x20;
const PIC_EOI: u8 = 0x20;

/// IRQ vector for the keyboard (PIC1 base 32 + IRQ1).
pub const KEYBOARD_VECTOR: u8 = 33;

// ── Scancode Set 1 → ASCII table (key-down only, US layout) ──────

/// Convert a Scancode Set 1 make-code to ASCII.
/// Returns `None` for non-printable keys or key-up (release) codes.
pub fn scancode_to_ascii(scancode: u8) -> Option<char> {
    // Bit 7 set = key release (break code) — ignore
    if scancode & 0x80 != 0 {
        return None;
    }

    // [040] Decoder — Scancode Set 1 US QWERTY layout
    let ch = match scancode {
        0x02 => '1', 0x03 => '2', 0x04 => '3', 0x05 => '4', 0x06 => '5',
        0x07 => '6', 0x08 => '7', 0x09 => '8', 0x0A => '9', 0x0B => '0',
        0x0C => '-', 0x0D => '=',
        0x0E => '\x08', // Backspace
        0x0F => '\t',   // Tab
        0x10 => 'q', 0x11 => 'w', 0x12 => 'e', 0x13 => 'r', 0x14 => 't',
        0x15 => 'y', 0x16 => 'u', 0x17 => 'i', 0x18 => 'o', 0x19 => 'p',
        0x1A => '[', 0x1B => ']',
        0x1C => '\n', // Enter
        0x1E => 'a', 0x1F => 's', 0x20 => 'd', 0x21 => 'f', 0x22 => 'g',
        0x23 => 'h', 0x24 => 'j', 0x25 => 'k', 0x26 => 'l',
        0x27 => ';', 0x28 => '\'', 0x29 => '`',
        0x2B => '\\',
        0x2C => 'z', 0x2D => 'x', 0x2E => 'c', 0x2F => 'v', 0x30 => 'b',
        0x31 => 'n', 0x32 => 'm',
        0x33 => ',', 0x34 => '.', 0x35 => '/',
        0x39 => ' ', // Space
        _ => return None,
    };

    Some(ch)
}

/// Read the PS/2 controller status register.
///
/// Returns the raw status byte; bit 0 = output buffer full (data ready).
pub fn read_status() -> u8 {
    unsafe { inb(PS2_STATUS) }
}

/// Read a scancode from the PS/2 data port.
///
/// Should only be called when [`read_status()`] indicates data is ready
/// (bit 0 set).
pub fn read_scancode() -> u8 {
    unsafe { inb(PS2_DATA) }
}

/// Enable the keyboard IRQ by unmasking IRQ1 in the legacy PIC.
///
/// The PIC was remapped (IRQ0-7 → vectors 32-39) but fully masked.
/// We unmask only bit 1 (IRQ1 = keyboard) on PIC1.
pub fn enable_irq() {
    unsafe {
        let mask = inb(PIC1_DATA);
        outb(PIC1_DATA, mask & !0x02); // clear bit 1 = unmask IRQ1
    }
}

/// Send End-of-Interrupt to PIC1 for IRQ1.
pub fn send_eoi() {
    unsafe {
        outb(PIC1_COMMAND, PIC_EOI);
    }
}

/// Last scancode received (for diagnostic / [039] verification).
static LAST_SCANCODE: Mutex<u8> = Mutex::new(0);

/// Store the last received scancode.
pub fn set_last_scancode(sc: u8) {
    *LAST_SCANCODE.lock() = sc;
}

/// Get the last received scancode.
pub fn last_scancode() -> u8 {
    *LAST_SCANCODE.lock()
}
