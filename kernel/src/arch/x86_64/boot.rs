// =============================================================================
// MinimalOS NextGen — Limine Boot Protocol Interface
// =============================================================================
//
// This module communicates with the Limine bootloader using its request/response
// protocol. Here's how it works:
//
// LIMINE PROTOCOL OVERVIEW:
//   1. The kernel binary contains static "request" structures in its .rodata
//   2. During boot, Limine scans the kernel binary for these magic patterns
//   3. Limine fulfills each request by writing a pointer to a "response" struct
//   4. When the kernel starts, it reads the response pointers to get boot info
//
//   This is elegant: the kernel declares what it NEEDS as static data,
//   and the bootloader provides it. No complex handshake protocol.
//
// WHAT LIMINE PROVIDES US:
//   - HHDM (Higher Half Direct Map) offset — where physical memory is mapped
//   - Memory map — which physical regions are free, reserved, or used
//   - Framebuffer info — pixel dimensions, pitch, bpp, address
//   - RSDP pointer — for ACPI table parsing (hardware discovery)
//   - Kernel address — where the kernel is loaded (physical + virtual)
//   - Boot modules — additional files loaded alongside the kernel (initramfs)
//
// =============================================================================

use limine::BaseRevision;
use limine::request::{
    ExecutableAddressRequest, FramebufferRequest, HhdmRequest, MemoryMapRequest,
    RsdpRequest,
};

// =============================================================================
// Limine Request Declarations
// =============================================================================
//
// Each request is a static with a specific Limine magic number.
// Limine scans the kernel binary at boot time, finds these requests by
// their magic bytes, and fills in the response pointers.
//
// The `#[used]` attribute prevents the compiler from optimizing these away
// (they look unused from the compiler's perspective since nothing in Rust
// code takes their address — Limine finds them by scanning raw bytes).
//
// The `#[link_section = ".limine_requests"]` places them in our dedicated
// linker section, ensuring they end up in a loadable segment that Limine
// can scan.
// =============================================================================

/// Request for the Higher Half Direct Map offset.
///
/// The HHDM is Limine's direct mapping of ALL physical memory at a fixed
/// virtual offset. This means:
///   phys_addr + hhdm_offset = virt_addr_in_kernel
///
/// Without this, we'd need to manually map physical memory before we could
/// access it — a chicken-and-egg problem since the page tables themselves
/// are in physical memory.
///
/// The HHDM offset is typically 0xFFFF_8000_0000_0000, but we don't
/// hardcode it — we read it from Limine's response.
/// Limine base revision tag — required by Limine v1+ protocol.
/// This tells Limine which revision of the protocol we support.
#[used]
#[unsafe(link_section = ".limine_requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

/// Request for the physical memory map.
///
/// This is the most critical boot information. It tells us which regions
/// of physical memory are:
///   - USABLE: free RAM we can allocate
///   - RESERVED: hardware-reserved (BIOS, ACPI, etc.)
///   - ACPI_RECLAIMABLE: ACPI tables (can be freed after parsing)
///   - ACPI_NVS: ACPI Non-Volatile Storage (never touch)
///   - BAD_MEMORY: defective RAM regions
///   - BOOTLOADER_RECLAIMABLE: Limine's own memory (can be freed later)
///   - KERNEL_AND_MODULES: where our kernel + initramfs are loaded
///   - FRAMEBUFFER: the framebuffer memory region
///
/// The memory map is sorted by base address (lowest first) and
/// non-overlapping. This makes it easy to iterate and build our
/// physical memory bitmap.
///
/// On your N3710 with 8GB RAM, expect something like:
///   0x000000-0x09FFFF: Usable (640KB legacy low memory)
///   0x0A0000-0x0FFFFF: Reserved (legacy VGA, ROM)
///   0x100000-0x1FFFFF: Kernel & modules (~1MB)
///   0x200000-0x1FFFFFFF: Usable (~510MB below 512MB)
///   ... more usable regions up to ~8GB ...
///   0xFEC00000-0xFEDFFFFF: Reserved (I/O APIC, HPET)
///   0xFEE00000-0xFEEFFFFF: Reserved (Local APIC)
#[used]
#[unsafe(link_section = ".limine_requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

/// Request for framebuffer information.
///
/// Limine asks UEFI to set up a graphical framebuffer (via GOP — Graphics
/// Output Protocol) and tells us about it:
///   - Physical address of the framebuffer memory
///   - Width and height in pixels
///   - Pitch (bytes per row — may include padding)
///   - Bits per pixel (typically 32: 8 each for R, G, B, unused)
///   - Color masks (which bits are red, green, blue)
///
/// On your HP Notebook (1366x768 display), the framebuffer will be
/// approximately 1366 × 768 × 4 = ~4MB.
///
/// We use this for text rendering in the early boot console.
/// Later, a userspace compositor will take over.
#[used]
#[unsafe(link_section = ".limine_requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

