// =============================================================================
// MinimalOS NextGen — ELF64 Loader
// =============================================================================
//
// Parses 64-bit ELF (Executable and Linkable Format) executables and loads
// their PT_LOAD segments into a target address space.
//
// ELF STRUCTURE:
//   Every ELF binary starts with an Elf64_Ehdr (ELF Header) at offset 0.
//   The ELF header points to an array of Elf64_Phdr (Program Headers).
//   Each program header describes a contiguous chunk of the binary that the
//   OS loader must place into memory.
//
// PT_LOAD SEGMENTS:
//   The loader only cares about segments with p_type == PT_LOAD.
//   For each PT_LOAD segment:
//     1. Allocate physical frames covering [p_vaddr, p_vaddr + p_memsz)
//     2. Map those frames at p_vaddr in the target address space
//     3. Copy p_filesz bytes from the ELF image
//     4. Zero the remaining (p_memsz - p_filesz) bytes — this is .bss
//     5. Apply page permissions from p_flags (PF_R, PF_W, PF_X)
//
// .BSS INITIALIZATION (The Critical Fix):
//   In a flat binary, uninitialized globals (.bss) are NOT in the file.
//   The ELF format encodes this as p_memsz > p_filesz: the loader zeroes
//   the gap. Without this, static variables contain garbage.
//
// ENTRY POINT:
//   The ELF header's e_entry field gives the virtual address where the
//   CPU should begin execution after loading is complete.
//
// LIMITATIONS:
//   - Static executables or PIE at fixed addresses (ET_EXEC or ET_DYN)
//   - No dynamic linking or relocations (userspace binaries at fixed addresses)
//   - No TLS, init/fini arrays, or other special sections
//   - Operates on a byte slice (ELF image already in memory)
//
// =============================================================================

use crate::kprintln;
use crate::memory::address::{PhysAddr, VirtAddr, PAGE_SIZE};
use crate::memory::pmm;
use crate::memory::vmm::{self, PageTableFlags};

// =============================================================================
// ELF64 Constants
// =============================================================================

/// ELF magic number: \x7FELF
const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// ELFCLASS64 — 64-bit object.
const ELFCLASS64: u8 = 2;

/// ELFDATA2LSB — Little-endian byte order.
const ELFDATA2LSB: u8 = 1;

/// ET_EXEC — Executable file (static, non-relocatable).
const ET_EXEC: u16 = 2;

/// ET_DYN — Shared object / Position-Independent Executable.
/// Rust's linker may produce ET_DYN even with a fixed-address linker script.
const ET_DYN: u16 = 3;

/// EM_X86_64 — AMD x86-64 architecture.
const EM_X86_64: u16 = 62;

/// PT_LOAD — Loadable segment.
const PT_LOAD: u32 = 1;

/// PF_X — Execute permission.
const PF_X: u32 = 1;

/// PF_W — Write permission.
const PF_W: u32 = 2;

/// PF_R — Read permission (always implied on x86_64).
#[allow(dead_code)]
const PF_R: u32 = 4;

// =============================================================================
// ELF64 Header (Elf64_Ehdr)
// =============================================================================

/// The main ELF header, located at offset 0 of every ELF binary.
///
/// Describes the binary's type, architecture, entry point, and the
/// location of the program header table.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Ehdr {
    /// ELF magic: [0x7F, 'E', 'L', 'F'].
    pub e_ident: [u8; 16],
    /// Object file type: ET_EXEC (2) for static executables.
    pub e_type: u16,
    /// Target architecture: EM_X86_64 (62).
    pub e_machine: u16,
    /// ELF version (always 1).
    pub e_version: u32,
    /// Virtual address of the entry point — where execution begins.
    pub e_entry: u64,
    /// Offset (in bytes from file start) of the program header table.
    pub e_phoff: u64,
    /// Offset of the section header table (unused by the loader).
    pub e_shoff: u64,
    /// Processor-specific flags (0 for x86_64).
    pub e_flags: u32,
    /// Size of this header (should be 64 for ELF64).
    pub e_ehsize: u16,
    /// Size of each program header entry.
    pub e_phentsize: u16,
    /// Number of program header entries.
    pub e_phnum: u16,
    /// Size of each section header entry (unused).
    pub e_shentsize: u16,
    /// Number of section header entries (unused).
    pub e_shnum: u16,
    /// Section header string table index (unused).
    pub e_shstrndx: u16,
}

