//! Legacy 8259 PIC (Programmable Interrupt Controller) driver.
//!
//! The 8259 PIC is the legacy interrupt controller used in x86 systems.
//! Modern systems use the APIC instead, but the 8259 PIC must be properly
//! remapped and disabled to prevent spurious interrupts from conflicting
//! with CPU exceptions (IRQ 0-7 overlap with exceptions 0-7 by default).

use crate::port::{inb, outb};

/// I/O port addresses for the master PIC.
const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;

/// I/O port addresses for the slave PIC.
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

/// ICW1: Initialization Command Word 1 - begin initialization sequence.
const ICW1_INIT: u8 = 0x10;
/// ICW1: ICW4 will be sent.
const ICW1_ICW4: u8 = 0x01;
/// ICW4: 8086/88 mode (as opposed to MCS-80/85 mode).
const ICW4_8086: u8 = 0x01;

/// Remap offset for PIC1 (IRQ 0-7 → vectors 32-39).
const PIC1_OFFSET: u8 = 32;
/// Remap offset for PIC2 (IRQ 8-15 → vectors 40-47).
const PIC2_OFFSET: u8 = 40;

/// Small I/O delay by writing to an unused port.
/// Some old hardware requires a delay between PIC commands.
#[inline]
fn io_wait() {
    unsafe {
        outb(0x80, 0);
    }
}

/// Remap the 8259 PIC interrupt vectors and then mask all IRQs.
///
/// By default, IRQ 0-7 are mapped to interrupt vectors 0x08-0x0F,
/// which overlap with CPU exception vectors. We remap them to
/// vectors 32-47 (out of the exception range), then mask all IRQs
/// to effectively disable the PIC.
///
/// This is required before enabling the APIC.
pub fn disable() {
    unsafe {
        // ICW1: Begin initialization (cascade mode, ICW4 needed)
        outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();
        outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();

        // ICW2: Set vector offsets
        outb(PIC1_DATA, PIC1_OFFSET);
        io_wait();
        outb(PIC2_DATA, PIC2_OFFSET);
        io_wait();

        // ICW3: Tell master PIC there is a slave PIC at IRQ2 (bit 2)
        outb(PIC1_DATA, 4);
        io_wait();
        // ICW3: Tell slave PIC its cascade identity (IRQ2 = 2)
        outb(PIC2_DATA, 2);
        io_wait();

        // ICW4: Set 8086 mode
        outb(PIC1_DATA, ICW4_8086);
        io_wait();
        outb(PIC2_DATA, ICW4_8086);
        io_wait();

        // Mask ALL IRQs on both PICs (0xFF = all bits set = all masked)
        outb(PIC1_DATA, 0xFF);
        outb(PIC2_DATA, 0xFF);
    }
}
