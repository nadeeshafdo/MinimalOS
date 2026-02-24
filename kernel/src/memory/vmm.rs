// =============================================================================
// MinimalOS NextGen — Virtual Memory Manager (Page Table Infrastructure)
// =============================================================================
//
// This module provides types and functions for manipulating x86_64 4-level
// page tables. It does NOT own or switch the active page tables (that happens
// in Sprint 3 when we have exception handlers for debugging crashes).
//
// x86_64 PAGING OVERVIEW:
//
//   Virtual addresses are translated through 4 levels of page tables:
//
//   PML4 (Level 4) → PDPT (Level 3) → PD (Level 2) → PT (Level 1) → Page
//
//   Each level is a 4 KiB table containing 512 entries (each 8 bytes).
//   Each entry holds:
//     - The physical address of the next-level table (or the final page)
//     - Permission flags (present, writable, user-accessible, etc.)
//     - Status flags (accessed, dirty)
//
//   ```text
//   63  62..52  51..12       11..9   8   7   6   5   4   3   2   1   0
//   ┌───┬──────┬────────────┬───────┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
//   │NXE│ Avail│ Phys Addr  │ Avail │ G │PS │ D │ A │PCD│PWT│U/S│R/W│ P │
//   └───┴──────┴────────────┴───────┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
//   ```
//
//   Bit 0 (P):   Present — entry is valid
//   Bit 1 (R/W): Read/Write — if 0, writes cause page fault
//   Bit 2 (U/S): User/Supervisor — if 0, user-mode access causes fault
//   Bit 3 (PWT): Page-level Write-Through — controls caching
//   Bit 4 (PCD): Page-level Cache Disable — controls caching
//   Bit 5 (A):   Accessed — set by CPU on any access
//   Bit 6 (D):   Dirty — set by CPU on write (valid in PT/leaf entries only)
//   Bit 7 (PS):  Page Size — in PD, makes a 2 MiB huge page (skips PT level)
//   Bit 8 (G):   Global — TLB entry survives CR3 switch (kernel pages)
//   Bit 63(NXE): No-Execute — if set, instruction fetch causes fault (W^X!)
//
// ADDRESS EXTRACTION:
//   The physical address stored in an entry is bits 51:12 (40 bits).
//   Mask: 0x000F_FFFF_FFFF_F000
//   This gives a page-aligned (4 KiB) physical address.
//
// WALKING THE PAGE TABLES:
//   Given a virtual address, we extract 4 × 9-bit indices:
//     PML4 index = bits [47:39]  → which PML4 entry
//     PDPT index = bits [38:30]  → which PDPT entry
//     PD index   = bits [29:21]  → which PD entry
//     PT index   = bits [20:12]  → which PT entry
//     Offset     = bits [11:0]   → byte within the 4 KiB page
//
//   At each level, we read an entry, check the Present bit, extract the
//   physical address, convert it to virtual via HHDM, and index into the
//   next table.
//
// W^X ENFORCEMENT:
//   A memory region should be either Writable or eXecutable, never both.
//   The NX (No-Execute) bit enables this:
//     - Code (.text):   PRESENT | EXECUTABLE         (no WRITABLE, no NX)
//     - Data (.data):   PRESENT | WRITABLE | NX      (no execute)
//     - Read-only:      PRESENT | NX                 (no write, no execute)
//     - Stack:          PRESENT | WRITABLE | NX      (no execute)
//
// =============================================================================

use bitflags::bitflags;

use crate::arch::cpu;
use crate::memory::address::{PhysAddr, VirtAddr};
use crate::memory::pmm;

// =============================================================================
// Page Table Flags
// =============================================================================

