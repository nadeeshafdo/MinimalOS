//! Page table management — [031] [032] [033].
//!
//! Provides 4-level x86-64 page table manipulation using the HHDM
//! to access physical page-table frames.  Intermediate tables are
//! allocated on demand from the bitmap PMM.

use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use super::pmm;

// ── Page table entry flags ────────────────────────────────────────

/// Bit-flag wrapper for page-table entry attributes.
#[derive(Clone, Copy)]
pub struct PageFlags(u64);

#[allow(dead_code)]
impl PageFlags {
	pub const PRESENT: Self = Self(1 << 0);
	pub const WRITABLE: Self = Self(1 << 1);
	pub const USER: Self = Self(1 << 2);
	pub const WRITE_THROUGH: Self = Self(1 << 3);
	pub const CACHE_DISABLE: Self = Self(1 << 4);
	pub const HUGE: Self = Self(1 << 7);
	pub const NO_EXECUTE: Self = Self(1 << 63);

	/// Convenience: kernel read-write page (Present + Writable).
	pub const KERNEL_RW: Self = Self(Self::PRESENT.0 | Self::WRITABLE.0);

	/// Convenience: user read-write page (Present + Writable + User).
	pub const USER_RW: Self = Self(Self::PRESENT.0 | Self::WRITABLE.0 | Self::USER.0);

	#[inline]
	pub const fn bits(self) -> u64 {
		self.0
	}

	#[inline]
	pub const fn empty() -> Self {
		Self(0)
	}

	#[inline]
	pub const fn contains(self, other: Self) -> bool {
		(self.0 & other.0) == other.0
	}
}

impl core::ops::BitOr for PageFlags {
	type Output = Self;
	#[inline]
	fn bitor(self, rhs: Self) -> Self {
		Self(self.0 | rhs.0)
	}
}

// ── Global HHDM offset (set once at init) ─────────────────────────

static HHDM: AtomicU64 = AtomicU64::new(0);

/// Mask to extract the physical address from a page-table entry.
const PHYS_ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

// ── Public API ────────────────────────────────────────────────────

/// Initialise the paging subsystem.  Must be called after PMM init.
pub fn init(hhdm_offset: u64) {
	HHDM.store(hhdm_offset, Ordering::Relaxed);
	klog::info!("[031] Paging subsystem initialised (HHDM={:#x})", hhdm_offset);
}

/// Return the HHDM offset (for use by other modules that need to
/// convert between physical and virtual addresses).
pub fn hhdm_offset() -> u64 {
	HHDM.load(Ordering::Relaxed)
}

/// Create a new user-space page table (PML4).
///
/// Allocates a fresh PML4 frame, copies the kernel's higher-half
/// entries (PML4[256..512]) so kernel code, HHDM, heap, and shared
/// regions are accessible, and returns the **physical address** of
/// the new PML4.
pub fn create_user_page_table() -> Option<u64> {
	let hhdm = HHDM.load(Ordering::Relaxed);

	// Allocate a fresh frame for the new PML4.
	let new_pml4_phys = pmm::alloc_frame()?;
	let new_pml4_virt = (hhdm + new_pml4_phys) as *mut u64;

	// Read the kernel's current PML4.
	let cr3: u64;
	unsafe {
		core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
	}
	let kernel_pml4_phys = cr3 & PHYS_ADDR_MASK;
	let kernel_pml4_virt = (hhdm + kernel_pml4_phys) as *const u64;

	unsafe {
		// Zero the entire new PML4 (lower half starts empty).
		ptr::write_bytes(new_pml4_virt as *mut u8, 0, 4096);

		// Copy higher-half entries (indices 256-511) from the kernel.
		// This shares: kernel code, HHDM, heap, APIC, shared window
		// buffers, and any other kernel-side mappings.
		for i in 256..512 {
			let entry = ptr::read_volatile(kernel_pml4_virt.add(i));
			ptr::write_volatile(new_pml4_virt.add(i), entry);
		}
	}

	Some(new_pml4_phys)
}

/// Pre-allocate the shared user-accessible region for window buffers.
///
/// Creates PML4\[384\] → PDPT so that all process page tables (which
/// copy PML4\[256..512\]) share the same window-buffer hierarchy.
/// Pages mapped within this region are automatically visible to
/// every process.
///
/// Must be called **once** during kernel init, before any process
/// is spawned.
///
/// # Safety
/// Must only be called once, after `init()`.
pub unsafe fn init_shared_user_region() {
	let hhdm = HHDM.load(Ordering::Relaxed);

	let cr3: u64;
	core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
	let pml4_phys = cr3 & PHYS_ADDR_MASK;

	// PML4 index 384 covers virtual addresses starting at
	// 0xFFFF_C000_0000_0000.  Pre-allocate the PDPT so the entry
	// exists before any process copies PML4[256..512].
	ensure_table(hhdm, pml4_phys, 384, PageFlags::USER_RW);

	klog::info!("Shared user region pre-allocated (PML4[384] for window buffers)");
}