// Compile-time assertion: Elf64_Ehdr must be exactly 64 bytes.
const _: () = assert!(core::mem::size_of::<Elf64Ehdr>() == 64);

// =============================================================================
// ELF64 Program Header (Elf64_Phdr)
// =============================================================================

/// A program header entry — describes one segment to be loaded into memory.
///
/// The loader iterates through the program header table and processes
/// each PT_LOAD segment. Other segment types (PT_NOTE, PT_GNU_STACK, etc.)
/// are silently ignored.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Phdr {
    /// Segment type: PT_LOAD (1) for loadable segments.
    pub p_type: u32,
    /// Segment flags: PF_R (4), PF_W (2), PF_X (1).
    pub p_flags: u32,
    /// Offset in the file where this segment's data starts.
    pub p_offset: u64,
    /// Virtual address where this segment should be loaded.
    pub p_vaddr: u64,
    /// Physical address (unused in user-mode loading, mirrors p_vaddr).
    pub p_paddr: u64,
    /// Number of bytes in the file for this segment.
    /// Data range: [p_offset, p_offset + p_filesz).
    pub p_filesz: u64,
    /// Number of bytes in memory for this segment.
    /// If p_memsz > p_filesz, the extra bytes are zeroed (.bss).
    pub p_memsz: u64,
    /// Alignment requirement (must be a power of 2).
    pub p_align: u64,
}

// Compile-time assertion: Elf64_Phdr must be exactly 56 bytes.
const _: () = assert!(core::mem::size_of::<Elf64Phdr>() == 56);

// =============================================================================
// ELF Validation Errors
// =============================================================================

/// Errors that can occur during ELF parsing and loading.
#[derive(Debug)]
pub enum ElfError {
    /// File is smaller than the minimum ELF header size.
    TooSmall,
    /// ELF magic (\x7FELF) mismatch.
    BadMagic,
    /// Not a 64-bit ELF (ELFCLASS64).
    Not64Bit,
    /// Not little-endian (ELFDATA2LSB).
    NotLittleEndian,
    /// Not a static executable (ET_EXEC).
    NotExecutable,
    /// Not targeting x86_64 (EM_X86_64).
    WrongArch,
    /// Program header table extends beyond the file.
    PhdrOutOfBounds,
    /// A PT_LOAD segment's file data extends beyond the file.
    SegmentOutOfBounds,
    /// A PT_LOAD segment has an invalid virtual address (kernel range).
    BadVaddr,
    /// Physical frame allocation failed during loading.
    OutOfMemory,
    /// Page mapping failed.
    MapError,
}

// =============================================================================
// ELF Parsing — Header Validation
// =============================================================================

/// Validates an ELF64 header and returns a reference to it.
///
/// Checks: magic, class (64-bit), endianness (LE), type (executable),
/// machine (x86_64), and that the program header table fits within the file.
pub fn validate_header(elf_data: &[u8]) -> Result<&Elf64Ehdr, ElfError> {
    if elf_data.len() < core::mem::size_of::<Elf64Ehdr>() {
        return Err(ElfError::TooSmall);
    }

    let ehdr = unsafe { &*(elf_data.as_ptr() as *const Elf64Ehdr) };

    // Validate ELF magic
    if ehdr.e_ident[0..4] != ELF_MAGIC {
        return Err(ElfError::BadMagic);
    }

    // Must be 64-bit
    if ehdr.e_ident[4] != ELFCLASS64 {
        return Err(ElfError::Not64Bit);
    }

    // Must be little-endian
    if ehdr.e_ident[5] != ELFDATA2LSB {
        return Err(ElfError::NotLittleEndian);
    }

    // Must be an executable (ET_EXEC) or PIE (ET_DYN).
    // Rust's linker on x86_64-unknown-none produces ET_DYN by default
    // even when using a custom linker script with fixed addresses.
    let e_type = ehdr.e_type;
    if e_type != ET_EXEC && e_type != ET_DYN {
        return Err(ElfError::NotExecutable);
    }

    // Must target x86_64
    let e_machine = ehdr.e_machine;
    if e_machine != EM_X86_64 {
        return Err(ElfError::WrongArch);
    }

    // Validate program header table bounds
    // Copy packed fields to locals to avoid unaligned reference UB
    let e_phoff = ehdr.e_phoff as usize;
    let e_phnum = ehdr.e_phnum as usize;
    let e_phentsize = ehdr.e_phentsize as usize;
    let phdr_end = e_phoff + e_phnum * e_phentsize;
    if phdr_end > elf_data.len() {
        return Err(ElfError::PhdrOutOfBounds);
    }

    Ok(ehdr)
}

