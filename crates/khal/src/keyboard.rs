//! PS/2 keyboard driver — [038]-[042].
//!
//! Uses the `pc-keyboard` crate for proper scancode decoding via a
//! three-layer state machine: scancode decoder → modifier tracker →
//! layout mapper.  Handles Shift, CapsLock, extended keys (arrows),
//! and key-release events correctly.

use crate::port::{inb, outb};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use spin::Mutex;

// ── PS/2 controller ports ─────────────────────────────────────────

/// Data port — read scancodes, write commands to device.
const PS2_DATA: u16 = 0x60;
/// Status / command port.
const PS2_STATUS: u16 = 0x64;

// ── PIC constants (IRQ1 = keyboard, remapped to vector 33) ────────

const PIC1_DATA: u16 = 0x21;
const PIC1_COMMAND: u16 = 0x20;
const PIC_EOI: u8 = 0x20;

/// IRQ vector for the keyboard (PIC1 base 32 + IRQ1).
pub const KEYBOARD_VECTOR: u8 = 33;

// ── Global keyboard state machine ─────────────────────────────────

/// The `pc-keyboard` state machine: decodes raw scancodes into key
/// events, tracks Shift / Ctrl / Alt / CapsLock state, and maps keys
/// through the US-104 QWERTY layout.
static KEYBOARD: Mutex<Option<Keyboard<layouts::Us104Key, ScancodeSet1>>> =
    Mutex::new(None);

/// Initialise the keyboard state machine.
///
/// Must be called once before [`handle_scancode()`].
pub fn init() {
    let kb = Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::MapLettersToUnicode,
    );
    *KEYBOARD.lock() = Some(kb);
}

/// Feed a raw scancode byte into the state machine.
///
/// Returns `Some(char)` when the scancode resolves to a printable
/// Unicode character (including Shift / CapsLock variants).
/// Returns `None` for key-release events, modifier-only presses,
/// and special keys (arrows, F-keys, etc.).
pub fn handle_scancode(scancode: u8) -> Option<char> {
    let mut guard = KEYBOARD.lock();
    let kb = guard.as_mut()?;

    // Layer 1: Scancode decoder — handles multi-byte sequences (0xE0 prefix).
    if let Ok(Some(event)) = kb.add_byte(scancode) {
        // Layer 2+3: Modifier tracker + layout mapper.
        if let Some(key) = kb.process_keyevent(event) {
            match key {
                DecodedKey::Unicode(ch) => return Some(ch),
                DecodedKey::RawKey(_key_code) => {
                    // TODO: handle arrows / F-keys for the shell
                    return None;
                }
            }
        }
    }

    None
}

// ── Low-level port helpers ────────────────────────────────────────

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