bitflags! {
    /// x86_64 page table entry flags.
    ///
    /// These control the permissions and behaviour of mapped pages.
    /// The flags are applied hierarchically — the effective permissions
    /// are the intersection (most restrictive) of all levels.
    ///
    /// Convention: intermediate tables (PML4 → PD) should be permissive
    /// (PRESENT | WRITABLE | USER if needed), with restrictions applied
    /// at the leaf level (PT entry or huge-page PD entry).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PageTableFlags: u64 {
        /// Page is present in physical memory.
        /// If clear, all other bits are ignored and access causes a page fault.
        const PRESENT       = 1 << 0;

        /// Page is writable. If clear, writes cause a page fault.
        /// For W^X: never combine with absence of NO_EXECUTE on code pages.
        const WRITABLE      = 1 << 1;

        /// Page is accessible from user mode (Ring 3).
        /// If clear, only kernel mode (Ring 0) can access.
        const USER          = 1 << 2;

        /// Write-through caching. Writes go to cache AND memory.
        /// Used for memory-mapped I/O where write ordering matters.
        const WRITE_THROUGH = 1 << 3;

        /// Disable caching for this page.
        /// Used for memory-mapped I/O (framebuffer, device registers).
        const NO_CACHE      = 1 << 4;

        /// CPU sets this bit on any access (read or write).
        /// Used by the OS to implement page aging / clock algorithm.
        const ACCESSED      = 1 << 5;

        /// CPU sets this bit on a write.
        /// Used by the OS to know which pages need writing to disk.
        const DIRTY         = 1 << 6;

        /// In PD entries: makes a 2 MiB huge page (skips PT level).
        /// In PDPT entries: makes a 1 GiB gigantic page (rare).
        /// Must be 0 in PML4 and PT entries.
        const HUGE_PAGE     = 1 << 7;

        /// Global page — TLB entry survives CR3 switches.
        /// Used for kernel mappings that are identical across all
        /// address spaces (avoids unnecessary TLB flushes on context switch).
        const GLOBAL        = 1 << 8;

        /// No-Execute (NX / XD). Instruction fetches cause a page fault.
        /// CRITICAL for W^X security: set this on all data/stack pages.
        /// Requires IA32_EFER.NXE to be enabled (Limine does this).
        const NO_EXECUTE    = 1 << 63;
    }
}

impl PageTableFlags {
    /// Flags for a kernel code page: readable + executable, not writable.
    ///
    /// .text sections use this — code should never be writable (W^X).
    pub const KERNEL_CODE: Self =
        Self::PRESENT.union(Self::GLOBAL);

    /// Flags for a kernel read-only data page: readable, not writable, not executable.
    ///
    /// .rodata sections use this.
    pub const KERNEL_RODATA: Self =
        Self::PRESENT.union(Self::GLOBAL).union(Self::NO_EXECUTE);

    /// Flags for a kernel read-write data page: readable + writable, not executable.
    ///
    /// .data, .bss, stack, heap use this.
    pub const KERNEL_DATA: Self =
        Self::PRESENT.union(Self::GLOBAL).union(Self::WRITABLE).union(Self::NO_EXECUTE);

    /// Flags for an intermediate (non-leaf) page table entry.
    ///
    /// Intermediate entries should be maximally permissive because the
    /// effective permissions are the intersection of all levels.
    /// Restrictions are applied at the leaf.
    pub const INTERMEDIATE: Self =
        Self::PRESENT.union(Self::WRITABLE);

    /// Same as INTERMEDIATE but also allows user-mode access.
    /// Used when the leaf entry needs USER set.
    pub const INTERMEDIATE_USER: Self =
        Self::PRESENT.union(Self::WRITABLE).union(Self::USER);
}

// =============================================================================
// Page Table Entry
// =============================================================================

/// A single entry in an x86_64 page table.
///
/// Each entry is 8 bytes. The table has 512 entries = 4096 bytes (one page).
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

/// Mask for extracting the physical address from a page table entry.
/// Bits 12 through 51 — the 40-bit physical page frame number.
const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

impl PageTableEntry {
    /// A non-present (zeroed) entry.
    pub const EMPTY: Self = Self(0);

    /// Returns the raw u64 value.
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Returns the flags portion of this entry.
    #[inline]
    pub fn flags(self) -> PageTableFlags {
        PageTableFlags::from_bits_truncate(self.0)
    }

    /// Returns the physical address stored in this entry.
    ///
    /// Only meaningful if the entry is present.
    #[inline]
    pub fn addr(self) -> PhysAddr {
        PhysAddr::new(self.0 & ADDR_MASK)
    }

    /// Returns `true` if the PRESENT bit is set.
    #[inline]
    pub fn is_present(self) -> bool {
        self.0 & PageTableFlags::PRESENT.bits() != 0
    }

    /// Returns `true` if this is a huge page entry (2 MiB or 1 GiB).
    ///
    /// Only valid in PD (level 2) and PDPT (level 3) entries.
    #[inline]
    pub fn is_huge(self) -> bool {
        self.0 & PageTableFlags::HUGE_PAGE.bits() != 0
    }

