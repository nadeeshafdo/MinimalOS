//! Global Descriptor Table (GDT) for x86_64.
//!
//! In long mode, the GDT is simplified compared to 32-bit mode.
//! Most segmentation is disabled, but a GDT is still required for:
//! - Defining code/data segments for different privilege levels
//! - Pointing to the TSS (Task State Segment)

use core::arch::asm;
use core::mem::size_of;

use super::tss::Tss;

/// Maximum number of GDT entries.
/// We need: Null, Kernel Code, Kernel Data, User Data, User Code, TSS (takes 2 entries = 16 bytes).
const GDT_ENTRIES: usize = 7;

/// A segment descriptor in the GDT.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct SegmentDescriptor(u64);

impl SegmentDescriptor {
    /// A null descriptor (required as the first GDT entry).
    pub const NULL: Self = Self(0);

    /// Create a 64-bit kernel code segment descriptor.
    ///
    /// Flags: Present, DPL=0, Code, Long mode, Readable
    pub const fn kernel_code() -> Self {
        // Access byte (bits 40-47): P=1, DPL=00, S=1, E=1, DC=0, RW=1, A=0 = 0x9A
        // Flags (bits 52-55): G=0, DB=0, L=1 (long mode), Reserved=0 = 0x2
        Self(0x00_2F_9A_00_0000_FFFF)
    }

    /// Create a 64-bit kernel data segment descriptor.
    ///
    /// Flags: Present, DPL=0, Data, Writable
    pub const fn kernel_data() -> Self {
        // Access byte: P=1, DPL=00, S=1, E=0, DC=0, RW=1, A=0 = 0x92
        Self(0x00_0F_92_00_0000_FFFF)
    }

    /// Create a 64-bit user data segment descriptor (Ring 3).
    ///
    /// Flags: Present, DPL=3, Data, Writable
    pub const fn user_data() -> Self {
        // Access byte: P=1, DPL=11, S=1, E=0, DC=0, RW=1, A=0 = 0xF2
        Self(0x00_0F_F2_00_0000_FFFF)
    }

    /// Create a 64-bit user code segment descriptor (Ring 3).
    ///
    /// Flags: Present, DPL=3, Code, Long mode, Readable
    pub const fn user_code() -> Self {
        // Access byte: P=1, DPL=11, S=1, E=1, DC=0, RW=1, A=0 = 0xFA
        // Flags: L=1 (long mode) = 0x2
        Self(0x00_2F_FA_00_0000_FFFF)
    }
}

/// A TSS descriptor in the GDT (128 bits / 2 entries).
///
/// In 64-bit mode, the TSS descriptor is 16 bytes wide and occupies
/// two consecutive GDT slots.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TssDescriptor {
    low: u64,
    high: u64,
}

impl TssDescriptor {
    /// Create a TSS descriptor from a TSS reference.
    pub fn new(tss: &Tss) -> Self {
        let tss_addr = tss as *const _ as u64;
        let tss_len = (size_of::<Tss>() - 1) as u64;

        let mut low: u64 = 0;

        // Limit bits 0-15
        low |= tss_len & 0xFFFF;
        // Base bits 0-15 (bits 16-31)
        low |= (tss_addr & 0xFFFF) << 16;
        // Base bits 16-23 (bits 32-39)
        low |= ((tss_addr >> 16) & 0xFF) << 32;
        // Access byte (bits 40-47): Present=1, DPL=0, Type=0x9 (64-bit TSS available)
        low |= 0x89u64 << 40;
        // Limit bits 16-19 (bits 48-51)
        low |= ((tss_len >> 16) & 0xF) << 48;
        // Base bits 24-31 (bits 56-63)
        low |= ((tss_addr >> 24) & 0xFF) << 56;

        // High: Base bits 32-63
        let high = tss_addr >> 32;

        Self { low, high }
    }
}

/// The Global Descriptor Table.
///
/// Layout (ordered for `syscall`/`sysret` compatibility):
/// - Entry 0: Null descriptor (required)
/// - Entry 1: Kernel code segment (0x08, Ring 0)
/// - Entry 2: Kernel data segment (0x10, Ring 0)
/// - Entry 3: User data segment  (0x18, Ring 3) — sysret loads SS from STAR+8
/// - Entry 4: User code segment  (0x20, Ring 3) — sysret loads CS from STAR+16
/// - Entry 5-6: TSS descriptor (0x28, 16 bytes, spans two entries)
#[repr(C, align(16))]
pub struct Gdt {
    entries: [u64; GDT_ENTRIES],
}

/// Segment selectors for GDT entries.
/// Each selector is the byte offset into the GDT.
pub struct Selectors {
    pub kernel_code: u16,
    pub kernel_data: u16,
    pub user_data: u16,
    pub user_code: u16,
    pub tss: u16,
}

impl Gdt {
    /// Create a new GDT with null, kernel, user, and TSS entries.
    pub fn new(tss: &Tss) -> (Self, Selectors) {
        let tss_desc = TssDescriptor::new(tss);

        let gdt = Self {
            entries: [
                SegmentDescriptor::NULL.0,            // 0x00: Null
                SegmentDescriptor::kernel_code().0,    // 0x08: Kernel Code (Ring 0)
                SegmentDescriptor::kernel_data().0,    // 0x10: Kernel Data (Ring 0)
                SegmentDescriptor::user_data().0,      // 0x18: User Data (Ring 3)
                SegmentDescriptor::user_code().0,      // 0x20: User Code (Ring 3)
                tss_desc.low,                          // 0x28: TSS low
                tss_desc.high,                         // 0x30: TSS high
            ],
        };

        let selectors = Selectors {
            kernel_code: 0x08,
            kernel_data: 0x10,
            user_data: 0x18 | 3,  // RPL=3
            user_code: 0x20 | 3,  // RPL=3
            tss: 0x28,
        };

        (gdt, selectors)
    }

    /// Load this GDT and switch to its segments.
    ///
    /// # Safety
    ///
    /// The GDT must remain valid for the entire lifetime of the system.
    /// The selectors must point to valid descriptors within this GDT.
    pub unsafe fn load(&'static self, selectors: &Selectors) {
        let ptr = GdtPointer {
            limit: (size_of::<Self>() - 1) as u16,
            base: self as *const _ as u64,
        };

        unsafe {
            // Load the GDT
            asm!(
                "lgdt [{}]",
                in(reg) &ptr,
                options(readonly, nostack, preserves_flags)
            );

            // Reload CS by doing a far return
            // Push the new code segment selector and the return address
            asm!(
                "push {sel}",
                "lea {tmp}, [rip + 2f]",
                "push {tmp}",
                "retfq",
                "2:",
                sel = in(reg) selectors.kernel_code as u64,
                tmp = lateout(reg) _,
                options(preserves_flags)
            );

            // Reload data segment registers
            asm!(
                "mov ds, {sel:x}",
                "mov es, {sel:x}",
                "mov ss, {sel:x}",
                sel = in(reg) selectors.kernel_data as u16,
                options(nostack, preserves_flags)
            );

            // Load the Task Register with the TSS selector
            asm!(
                "ltr {sel:x}",
                sel = in(reg) selectors.tss,
                options(nostack, preserves_flags)
            );
        }
    }
}

/// Pointer structure for the `lgdt` instruction.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}
