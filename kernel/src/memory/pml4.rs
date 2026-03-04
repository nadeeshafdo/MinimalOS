// =============================================================================
// MinimalOS NextGen — Pristine Kernel PML4 Builder
// =============================================================================
//
// Builds a clean PML4 from scratch, replacing Limine's contaminated page
// tables. Maps only what the kernel actually needs:
//
//   1. HHDM: all physical RAM at HHDM_OFFSET + phys (2M huge pages)
//   2. Kernel ELF sections with strict W^X permissions (4K pages)
//   3. MMIO: LAPIC/IOAPIC with uncacheable flags (4K pages)
//
// After activation via CR3 swap, the kernel runs on clean page tables
// with no identity mappings, no bootloader ghosts, no UEFI residue.
//
// =============================================================================

use core::sync::atomic::{AtomicU64, Ordering};

use crate::kprintln;
use crate::arch::boot;
use crate::arch::cpu;
use crate::memory::address::{self, PhysAddr, VirtAddr, PAGE_SIZE};
use crate::memory::vmm::{self, PageTableFlags};

/// Physical address of the pristine kernel PML4.
/// APs read this (via `sym` + RIP-relative) to sync their CR3 on wakeup.
pub static KERNEL_PML4: AtomicU64 = AtomicU64::new(0);

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

const SIZE_2M: u64 = 2 * 1024 * 1024;

