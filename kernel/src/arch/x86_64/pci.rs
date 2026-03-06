// =============================================================================
// MinimalOS NextGen — PCI Configuration Space Driver (Sprint 11)
// =============================================================================
//
// Enumerates the PCI bus using the legacy Port I/O mechanism:
//   - CONFIG_ADDRESS (0xCF8): 32-bit address register
//   - CONFIG_DATA    (0xCFC): 32-bit data register
//
// This is the most universally supported PCI access method on x86_64.
// MCFG (PCIe MMIO configuration) can be added later for extended config space.
//
// Address format (CONFIG_ADDRESS):
//   Bit 31:      Enable bit (must be 1)
//   Bits 23-16:  Bus number (0-255)
//   Bits 15-11:  Device number (0-31)
//   Bits 10-8:   Function number (0-7)
//   Bits 7-2:    Register offset (6 bits, 4-byte aligned)
//   Bits 1-0:    Always 0
//
// =============================================================================

use core::arch::asm;
use crate::kprintln;

/// PCI Configuration Address port (write-only for address selection).
const CONFIG_ADDRESS: u16 = 0xCF8;

/// PCI Configuration Data port (read/write for register access).
const CONFIG_DATA: u16 = 0xCFC;

// ─── Raw PCI Configuration Space Access ─────────────────────────────────────

/// Reads a 32-bit register from the PCI configuration space.
///
/// # Arguments
/// - `bus`:    PCI bus number (0-255)
/// - `device`: Device number on the bus (0-31)
/// - `func`:   Function within a multi-function device (0-7)
/// - `offset`: Register offset (must be 4-byte aligned, low 2 bits ignored)
///
/// # Safety
/// Performs raw x86 I/O port operations. Must be called from Ring 0.
#[inline]
pub unsafe fn read_config_32(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let address: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    // Write address to CONFIG_ADDRESS
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") CONFIG_ADDRESS,
            in("eax") address,
            options(nomem, nostack, preserves_flags),
        );
    }

    // Read 32-bit data from CONFIG_DATA
    let data: u32;
    unsafe {
        asm!(
            "in eax, dx",
            out("eax") data,
            in("dx") CONFIG_DATA,
            options(nomem, nostack, preserves_flags),
        );
    }
    data
}

/// Writes a 32-bit value to the PCI configuration space.
///
/// # Safety
/// Performs raw x86 I/O port operations. Must be called from Ring 0.
/// Writing to the wrong register can brick a device or corrupt system state.
#[inline]
pub unsafe fn write_config_32(bus: u8, device: u8, func: u8, offset: u8, value: u32) {
    let address: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    // Write address to CONFIG_ADDRESS
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") CONFIG_ADDRESS,
            in("eax") address,
            options(nomem, nostack, preserves_flags),
        );
    }

    // Write 32-bit data to CONFIG_DATA
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") CONFIG_DATA,
            in("eax") value,
            options(nomem, nostack, preserves_flags),
        );
    }
}

// ─── BAR Decoding ───────────────────────────────────────────────────────────

/// Decoded PCI Base Address Register — the physical coordinates of a device.
#[derive(Debug)]
pub enum BarType {
    /// Memory-Mapped I/O region.
    Memory { base_addr: u64, size: u64, prefetchable: bool },
    /// I/O port range.
    Io { port_base: u16, size: u16 },
}

