//! I/O APIC (Input/Output Advanced Programmable Interrupt Controller) driver.
//!
//! The I/O APIC receives external hardware interrupts (keyboard, mouse, etc.)
//! and routes them to the appropriate Local APIC via redirection entries.
//! This is the modern interrupt routing mechanism that replaces the legacy 8259 PIC.
//!
//! On x86-64 systems with APIC, the I/O APIC is typically located at physical
//! address 0xFEC0_0000. Each I/O APIC pin has a 64-bit redirection entry that
//! specifies the destination vector, delivery mode, and target CPU.

use core::ptr;

/// Standard I/O APIC physical base address.
pub const IOAPIC_PHYS_BASE: u64 = 0xFEC0_0000;

/// Register select (IOREGSEL) offset from base — write the register index here.
#[allow(dead_code)]
const IOREGSEL_OFFSET: u64 = 0x00;
/// Data window (IOWIN) offset from base — read/write the selected register here.
const IOWIN_OFFSET: u64 = 0x10;

/// I/O APIC register indices.
const IOAPICID: u32 = 0x00;
const IOAPICVER: u32 = 0x01;

/// Redirection Table Entry base index (each entry uses 2 consecutive indices).
const IOREDTBL_BASE: u32 = 0x10;

/// Virtual base address of the I/O APIC MMIO region (set during init).
static mut IOAPIC_BASE: u64 = 0;

// ── Raw register access ──────────────────────────────────────────

/// Read a 32-bit I/O APIC register by index.
#[inline]
unsafe fn read_reg(index: u32) -> u32 {
	let base = IOAPIC_BASE;
	ptr::write_volatile(base as *mut u32, index);
	ptr::read_volatile((base + IOWIN_OFFSET) as *const u32)
}

/// Write a 32-bit I/O APIC register by index.
#[inline]
unsafe fn write_reg(index: u32, value: u32) {
	let base = IOAPIC_BASE;
	ptr::write_volatile(base as *mut u32, index);
	ptr::write_volatile((base + IOWIN_OFFSET) as *mut u32, value);
}

// ── Public API ───────────────────────────────────────────────────

/// Initialise the I/O APIC.
///
/// Maps the I/O APIC MMIO region using the HHDM offset, reads its
/// version register, and masks all redirection entries so no stale
/// interrupts fire before they are explicitly enabled.
///
/// Returns `(id, max_entries)`.
pub fn init(hhdm_offset: u64) -> (u32, u32) {
	unsafe {
		IOAPIC_BASE = hhdm_offset + IOAPIC_PHYS_BASE;

		let id = read_reg(IOAPICID) >> 24;
		let ver = read_reg(IOAPICVER);
		let max_entries = ((ver >> 16) & 0xFF) + 1;

		// Mask every redirection entry (bit 16 = mask).
		for irq in 0..max_entries {
			let lo_index = IOREDTBL_BASE + irq * 2;
			let lo = read_reg(lo_index);
			write_reg(lo_index, lo | (1 << 16)); // set mask bit
		}

		(id, max_entries)
	}
}

/// Enable an ISA IRQ by programming its I/O APIC redirection entry.
///
/// ISA interrupts are edge-triggered, active-high by default.
/// The interrupt is delivered as a fixed interrupt to the BSP
/// (APIC ID 0).
///
/// # Arguments
///
/// * `irq`    - ISA IRQ number (0–23)
/// * `vector` - IDT vector number to deliver
pub fn enable_irq(irq: u8, vector: u8) {
	let lo_index = IOREDTBL_BASE + (irq as u32) * 2;
	let hi_index = lo_index + 1;

	// Low 32 bits of the redirection entry:
	//   bits  0-7  : vector
	//   bits  8-10 : delivery mode (000 = Fixed)
	//   bit   11   : destination mode (0 = physical)
	//   bit   13   : pin polarity (0 = active high)
	//   bit   15   : trigger mode (0 = edge)
	//   bit   16   : mask (0 = enabled)
	let lo: u32 = vector as u32; // everything else 0 → fixed, physical, active-high, edge, unmasked

	// High 32 bits: destination APIC ID in bits 24-31.
	let hi: u32 = 0 << 24; // APIC ID 0 (BSP)

	unsafe {
		write_reg(hi_index, hi);
		write_reg(lo_index, lo);
	}
}

/// Mask (disable) an ISA IRQ in the I/O APIC.
#[allow(dead_code)]
pub fn disable_irq(irq: u8) {
	let lo_index = IOREDTBL_BASE + (irq as u32) * 2;
	unsafe {
		let lo = read_reg(lo_index);
		write_reg(lo_index, lo | (1 << 16));
	}
}