/// Map a 4 KiB virtual page in a **specific** page table.
///
/// Like [`map_page`], but operates on the given PML4 physical address
/// instead of the current CR3.  No TLB flush is performed — the
/// caller must flush or switch CR3 as appropriate.
///
/// # Safety
/// Same requirements as `map_page`, plus `pml4_phys` must be a valid
/// PML4 frame allocated via [`create_user_page_table`].
pub unsafe fn map_page_in(pml4_phys: u64, virt: u64, phys: u64, flags: PageFlags) {
	debug_assert!(virt & 0xFFF == 0, "map_page_in: virt not page-aligned");
	debug_assert!(phys & 0xFFF == 0, "map_page_in: phys not frame-aligned");

	let hhdm = HHDM.load(Ordering::Relaxed);

	let pml4_idx = ((virt >> 39) & 0x1FF) as usize;
	let pdpt_idx = ((virt >> 30) & 0x1FF) as usize;
	let pd_idx = ((virt >> 21) & 0x1FF) as usize;
	let pt_idx = ((virt >> 12) & 0x1FF) as usize;

	let pdpt_phys = ensure_table(hhdm, pml4_phys, pml4_idx, flags);
	let pd_phys = ensure_table(hhdm, pdpt_phys, pdpt_idx, flags);
	let pt_phys = ensure_table(hhdm, pd_phys, pd_idx, flags);

	let pt_virt = (hhdm + pt_phys) as *mut u64;
	let entry = phys | flags.bits() | PageFlags::PRESENT.bits();
	ptr::write_volatile(pt_virt.add(pt_idx), entry);
}

/// Translate a virtual address in a **specific** page table.
///
/// Like [`translate`], but walks the given PML4 instead of the
/// current CR3.
pub unsafe fn translate_in(pml4_phys: u64, virt: u64) -> Option<u64> {
	let hhdm = HHDM.load(Ordering::Relaxed);

	let pml4_idx = ((virt >> 39) & 0x1FF) as usize;
	let pdpt_idx = ((virt >> 30) & 0x1FF) as usize;
	let pd_idx = ((virt >> 21) & 0x1FF) as usize;
	let pt_idx = ((virt >> 12) & 0x1FF) as usize;

	let pml4_virt = (hhdm + pml4_phys) as *const u64;
	let pml4e = ptr::read_volatile(pml4_virt.add(pml4_idx));
	if pml4e & PageFlags::PRESENT.bits() == 0 {
		return None;
	}

	let pdpt_virt = (hhdm + (pml4e & PHYS_ADDR_MASK)) as *const u64;
	let pdpte = ptr::read_volatile(pdpt_virt.add(pdpt_idx));
	if pdpte & PageFlags::PRESENT.bits() == 0 {
		return None;
	}
	if pdpte & PageFlags::HUGE.bits() != 0 {
		return Some((pdpte & 0x000F_FFFF_C000_0000) | (virt & 0x3FFF_FFFF));
	}

	let pd_virt = (hhdm + (pdpte & PHYS_ADDR_MASK)) as *const u64;
	let pde = ptr::read_volatile(pd_virt.add(pd_idx));
	if pde & PageFlags::PRESENT.bits() == 0 {
		return None;
	}
	if pde & PageFlags::HUGE.bits() != 0 {
		return Some((pde & 0x000F_FFFF_FFE0_0000) | (virt & 0x1F_FFFF));
	}

	let pt_virt = (hhdm + (pde & PHYS_ADDR_MASK)) as *const u64;
	let pte = ptr::read_volatile(pt_virt.add(pt_idx));
	if pte & PageFlags::PRESENT.bits() == 0 {
		return None;
	}

	Some((pte & PHYS_ADDR_MASK) | (virt & 0xFFF))
}

/// Map a 4 KiB virtual page to a physical frame.
///
/// Walks PML4 → PDPT → PD → PT, allocating intermediate tables
/// from the PMM when entries are not yet present.
///
/// # Safety
///
/// Caller must ensure `virt` is page-aligned, `phys` is frame-aligned,
/// and the mapping does not conflict with existing critical mappings.
pub unsafe fn map_page(virt: u64, phys: u64, flags: PageFlags) {
	debug_assert!(virt & 0xFFF == 0, "map_page: virt not page-aligned");
	debug_assert!(phys & 0xFFF == 0, "map_page: phys not frame-aligned");

	let hhdm = HHDM.load(Ordering::Relaxed);

	// Read CR3 → PML4 physical base
	let cr3: u64;
	core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
	let pml4_phys = cr3 & PHYS_ADDR_MASK;

	// Indices into each level
	let pml4_idx = ((virt >> 39) & 0x1FF) as usize;
	let pdpt_idx = ((virt >> 30) & 0x1FF) as usize;
	let pd_idx = ((virt >> 21) & 0x1FF) as usize;
	let pt_idx = ((virt >> 12) & 0x1FF) as usize;

	// Walk / create: PML4 → PDPT → PD → PT
	// When the leaf page is USER-accessible, intermediate entries
	// must also have the USER bit set.
	let pdpt_phys = ensure_table(hhdm, pml4_phys, pml4_idx, flags);
	let pd_phys = ensure_table(hhdm, pdpt_phys, pdpt_idx, flags);
	let pt_phys = ensure_table(hhdm, pd_phys, pd_idx, flags);

	// Write the final PT entry
	let pt_virt = (hhdm + pt_phys) as *mut u64;
	let entry = phys | flags.bits() | PageFlags::PRESENT.bits();
	ptr::write_volatile(pt_virt.add(pt_idx), entry);

	// Flush TLB for this page
	core::arch::asm!(
		"invlpg [{}]",
		in(reg) virt,
		options(nostack, preserves_flags),
	);
}