/// Reads and decodes a single BAR for a given PCI function.
///
/// Uses the standard size-probing technique:
///   1. Save original BAR value
///   2. Disable I/O + Memory decoding (Command register bits 0-1)
///   3. Write 0xFFFFFFFF, read back to determine size mask
///   4. Restore original BAR value and Command register
///
/// Returns `None` if the BAR is unimplemented (reads as zero).
///
/// # Arguments
/// - `bar_index`: BAR number 0-5 (offset 0x10 + index*4)
///
/// # Safety
/// Temporarily writes to PCI config space. Must be called from Ring 0
/// with interrupts in a safe state (no concurrent PCI access).
pub unsafe fn read_bar(bus: u8, device: u8, func: u8, bar_index: u8) -> Option<BarType> {
    let offset = 0x10 + (bar_index * 4);
    let bar_val = unsafe { read_config_32(bus, device, func, offset) };

    if bar_val == 0 {
        return None; // Unimplemented BAR
    }

    // Save and disable I/O + Memory decoding during size probe
    let old_command = unsafe { read_config_32(bus, device, func, 0x04) };
    unsafe { write_config_32(bus, device, func, 0x04, old_command & !0x03) };

    // Write all-ones, read back the size mask
    unsafe { write_config_32(bus, device, func, offset, 0xFFFF_FFFF) };
    let size_mask = unsafe { read_config_32(bus, device, func, offset) };
    unsafe { write_config_32(bus, device, func, offset, bar_val) }; // Restore

    // Restore command register
    unsafe { write_config_32(bus, device, func, 0x04, old_command) };

    if (bar_val & 0x01) == 1 {
        // ── I/O Space BAR ──
        let port_base = (bar_val & 0xFFFC) as u16;
        let size = (!(size_mask & 0xFFFC)).wrapping_add(1) as u16;
        Some(BarType::Io { port_base, size })
    } else {
        // ── Memory-Mapped I/O BAR ──
        let type_flag = (bar_val >> 1) & 0x03;
        let prefetchable = (bar_val & 0x08) != 0;

        let mut base_addr = (bar_val & 0xFFFF_FFF0) as u64;
        let mut size_mask_64 = (size_mask & 0xFFFF_FFF0) as u64;

        if type_flag == 2 {
            // 64-bit BAR — upper 32 bits live in the next register
            let upper_val = unsafe { read_config_32(bus, device, func, offset + 4) };
            base_addr |= (upper_val as u64) << 32;

            // Probe upper 32 bits for size
            unsafe { write_config_32(bus, device, func, offset + 4, 0xFFFF_FFFF) };
            let upper_mask = unsafe { read_config_32(bus, device, func, offset + 4) };
            unsafe { write_config_32(bus, device, func, offset + 4, upper_val) }; // Restore

            size_mask_64 |= (upper_mask as u64) << 32;
        }

        let size = if type_flag == 2 {
            // 64-bit BAR: negate the full 64-bit mask
            (!size_mask_64).wrapping_add(1)
        } else {
            // 32-bit BAR: negate in 32-bit arithmetic to avoid upper-bit pollution
            let mask_32 = (size_mask & 0xFFFF_FFF0) as u32;
            (!mask_32).wrapping_add(1) as u64
        };
        Some(BarType::Memory { base_addr, size, prefetchable })
    }
}

// ─── PCI Bus Enumeration ────────────────────────────────────────────────────

/// Enumerates the entire PCI configuration space (buses 0-255).
///
/// For each valid device/function found, logs the vendor ID, device ID,
/// class code, and subclass to the kernel serial console.
///
/// Uses brute-force scanning: checks all 256 buses × 32 devices.
/// For each device, checks function 0 first, then probes functions 1-7
/// only if the device is multi-function (header type bit 7 set).
pub fn enumerate_buses() {
    kprintln!("[pci] Enumerating PCI configuration space...");

    let mut device_count: u32 = 0;

    for bus in 0u16..=255 {
        for device in 0u8..32 {
            // Check Function 0 — if vendor is 0xFFFF, no device present
            let vendor_id = unsafe { read_config_32(bus as u8, device, 0, 0) } & 0xFFFF;
            if vendor_id == 0xFFFF {
                continue;
            }

            log_device(bus as u8, device, 0);
            probe_bars_if_virtio(bus as u8, device, 0);
            device_count += 1;

            // Check if multi-function (Header Type bit 7)
            let header_type = (unsafe { read_config_32(bus as u8, device, 0, 0x0C) } >> 16) & 0xFF;
            if (header_type & 0x80) != 0 {
                for func in 1u8..8 {
                    let vendor = unsafe { read_config_32(bus as u8, device, func, 0) } & 0xFFFF;
                    if vendor != 0xFFFF {
                        log_device(bus as u8, device, func);
                        probe_bars_if_virtio(bus as u8, device, func);
                        device_count += 1;
                    }
                }
            }
        }
    }

    kprintln!("[pci] Enumeration complete: {} device(s) found", device_count);
}