/// Builds a pristine kernel PML4 and returns its physical address.
///
/// Maps:
/// - Full HHDM (all memory map entries, including bootloader reclaimable)
/// - Kernel higher-half sections with W^X enforcement
/// - LAPIC and I/O APIC MMIO regions (uncacheable)
pub fn build() -> PhysAddr {
    let pml4 = vmm::new_table().expect("pml4: failed to allocate PML4 frame");

    kprintln!("[pml4] Building pristine kernel page tables...");

    // ---- 1. Map the HHDM ----
    // Every physical memory region gets mapped at HHDM_OFFSET + phys.
    // Use 2M huge pages for speed. For regions not 2M-aligned, fall back to 4K.
    let hhdm = address::hhdm_offset();
    let memory_map = boot::get_memory_map();
    let mut hhdm_pages_2m = 0u64;
    let mut hhdm_pages_4k = 0u64;

    // HHDM data flags: R+W+NX (standard data, not executable)
    let hhdm_flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::GLOBAL
        | PageTableFlags::NO_EXECUTE;

    for entry in memory_map.iter() {
        let base = entry.base;
        let length = entry.length;
        let end = base + length;

        // Skip bad memory — nothing to map
        if entry.entry_type == limine::memory_map::EntryType::BAD_MEMORY {
            continue;
        }

        // Map this region into the HHDM
        let mut phys = base;
        while phys < end {
            let virt = VirtAddr::new(hhdm + phys);
            let remaining = end - phys;

            // Use 2M huge pages if both base and remaining are 2M-aligned
            if phys & (SIZE_2M - 1) == 0 && remaining >= SIZE_2M {
                match unsafe { vmm::map_huge_page_2m(pml4, virt, PhysAddr::new(phys), hhdm_flags) } {
                    Ok(()) => { hhdm_pages_2m += 1; }
                    Err(vmm::MapError::AlreadyMapped) => {}
                    Err(e) => {
                        kprintln!("[pml4] WARNING: HHDM 2M map failed @ {:#018X}: {:?}", phys, e);
                    }
                }
                phys += SIZE_2M;
            } else {
                // Fall back to 4K page
                let phys_aligned = phys & !0xFFF;
                let virt_aligned = VirtAddr::new(hhdm + phys_aligned);
                match unsafe { vmm::map_page(pml4, virt_aligned, PhysAddr::new(phys_aligned), hhdm_flags) } {
                    Ok(()) => { hhdm_pages_4k += 1; }
                    Err(vmm::MapError::AlreadyMapped) => {}
                    Err(e) => {
                        kprintln!("[pml4] WARNING: HHDM 4K map failed @ {:#018X}: {:?}", phys_aligned, e);
                    }
                }
                phys = phys_aligned + PAGE_SIZE;
            }
        }
    }

    kprintln!("[pml4] HHDM: {} 2M pages + {} 4K pages mapped", hhdm_pages_2m, hhdm_pages_4k);

    // ---- 2. Map kernel higher-half sections with W^X ----
    let (kernel_phys_base, kernel_virt_base) = boot::get_kernel_address();
    let text_start = unsafe { &_text_start as *const u8 as u64 };
    let text_end = unsafe { &_text_end as *const u8 as u64 };
    let rodata_start = unsafe { &_rodata_start as *const u8 as u64 };
    let rodata_end = unsafe { &_rodata_end as *const u8 as u64 };
    let data_start = unsafe { &_data_start as *const u8 as u64 };
    let data_end = unsafe { &_data_end as *const u8 as u64 };
    let bss_start = unsafe { &_bss_start as *const u8 as u64 };
    let bss_end = unsafe { &_bss_end as *const u8 as u64 };
    let kernel_start = unsafe { &_kernel_start as *const u8 as u64 };
    let kernel_end = unsafe { &_kernel_end as *const u8 as u64 };

    kprintln!("[pml4] Kernel phys={:#018X} virt={:#018X}", kernel_phys_base, kernel_virt_base);
    kprintln!("[pml4]   .text:   {:#018X} — {:#018X} (R+X)", text_start, text_end);
    kprintln!("[pml4]   .rodata: {:#018X} — {:#018X} (R)", rodata_start, rodata_end);
    kprintln!("[pml4]   .data:   {:#018X} — {:#018X} (R+W+NX)", data_start, data_end);
    kprintln!("[pml4]   .bss:    {:#018X} — {:#018X} (R+W+NX)", bss_start, bss_end);

    // Map each page of the kernel higher-half with correct section permissions
    let start_page = kernel_start & !0xFFF;
    let end_page = (kernel_end + 0xFFF) & !0xFFF;
    let mut kernel_pages = 0u64;

    let mut addr = start_page;
    while addr < end_page {
        let flags = classify_flags(addr, text_start, text_end, rodata_start, rodata_end,
                                    data_start, data_end, bss_start, bss_end);

        // Physical address = kernel_phys_base + offset from kernel_virt_base
        let offset = addr - kernel_virt_base;
        let phys = PhysAddr::new(kernel_phys_base + offset);

        match unsafe { vmm::map_page(pml4, VirtAddr::new(addr), phys, flags) } {
            Ok(()) => { kernel_pages += 1; }
            Err(vmm::MapError::AlreadyMapped) => {
                // Could overlap with HHDM if kernel is in HHDM range — skip
            }
            Err(e) => {
                kprintln!("[pml4] WARNING: kernel map failed @ {:#018X}: {:?}", addr, e);
            }
        }

        addr += PAGE_SIZE;
    }

    kprintln!("[pml4] Kernel: {} 4K pages mapped with W^X", kernel_pages);

    // ---- 3. Map MMIO regions (uncacheable, PCD=1, PWT=0) ----
    let mmio_flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::NO_EXECUTE
        | PageTableFlags::NO_CACHE    // PCD=1
        | PageTableFlags::GLOBAL;
    // NOTE: NO WRITE_THROUGH — PWT must be 0 for strictly uncacheable MMIO

    // LAPIC @ 0xFEE00000
    let lapic_phys = 0xFEE0_0000u64;
    match unsafe { vmm::map_page(pml4, VirtAddr::new(hhdm + lapic_phys),
                                  PhysAddr::new(lapic_phys), mmio_flags) } {
        Ok(()) | Err(vmm::MapError::AlreadyMapped) => {}
        Err(e) => kprintln!("[pml4] WARNING: LAPIC MMIO map failed: {:?}", e),
    }

    // I/O APIC @ 0xFEC00000
    let ioapic_phys = 0xFEC0_0000u64;
    match unsafe { vmm::map_page(pml4, VirtAddr::new(hhdm + ioapic_phys),
                                  PhysAddr::new(ioapic_phys), mmio_flags) } {
        Ok(()) | Err(vmm::MapError::AlreadyMapped) => {}
        Err(e) => kprintln!("[pml4] WARNING: I/O APIC MMIO map failed: {:?}", e),
    }

    kprintln!("[pml4] MMIO: LAPIC + I/O APIC mapped (uncacheable, PCD=1 PWT=0)");

    // Store for AP access
    KERNEL_PML4.store(pml4.as_u64(), Ordering::SeqCst);

    kprintln!("[pml4] Pristine PML4 built at phys {}", pml4);
    pml4
}

