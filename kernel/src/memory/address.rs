// =============================================================================
// MinimalOS NextGen — Physical and Virtual Address Types
// =============================================================================
//
// In OS development, confusing a physical address with a virtual address is
// one of the most common and catastrophic bugs. You write to what you think
// is a physical framebuffer address, but it's actually a virtual address —
// you corrupt random memory and the system crashes mysteriously.
//
// SOLUTION: Newtype pattern.
//   PhysAddr and VirtAddr are separate types. The compiler prevents you from
//   using one where the other is expected. Converting between them requires
//   explicit function calls that document the relationship.
//
// x86_64 ADDRESS SPACE:
//   - Physical: 52 bits max (4 PB), but N3710 has 34-bit physical = 16GB max
//   - Virtual: 48 bits used (the "canonical" range), 16 bits sign-extended
//   - Canonical virtual addresses:
//     - Lower half: 0x0000_0000_0000_0000 — 0x0000_7FFF_FFFF_FFFF (user space)
//     - Upper half: 0xFFFF_8000_0000_0000 — 0xFFFF_FFFF_FFFF_FFFF (kernel space)
//     - The gap in the middle is "non-canonical" — accessing it causes a GPF
//
// HHDM (Higher Half Direct Map):
//   Limine maps ALL physical memory at a fixed virtual offset.
//   If the HHDM offset is 0xFFFF_8000_0000_0000, then:
//     Physical address 0x0000_1000 → Virtual 0xFFFF_8000_0000_1000
//   This lets the kernel access any physical memory using virtual addresses,
//   without having to set up temporary mappings.
//
// =============================================================================

use core::fmt;

/// The virtual offset where Limine maps all physical memory.
/// This is set during boot from the Limine HHDM response.
/// Before boot info is parsed, this is 0 (and must not be used).
static mut HHDM_OFFSET: u64 = 0;

/// One-time initialization of the HHDM offset from Limine boot info.
///
/// # Safety
/// Must be called exactly once during early boot, before any
/// `PhysAddr::to_virt()` calls, and before SMP init (single-core only).
pub unsafe fn init_hhdm(offset: u64) {
    unsafe { HHDM_OFFSET = offset; }
}

/// Returns the configured HHDM offset.
///
/// # Panics
/// Debug-asserts that the offset has been initialized (non-zero).
#[inline]
pub fn hhdm_offset() -> u64 {
    // SAFETY: Read-only after init_hhdm() during single-core boot.
    let offset = unsafe { HHDM_OFFSET };
    debug_assert!(offset != 0, "HHDM offset not initialized — call init_hhdm() first");
    offset
}

// =============================================================================
// PhysAddr — A physical memory address
// =============================================================================

/// A physical memory address.
///
/// Physical addresses refer to locations in the system's physical RAM
/// (or memory-mapped I/O). They are what the CPU sends on the memory bus
/// after page table translation.
///
/// On the N3710, physical addresses are 34 bits wide (max 16GB addressable).
/// We store them as u64 for consistency with the x86_64 architecture.
///
/// # Examples
/// ```
/// let addr = PhysAddr::new(0x1000);  // Physical page at 4KB
/// assert!(addr.is_page_aligned());
/// let virt = addr.to_virt();         // Get the HHDM virtual mapping
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PhysAddr(u64);

impl PhysAddr {
    /// Creates a new physical address.
    ///
    /// On x86_64, physical addresses must fit in 52 bits (architectural max).
    /// On N3710, only 34 bits are actually used, but we allow the full range
    /// for forward compatibility.
    ///
    /// # Panics
    /// Debug-asserts that the address fits in 52 bits.
    #[inline]
    pub const fn new(addr: u64) -> Self {
        // Physical addresses must fit in 52 bits (x86_64 architectural limit).
        // Bits 52-63 must be zero.
        debug_assert!(
            addr & 0xFFF0_0000_0000_0000 == 0,
            "Physical address exceeds 52-bit limit"
        );
        Self(addr)
    }

    /// Creates a physical address without validation.
    ///
    /// # Safety
    /// The caller must ensure the address is valid (fits in 52 bits).
    /// Use this only in performance-critical paths where the address
    /// is known to be valid (e.g., read from a page table entry).
    #[inline]
    pub const unsafe fn new_unchecked(addr: u64) -> Self {
        Self(addr)
    }

    /// Returns the raw u64 value of this physical address.
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Converts this physical address to its HHDM virtual mapping.
    ///
    /// This works because Limine maps all physical memory at a fixed
    /// virtual offset. Adding the offset gives the virtual address where
    /// the kernel can access this physical memory.
    ///
    /// # Panics
    /// Debug-asserts that the HHDM offset has been initialized.
    #[inline]
    pub fn to_virt(self) -> VirtAddr {
        VirtAddr::new(self.0 + hhdm_offset())
    }