    /// Returns `true` if the entry is empty (all bits zero).
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Sets this entry to map `addr` with the given `flags`.
    ///
    /// The address must be page-aligned (bits 0–11 = 0).
    #[inline]
    pub fn set(&mut self, addr: PhysAddr, flags: PageTableFlags) {
        debug_assert!(
            addr.is_page_aligned(),
            "VMM: page table entry address must be page-aligned"
        );
        self.0 = (addr.as_u64() & ADDR_MASK) | flags.bits();
    }

    /// Clears this entry (sets all bits to 0 = non-present).
    #[inline]
    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

impl core::fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_present() {
            write!(f, "PTE({} {:?})", self.addr(), self.flags())
        } else {
            write!(f, "PTE(empty)")
        }
    }
}

// =============================================================================
// Page Table
// =============================================================================

/// A 4-level x86_64 page table.
///
/// Contains exactly 512 entries, each 8 bytes, totaling 4096 bytes (one page).
/// This type is 4 KiB aligned so it can be placed directly in a physical frame.
///
/// # Level naming convention
///   Level 4: PML4   (Page Map Level 4)      — the root, pointed to by CR3
///   Level 3: PDPT   (Page Directory Pointer Table)
///   Level 2: PD     (Page Directory)
///   Level 1: PT     (Page Table)             — the leaf, points to 4 KiB pages
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    /// Returns a reference to the entry at `index`.
    ///
    /// # Panics
    /// If `index >= 512`.
    #[inline]
    pub fn entry(&self, index: usize) -> &PageTableEntry {
        &self.entries[index]
    }

    /// Returns a mutable reference to the entry at `index`.
    ///
    /// # Panics
    /// If `index >= 512`.
    #[inline]
    pub fn entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }

    /// Zeroes all entries (makes them non-present).
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.clear();
        }
    }

    /// Returns an iterator over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &PageTableEntry> {
        self.entries.iter()
    }
}

