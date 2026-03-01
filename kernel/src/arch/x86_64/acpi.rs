// =============================================================================
// MinimalOS NextGen — ACPI Table Parser (MADT Extraction)
// =============================================================================
//
// Minimal ACPI parser for extracting interrupt controller topology from the
// MADT (Multiple APIC Description Table). This is the only ACPI table we
// parse in Sprint 3 — future sprints add FADT, HPET, etc.
//
// ACPI TABLE HIERARCHY:
//   RSDP → XSDT (or RSDT) → MADT ("APIC" signature)
//
// CRITICAL: XSDT vs RSDT
// ======================
// On modern UEFI firmware, ACPI tables (including the MADT) are frequently
// allocated above the 4 GiB boundary. The RSDT uses 32-bit pointers, which
// would silently truncate these addresses. We MUST use the XSDT (64-bit
// pointers) when the RSDP revision indicates ACPI 2.0+ (revision >= 2).
// The RSDT is only used as a fallback for legacy BIOS boot.
//
// MADT STRUCTURE:
//   Header (44 bytes) = standard SDT header (36 bytes) + LAPIC address (4 bytes) + flags (4 bytes)
//   Followed by variable-length entries:
//     Type 0: Processor Local APIC  (8 bytes)  — one per CPU core
//     Type 1: I/O APIC              (12 bytes) — one per I/O APIC
//     Type 2: Interrupt Source Override (10 bytes) — ISA IRQ remapping
//     Type 4: Local APIC NMI        (6 bytes)  — NMI wiring
//
// =============================================================================

use crate::kprintln;
use crate::memory::address::PhysAddr;
use core::mem;
use core::ptr;

// =============================================================================
// ACPI structures (packed, as they appear in memory)
// =============================================================================

/// RSDP — Root System Description Pointer (ACPI 1.0 portion).
///
/// Found by the bootloader (Limine passes us the address).
/// The `revision` field determines whether we use RSDT (v0) or XSDT (v2+).
#[repr(C, packed)]
struct RsdpV1 {
    signature: [u8; 8],     // "RSD PTR "
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,           // 0 = ACPI 1.0 (RSDT), 2+ = ACPI 2.0+ (XSDT)
    rsdt_address: u32,      // 32-bit physical address of RSDT
}

/// RSDP 2.0 extension — adds the 64-bit XSDT address.
#[repr(C, packed)]
struct RsdpV2 {
    v1: RsdpV1,
    length: u32,            // Total RSDP length
    xsdt_address: u64,      // 64-bit physical address of XSDT
    ext_checksum: u8,
    reserved: [u8; 3],
}

/// Standard ACPI System Description Table header.
///
/// Every ACPI table starts with this 36-byte header.
#[repr(C, packed)]
struct SdtHeader {
    signature: [u8; 4],     // e.g., "XSDT", "APIC", "FACP"
    length: u32,            // Total table length including header
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

// =============================================================================
// MADT entry structures
// =============================================================================

/// MADT entry header — type + length, followed by type-specific data.
#[repr(C, packed)]
struct MadtEntryHeader {
    entry_type: u8,
    length: u8,
}

/// Type 0: Processor Local APIC.
#[repr(C, packed)]
struct MadtLocalApic {
    header: MadtEntryHeader,
    acpi_processor_id: u8,
    apic_id: u8,
    flags: u32,             // Bit 0: enabled, Bit 1: online-capable
}

/// Type 1: I/O APIC.
#[repr(C, packed)]
struct MadtIoApic {
    header: MadtEntryHeader,
    io_apic_id: u8,
    reserved: u8,
    io_apic_address: u32,
    global_system_interrupt_base: u32,
}

/// Type 2: Interrupt Source Override.
///
/// Remaps an ISA interrupt to a different GSI (Global System Interrupt).
/// Example: ISA IRQ 0 (PIT timer) is commonly remapped to GSI 2.
#[repr(C, packed)]
struct MadtIso {
    header: MadtEntryHeader,
    bus_source: u8,         // Always 0 (ISA)
    irq_source: u8,         // ISA IRQ number
    global_system_interrupt: u32, // GSI it maps to
    flags: u16,             // Polarity and trigger mode
}

// =============================================================================
// Public types — returned from parse_madt()
// =============================================================================

/// Information about a single I/O APIC found in the MADT.
#[derive(Debug, Clone)]
pub struct IoApicInfo {
    /// I/O APIC ID (for destination in redirection entries).
    pub id: u8,
    /// Physical base address of the I/O APIC registers.
    pub address: u64,
    /// First GSI (Global System Interrupt) handled by this I/O APIC.
    pub gsi_base: u32,
}

/// An Interrupt Source Override — remaps an ISA IRQ to a different GSI.
#[derive(Debug, Clone)]
pub struct IsoOverride {
    /// Original ISA IRQ number (e.g., 0 for PIT timer).
    pub irq_source: u8,
    /// GSI this IRQ is remapped to (e.g., 2).
    pub gsi: u32,
    /// Polarity: 0 = conforms to bus, 1 = active high, 3 = active low.
    pub polarity: u8,
    /// Trigger mode: 0 = conforms to bus, 1 = edge, 3 = level.
    pub trigger: u8,
}

/// Information about a CPU core found in the MADT.
#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// ACPI processor ID.
    pub acpi_id: u8,
    /// Local APIC ID (used for IPI targeting).
    pub apic_id: u8,
    /// Whether this CPU core is enabled.
    pub enabled: bool,
}

