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
            device_count += 1;

            // Check if multi-function (Header Type bit 7)
            let header_type = (unsafe { read_config_32(bus as u8, device, 0, 0x0C) } >> 16) & 0xFF;
            if (header_type & 0x80) != 0 {
                for func in 1u8..8 {
                    let vendor = unsafe { read_config_32(bus as u8, device, func, 0) } & 0xFFFF;
                    if vendor != 0xFFFF {
                        log_device(bus as u8, device, func);
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
