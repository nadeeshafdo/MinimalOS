// =============================================================================
// MinimalOS NextGen — Kernel W^X Remap
// =============================================================================
//
// Enforces Write XOR Execute (W^X) permissions on the kernel's ELF sections
// by modifying the page table entries in-place.
//
// PROBLEM:
//   Limine's page tables map the kernel with permissive RWX permissions
//   (or may use 2 MiB huge pages). This means an attacker who corrupts
//   .data could execute it, and .text is writable (code injection).
//
// SOLUTION:
//   Walk the kernel's virtual address range and apply strict per-section
//   permissions:
//     .text:   PRESENT | GLOBAL               (R+X, not writable)
//     .rodata: PRESENT | GLOBAL | NO_EXECUTE   (R, not writable, not exec)
//     .data:   PRESENT | WRITABLE | GLOBAL | NO_EXECUTE  (R+W, not exec)
//     .bss:    PRESENT | WRITABLE | GLOBAL | NO_EXECUTE  (R+W, not exec)
//
//   If Limine uses 2 MiB huge pages, we split them at section boundaries
//   into 4 KiB pages so each page can have individual permissions.
//
// =============================================================================

use crate::kprintln;
use crate::arch::cpu;
use crate::memory::address::{PhysAddr, VirtAddr, PAGE_SIZE};
use crate::memory::vmm::{self, PageTableFlags, RemapError};

// Linker-provided section boundaries
unsafe extern "C" {
    static _text_start: u8;
    static _text_end: u8;
    static _rodata_start: u8;
    static _rodata_end: u8;
    static _data_start: u8;
    static _data_end: u8;
    static _bss_start: u8;
    static _bss_end: u8;
    static _kernel_start: u8;
    static _kernel_end: u8;
}

/// Section classification for a kernel virtual address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Text,
    Rodata,
    Data,
    Bss,
    /// Between sections (padding/alignment) — treated as data (R+W+NX).
    Gap,
}

impl Section {
    /// Returns the page table flags for this section.
    fn flags(self) -> PageTableFlags {
        match self {
            Section::Text => PageTableFlags::KERNEL_CODE,
            Section::Rodata => PageTableFlags::KERNEL_RODATA,
            Section::Data | Section::Bss | Section::Gap => PageTableFlags::KERNEL_DATA,
        }
    }
}

/// Determines which kernel section a virtual address belongs to.
fn classify_page(addr: u64) -> Section {
    let text_start = unsafe { &_text_start as *const u8 as u64 };
    let text_end = unsafe { &_text_end as *const u8 as u64 };
    let rodata_start = unsafe { &_rodata_start as *const u8 as u64 };
    let rodata_end = unsafe { &_rodata_end as *const u8 as u64 };
    let data_start = unsafe { &_data_start as *const u8 as u64 };
    let data_end = unsafe { &_data_end as *const u8 as u64 };
    let bss_start = unsafe { &_bss_start as *const u8 as u64 };
    let bss_end = unsafe { &_bss_end as *const u8 as u64 };

    if addr >= text_start && addr < text_end {
        Section::Text
    } else if addr >= rodata_start && addr < rodata_end {
        Section::Rodata
    } else if addr >= data_start && addr < data_end {
        Section::Data
    } else if addr >= bss_start && addr < bss_end {
        Section::Bss
    } else {
        // Between sections (alignment padding) — default to safe data perms.
        Section::Gap
    }
}

/// Enforces Write XOR Execute permissions on the kernel's ELF sections.
///
/// This modifies Limine's existing page tables in-place. If Limine used
/// 2 MiB huge pages for the kernel range, they are split into 4 KiB pages
/// at section boundaries.
///
/// # Safety
/// - Must be called after the IDT is loaded (page faults are handled).
/// - Must be called on the BSP before SMP init (single-core only).
/// - Should be called after all Phase 4 MMIO mappings are complete.
pub fn enforce_wxn() {
    let kernel_start = unsafe { &_kernel_start as *const u8 as u64 };
    let kernel_end = unsafe { &_kernel_end as *const u8 as u64 };
    let text_start = unsafe { &_text_start as *const u8 as u64 };
    let text_end = unsafe { &_text_end as *const u8 as u64 };
    let rodata_start = unsafe { &_rodata_start as *const u8 as u64 };
    let rodata_end = unsafe { &_rodata_end as *const u8 as u64 };
    let data_start = unsafe { &_data_start as *const u8 as u64 };
    let data_end = unsafe { &_data_end as *const u8 as u64 };
    let bss_start = unsafe { &_bss_start as *const u8 as u64 };
    let bss_end = unsafe { &_bss_end as *const u8 as u64 };

    kprintln!("[remap] Enforcing W^X on kernel sections:");
    kprintln!("[remap]   .text:   {:#018X} — {:#018X} (R+X)",
        text_start, text_end);
    kprintln!("[remap]   .rodata: {:#018X} — {:#018X} (R)",
        rodata_start, rodata_end);
    kprintln!("[remap]   .data:   {:#018X} — {:#018X} (R+W+NX)",
        data_start, data_end);
    kprintln!("[remap]   .bss:    {:#018X} — {:#018X} (R+W+NX)",
        bss_start, bss_end);

    let cr3 = PhysAddr::new(cpu::read_cr3() & !0xFFF);

    // Page-align kernel range
    let start_page = kernel_start & !0xFFF;
    let end_page = (kernel_end + 0xFFF) & !0xFFF;
    let total_pages = (end_page - start_page) / PAGE_SIZE;

    kprintln!("[remap] Remapping {} kernel pages ({} KiB)...",
        total_pages, total_pages * 4);

    let mut remapped = 0u64;
    let mut split = 0u64;
    let mut addr = start_page;

    while addr < end_page {
        let virt = VirtAddr::new(addr);
        let section = classify_page(addr);
        let flags = section.flags();

        // Try to remap directly
        match unsafe { vmm::remap_page(cr3, virt, flags) } {
            Ok(()) => {
                cpu::invlpg(addr);
                remapped += 1;
            }
            Err(RemapError::HugePageNeedsSplit) => {
                // Split the 2 MiB huge page, then retry
                match unsafe { vmm::split_huge_page(cr3, virt) } {
                    Ok(()) => {
                        split += 1;
                        // Now retry the remap on this 4K page
                        match unsafe { vmm::remap_page(cr3, virt, flags) } {
                            Ok(()) => {
                                cpu::invlpg(addr);
                                remapped += 1;
                            }
                            Err(e) => {
                                kprintln!("[remap] FATAL: remap after split failed @ {:#018X}: {:?}",
                                    addr, e);
                            }
                        }
                    }
                    Err(e) => {
                        kprintln!("[remap] FATAL: split_huge_page failed @ {:#018X}: {:?}",
                            addr, e);
                    }
                }
            }
            Err(RemapError::NotMapped) => {
                // Page not currently mapped — skip (gap page between sections).
                // This can happen if the linker places sections non-contiguously.
            }
            Err(e) => {
                kprintln!("[remap] WARNING: remap failed @ {:#018X}: {:?}", addr, e);
            }
        }

        addr += PAGE_SIZE;
    }

    // Flush entire TLB to ensure all new permissions take effect
    unsafe { vmm::flush_all(); }

    kprintln!("[remap] W^X enforcement complete: {} pages remapped, {} huge pages split",
        remapped, split);
}