/// All information extracted from the MADT.
pub struct MadtInfo {
    /// Physical address of the Local APIC registers (typically 0xFEE00000).
    pub lapic_addr: u64,
    /// CPU cores discovered.
    pub cpus: [CpuInfo; 16],  // Max 16 CPUs (N3710 has 4)
    pub cpu_count: usize,
    /// I/O APICs discovered.
    pub ioapics: [IoApicInfo; 4], // Max 4 I/O APICs (typically 1)
    pub ioapic_count: usize,
    /// Interrupt Source Overrides.
    pub overrides: [IsoOverride; 16], // Max 16 overrides
    pub override_count: usize,
}

impl MadtInfo {
    fn new() -> Self {
        Self {
            lapic_addr: 0,
            cpus: core::array::from_fn(|_| CpuInfo { acpi_id: 0, apic_id: 0, enabled: false }),
            cpu_count: 0,
            ioapics: core::array::from_fn(|_| IoApicInfo { id: 0, address: 0, gsi_base: 0 }),
            ioapic_count: 0,
            overrides: core::array::from_fn(|_| IsoOverride { irq_source: 0, gsi: 0, polarity: 0, trigger: 0 }),
            override_count: 0,
        }
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Parses the ACPI MADT table to extract interrupt controller topology.
///
/// # Parameters
/// - `rsdp_phys`: Physical address of the RSDP.
///   On Limine base revision 3, RSDP response returns a physical address.
///
/// # Returns
/// A `MadtInfo` struct containing LAPIC address, CPU list, I/O APIC list,
/// and interrupt source overrides.
///
/// # Panics
/// - If RSDP signature is invalid
/// - If ACPI revision is 0 (RSDT-only) — we require XSDT
/// - If MADT is not found in the XSDT
pub fn parse_madt(rsdp_phys: u64) -> MadtInfo {
    // =========================================================================
    // Step 1: Validate and parse the RSDP
    // =========================================================================
    kprintln!("[acpi] RSDP physical: {:#018X}", rsdp_phys);
    let rsdp_virt = PhysAddr::new(rsdp_phys).to_virt();
    kprintln!("[acpi] RSDP virtual (HHDM): {:#018X}", rsdp_virt.as_u64());
    let rsdp_v1 = unsafe { &*rsdp_virt.as_ptr::<RsdpV1>() };

    // Validate RSDP signature
    if &rsdp_v1.signature != b"RSD PTR " {
        panic!("[acpi] Invalid RSDP signature");
    }

    kprintln!("[acpi] RSDP revision: {}", rsdp_v1.revision);

    // =========================================================================
    // Step 2: Get the root table (XSDT required for ACPI 2.0+)
    // =========================================================================
    if rsdp_v1.revision < 2 {
        // ACPI 1.0 — RSDT only (32-bit pointers).
        // On UEFI, tables above 4 GiB would have truncated addresses.
        // We support it as a fallback but log a warning.
        kprintln!("[acpi] WARNING: ACPI 1.0 (RSDT) — 32-bit pointers, may miss high tables");
        let rsdt_addr = unsafe { ptr::addr_of!(rsdp_v1.rsdt_address).read_unaligned() };
        return parse_from_rsdt(rsdt_addr as u64);
    }

    // ACPI 2.0+ — use XSDT (64-bit pointers).
    let rsdp_v2 = unsafe { &*rsdp_virt.as_ptr::<RsdpV2>() };
    let xsdt_phys = unsafe { ptr::addr_of!(rsdp_v2.xsdt_address).read_unaligned() };

    kprintln!("[acpi] XSDT at physical: {:#018X}", xsdt_phys);

    parse_from_xsdt(xsdt_phys)
}

// =============================================================================
// Root table parsers
// =============================================================================

/// Parses tables from the XSDT (64-bit pointers). Preferred path.
fn parse_from_xsdt(xsdt_phys: u64) -> MadtInfo {
    let xsdt_virt = PhysAddr::new(xsdt_phys).to_virt();
    let header = unsafe { &*xsdt_virt.as_ptr::<SdtHeader>() };

    // Validate XSDT signature
    if &header.signature != b"XSDT" {
        panic!("[acpi] Invalid XSDT signature: {:?}", header.signature);
    }

    let header_size = mem::size_of::<SdtHeader>();
    let table_length = unsafe { ptr::addr_of!(header.length).read_unaligned() } as usize;
    let entries_bytes = table_length.saturating_sub(header_size);
    let entry_count = entries_bytes / 8; // 64-bit pointers

    kprintln!("[acpi] XSDT: {} table entries", entry_count);

    // Walk all XSDT entries (each is a 64-bit physical address of an SDT).
    for i in 0..entry_count {
        let entry_offset = header_size + i * 8;
        let entry_virt = PhysAddr::new(xsdt_phys + entry_offset as u64).to_virt();
        let table_phys = unsafe { entry_virt.as_ptr::<u64>().read_unaligned() };

        let table_header = unsafe { &*PhysAddr::new(table_phys).to_virt().as_ptr::<SdtHeader>() };
        let sig = &table_header.signature;

        if sig == b"APIC" {
            kprintln!("[acpi] Found MADT at physical: {:#018X}", table_phys);
            return parse_madt_table(table_phys);
        }
    }

    panic!("[acpi] MADT (\"APIC\") not found in XSDT");
}

/// Parses tables from the RSDT (32-bit pointers). Fallback for BIOS.
fn parse_from_rsdt(rsdt_phys: u64) -> MadtInfo {
    let rsdt_virt = PhysAddr::new(rsdt_phys).to_virt();
    let header = unsafe { &*rsdt_virt.as_ptr::<SdtHeader>() };

    if &header.signature != b"RSDT" {
        panic!("[acpi] Invalid RSDT signature: {:?}", header.signature);
    }

    let header_size = mem::size_of::<SdtHeader>();
    let table_length = unsafe { ptr::addr_of!(header.length).read_unaligned() } as usize;
    let entries_bytes = table_length.saturating_sub(header_size);
    let entry_count = entries_bytes / 4; // 32-bit pointers

    kprintln!("[acpi] RSDT: {} table entries (32-bit pointers)", entry_count);

    for i in 0..entry_count {
        let entry_offset = header_size + i * 4;
        let entry_virt = PhysAddr::new(rsdt_phys + entry_offset as u64).to_virt();
        let table_phys = unsafe { entry_virt.as_ptr::<u32>().read_unaligned() } as u64;

        let table_header = unsafe { &*PhysAddr::new(table_phys).to_virt().as_ptr::<SdtHeader>() };
        let sig = &table_header.signature;

        if sig == b"APIC" {
            kprintln!("[acpi] Found MADT at physical: {:#018X}", table_phys);
            return parse_madt_table(table_phys);
        }
    }

    panic!("[acpi] MADT (\"APIC\") not found in RSDT");
}

// =============================================================================
// MADT parser
// =============================================================================

/// Parses the MADT table and extracts all relevant entries.
fn parse_madt_table(madt_phys: u64) -> MadtInfo {
    let madt_virt = PhysAddr::new(madt_phys).to_virt();
    let header = unsafe { &*madt_virt.as_ptr::<SdtHeader>() };
    let table_length = unsafe { ptr::addr_of!(header.length).read_unaligned() } as usize;

    // Bytes 36-39: Local APIC address (uint32)
    // Bytes 40-43: Flags (uint32)
    let lapic_addr_ptr = (madt_virt.as_u64() + 36) as *const u32;
    let lapic_addr = unsafe { *lapic_addr_ptr } as u64;

    let mut info = MadtInfo::new();
    info.lapic_addr = lapic_addr;

    kprintln!("[acpi] MADT: LAPIC address = {:#010X}", lapic_addr);

    // Parse variable-length entries starting at offset 44.
    let entries_start = madt_virt.as_u64() + 44;
    let entries_end = madt_virt.as_u64() + table_length as u64;
    let mut offset = entries_start;

    while offset + 2 <= entries_end {
        let entry_header = unsafe { &*(offset as *const MadtEntryHeader) };
        let entry_type = entry_header.entry_type;
        let entry_length = entry_header.length as u64;

        if entry_length < 2 {
            // Malformed entry — stop parsing.
            kprintln!("[acpi] WARNING: MADT entry with length < 2 at offset {:#X}", offset);
            break;
        }

        match entry_type {
            // Type 0: Processor Local APIC
            0 => {
                if info.cpu_count < info.cpus.len() {
                    let entry = unsafe { &*(offset as *const MadtLocalApic) };
                    let flags = unsafe { ptr::addr_of!(entry.flags).read_unaligned() };
                    let enabled = flags & 1 != 0;
                    let online_capable = flags & 2 != 0;

                    if enabled || online_capable {
                        info.cpus[info.cpu_count] = CpuInfo {
                            acpi_id: entry.acpi_processor_id,
                            apic_id: entry.apic_id,
                            enabled,
                        };
                        info.cpu_count += 1;
                    }
                }
            }

            // Type 1: I/O APIC
            1 => {
                if info.ioapic_count < info.ioapics.len() {
                    let entry = unsafe { &*(offset as *const MadtIoApic) };
                    let io_addr = unsafe { ptr::addr_of!(entry.io_apic_address).read_unaligned() };
                    let gsi_base = unsafe { ptr::addr_of!(entry.global_system_interrupt_base).read_unaligned() };
                    info.ioapics[info.ioapic_count] = IoApicInfo {
                        id: entry.io_apic_id,
                        address: io_addr as u64,
                        gsi_base,
                    };
                    info.ioapic_count += 1;
                    kprintln!("[acpi]   I/O APIC #{}: addr={:#010X}, GSI base={}",
                        entry.io_apic_id, io_addr, gsi_base);
                }
            }

            // Type 2: Interrupt Source Override
            2 => {
                if info.override_count < info.overrides.len() {
                    let entry = unsafe { &*(offset as *const MadtIso) };
                    let flags = unsafe { ptr::addr_of!(entry.flags).read_unaligned() };
                    let gsi = unsafe { ptr::addr_of!(entry.global_system_interrupt).read_unaligned() };
                    let polarity = (flags & 0x03) as u8;
                    let trigger = ((flags >> 2) & 0x03) as u8;

                    info.overrides[info.override_count] = IsoOverride {
                        irq_source: entry.irq_source,
                        gsi,
                        polarity,
                        trigger,
                    };
                    info.override_count += 1;
                    kprintln!("[acpi]   ISO: IRQ {} → GSI {} (pol={}, trig={})",
                        entry.irq_source, gsi, polarity, trigger);
                }
            }

            // Type 4: Local APIC NMI — logged but not acted on yet
            4 => {
                kprintln!("[acpi]   Local APIC NMI entry (type 4, length {})", entry_length);
            }

            // Other types: skip silently
            _ => {}
        }

        offset += entry_length;
    }

    kprintln!("[acpi] MADT summary: {} CPUs, {} I/O APICs, {} overrides",
        info.cpu_count, info.ioapic_count, info.override_count);

    info
}