/// Returns an iterator over the program headers in the ELF file.
///
/// # Safety
/// The caller must have validated the header first via `validate_header`.
pub fn program_headers<'a>(elf_data: &'a [u8], ehdr: &Elf64Ehdr) -> &'a [Elf64Phdr] {
    // Copy packed fields to locals to avoid unaligned reference UB
    let offset = ehdr.e_phoff as usize;
    let count = ehdr.e_phnum as usize;
    unsafe {
        core::slice::from_raw_parts(
            elf_data.as_ptr().add(offset) as *const Elf64Phdr,
            count,
        )
    }
}

// =============================================================================
// ELF Loading — Map PT_LOAD Segments
// =============================================================================

/// Result of successfully loading an ELF binary.
pub struct ElfLoadResult {
    /// Entry point virtual address (e_entry from the ELF header).
    pub entry_point: u64,
    /// Total number of pages mapped.
    pub pages_mapped: usize,
    /// Total bytes of file data copied.
    pub bytes_copied: usize,
    /// Total bytes of BSS zeroed (p_memsz - p_filesz).
    pub bss_zeroed: usize,
}

/// Loads an ELF64 executable into the given address space.
///
/// For each PT_LOAD segment in the ELF:
///   1. Validates the segment's virtual address is in the user-space range.
///   2. Allocates zeroed physical frames for ceil(p_memsz / PAGE_SIZE) pages.
///   3. Maps each page at the correct virtual address with USER flag.
///   4. Copies p_filesz bytes from the ELF image into the mapped pages.
///   5. Remaining bytes (p_memsz - p_filesz) are already zero from alloc_frame_zeroed.
///
/// Page permissions (W^X) are derived from p_flags:
///   - PF_X && !PF_W → PRESENT | USER                     (execute-only)
///   - PF_W && !PF_X → PRESENT | USER | WRITABLE | NX     (data, stack)
///   - PF_R only     → PRESENT | USER | NX                 (read-only data)
///
/// # Parameters
/// - `elf_data`: The complete ELF file as a byte slice (from TarFS or initrd).
/// - `pml4_phys`: Physical address of the target PML4 page table.
///
/// # Returns
/// - `Ok(ElfLoadResult)` with entry point and load statistics.
/// - `Err(ElfError)` if validation or loading fails.
///
/// # Safety
/// - `pml4_phys` must be a valid PML4 table accessible via HHDM.
/// - The caller must flush the TLB after all mappings are complete.
pub fn load(elf_data: &[u8], pml4_phys: PhysAddr) -> Result<ElfLoadResult, ElfError> {
    let ehdr = validate_header(elf_data)?;
    let phdrs = program_headers(elf_data, ehdr);

    let mut total_pages = 0usize;
    let mut total_copied = 0usize;
    let mut total_bss = 0usize;

    for phdr in phdrs {
        // Skip non-loadable segments.
        if phdr.p_type != PT_LOAD {
            continue;
        }

        let vaddr = phdr.p_vaddr;
        let filesz = phdr.p_filesz as usize;
        let memsz = phdr.p_memsz as usize;
        let offset = phdr.p_offset as usize;
        let flags = phdr.p_flags;

        // Validate: segment file data must be within the ELF image.
        if offset + filesz > elf_data.len() {
            return Err(ElfError::SegmentOutOfBounds);
        }

        // Validate: virtual address must be in user-space (lower half).
        if vaddr >= 0x0000_8000_0000_0000 {
            return Err(ElfError::BadVaddr);
        }

        // Calculate page-aligned range.
        let page_start = vaddr & !0xFFF;
        let page_end = (vaddr + memsz as u64 + 0xFFF) & !0xFFF;
        let num_pages = ((page_end - page_start) / PAGE_SIZE as u64) as usize;

        // Determine page flags from ELF segment permissions.
        let page_flags = elf_flags_to_page_flags(flags);

        kprintln!("[elf] PT_LOAD: vaddr={:#010X} filesz={:#X} memsz={:#X} pages={} flags={}{}{}",
            vaddr, filesz, memsz, num_pages,
            if flags & PF_R != 0 { "R" } else { "-" },
            if flags & PF_W != 0 { "W" } else { "-" },
            if flags & PF_X != 0 { "X" } else { "-" },
        );

        // Allocate and map each page, then copy data.
        for page_idx in 0..num_pages {
            let page_virt = VirtAddr::new(page_start + page_idx as u64 * PAGE_SIZE as u64);

            // Try to allocate and map a new page. If the page is already
            // mapped (two PT_LOAD segments sharing the same page), reuse it.
            let page_phys = match pmm::alloc_frame_zeroed() {
                Some(frame) => {
                    match unsafe { vmm::map_page(pml4_phys, page_virt, frame, page_flags) } {
                        Ok(()) => {
                            crate::arch::cpu::invlpg(page_virt.as_u64());
                            total_pages += 1;
                            frame
                        }
                        Err(vmm::MapError::AlreadyMapped) => {
                            // Page already mapped by a previous segment.
                            // Free the unused frame and look up the existing mapping.
                            pmm::free_frame(frame);
                            vmm::translate(pml4_phys, page_virt)
                                .ok_or(ElfError::MapError)?
                        }
                        Err(_) => return Err(ElfError::MapError),
                    }
                }
                None => return Err(ElfError::OutOfMemory),
            };

            // Copy file data into this page.
            // The segment may not start on a page boundary (though it usually does).
            // We need to figure out which bytes of the segment fall within this page.
            let page_base_va = page_virt.as_u64();
            let seg_start_va = vaddr;
            let seg_file_end_va = vaddr + filesz as u64;

            // Range of this page in virtual address space:
            let page_va_lo = page_base_va;
            let page_va_hi = page_base_va + PAGE_SIZE as u64;

            // Overlap between [seg_start_va, seg_file_end_va) and [page_va_lo, page_va_hi)?
            let copy_lo = seg_start_va.max(page_va_lo);
            let copy_hi = seg_file_end_va.min(page_va_hi);

            if copy_lo < copy_hi {
                let copy_len = (copy_hi - copy_lo) as usize;
                let src_offset = offset + (copy_lo - seg_start_va) as usize;
                let dst_offset = (copy_lo - page_base_va) as usize;

                let dst = (page_phys.to_virt().as_u64() + dst_offset as u64) as *mut u8;
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        elf_data.as_ptr().add(src_offset),
                        dst,
                        copy_len,
                    );
                }
                total_copied += copy_len;
            }

            // BSS bytes: any part of [filesz, memsz) that falls in this page
            // is already zeroed by alloc_frame_zeroed() — no extra work needed.
        }

        // Accumulate BSS statistics.
        if memsz > filesz {
            total_bss += memsz - filesz;
        }
    }

    Ok(ElfLoadResult {
        entry_point: ehdr.e_entry,
        pages_mapped: total_pages,
        bytes_copied: total_copied,
        bss_zeroed: total_bss,
    })
}

/// Converts ELF segment permission flags (PF_R/PF_W/PF_X) to x86_64
/// page table flags with W^X enforcement.
fn elf_flags_to_page_flags(elf_flags: u32) -> PageTableFlags {
    let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER;

    if elf_flags & PF_W != 0 {
        flags |= PageTableFlags::WRITABLE;
    }

    // x86_64 NX bit: set NO_EXECUTE unless the segment is executable.
    if elf_flags & PF_X == 0 {
        flags |= PageTableFlags::NO_EXECUTE;
    }

    flags
}
