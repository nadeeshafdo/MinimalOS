//! Physical and virtual memory management.

pub mod heap;
pub mod paging;
pub mod pmm;

use core::ptr;
use limine::memory_map::{Entry, EntryType};

/// Page table entry flags.
const PTE_PRESENT: u64 = 1 << 0;
const PTE_WRITABLE: u64 = 1 << 1;
const PTE_HUGE: u64 = 1 << 7; // Page Size bit for 2MiB pages
const PTE_PCD: u64 = 1 << 4; // Page Cache Disable (for MMIO)
const PTE_PWT: u64 = 1 << 3; // Page Write-Through

/// A statically allocated 4KiB page for the Page Directory table.
/// Used to map the APIC MMIO region when no dynamic allocator is available.
#[repr(C, align(4096))]
struct PageTable {
	entries: [u64; 512],
}

/// Static PD page for APIC MMIO mapping.
static mut APIC_PD_PAGE: PageTable = PageTable { entries: [0; 512] };

/// Map the APIC MMIO region into the HHDM virtual address space.
///
/// This creates page table entries so that `hhdm_offset + apic_phys`
/// is a valid virtual address pointing to the APIC MMIO registers.
///
/// # Arguments
///
/// * `hhdm_offset` - The Higher Half Direct Map offset from Limine
/// * `apic_phys` - Physical base address of the APIC (typically 0xFEE00000)
///
/// # Safety
///
/// This function modifies page tables directly and must only be called
/// once during early kernel initialization.
pub unsafe fn map_apic_mmio(hhdm_offset: u64, apic_phys: u64) {
	// Read CR3 to get the PML4 physical base address
	let cr3: u64;
	core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
	let pml4_phys = cr3 & 0x000F_FFFF_FFFF_F000; // Mask to physical address

	// Convert PML4 physical address to virtual via HHDM
	let pml4_virt = (hhdm_offset + pml4_phys) as *mut u64;

	// Target virtual address: hhdm_offset + apic_phys
	// Compute page table indices for this virtual address
	let virt_addr = hhdm_offset + apic_phys;
	let pml4_idx = ((virt_addr >> 39) & 0x1FF) as usize;
	let pdpt_idx = ((virt_addr >> 30) & 0x1FF) as usize;
	let pd_idx = ((virt_addr >> 21) & 0x1FF) as usize;

	klog::debug!("Mapping APIC MMIO: phys={:#x} -> virt={:#x}", apic_phys, virt_addr);
	klog::debug!("  PML4[{}] -> PDPT[{}] -> PD[{}] (2MiB huge page)", pml4_idx, pdpt_idx, pd_idx);

	// Step 1: Read PML4[pml4_idx] to get the PDPT
	let pml4_entry = ptr::read_volatile(pml4_virt.add(pml4_idx));
	if pml4_entry & PTE_PRESENT == 0 {
		klog::error!("PML4[{}] not present! Cannot map APIC.", pml4_idx);
		return;
	}
	let pdpt_phys = pml4_entry & 0x000F_FFFF_FFFF_F000;
	let pdpt_virt = (hhdm_offset + pdpt_phys) as *mut u64;

	// Step 2: Check PDPT[pdpt_idx] — if not present, create it
	let pdpt_entry = ptr::read_volatile(pdpt_virt.add(pdpt_idx));
	if pdpt_entry & PTE_PRESENT == 0 {
		klog::debug!("  PDPT[{}] not present, allocating PD page...", pdpt_idx);

		// Use our static PD page
		let pd_page_virt = &raw mut APIC_PD_PAGE as *mut PageTable;

		// Zero the PD page
		ptr::write_bytes(pd_page_virt as *mut u8, 0, 4096);

		// Convert PD virtual address to physical
		// The static is in kernel BSS, which is in the higher half.
		// Kernel virtual address = 0xFFFFFFFF80000000 + kernel_offset
		// Physical = virtual - kernel_base (if identity mapped in HHDM)
		// Since the kernel is loaded via Limine, the kernel's virtual addresses
		// in the higher half correspond to physical addresses accessible via HHDM.
		// physical = virtual - hhdm_offset (for HHDM-mapped pages)
		// But kernel BSS is at 0xFFFFFFFF80xxxxxx, not in HHDM range.
		// We need to find the physical address of our static page.
		//
		// The kernel is linked at 0xFFFFFFFF80000000 (higher half).
		// Limine loads the kernel at some physical address and maps it there.
		// To find the physical address, we can use the Limine kernel address feature,
		// or compute it from the HHDM mapping.
		//
		// Simpler approach: the kernel's higher-half mapping at 0xFFFFFFFF80000000
		// maps physical addresses starting from the kernel's physical load address.
		// We can compute: phys = virt - 0xFFFFFFFF80000000 + kernel_phys_base
		//
		// But we don't know kernel_phys_base without another Limine request.
		// Alternative: walk the page tables for our own address to find the physical address.
		let pd_virt_addr = pd_page_virt as u64;
		let pd_phys = virt_to_phys(hhdm_offset, pml4_virt, pd_virt_addr);

		klog::debug!("  PD page: virt={:#x}, phys={:#x}", pd_virt_addr, pd_phys);

		// Create PDPT entry pointing to our PD page
		let new_pdpt_entry = pd_phys | PTE_PRESENT | PTE_WRITABLE;
		ptr::write_volatile(pdpt_virt.add(pdpt_idx), new_pdpt_entry);
	}

	// Re-read PDPT entry to get PD base
	let pdpt_entry = ptr::read_volatile(pdpt_virt.add(pdpt_idx));
	let pd_phys = pdpt_entry & 0x000F_FFFF_FFFF_F000;
	let pd_virt = (hhdm_offset + pd_phys) as *mut u64;

	// Step 3: Create PD[pd_idx] as a 2MiB huge page
	// Map the 2MiB region containing the APIC physical address
	let huge_page_phys = apic_phys & !0x1F_FFFF; // 2MiB aligned
	let pd_entry = huge_page_phys | PTE_PRESENT | PTE_WRITABLE | PTE_HUGE | PTE_PCD | PTE_PWT;
	ptr::write_volatile(pd_virt.add(pd_idx), pd_entry);

	klog::debug!("  PD[{}] = {:#018x} (2MiB page at phys {:#x})", pd_idx, pd_entry, huge_page_phys);

	// Flush TLB for the mapped address
	core::arch::asm!(
		"invlpg [{}]",
		in(reg) virt_addr,
		options(nostack, preserves_flags)
	);

	klog::info!("APIC MMIO mapped: {:#x} -> {:#x}", apic_phys, virt_addr);
}

