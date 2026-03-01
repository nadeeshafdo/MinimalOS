// =============================================================================
// MinimalOS NextGen — Global Descriptor Table (GDT) + Task State Segment (TSS)
// =============================================================================
//
// On x86_64, segmentation is mostly vestigial — flat model, all bases 0.
// But the GDT is still required for:
//   1. Privilege level transitions (Ring 0 ↔ Ring 3 via CS/SS selectors)
//   2. SYSCALL/SYSRET configuration (STAR MSR encodes GDT selectors)
//   3. TSS — holds RSP0 (kernel stack for Ring 3→0) and IST stacks
//
// SEGMENT LAYOUT (order matters for SYSRET compatibility):
//   Index 0  (0x00): Null descriptor       — required by CPU
//   Index 1  (0x08): Kernel Code 64-bit     — Ring 0, Execute/Read
//   Index 2  (0x10): Kernel Data 64-bit     — Ring 0, Read/Write
//   Index 3  (0x18): User Data 64-bit       — Ring 3, Read/Write
//   Index 4  (0x20): User Code 64-bit       — Ring 3, Execute/Read
//   Index 5-6(0x28): TSS descriptor         — 16 bytes (spans two slots)
//
// SYSRET CONSTRAINT:
//   SYSCALL loads CS from STAR[47:32].
//   SYSRET loads CS from STAR[63:48] + 16, SS from STAR[63:48] + 8.
//   So User Data must be at STAR[63:48]+8 and User Code at STAR[63:48]+16.
//   With STAR[63:48] = 0x10 (kernel data):
//     User Data = 0x10 + 0x08 = 0x18  ← index 3
//     User Code = 0x10 + 0x10 = 0x20  ← index 4
//   Both ORed with RPL=3 → selectors 0x1B and 0x23.
//
// IST1 GUARD PAGE:
//   The double-fault handler uses IST1 — a dedicated stack so it works
//   even if the kernel stack overflows. We allocate 4 contiguous pages
//   (16 KiB) but leave the bottom page UNMAPPED as a guard. If the
//   handler's call chain (especially core::fmt from kprintln!) overflows,
//   it hits the guard page and triple-faults cleanly instead of silently
//   corrupting adjacent memory.
//
// =============================================================================

use core::arch::asm;
use core::mem;
use core::ptr;

use crate::kprintln;
use crate::memory::address::PAGE_SIZE;
use crate::memory::pmm;
use crate::memory::vmm;

// =============================================================================
// Segment selectors — used by the rest of the kernel
// =============================================================================

/// Kernel code segment selector (Ring 0).
pub const KERNEL_CS: u16 = 0x08;

/// Kernel data segment selector (Ring 0).
pub const KERNEL_DS: u16 = 0x10;

/// User data segment selector (Ring 3). RPL = 3.
pub const USER_DS: u16 = 0x18 | 3; // 0x1B

/// User code segment selector (Ring 3). RPL = 3.
pub const USER_CS: u16 = 0x20 | 3; // 0x23

/// TSS segment selector.
const TSS_SELECTOR: u16 = 0x28;

// =============================================================================
// TSS (Task State Segment)
// =============================================================================

/// The x86_64 TSS structure.
///
/// In long mode, the TSS is used for:
///   - `rsp0`: stack pointer loaded on Ring 3 → Ring 0 transition
///   - `ist[0..6]`: Interrupt Stack Table entries for critical exceptions
///   - `iomap_base`: I/O permission bitmap offset (not used, set to sizeof TSS)
#[repr(C, packed)]
struct Tss {
    reserved1: u32,
    /// Kernel stack pointers for each privilege level transition.
    /// rsp[0] = Ring 3 → Ring 0 stack (the one that matters).
    rsp: [u64; 3],
    reserved2: u64,
    /// Interrupt Stack Table entries. IST1 = ist[0], IST7 = ist[6].
    /// Each points to the TOP of a dedicated stack.
    ist: [u64; 7],
    reserved3: u64,
    reserved4: u16,
    /// Offset to I/O permission bitmap. Set to `sizeof(TSS)` to indicate
    /// no I/O bitmap (all ports denied by default in Ring 3).
    iomap_base: u16,
}

// =============================================================================
// GDT entry types
// =============================================================================

/// A single 8-byte GDT descriptor (used for code/data segments).
#[derive(Clone, Copy)]
#[repr(transparent)]
struct GdtEntry(u64);

impl GdtEntry {
    /// Null descriptor — required as index 0.
    const NULL: Self = Self(0);

    /// Creates a 64-bit code segment descriptor.
    ///
    /// In long mode, the base and limit are ignored. Only the flags matter:
    ///   - L bit (bit 53): 1 = 64-bit code segment
    ///   - D bit (bit 54): 0 = must be 0 when L=1
    ///   - P bit (bit 47): 1 = present
    ///   - DPL (bits 45-46): privilege level
    ///   - S bit (bit 44): 1 = code/data (not system)
    ///   - Type (bits 40-43): 0b1010 = Execute/Read
    const fn code64(dpl: u8) -> Self {
        let mut val: u64 = 0;
        val |= 1 << 53;                        // L = 1 (long mode)
        val |= 1 << 47;                        // P = 1 (present)
        val |= ((dpl as u64) & 3) << 45;       // DPL
        val |= 1 << 44;                        // S = 1 (code/data)
        val |= 0b1010 << 40;                   // Type = Execute/Read
        Self(val)
    }