/// Activates the pristine PML4 by swapping CR3.
///
/// After this call, the kernel runs on clean page tables.
/// The current stack must be mapped in the HHDM of the new PML4.
///
/// # Safety
/// The new PML4 must map:
/// - Current kernel code (or we crash on the next instruction)
/// - Current stack (or we crash on the next memory access)
/// - Serial/framebuffer MMIO (or we lose output)
pub unsafe fn activate(pml4: PhysAddr) {
    kprintln!("[pml4] Activating pristine PML4 (CR3 swap)...");
    unsafe { cpu::write_cr3(pml4.as_u64()); }
    // If we get here, the swap succeeded — stack and code are mapped correctly
    kprintln!("[pml4] CR3 swap complete — running on pristine page tables");
}

/// Builds a user-process PML4 with an isolated lower half.
///
/// The user PML4 shares the kernel's higher-half mappings (indices 256–511)
/// via a shallow copy from the pristine KERNEL_PML4. This guarantees that:
///
///   1. SYSCALL entry works — the CPU does NOT swap CR3 on SYSCALL, so the
///      kernel code, HHDM, heap, stacks, and MMIO must be reachable.
///   2. Interrupt/exception handlers work — same reason, IDT stub runs on
///      the user's page tables until an explicit CR3 swap (if any).
///   3. Ring 3 code CANNOT access the kernel half — all higher-half PTEs
///      have the Supervisor bit (U/S=0), so the CPU blocks Ring 3 reads.
///
/// The lower half (indices 0–255, virtual 0x0 – 0x00007FFF_FFFFFFFF) starts
/// completely empty. User code/data/stack pages are mapped individually by
/// the ELF loader and memory mapping syscalls.
///
/// # Returns
/// Physical address of the new PML4 frame.
///
/// # Panics
/// If the PMM cannot allocate a frame.
pub fn build_user_pml4() -> PhysAddr {
    let user_pml4 = vmm::new_table()
        .expect("[pml4] FATAL: cannot allocate user PML4 frame");

    let kernel_pml4_phys = KERNEL_PML4.load(Ordering::SeqCst);
    assert!(kernel_pml4_phys != 0, "[pml4] FATAL: KERNEL_PML4 not initialized");

    // Access both PML4 tables via HHDM.
    // Each PML4 is a 4K page containing 512 entries × 8 bytes = 4096 bytes.
    let hhdm = address::hhdm_offset();
    let kernel_pml4_ptr = (hhdm + kernel_pml4_phys) as *const u64;
    let user_pml4_ptr = (hhdm + user_pml4.as_u64()) as *mut u64;

    // Copy the top 256 entries (indices 256..511) — the kernel half.
    // These PML4 entries point to PDPT pages that are shared between ALL
    // address spaces. Any kernel mapping change (new HHDM page, kernel
    // module load, etc.) is automatically visible in every user PML4.
    //
    // The bottom 256 entries (indices 0..255) remain zeroed — completely
    // unmapped. User pages will be mapped individually.
    unsafe {
        core::ptr::copy_nonoverlapping(
            kernel_pml4_ptr.add(256),   // src: kernel PML4[256]
            user_pml4_ptr.add(256),     // dst: user PML4[256]
            256,                        // count: 256 entries
        );
    }

    kprintln!("[pml4] User PML4 built at phys {} (kernel half mirrored from {:#010X})",
        user_pml4, kernel_pml4_phys);

    user_pml4
}

/// Determines W^X flags for a kernel virtual address based on section.
fn classify_flags(
    addr: u64,
    text_start: u64, text_end: u64,
    rodata_start: u64, rodata_end: u64,
    data_start: u64, data_end: u64,
    bss_start: u64, bss_end: u64,
) -> PageTableFlags {
    if addr >= text_start && addr < text_end {
        // .text: executable, read-only
        PageTableFlags::KERNEL_CODE
    } else if addr >= rodata_start && addr < rodata_end {
        // .rodata: read-only, not executable
        PageTableFlags::KERNEL_RODATA
    } else if (addr >= data_start && addr < data_end) ||
              (addr >= bss_start && addr < bss_end) {
        // .data/.bss: read-write, not executable
        PageTableFlags::KERNEL_DATA
    } else {
        // Gap between sections: default to data permissions (safe)
        PageTableFlags::KERNEL_DATA
    }
}