/// Translate a virtual address to physical by walking the page tables.
///
/// This handles both 4KiB pages and 2MiB huge pages.
unsafe fn virt_to_phys(hhdm_offset: u64, pml4_virt: *const u64, virt_addr: u64) -> u64 {
	let pml4_idx = ((virt_addr >> 39) & 0x1FF) as usize;
	let pdpt_idx = ((virt_addr >> 30) & 0x1FF) as usize;
	let pd_idx = ((virt_addr >> 21) & 0x1FF) as usize;
	let pt_idx = ((virt_addr >> 12) & 0x1FF) as usize;

	let pml4_entry = ptr::read_volatile(pml4_virt.add(pml4_idx));
	if pml4_entry & PTE_PRESENT == 0 {
		return 0;
	}

	let pdpt_virt = (hhdm_offset + (pml4_entry & 0x000F_FFFF_FFFF_F000)) as *const u64;
	let pdpt_entry = ptr::read_volatile(pdpt_virt.add(pdpt_idx));
	if pdpt_entry & PTE_PRESENT == 0 {
		return 0;
	}
	// Check for 1GiB huge page
	if pdpt_entry & PTE_HUGE != 0 {
		return (pdpt_entry & 0x000F_FFFF_C000_0000) | (virt_addr & 0x3FFF_FFFF);
	}

	let pd_virt = (hhdm_offset + (pdpt_entry & 0x000F_FFFF_FFFF_F000)) as *const u64;
	let pd_entry = ptr::read_volatile(pd_virt.add(pd_idx));
	if pd_entry & PTE_PRESENT == 0 {
		return 0;
	}
	// Check for 2MiB huge page
	if pd_entry & PTE_HUGE != 0 {
		return (pd_entry & 0x000F_FFFF_FFE0_0000) | (virt_addr & 0x1F_FFFF);
	}

	let pt_virt = (hhdm_offset + (pd_entry & 0x000F_FFFF_FFFF_F000)) as *const u64;
	let pt_entry = ptr::read_volatile(pt_virt.add(pt_idx));
	if pt_entry & PTE_PRESENT == 0 {
		return 0;
	}

	(pt_entry & 0x000F_FFFF_FFFF_F000) | (virt_addr & 0xFFF)
}

/// Human-readable name for a memory map entry type.
fn entry_type_name(et: EntryType) -> &'static str {
	match et {
		EntryType::USABLE => "Usable",
		EntryType::RESERVED => "Reserved",
		EntryType::ACPI_RECLAIMABLE => "ACPI Reclaimable",
		EntryType::ACPI_NVS => "ACPI NVS",
		EntryType::BAD_MEMORY => "Bad Memory",
		EntryType::BOOTLOADER_RECLAIMABLE => "Bootloader Reclaimable",
		EntryType::EXECUTABLE_AND_MODULES => "Kernel/Modules",
		EntryType::FRAMEBUFFER => "Framebuffer",
		_ => "Unknown",
	}
}

/// Iterate the Limine memory map and log each region.
///
/// Returns `(total_ram, usable_ram)` in bytes.
/// - `total_ram` includes all non-bad-memory regions (physical footprint).
/// - `usable_ram` includes only `USABLE` regions available for allocation.
pub fn census(entries: &[&Entry]) -> (u64, u64) {
	klog::info!("[027] Memory Map Census ({} entries):", entries.len());
	klog::info!("  {:<20} {:>16}  {:>12}  {}", "Type", "Base", "Length", "End");
	klog::info!("  {:-<20} {:-<16}  {:-<12}  {:-<16}", "", "", "", "");

	let mut total_ram: u64 = 0;
	let mut usable_ram: u64 = 0;

	for entry in entries {
		let base = entry.base;
		let length = entry.length;
		let et = entry.entry_type;
		let end = base + length;

		klog::info!(
			"  {:<20} {:#016x}  {:>10} KiB  {:#016x}",
			entry_type_name(et),
			base,
			length / 1024,
			end,
		);

		// Sum usable RAM (what we can allocate from)
		if et == EntryType::USABLE {
			usable_ram += length;
		}

		// Sum total physical footprint (everything except bad memory)
		if et != EntryType::BAD_MEMORY {
			total_ram += length;
		}
	}

	klog::info!("  {:-<20} {:-<16}  {:-<12}  {:-<16}", "", "", "", "");
	klog::info!(
		"  Total RAM:  {} MiB ({} bytes)",
		total_ram / (1024 * 1024),
		total_ram,
	);
	klog::info!(
		"  Usable RAM: {} MiB ({} bytes)",
		usable_ram / (1024 * 1024),
		usable_ram,
	);

	(total_ram, usable_ram)
}
