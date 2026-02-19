//! PS/2 mouse driver — [075]-[076].
//!
//! Initialises the PS/2 auxiliary device (mouse) and decodes the
//! standard 3-byte packet format into `(dx, dy, buttons)` tuples.
//!
//! The PS/2 mouse sits on the second port of the i8042 controller.
//! Commands are sent via port 0x64 (write 0xD4 prefix) then 0x60.
//! Data arrives on port 0x60 as IRQ12 (PIC2 IRQ4 → vector 44).

use crate::port::{inb, outb};
use spin::Mutex;

// ── PS/2 controller ports ─────────────────────────────────────────

const PS2_DATA: u16 = 0x60;
const PS2_STATUS: u16 = 0x64;
const PS2_COMMAND: u16 = 0x64;

// ── PIC constants (IRQ12 = mouse) ─────────────────────────────────

const PIC1_DATA: u16 = 0x21;
const PIC2_DATA: u16 = 0xA1;
const PIC2_COMMAND: u16 = 0xA0;
const PIC1_COMMAND: u16 = 0x20;
const PIC_EOI: u8 = 0x20;

/// IRQ vector for the mouse (PIC2 base 40 + IRQ4 = 44).
pub const MOUSE_VECTOR: u8 = 44;

// ── Mouse button flags ────────────────────────────────────────────

/// Left button is held down.
pub const BTN_LEFT: u8 = 0x01;
/// Right button is held down.
pub const BTN_RIGHT: u8 = 0x02;
/// Middle button is held down.
pub const BTN_MIDDLE: u8 = 0x04;

// ── [076] Mouse packet decoder ────────────────────────────────────

/// A decoded mouse movement event.
#[derive(Debug, Clone, Copy)]
pub struct MousePacket {
    /// Horizontal movement (signed, positive = right).
    pub dx: i16,
    /// Vertical movement (signed, positive = up / negative = down).
    pub dy: i16,
    /// Button state (BTN_LEFT | BTN_RIGHT | BTN_MIDDLE).
    pub buttons: u8,
}

/// Internal state machine for assembling 3-byte packets.
struct PacketDecoder {
    /// Byte buffer [status, dx, dy].
    buf: [u8; 3],
    /// Index of the next byte to receive (0, 1, or 2).
    index: u8,
}

impl PacketDecoder {
    const fn new() -> Self {
        Self {
            buf: [0; 3],
            index: 0,
        }
    }

    /// Feed one byte from the mouse.
    ///
    /// Returns `Some(MousePacket)` when a complete 3-byte packet is
    /// assembled and decoded.
    fn feed(&mut self, byte: u8) -> Option<MousePacket> {
        // Byte 0 (status) must always have bit 3 set (sync bit).
        // If it doesn't and we're expecting byte 0, re-sync.
        if self.index == 0 && (byte & 0x08) == 0 {
            // Out of sync — discard and wait for a valid status byte.
            return None;
        }

        self.buf[self.index as usize] = byte;
        self.index += 1;

        if self.index < 3 {
            return None; // packet incomplete
        }

        // Full packet received — decode.
        self.index = 0;

        let status = self.buf[0];
        let raw_dx = self.buf[1] as i16;
        let raw_dy = self.buf[2] as i16;

        // Apply sign extension from the status byte (bits 4-5).
        let dx = if status & 0x10 != 0 { raw_dx - 256 } else { raw_dx };
        let dy = if status & 0x20 != 0 { raw_dy - 256 } else { raw_dy };

        let buttons = status & 0x07;

        Some(MousePacket { dx, dy, buttons })
    }
}

static DECODER: Mutex<PacketDecoder> = Mutex::new(PacketDecoder::new());

/// Feed a raw byte from the mouse IRQ handler.
///
/// Returns `Some(MousePacket)` when a complete 3-byte packet is decoded.
pub fn handle_byte(byte: u8) -> Option<MousePacket> {
    DECODER.lock().feed(byte)
}

// ── [075] PS/2 mouse initialisation ───────────────────────────────

/// Wait until the PS/2 controller's input buffer is empty (ready to
/// accept a command).
fn wait_write() {
    for _ in 0..100_000 {
        if unsafe { inb(PS2_STATUS) } & 0x02 == 0 {
            return;
        }
    }
}

/// Wait until the PS/2 controller's output buffer has data to read.
fn wait_read() {
    for _ in 0..100_000 {
        if unsafe { inb(PS2_STATUS) } & 0x01 != 0 {
            return;
        }
    }
}

/// Send a command byte to the PS/2 controller (port 0x64).
fn controller_cmd(cmd: u8) {
    wait_write();
    unsafe { outb(PS2_COMMAND, cmd); }
}

/// Send a data byte to the mouse via the controller's auxiliary port.
fn mouse_write(byte: u8) {
    controller_cmd(0xD4); // prefix: next byte goes to auxiliary device
    wait_write();
    unsafe { outb(PS2_DATA, byte); }
}

/// Read one byte from the PS/2 data port (with timeout).
fn mouse_read() -> u8 {
    wait_read();
    unsafe { inb(PS2_DATA) }
}

/// Initialise the PS/2 mouse.
///
/// Enables the auxiliary port, resets the mouse, sets defaults, and
/// enables data reporting.  Must be called before enabling IRQ12.
pub fn init() {
    // 1. Enable the auxiliary (mouse) port on the PS/2 controller.
    controller_cmd(0xA8);

    // 2. Read the current controller configuration byte.
    controller_cmd(0x20);
    let config = mouse_read();

    // 3. Enable IRQ12 (bit 1) and clear the auxiliary clock disable
    //    bit (bit 5) in the config byte.
    let new_config = (config | 0x02) & !0x20;
    controller_cmd(0x60); // write configuration
    wait_write();
    unsafe { outb(PS2_DATA, new_config); }

    // 4. Send "Set Defaults" (0xF6) to the mouse.
    mouse_write(0xF6);
    let _ack = mouse_read(); // expect 0xFA

    // 5. Enable data reporting (0xF4) — mouse will start sending
    //    packets on movement / button presses.
    mouse_write(0xF4);
    let _ack = mouse_read(); // expect 0xFA
}

/// Enable IRQ12 in the legacy PIC (unmask PIC2 bit 4 and PIC1 cascade).
pub fn enable_irq() {
    unsafe {
        // Unmask IRQ2 on PIC1 (cascade line to PIC2).
        let mask1 = inb(PIC1_DATA);
        outb(PIC1_DATA, mask1 & !0x04);

        // Unmask IRQ12 on PIC2 (IRQ12 = bit 4 of PIC2).
        let mask2 = inb(PIC2_DATA);
        outb(PIC2_DATA, mask2 & !0x10);
    }
}

/// Send End-of-Interrupt for IRQ12 (must EOI both PIC2 and PIC1).
pub fn send_eoi() {
    unsafe {
        outb(PIC2_COMMAND, PIC_EOI);
        outb(PIC1_COMMAND, PIC_EOI);
    }
}

/// Read a byte from the PS/2 data port (for use in the IRQ handler).
pub fn read_data() -> u8 {
    unsafe { inb(PS2_DATA) }
}

/// Check whether the PS/2 status register indicates mouse data
/// (bit 0 = output buffer full, bit 5 = auxiliary data).
pub fn is_mouse_data() -> bool {
    let status = unsafe { inb(PS2_STATUS) };
    (status & 0x21) == 0x21 // bit 0 + bit 5
}
