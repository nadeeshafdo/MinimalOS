// =============================================================================
// MinimalOS NextGen — I/O APIC Driver
// =============================================================================
//
// The I/O APIC routes external hardware interrupts (keyboard, serial, disk)
// to CPU cores. It replaces the legacy 8259A PIC, which must be disabled
// before the I/O APIC takes over.
//
// REGISTER ACCESS:
//   The I/O APIC uses indirect MMIO: two registers at the base address.
//     - IOREGSEL (offset 0x00): write the register index here
//     - IOWIN   (offset 0x10): read/write the register value here
//
//   To read register N: write N to IOREGSEL, read from IOWIN.
//   To write register N: write N to IOREGSEL, write to IOWIN.
//
// REDIRECTION TABLE:
//   Each I/O APIC pin has a 64-bit Redirection Table Entry (RTE).
//   RTEs are stored as pairs of 32-bit registers:
//     IOREDTBL[n] low  = register 0x10 + 2*n
//     IOREDTBL[n] high = register 0x11 + 2*n
//
//   RTE format:
//     Bits  0-7:   Vector (IDT entry)
//     Bits  8-10:  Delivery mode (000=Fixed, 001=Lowest Priority, etc.)
//     Bit   11:    Destination mode (0=Physical, 1=Logical)
//     Bit   12:    Delivery status (read-only: 0=idle, 1=pending)
//     Bit   13:    Pin polarity (0=active high, 1=active low)
//     Bit   14:    Remote IRR (read-only, for level-triggered)
//     Bit   15:    Trigger mode (0=edge, 1=level)
//     Bit   16:    Mask (1=disabled)
//     Bits 56-63:  Destination APIC ID (in high 32-bit register, bits 24-31)
//
// LEGACY IRQ MAPPING:
//   ISA IRQs 0-15 are typically mapped to I/O APIC pins 0-15, but the
//   MADT may contain Interrupt Source Override (ISO) entries that remap
//   specific IRQs. Common example: IRQ 0 (PIT) → GSI 2.
//
// =============================================================================

use core::sync::atomic::{AtomicU64, Ordering};

use crate::arch::x86_64::acpi::IsoOverride;
use crate::kprintln;
use crate::memory::address::PhysAddr;

// =============================================================================
// I/O APIC register indices
// =============================================================================

const IOAPICID: u32  = 0x00;
const IOAPICVER: u32 = 0x01;
const IOAPICARB: u32 = 0x02;

/// Returns the register index for the low 32 bits of RTE `n`.
const fn rte_low(n: u32) -> u32 { 0x10 + 2 * n }

/// Returns the register index for the high 32 bits of RTE `n`.
const fn rte_high(n: u32) -> u32 { 0x11 + 2 * n }

// =============================================================================
// Legacy 8259A PIC ports
// =============================================================================

const PIC1_DATA: u16 = 0x21;
const PIC2_DATA: u16 = 0xA1;

// =============================================================================
// Global state
// =============================================================================

/// Virtual base address of the I/O APIC registers.
static IOAPIC_BASE: AtomicU64 = AtomicU64::new(0);

/// GSI base of the currently initialized I/O APIC.
static GSI_BASE: AtomicU64 = AtomicU64::new(0);

/// Maximum number of redirection entries (from IOAPICVER).
static MAX_ENTRIES: AtomicU64 = AtomicU64::new(0);

// =============================================================================
// MMIO indirect register access
// =============================================================================

/// Reads an I/O APIC register by index.
#[inline]
fn read_ioapic(reg: u32) -> u32 {
    let base = IOAPIC_BASE.load(Ordering::Relaxed);
    debug_assert!(base != 0, "I/O APIC not initialized");
    unsafe {
        // Write register index to IOREGSEL (offset 0x00)
        core::ptr::write_volatile(base as *mut u32, reg);
        // Read value from IOWIN (offset 0x10)
        core::ptr::read_volatile((base + 0x10) as *const u32)
    }
}