/// Translate a virtual address to its physical address by walking
/// the current page tables.
///
/// Returns `None` if any level is not present.
/// Handles 4 KiB, 2 MiB huge, and 1 GiB huge pages.
pub unsafe fn translate(virt: u64) -> Option<u64> {
	let hhdm = HHDM.load(Ordering::Relaxed);

	let cr3: u64;
	core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
	let pml4_phys = cr3 & PHYS_ADDR_MASK;

	let pml4_idx = ((virt >> 39) & 0x1FF) as usize;
	let pdpt_idx = ((virt >> 30) & 0x1FF) as usize;
	let pd_idx = ((virt >> 21) & 0x1FF) as usize;
	let pt_idx = ((virt >> 12) & 0x1FF) as usize;

	// PML4
	let pml4_virt = (hhdm + pml4_phys) as *const u64;
	let pml4e = ptr::read_volatile(pml4_virt.add(pml4_idx));
	if pml4e & PageFlags::PRESENT.bits() == 0 {
		return None;
	}

	// PDPT
	let pdpt_virt = (hhdm + (pml4e & PHYS_ADDR_MASK)) as *const u64;
	let pdpte = ptr::read_volatile(pdpt_virt.add(pdpt_idx));
	if pdpte & PageFlags::PRESENT.bits() == 0 {
		return None;
	}
	if pdpte & PageFlags::HUGE.bits() != 0 {
		// 1 GiB huge page
		return Some((pdpte & 0x000F_FFFF_C000_0000) | (virt & 0x3FFF_FFFF));
	}

	// PD
	let pd_virt = (hhdm + (pdpte & PHYS_ADDR_MASK)) as *const u64;
	let pde = ptr::read_volatile(pd_virt.add(pd_idx));
	if pde & PageFlags::PRESENT.bits() == 0 {
		return None;
	}
	if pde & PageFlags::HUGE.bits() != 0 {
		// 2 MiB huge page
		return Some((pde & 0x000F_FFFF_FFE0_0000) | (virt & 0x1F_FFFF));
	}

	// PT
	let pt_virt = (hhdm + (pde & PHYS_ADDR_MASK)) as *const u64;
	let pte = ptr::read_volatile(pt_virt.add(pt_idx));
	if pte & PageFlags::PRESENT.bits() == 0 {
		return None;
	}

	Some((pte & PHYS_ADDR_MASK) | (virt & 0xFFF))
}

// ── Internal helpers ──────────────────────────────────────────────

/// Ensure `table[index]` points to a valid next-level table.
/// If the entry is not present, allocate a zeroed frame from the PMM
/// and install it.  Returns the **physical address** of the next-level table.
///
/// When `leaf_flags` contains USER, intermediate entries are also
/// marked USER so that user-mode accesses can traverse the full walk.
unsafe fn ensure_table(hhdm: u64, table_phys: u64, index: usize, leaf_flags: PageFlags) -> u64 {
	let table_virt = (hhdm + table_phys) as *mut u64;
	let entry = ptr::read_volatile(table_virt.add(index));

	if entry & PageFlags::PRESENT.bits() != 0 {
		// Already present — ensure USER bit is set if the leaf needs it.
		if leaf_flags.contains(PageFlags::USER) && (entry & PageFlags::USER.bits() == 0) {
			ptr::write_volatile(
				table_virt.add(index),
				entry | PageFlags::USER.bits(),
			);
		}
		return entry & PHYS_ADDR_MASK;
	}

	// Allocate a new 4 KiB frame for the next-level table
	let new_frame = pmm::alloc_frame()
		.expect("paging: out of physical memory for page table");

	// Zero the new frame
	let new_frame_virt = (hhdm + new_frame) as *mut u8;
	ptr::write_bytes(new_frame_virt, 0, 4096);

	// Install the entry: Present + Writable.
	// If the leaf page will be USER-accessible, set USER here too
	// so the CPU walk can reach the leaf from Ring 3.
	let mut flags_bits = PageFlags::PRESENT.bits() | PageFlags::WRITABLE.bits();
	if leaf_flags.contains(PageFlags::USER) {
		flags_bits |= PageFlags::USER.bits();
	}
	let new_entry = new_frame | flags_bits;
	ptr::write_volatile(table_virt.add(index), new_entry);

	new_frame
}