    /// Creates a 64-bit data segment descriptor.
    ///
    /// In long mode, only P, DPL, S, and Type matter.
    ///   - Type (bits 40-43): 0b0010 = Read/Write
    const fn data64(dpl: u8) -> Self {
        let mut val: u64 = 0;
        val |= 1 << 47;                        // P = 1
        val |= ((dpl as u64) & 3) << 45;       // DPL
        val |= 1 << 44;                        // S = 1
        val |= 0b0010 << 40;                   // Type = Read/Write
        Self(val)
    }
}

/// A 16-byte TSS descriptor (occupies two GDT slots).
///
/// In 64-bit mode, the TSS descriptor is extended to 16 bytes to hold the
/// full 64-bit base address. It spans two consecutive GDT entries.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct TssDescriptor {
    low: u64,
    high: u64,
}

impl TssDescriptor {
    /// Creates a TSS descriptor from the TSS base address and limit.
    fn new(base: u64, limit: u16) -> Self {
        let base_lo = base & 0xFFFF;
        let base_mid = (base >> 16) & 0xFF;
        let base_hi_lo = (base >> 24) & 0xFF;
        let base_hi_hi = base >> 32;
        let limit_lo = limit as u64 & 0xFFFF;

        // Low 8 bytes:
        //   [15:0]   limit_lo
        //   [31:16]  base_lo
        //   [39:32]  base_mid
        //   [43:40]  type = 0b1001 (available 64-bit TSS)
        //   [44]     S = 0 (system segment)
        //   [46:45]  DPL = 0
        //   [47]     P = 1 (present)
        //   [51:48]  limit_hi (0 for small TSS)
        //   [55:52]  flags (0)
        //   [63:56]  base_hi_lo
        let low = limit_lo
            | (base_lo << 16)
            | (base_mid << 32)
            | (0b1001u64 << 40)       // Type: available 64-bit TSS
            | (1u64 << 47)            // Present
            | (base_hi_lo << 56);

        // High 8 bytes:
        //   [31:0]   base[63:32]
        //   [63:32]  reserved (0)
        let high = base_hi_hi;

        Self { low, high }
    }
}

// =============================================================================
// GDT table
// =============================================================================

/// The GDT: 5 regular entries (null + 4 segments) + 1 TSS (2 slots) = 7 slots.
/// Stored as raw u64s because the TSS descriptor spans two slots.
const GDT_ENTRY_COUNT: usize = 7;

/// The GDTR value passed to `lgdt`.
#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}

// =============================================================================
// Static storage
// =============================================================================
//
// SAFETY: These statics are only modified during single-core early boot
// (before SMP init). After init(), they are read-only.

static mut GDT: [u64; GDT_ENTRY_COUNT] = [0; GDT_ENTRY_COUNT];
static mut TSS: Tss = Tss {
    reserved1: 0,
    rsp: [0; 3],
    reserved2: 0,
    ist: [0; 7],
    reserved3: 0,
    reserved4: 0,
    iomap_base: 0,
};

// =============================================================================
// Initialization
// =============================================================================

