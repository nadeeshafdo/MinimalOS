//! PS/2 keyboard driver — [038]-[042], [074].
//!
//! Uses the `pc-keyboard` crate for proper scancode decoding via a
//! three-layer state machine: scancode decoder → modifier tracker →
//! layout mapper.  Handles Shift, CapsLock, extended keys (arrows),
//! and key-release events correctly.
//!
//! [074] Exposes structured `KeyEvent` with press/release state,
//! decoded key, and raw scancode — used by the kernel EventBuffer.

use crate::port::{inb, outb};
use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};
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

// ── [074] Structured key event ────────────────────────────────────

/// Whether a key was pressed or released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

/// A decoded key — either a Unicode character or a raw keycode.
#[derive(Debug, Clone, Copy)]
pub enum KeyKind {
    /// Printable character (affected by Shift / CapsLock).
    Char(char),
    /// Non-printable key (arrows, F-keys, modifiers, etc.).
    Raw(KeyCode),
}

/// [074] A structured keyboard event carrying press/release state,
/// the decoded key, and the originating scancode byte.
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub state: KeyState,
    pub key: KeyKind,
    pub scancode: u8,
}

// ── Global keyboard state machine ─────────────────────────────────

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

/// [074] Feed a raw scancode and return a structured `KeyEvent`.
///
/// Returns `Some(KeyEvent)` for every key press *and* release that
/// the state machine can decode.  This is richer than [`handle_scancode`]
/// which only returns characters on press.
pub fn handle_scancode_event(scancode: u8) -> Option<KeyEvent> {
    let mut guard = KEYBOARD.lock();
    let kb = guard.as_mut()?;

    if let Ok(Some(event)) = kb.add_byte(scancode) {
        let state = match event.state {
            pc_keyboard::KeyState::Down => KeyState::Pressed,
            pc_keyboard::KeyState::Up => KeyState::Released,
            _ => return None,
        };

        // Save the raw keycode before consuming the event.
        let raw_code = event.code;

        // Try to decode to a character via layout.
        if let Some(decoded) = kb.process_keyevent(event) {
            let key = match decoded {
                DecodedKey::Unicode(ch) => KeyKind::Char(ch),
                DecodedKey::RawKey(code) => KeyKind::Raw(code),
            };
            return Some(KeyEvent { state, key, scancode });
        }

        // Modifier-only press/release (Shift, Ctrl, etc.) —
        // process_keyevent returns None, but we still have a code.
        return Some(KeyEvent {
            state,
            key: KeyKind::Raw(raw_code),
            scancode,
        });
    }

    None
}

/// Feed a raw scancode byte into the state machine.
///
/// Returns `Some(char)` when the scancode resolves to a printable
/// Unicode character on *press*.  Returns `None` for releases,
/// modifier-only presses, and special keys.
///
/// This is the simple API kept for backward compatibility; prefer
/// [`handle_scancode_event()`] for full event information.
pub fn handle_scancode(scancode: u8) -> Option<char> {
    let mut guard = KEYBOARD.lock();
    let kb = guard.as_mut()?;

    if let Ok(Some(event)) = kb.add_byte(scancode) {
        if let Some(key) = kb.process_keyevent(event) {
            match key {
                DecodedKey::Unicode(ch) => return Some(ch),
                DecodedKey::RawKey(_) => return None,
            }
        }
    }

    None
}

// ── Low-level port helpers ────────────────────────────────────────

pub fn read_status() -> u8 { unsafe { inb(PS2_STATUS) } }
pub fn read_scancode() -> u8 { unsafe { inb(PS2_DATA) } }

pub fn enable_irq() {
    unsafe {
        let mask = inb(PIC1_DATA);
        outb(PIC1_DATA, mask & !0x02);
    }
}

pub fn send_eoi() {
    unsafe { outb(PIC1_COMMAND, PIC_EOI); }
}