    /// Returns true if this address is aligned to a 4KB page boundary.
    #[inline]
    pub const fn is_page_aligned(self) -> bool {
        self.0 & 0xFFF == 0
    }

    /// Aligns this address down to the nearest 4KB page boundary.
    ///
    /// # Examples
    /// ```
    /// assert_eq!(PhysAddr::new(0x1234).page_align_down(), PhysAddr::new(0x1000));
    /// assert_eq!(PhysAddr::new(0x1000).page_align_down(), PhysAddr::new(0x1000));
    /// ```
    #[inline]
    pub const fn page_align_down(self) -> Self {
        Self(self.0 & !0xFFF)
    }

    /// Aligns this address up to the nearest 4KB page boundary.
    ///
    /// # Panics
    /// Debug-asserts that the result doesn't overflow.
    #[inline]
    pub const fn page_align_up(self) -> Self {
        let aligned = (self.0 + 0xFFF) & !0xFFF;
        debug_assert!(aligned >= self.0, "PhysAddr::page_align_up overflow");
        Self(aligned)
    }

    /// Creates a zero physical address (often used as a null/invalid marker).
    #[inline]
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Returns true if this is the zero address.
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Display a physical address in the standard `0xDEAD_BEEF` format.
/// The `P:` prefix distinguishes it from virtual addresses in log output.
impl fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P:{:#010X}", self.0)
    }
}

impl fmt::Display for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P:{:#010X}", self.0)
    }
}

/// Allow adding a byte offset to a physical address.
/// Useful for calculating addresses within a memory region.
impl core::ops::Add<u64> for PhysAddr {
    type Output = Self;
    #[inline]
    fn add(self, offset: u64) -> Self {
        Self::new(self.0 + offset)
    }
}

/// Allow subtracting a byte offset from a physical address.
impl core::ops::Sub<u64> for PhysAddr {
    type Output = Self;
    #[inline]
    fn sub(self, offset: u64) -> Self {
        Self::new(self.0 - offset)
    }
}

/// Allow calculating the distance between two physical addresses.
impl core::ops::Sub<PhysAddr> for PhysAddr {
    type Output = u64;
    #[inline]
    fn sub(self, other: PhysAddr) -> u64 {
        self.0 - other.0
    }
}

// =============================================================================
// VirtAddr — A virtual memory address
// =============================================================================

/// A virtual memory address.
///
/// Virtual addresses are what the CPU uses for all memory accesses.
/// They go through the page table translation (PML4 → PDPT → PD → PT)
/// to produce a physical address.
///
/// On x86_64, virtual addresses are 48 bits wide with sign extension:
///   - Bits 0-47: the actual address (256TB address space)
///   - Bits 48-63: must be copies of bit 47 (sign extension = "canonical")
///   - Lower half (bit 47 = 0): 0x0000_0000_0000_0000 — 0x0000_7FFF_FFFF_FFFF
///   - Upper half (bit 47 = 1): 0xFFFF_8000_0000_0000 — 0xFFFF_FFFF_FFFF_FFFF
///   - Accessing a non-canonical address causes a General Protection Fault
///
/// # Examples
/// ```
/// let addr = VirtAddr::new(0xFFFF_8000_0000_1000);  // Kernel space
/// assert!(addr.is_kernel());
/// assert!(addr.is_page_aligned());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct VirtAddr(u64);

impl VirtAddr {
    /// Creates a new virtual address.
    ///
    /// The address must be canonical (bits 48-63 = copies of bit 47).
    /// Non-canonical addresses would cause a GPF if used.
    ///
    /// # Panics
    /// Debug-asserts that the address is canonical.
    #[inline]
    pub const fn new(addr: u64) -> Self {
        // Check canonicality: bits 48-63 must equal bit 47.
        // If bit 47 is 0 → bits 48-63 must all be 0.
        // If bit 47 is 1 → bits 48-63 must all be 1.
        //
        // We do this by sign-extending bit 47 across bits 48-63
        // and checking if it matches the original.
        let canonical = ((addr << 16) as i64 >> 16) as u64;
        debug_assert!(
            addr == canonical,
            "Non-canonical virtual address"
        );
        Self(addr)
    }

    /// Creates a virtual address without canonicality validation.
    ///
    /// # Safety
    /// The caller must ensure the address is canonical.
    #[inline]
    pub const unsafe fn new_unchecked(addr: u64) -> Self {
        Self(addr)
    }