/// Initializes the GDT and TSS.
///
/// Must be called during early boot, before IDT setup (the IDT references
/// the GDT's code segment selector and IST indices from the TSS).
///
/// # What this does
/// 1. Allocates 4 contiguous pages for IST1 (double-fault stack)
/// 2. Unmaps the bottom page as a guard page
/// 3. Populates the TSS with IST1 and iomap_base
/// 4. Builds the GDT entries (null, kernel CS/DS, user DS/CS, TSS)
/// 5. Loads the GDT via `lgdt`
/// 6. Reloads all segment registers (CS via far return, data via mov)
/// 7. Loads the TSS via `ltr`
pub fn init() {
    // =========================================================================
    // Step 1: Allocate the IST1 double-fault stack (4 pages = 16 KiB)
    // =========================================================================
    //
    // Layout (addresses grow upward):
    //   page 0: GUARD PAGE — unmapped, catches stack overflow
    //   page 1: usable stack (bottom)
    //   page 2: usable stack
    //   page 3: usable stack (top) — IST1 points to top of this page
    //
    // Stack grows downward, so IST1 = base + 4 * PAGE_SIZE = top of page 3.
    // If the handler overflows past page 1, it hits the unmapped guard page
    // and the CPU triple-faults cleanly.

    let ist_phys = pmm::alloc_contiguous(4)
        .expect("[gdt] FATAL: cannot allocate IST1 double-fault stack (4 pages)");

    let ist_virt = ist_phys.to_virt();

    // Unmap the bottom page (guard page).
    // SAFETY: We just allocated these pages. Unmapping page 0 of the 4-page
    // block leaves it as a hardware guard. The HHDM mapping set up by Limine
    // covers all physical memory, so we need to remove this specific page
    // from the active page tables.
    unsafe {
        let result = vmm::unmap_page(vmm::active_pml4(), ist_virt);
        match result {
            Ok(_) => {},
            Err(_) => {
                // If the page wasn't mapped as a 4K page (e.g., it's part of
                // a 2M huge page in Limine's HHDM), we can't unmap it individually.
                // Log a warning but continue — the guard is best-effort.
                kprintln!("[gdt] WARNING: Could not unmap IST1 guard page (huge page?)");
            }
        }
        vmm::flush(ist_virt);
    }

    // IST1 points to the TOP of the stack (stacks grow down on x86).
    let ist1_top = ist_virt.as_u64() + (4 * PAGE_SIZE as u64);

    kprintln!("[gdt] IST1 stack: {:#018X} — {:#018X} (guard page at {:#018X})",
        ist_virt.as_u64() + PAGE_SIZE as u64, ist1_top, ist_virt.as_u64());

    // =========================================================================
    // Step 2: Populate the TSS
    // =========================================================================
    unsafe {
        // RSP0 will be set per-thread during context switch (Sprint 4).
        // For now, leave it zeroed (we don't do Ring 3 → Ring 0 yet).
        TSS.rsp[0] = 0;

        // IST1 = double-fault stack top.
        TSS.ist[0] = ist1_top;

        // I/O permission bitmap offset. Setting this to the size of the TSS
        // means "no I/O bitmap present" — all I/O ports require IOPL 0.
        TSS.iomap_base = mem::size_of::<Tss>() as u16;
    }

    // =========================================================================
    // Step 3: Build the GDT
    // =========================================================================
    let tss_base = ptr::addr_of!(TSS) as u64;
    let tss_limit = (mem::size_of::<Tss>() - 1) as u16;
    let tss_desc = TssDescriptor::new(tss_base, tss_limit);

    unsafe {
        GDT[0] = GdtEntry::NULL.0;                     // 0x00: null
        GDT[1] = GdtEntry::code64(0).0;                // 0x08: kernel code
        GDT[2] = GdtEntry::data64(0).0;                // 0x10: kernel data
        GDT[3] = GdtEntry::data64(3).0;                // 0x18: user data
        GDT[4] = GdtEntry::code64(3).0;                // 0x20: user code
        GDT[5] = tss_desc.low;                          // 0x28: TSS low
        GDT[6] = tss_desc.high;                         // 0x30: TSS high
    }

    // =========================================================================
    // Step 4: Load the GDT via LGDT
    // =========================================================================
    let gdt_ptr = GdtPointer {
        limit: (GDT_ENTRY_COUNT * mem::size_of::<u64>() - 1) as u16,
        base: ptr::addr_of!(GDT) as u64,
    };

    unsafe {
        asm!(
            "lgdt [{}]",
            in(reg) &gdt_ptr,
            options(nostack, preserves_flags)
        );
    }

    // =========================================================================
    // Step 5: Reload segment registers
    // =========================================================================
    //
    // After lgdt, the old selectors from Limine's GDT are still loaded.
    // We must reload them to point to OUR GDT entries.
    //
    // CS is special: you can't `mov cs, ax`. We use a far return:
    //   push new_cs → push return_addr → retfq
    //
    // Data segments (DS, ES, SS, FS, GS) can be loaded with MOV.
    unsafe {
        // Reload CS via far return
        asm!(
            "push {sel}",        // Push new CS selector
            "lea {tmp}, [rip + 2f]", // Get address of label 2
            "push {tmp}",        // Push return address
            "retfq",             // Far return: pops RIP and CS
            "2:",                // We land here with new CS
            sel = in(reg) KERNEL_CS as u64,
            tmp = lateout(reg) _,
            options(preserves_flags),
        );

        // Reload data segment registers
        asm!(
            "mov ds, {sel:x}",
            "mov es, {sel:x}",
            "mov ss, {sel:x}",
            sel = in(reg) KERNEL_DS as u32,
            options(nostack, preserves_flags),
        );

        // Clear FS and GS — they'll be set up for per-CPU data in Sprint 4.
        asm!(
            "mov fs, {zero:x}",
            "mov gs, {zero:x}",
            zero = in(reg) 0u32,
            options(nostack, preserves_flags),
        );
    }

    // =========================================================================
    // Step 6: Load the TSS via LTR
    // =========================================================================
    unsafe {
        asm!(
            "ltr {sel:x}",
            sel = in(reg) TSS_SELECTOR as u16,
            options(nostack, preserves_flags),
        );
    }

    kprintln!("[gdt] GDT loaded: null + KERNEL_CS({:#04X}) + KERNEL_DS({:#04X}) + USER_DS({:#04X}) + USER_CS({:#04X}) + TSS({:#04X})",
        KERNEL_CS, KERNEL_DS, USER_DS, USER_CS, TSS_SELECTOR);
}