impl core::ops::Index<usize> for PageTable {
    type Output = PageTableEntry;
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl core::ops::IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

// =============================================================================
// Error types
// =============================================================================

/// Error returned when a page mapping operation fails.
#[derive(Debug)]
pub enum MapError {
    /// The virtual address is already mapped to a physical frame.
    AlreadyMapped,
    /// The physical memory manager has no free frames for a new page table.
    OutOfMemory,
    /// A huge page exists at an intermediate level, blocking the walk.
    HugePageConflict,
}

/// Error returned when an unmap operation fails.
#[derive(Debug)]
pub enum UnmapError {
    /// The virtual address is not currently mapped.
    NotMapped,
    /// A huge page exists at an intermediate level (can't unmap 4K within it).
    HugePageConflict,
}

// =============================================================================
// Page table operations
// =============================================================================

/// Returns the physical address of the currently active PML4 (from CR3).
///
/// The lower 12 bits of CR3 contain flags (PCID on newer CPUs); we mask
/// them out to get the page-aligned physical address.
#[inline]
pub fn active_pml4() -> PhysAddr {
    PhysAddr::new(cpu::read_cr3() & ADDR_MASK)
}

/// Allocates a new zeroed page table from the physical memory manager.
///
/// Returns the physical address of the new table. All 512 entries are
/// initialized to 0 (non-present), so no accidental mappings exist.
///
/// # Returns
/// `Some(PhysAddr)` — physical address of the new, zeroed page table.
/// `None` — out of physical memory.
pub fn new_table() -> Option<PhysAddr> {
    pmm::alloc_frame_zeroed()
}

/// Maps a 4 KiB virtual page to a physical frame.
///
/// Walks the 4-level page table hierarchy starting from `pml4_phys`,
/// creating intermediate tables as needed (allocated from PMM).
///
/// # Parameters
/// - `pml4_phys`: Physical address of the root PML4 table.
/// - `virt`: The virtual address to map (must be page-aligned).
/// - `phys`: The physical frame to map to (must be page-aligned).
/// - `flags`: Permission and attribute flags for the mapping.
///
/// # Returns
/// `Ok(())` on success, `Err(MapError)` if the mapping could not be created.
///
/// # Safety
/// - `pml4_phys` must point to a valid PML4 table accessible via HHDM.
/// - The caller must ensure the mapping is correct and won't cause crashes
///   (e.g., don't unmap the currently executing code).
/// - Caller must flush the TLB for `virt` after calling this (use `flush()`).
pub unsafe fn map_page(
    pml4_phys: PhysAddr,
    virt: VirtAddr,
    phys: PhysAddr,
    flags: PageTableFlags,
) -> Result<(), MapError> {
    debug_assert!(virt.is_page_aligned(), "VMM: virt address not page-aligned");
    debug_assert!(phys.is_page_aligned(), "VMM: phys address not page-aligned");

    let indices = virt.page_table_indices();
    // indices: [PT, PD, PDPT, PML4] (level 1 first)

    // Determine intermediate flags based on whether this is a user mapping.
    let inter_flags = if flags.contains(PageTableFlags::USER) {
        PageTableFlags::INTERMEDIATE_USER
    } else {
        PageTableFlags::INTERMEDIATE
    };

    // Walk PML4 → PDPT
    let pml4 = unsafe { &mut *pml4_phys.to_virt().as_mut_ptr::<PageTable>() };
    let pdpt_phys = get_or_create_next_table(
        &mut pml4[indices[3] as usize],
        inter_flags,
    )?;

    // Walk PDPT → PD
    let pdpt = unsafe { &mut *pdpt_phys.to_virt().as_mut_ptr::<PageTable>() };
    let pdpt_entry = &pdpt[indices[2] as usize];
    if pdpt_entry.is_present() && pdpt_entry.is_huge() {
        return Err(MapError::HugePageConflict);
    }
    let pd_phys = get_or_create_next_table(
        &mut pdpt[indices[2] as usize],
        inter_flags,
    )?;

    // Walk PD → PT
    let pd = unsafe { &mut *pd_phys.to_virt().as_mut_ptr::<PageTable>() };
    let pd_entry = &pd[indices[1] as usize];
    if pd_entry.is_present() && pd_entry.is_huge() {
        return Err(MapError::HugePageConflict);
    }
    let pt_phys = get_or_create_next_table(
        &mut pd[indices[1] as usize],
        inter_flags,
    )?;

    // Set the PT (leaf) entry
    let pt = unsafe { &mut *pt_phys.to_virt().as_mut_ptr::<PageTable>() };
    let leaf = &mut pt[indices[0] as usize];

    if leaf.is_present() {
        return Err(MapError::AlreadyMapped);
    }

    leaf.set(phys, flags);
    Ok(())
}

/// Unmaps a 4 KiB virtual page, returning the physical frame it was mapped to.
///
/// Does NOT free the physical frame — the caller decides what to do with it
/// (it might be shared, memory-mapped I/O, etc.).
///
/// # Parameters
/// - `pml4_phys`: Physical address of the root PML4 table.
/// - `virt`: The virtual address to unmap (must be page-aligned).
///
/// # Returns
/// `Ok(PhysAddr)` — the physical frame that was mapped.
/// `Err(UnmapError)` — the address wasn't mapped or a huge page blocks it.
///
/// # Safety
/// - `pml4_phys` must point to a valid PML4 table accessible via HHDM.
/// - Don't unmap memory that's currently in use (stack, code, page tables).
/// - Caller must flush the TLB for `virt` after calling this.
pub unsafe fn unmap_page(
    pml4_phys: PhysAddr,
    virt: VirtAddr,
) -> Result<PhysAddr, UnmapError> {
    debug_assert!(virt.is_page_aligned(), "VMM: virt address not page-aligned");

    let indices = virt.page_table_indices();

    // Walk to the PT entry
    let pml4 = unsafe { &*pml4_phys.to_virt().as_ptr::<PageTable>() };
    let pml4_entry = &pml4[indices[3] as usize];
    if !pml4_entry.is_present() {
        return Err(UnmapError::NotMapped);
    }

    let pdpt = unsafe { &*pml4_entry.addr().to_virt().as_ptr::<PageTable>() };
    let pdpt_entry = &pdpt[indices[2] as usize];
    if !pdpt_entry.is_present() {
        return Err(UnmapError::NotMapped);
    }
    if pdpt_entry.is_huge() {
        return Err(UnmapError::HugePageConflict);
    }

    let pd = unsafe { &*pdpt_entry.addr().to_virt().as_ptr::<PageTable>() };
    let pd_entry = &pd[indices[1] as usize];
    if !pd_entry.is_present() {
        return Err(UnmapError::NotMapped);
    }
    if pd_entry.is_huge() {
        return Err(UnmapError::HugePageConflict);
    }

    let pt = unsafe { &mut *pd_entry.addr().to_virt().as_mut_ptr::<PageTable>() };
    let leaf = &mut pt[indices[0] as usize];

    if !leaf.is_present() {
        return Err(UnmapError::NotMapped);
    }

    let phys = leaf.addr();
    leaf.clear();
    Ok(phys)
}

/// Translates a virtual address to its physical address by walking the
/// current page tables.
///
/// # Parameters
/// - `pml4_phys`: Physical address of the PML4 to walk.
/// - `virt`: The virtual address to translate.
///
/// # Returns
/// `Some(PhysAddr)` — the physical address (including page offset).
/// `None` — the address is not mapped.
pub fn translate(pml4_phys: PhysAddr, virt: VirtAddr) -> Option<PhysAddr> {
    let indices = virt.page_table_indices();
    let offset = virt.page_offset() as u64;

    // Level 4: PML4
    let pml4 = unsafe { &*pml4_phys.to_virt().as_ptr::<PageTable>() };
    let pml4_entry = &pml4[indices[3] as usize];
    if !pml4_entry.is_present() {
        return None;
    }

    // Level 3: PDPT
    let pdpt = unsafe { &*pml4_entry.addr().to_virt().as_ptr::<PageTable>() };
    let pdpt_entry = &pdpt[indices[2] as usize];
    if !pdpt_entry.is_present() {
        return None;
    }
    if pdpt_entry.is_huge() {
        // 1 GiB page — add the 30-bit offset
        let gib_offset = virt.as_u64() & 0x3FFF_FFFF; // bits [29:0]
        return Some(PhysAddr::new((pdpt_entry.addr().as_u64() & !0x3FFF_FFFF) + gib_offset));
    }

    // Level 2: PD
    let pd = unsafe { &*pdpt_entry.addr().to_virt().as_ptr::<PageTable>() };
    let pd_entry = &pd[indices[1] as usize];
    if !pd_entry.is_present() {
        return None;
    }
    if pd_entry.is_huge() {
        // 2 MiB page — add the 21-bit offset
        let mib_offset = virt.as_u64() & 0x1F_FFFF; // bits [20:0]
        return Some(PhysAddr::new((pd_entry.addr().as_u64() & !0x1F_FFFF) + mib_offset));
    }

    // Level 1: PT
    let pt = unsafe { &*pd_entry.addr().to_virt().as_ptr::<PageTable>() };
    let pt_entry = &pt[indices[0] as usize];
    if !pt_entry.is_present() {
        return None;
    }

    // 4 KiB page — add the 12-bit offset
    Some(PhysAddr::new(pt_entry.addr().as_u64() + offset))
}

/// Flushes the TLB entry for a single virtual address.
///
/// Must be called after modifying a page table entry to ensure the CPU
/// uses the new mapping. On multi-core systems, other cores need a
/// TLB shootdown IPI (not yet implemented — single core in Sprint 2).
///
/// On N3710: INVLPG ≈ 10–20 cycles (vs 50–100 for full flush).
#[inline]
pub fn flush(virt: VirtAddr) {
    cpu::invlpg(virt.as_u64());
}

/// Flushes the entire TLB by reloading CR3.
///
/// Use this after bulk page table changes (e.g., remapping the kernel).
/// On N3710: full TLB flush ≈ 50–100 cycles.
///
/// # Safety
/// The current CR3 must still point to a valid PML4.
pub unsafe fn flush_all() {
    let cr3 = cpu::read_cr3();
    unsafe { cpu::write_cr3(cr3); }
}

// =============================================================================
// Internal helpers
// =============================================================================

/// If the entry is present, return the physical address it points to.
/// If not, allocate a new zeroed page table, set the entry, and return its address.
fn get_or_create_next_table(
    entry: &mut PageTableEntry,
    flags: PageTableFlags,
) -> Result<PhysAddr, MapError> {
    if entry.is_present() {
        // Table already exists — return its physical address.
        // Note: we don't modify the existing flags. If the caller needs
        // USER access and the existing entry doesn't have it, a future
        // sprint will add flag upgrading logic.
        Ok(entry.addr())
    } else {
        // Allocate a new page table frame (zeroed = all entries non-present).
        let frame = pmm::alloc_frame_zeroed().ok_or(MapError::OutOfMemory)?;
        entry.set(frame, flags);
        Ok(frame)
    }
}