/// Request for ACPI RSDP (Root System Description Pointer).
///
/// The RSDP is the entry point for ACPI table discovery. ACPI tables
/// describe the system's hardware configuration:
///   - MADT: Interrupt controller (APIC) configuration → needed for SMP
///   - HPET: High Precision Event Timer → needed for timer calibration
///   - FADT: Power management → needed for shutdown/sleep
///
/// We store the RSDP pointer and parse ACPI tables when needed.
#[used]
#[unsafe(link_section = ".limine_requests")]
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

/// Request for kernel's physical and virtual load addresses.
///
/// Tells us where Limine loaded the kernel:
///   - Physical base: where the kernel is in physical RAM
///   - Virtual base: 0xFFFFFFFF80200000 (higher-half + 2MB offset)
///
/// We need this to:
///   1. Mark the kernel's physical pages as "used" in the PMM bitmap
///   2. Calculate the kernel size (end - start)
///   3. Set correct page permissions per section
#[used]
#[unsafe(link_section = ".limine_requests")]
static KERNEL_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();

// =============================================================================
// Boot Information API
// =============================================================================
//
// These functions provide safe, typed access to Limine's boot information.
// They should only be called after Limine has filled in the responses
// (i.e., only from kmain and onwards — not before the kernel is entered).
// =============================================================================

/// Information about the framebuffer, extracted from Limine's response.
///
/// Stored as simple values so we don't need to keep the Limine response
/// structures around after boot.
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    /// Pointer to the start of framebuffer memory (virtual address).
    /// Write ARGB pixels here to display on screen.
    pub address: *mut u8,

    /// Width of the framebuffer in pixels.
    pub width: u64,

    /// Height of the framebuffer in pixels.
    pub height: u64,

    /// Bytes per horizontal line (may be > width * bpp/8 due to padding).
    /// Always use pitch to calculate row addresses, never width * bpp.
    pub pitch: u64,

    /// Bits per pixel (typically 32 for ARGB8888).
    pub bpp: u16,
}

/// Retrieves the HHDM (Higher Half Direct Map) offset from Limine.
///
/// This is the virtual address offset where Limine maps all physical memory.
/// Physical address P is accessible at virtual address P + hhdm_offset.
///
/// # Panics
/// Panics if the Limine HHDM response is not available (should never happen
/// if Limine booted us correctly).
pub fn get_hhdm_offset() -> u64 {
    HHDM_REQUEST
        .get_response()
        .expect("Limine HHDM response not available — boot protocol error")
        .offset()
}

/// Retrieves the physical memory map from Limine.
///
/// Returns a slice of memory map entries, sorted by base address.
/// Each entry describes a contiguous region of physical memory with its
/// type (usable, reserved, etc.).
///
/// Iterate over entries and look for `USABLE` regions to build the
/// physical memory manager's free page bitmap.
///
/// # Panics
/// Panics if the memory map response is not available.
pub fn get_memory_map() -> &'static [&'static limine::memory_map::Entry] {
    MEMORY_MAP_REQUEST
        .get_response()
        .expect("Limine memory map response not available — boot protocol error")
        .entries()
}

/// Retrieves framebuffer information from Limine.
///
/// Returns `Some(FramebufferInfo)` if a framebuffer is available,
/// `None` if Limine couldn't set up a graphical mode (unlikely on UEFI).
///
/// We take the first framebuffer if multiple are available (unusual case).
pub fn get_framebuffer_info() -> Option<FramebufferInfo> {
    let response = FRAMEBUFFER_REQUEST.get_response()?;
    let mut framebuffers = response.framebuffers();
    let fb = framebuffers.next()?;

    Some(FramebufferInfo {
        address: fb.addr() as *mut u8,
        width: fb.width(),
        height: fb.height(),
        pitch: fb.pitch(),
        bpp: fb.bpp(),
    })
}

/// Retrieves the RSDP (Root System Description Pointer) address.
///
/// Returns the virtual address of the ACPI RSDP structure.
/// This is the entry point for all ACPI table discovery.
///
/// Returns `None` if ACPI is not available (extremely rare on UEFI systems).
pub fn get_rsdp_address() -> Option<u64> {
    let response = RSDP_REQUEST.get_response()?;
    Some(response.address() as u64)
}

/// Retrieves the kernel's load addresses.
///
/// Returns `(physical_base, virtual_base)`:
///   - `physical_base`: where the kernel ELF is in physical RAM
///   - `virtual_base`: the higher-half virtual address (should match linker script)
///
/// # Panics
/// Panics if the kernel address response is not available.
pub fn get_kernel_address() -> (u64, u64) {
    let response = KERNEL_ADDRESS_REQUEST
        .get_response()
        .expect("Limine kernel address response not available");
    (response.physical_base(), response.virtual_base())
}