    /// Returns the raw u64 value of this virtual address.
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Converts this virtual address to a raw pointer.
    ///
    /// This is the bridge between our type-safe address world and Rust's
    /// pointer world. Used when we actually need to read/write memory.
    #[inline]
    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    /// Converts this virtual address to a mutable raw pointer.
    #[inline]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    /// Returns true if this is a kernel-space address (upper half).
    ///
    /// Kernel addresses have bit 47 set, which means bits 48-63 are all 1.
    /// These addresses are in the range 0xFFFF_8000... to 0xFFFF_FFFF...
    #[inline]
    pub const fn is_kernel(self) -> bool {
        self.0 >= 0xFFFF_8000_0000_0000
    }

    /// Returns true if this is a user-space address (lower half).
    ///
    /// User addresses have bit 47 clear, which means bits 48-63 are all 0.
    /// These addresses are in the range 0x0000_0000... to 0x0000_7FFF...
    #[inline]
    pub const fn is_user(self) -> bool {
        self.0 < 0x0000_8000_0000_0000
    }

    /// Returns true if this address is aligned to a 4KB page boundary.
    #[inline]
    pub const fn is_page_aligned(self) -> bool {
        self.0 & 0xFFF == 0
    }

    /// Aligns this address down to the nearest 4KB page boundary.
    #[inline]
    pub const fn page_align_down(self) -> Self {
        Self(self.0 & !0xFFF)
    }

    /// Aligns this address up to the nearest 4KB page boundary.
    #[inline]
    pub const fn page_align_up(self) -> Self {
        Self((self.0 + 0xFFF) & !0xFFF)
    }

    /// Extracts the page table indices from this virtual address.
    ///
    /// A 48-bit virtual address is split into indices for each level of
    /// the 4-level page table hierarchy:
    ///
    /// ```text
    /// 63       48 47    39 38    30 29    21 20    12 11       0
    /// ┌──────────┬────────┬────────┬────────┬────────┬─────────┐
    /// │ sign ext │ PML4   │  PDPT  │   PD   │   PT   │ Offset  │
    /// │ (16 bit) │ (9bit) │ (9bit) │ (9bit) │ (9bit) │ (12bit) │
    /// └──────────┴────────┴────────┴────────┴────────┴─────────┘
    ///              idx[3]   idx[2]   idx[1]   idx[0]
    /// ```
    ///
    /// Each index is 9 bits → 512 entries per table → 4KB per table (512 × 8B).
    ///
    /// # Returns
    /// `[PT index, PD index, PDPT index, PML4 index]` — note: index 0 is the
    /// lowest level (PT), which is the most intuitive for array iteration.
    #[inline]
    pub const fn page_table_indices(self) -> [u16; 4] {
        [
            ((self.0 >> 12) & 0x1FF) as u16, // PT index    (level 1)
            ((self.0 >> 21) & 0x1FF) as u16, // PD index    (level 2)
            ((self.0 >> 30) & 0x1FF) as u16, // PDPT index  (level 3)
            ((self.0 >> 39) & 0x1FF) as u16, // PML4 index  (level 4)
        ]
    }

    /// Extracts the 12-bit page offset (the part within a 4KB page).
    #[inline]
    pub const fn page_offset(self) -> u16 {
        (self.0 & 0xFFF) as u16
    }

    /// Creates a zero virtual address (null pointer equivalent).
    #[inline]
    pub const fn zero() -> Self {
        Self(0)
    }
}

/// Display a virtual address with `V:` prefix to distinguish from physical.
impl fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V:{:#018X}", self.0)
    }
}

impl fmt::Display for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V:{:#018X}", self.0)
    }
}

/// Allow adding a byte offset to a virtual address.
impl core::ops::Add<u64> for VirtAddr {
    type Output = Self;
    #[inline]
    fn add(self, offset: u64) -> Self {
        Self::new(self.0 + offset)
    }
}

/// Allow subtracting a byte offset from a virtual address.
impl core::ops::Sub<u64> for VirtAddr {
    type Output = Self;
    #[inline]
    fn sub(self, offset: u64) -> Self {
        Self::new(self.0 - offset)
    }
}

/// Allow calculating the distance between two virtual addresses.
impl core::ops::Sub<VirtAddr> for VirtAddr {
    type Output = u64;
    #[inline]
    fn sub(self, other: VirtAddr) -> u64 {
        self.0 - other.0
    }
}

// =============================================================================
// Page size constants
// =============================================================================

/// Size of a standard page (4 KiB).
pub const PAGE_SIZE: u64 = 4096;

/// Size of a large/huge page (2 MiB).
pub const HUGE_PAGE_SIZE: u64 = 2 * 1024 * 1024;

/// Bit shift for standard pages (4K = 2^12).
pub const PAGE_SHIFT: u64 = 12;