/// Reads and logs the identification registers for a single PCI function.
fn log_device(bus: u8, device: u8, func: u8) {
    // Register 0x00: Vendor ID (low 16) | Device ID (high 16)
    let reg0 = unsafe { read_config_32(bus, device, func, 0x00) };
    let vendor_id = reg0 & 0xFFFF;
    let device_id = reg0 >> 16;

    // Register 0x08: Revision (7:0) | Prog IF (15:8) | Subclass (23:16) | Class (31:24)
    let class_info = unsafe { read_config_32(bus, device, func, 0x08) };
    let class = (class_info >> 24) & 0xFF;
    let subclass = (class_info >> 16) & 0xFF;
    let prog_if = (class_info >> 8) & 0xFF;

    kprintln!(
        "[pci]   {:02X}:{:02X}.{} — Vendor:{:04X} Device:{:04X} | Class:{:02X} Sub:{:02X} ProgIF:{:02X} ({})",
        bus, device, func,
        vendor_id, device_id,
        class, subclass, prog_if,
        class_name(class, subclass),
    );
}

/// Probes and logs all BARs for Virtio devices (Vendor 0x1AF4).
///
/// When a Virtio device (specifically the Block device 0x1001) is found,
/// this decodes all 6 BARs and logs their physical coordinates.
fn probe_bars_if_virtio(bus: u8, device: u8, func: u8) {
    let reg0 = unsafe { read_config_32(bus, device, func, 0x00) };
    let vendor_id = reg0 & 0xFFFF;
    let device_id = reg0 >> 16;

    // Only probe Virtio devices (Red Hat / Virtio vendor)
    if vendor_id != 0x1AF4 {
        return;
    }

    kprintln!(
        "[pci]   ╰─ Virtio device {:04X}:{:04X} — probing BARs...",
        vendor_id, device_id
    );

    let mut bar_idx = 0u8;
    while bar_idx < 6 {
        if let Some(bar) = unsafe { read_bar(bus, device, func, bar_idx) } {
            match &bar {
                BarType::Io { port_base, size } => {
                    kprintln!(
                        "[pci]     BAR {}: I/O  port=0x{:04X} size={} bytes",
                        bar_idx, port_base, size
                    );
                }
                BarType::Memory { base_addr, size, prefetchable } => {
                    kprintln!(
                        "[pci]     BAR {}: MMIO base=0x{:016X} size={} bytes{}",
                        bar_idx, base_addr, size,
                        if *prefetchable { " [prefetchable]" } else { "" }
                    );
                    // 64-bit MMIO BAR consumes the next BAR slot
                    if let BarType::Memory { .. } = &bar {
                        let bar_val = unsafe { read_config_32(bus, device, func, 0x10 + bar_idx * 4) };
                        let type_flag = (bar_val >> 1) & 0x03;
                        if type_flag == 2 {
                            bar_idx += 1; // Skip upper-half BAR
                        }
                    }
                }
            }
        }
        bar_idx += 1;
    }
}

/// Returns a human-readable name for a PCI class/subclass pair.
///
/// Covers the most common classes seen in QEMU virtual machines.
fn class_name(class: u32, subclass: u32) -> &'static str {
    match (class, subclass) {
        (0x00, 0x00) => "Non-VGA Unclassified",
        (0x00, 0x01) => "VGA-Compatible Unclassified",
        (0x01, 0x00) => "SCSI Bus Controller",
        (0x01, 0x01) => "IDE Controller",
        (0x01, 0x06) => "SATA Controller",
        (0x01, 0x08) => "NVM Controller",
        (0x02, 0x00) => "Ethernet Controller",
        (0x03, 0x00) => "VGA Controller",
        (0x04, 0x00) => "Video Device",
        (0x04, 0x01) => "Audio Device",
        (0x06, 0x00) => "Host Bridge",
        (0x06, 0x01) => "ISA Bridge",
        (0x06, 0x04) => "PCI-to-PCI Bridge",
        (0x06, 0x80) => "Other Bridge",
        (0x07, 0x00) => "Serial Controller",
        (0x08, 0x00) => "PIC",
        (0x08, 0x80) => "Other System Peripheral",
        (0x0C, 0x03) => "USB Controller",
        (0x0C, 0x05) => "SMBus Controller",
        _             => "Unknown",
    }
}