/// Writes an I/O APIC register by index.
#[inline]
fn write_ioapic(reg: u32, value: u32) {
    let base = IOAPIC_BASE.load(Ordering::Relaxed);
    debug_assert!(base != 0, "I/O APIC not initialized");
    unsafe {
        core::ptr::write_volatile(base as *mut u32, reg);
        core::ptr::write_volatile((base + 0x10) as *mut u32, value);
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Disables the legacy 8259A PIC.
///
/// Must be called before enabling I/O APIC routing to prevent the 8259A
/// from delivering spurious interrupts. We mask all 16 IRQs on both PICs.
pub fn disable_pic() {
    unsafe {
        // Mask all IRQs on the master PIC (IRQ 0-7)
        port_out_u8(PIC1_DATA, 0xFF);
        // Mask all IRQs on the slave PIC (IRQ 8-15)
        port_out_u8(PIC2_DATA, 0xFF);
    }
    kprintln!("[pic] Legacy 8259 PIC disabled (all IRQs masked)");
}

/// Initializes an I/O APIC.
///
/// # Parameters
/// - `base_addr`: Physical address of the I/O APIC registers (from MADT)
/// - `gsi_base`: First GSI handled by this I/O APIC (from MADT)
/// - `overrides`: Interrupt Source Override entries from the MADT
///
/// # Steps
/// 1. Store the HHDM-mapped virtual base
/// 2. Read the IOAPICVER to get max redirection entries
/// 3. Mask all entries (disable all pins)
pub fn init(base_addr: PhysAddr, gsi_base: u32, overrides: &[IsoOverride]) {
    let base_virt = base_addr.to_virt().as_u64();
    IOAPIC_BASE.store(base_virt, Ordering::Relaxed);
    GSI_BASE.store(gsi_base as u64, Ordering::Relaxed);

    let id = (read_ioapic(IOAPICID) >> 24) & 0x0F;
    let ver = read_ioapic(IOAPICVER);
    let max_redir = ((ver >> 16) & 0xFF) as u32;
    MAX_ENTRIES.store(max_redir as u64, Ordering::Relaxed);

    kprintln!("[ioapic] I/O APIC ID={}, version={:#04X}, {} entries (GSI base={})",
        id, ver & 0xFF, max_redir + 1, gsi_base);

    // Mask all redirection entries initially.
    for i in 0..=max_redir {
        let lo = read_ioapic(rte_low(i));
        write_ioapic(rte_low(i), lo | (1 << 16)); // Set mask bit
    }

    // Log any overrides that apply to this I/O APIC's GSI range.
    for ovr in overrides {
        if ovr.gsi >= gsi_base && ovr.gsi <= gsi_base + max_redir {
            kprintln!("[ioapic]   Override: IRQ {} → GSI {} (pol={}, trig={})",
                ovr.irq_source, ovr.gsi, ovr.polarity, ovr.trigger);
        }
    }
}

/// Enables an IRQ by writing a Redirection Table Entry.
///
/// # Parameters
/// - `gsi`: Global System Interrupt number (use the ISO override GSI if applicable)
/// - `vector`: IDT vector to fire (e.g., 32 for LAPIC timer, 36 for COM1)
/// - `dest_apic_id`: Target LAPIC ID (0 for BSP)
///
/// # Panics
/// Debug-asserts that the GSI is within this I/O APIC's range.
pub fn enable_irq(gsi: u32, vector: u8, dest_apic_id: u8) {
    let base_gsi = GSI_BASE.load(Ordering::Relaxed) as u32;
    let max = MAX_ENTRIES.load(Ordering::Relaxed) as u32;
    let pin = gsi - base_gsi;

    debug_assert!(pin <= max,
        "GSI {} is outside I/O APIC range (base={}, max={})", gsi, base_gsi, max);

    // Build the RTE:
    //   Bits 0-7:  Vector
    //   Bits 8-10: Delivery mode = 000 (Fixed)
    //   Bit 11:    Destination mode = 0 (Physical)
    //   Bit 13:    Pin polarity = 0 (active high, default for ISA)
    //   Bit 15:    Trigger mode = 0 (edge, default for ISA)
    //   Bit 16:    Mask = 0 (enabled)
    let lo = vector as u32; // Everything else is 0 = sensible defaults

    // High 32 bits: bits 24-31 = destination APIC ID
    let hi = (dest_apic_id as u32) << 24;

    write_ioapic(rte_low(pin), lo);
    write_ioapic(rte_high(pin), hi);

    kprintln!("[ioapic] Enabled GSI {} → vector {} → APIC ID {}", gsi, vector, dest_apic_id);
}

/// Enables an IRQ with custom polarity and trigger mode.
///
/// Used when an Interrupt Source Override specifies non-default settings.
pub fn enable_irq_with_flags(
    gsi: u32,
    vector: u8,
    dest_apic_id: u8,
    active_low: bool,
    level_triggered: bool,
) {
    let base_gsi = GSI_BASE.load(Ordering::Relaxed) as u32;
    let pin = gsi - base_gsi;

    let mut lo = vector as u32;
    if active_low {
        lo |= 1 << 13; // Pin polarity: active low
    }
    if level_triggered {
        lo |= 1 << 15; // Trigger mode: level
    }

    let hi = (dest_apic_id as u32) << 24;

    write_ioapic(rte_low(pin), lo);
    write_ioapic(rte_high(pin), hi);

    kprintln!("[ioapic] Enabled GSI {} → vector {} ({}{})",
        gsi, vector,
        if active_low { "active-low" } else { "active-high" },
        if level_triggered { ", level" } else { ", edge" });
}

/// Masks (disables) an IRQ at the I/O APIC.
pub fn mask_irq(gsi: u32) {
    let base_gsi = GSI_BASE.load(Ordering::Relaxed) as u32;
    let pin = gsi - base_gsi;

    let lo = read_ioapic(rte_low(pin));
    write_ioapic(rte_low(pin), lo | (1 << 16));
}

// =============================================================================
// I/O port helpers
// =============================================================================

#[inline]
unsafe fn port_out_u8(port: u16, value: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("al") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags),
        );
    }
}
