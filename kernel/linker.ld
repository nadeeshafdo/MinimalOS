/* ===========================================================================
 * MinimalOS NextGen — Kernel Linker Script
 * ===========================================================================
 *
 * This script controls the memory layout of the kernel binary. It tells the
 * linker WHERE to place each section (.text, .rodata, .data, .bss) in the
 * virtual address space.
 *
 * KEY CONCEPT: Higher-Half Kernel
 * ================================
 * The kernel is mapped at the top of the 64-bit virtual address space:
 *   Virtual base:  0xFFFFFFFF80000000  (top 2GB)
 *   Physical base:  wherever Limine loads us (typically ~1MB)
 *
 * The Limine bootloader:
 *   1. Loads the kernel ELF into physical memory
 *   2. Sets up page tables mapping the kernel at 0xFFFFFFFF80000000
 *   3. Jumps to our entry point with paging already enabled
 *
 * WHY higher-half?
 *   - Leaves the entire lower half (0x0000...0x7FFF...) for userspace
 *   - Kernel code uses addresses starting with 0xFFFF... 
 *   - User code uses addresses starting with 0x0000...
 *   - The x86_64 "canonical address" hole in the middle acts as a natural
 *     guard between user and kernel space
 *   - The `kernel` code model (-C code-model=kernel) generates efficient
 *     RIP-relative addressing for this region
 *
 * SECTIONS EXPLAINED:
 *   .text    — executable code (mapped Read + Execute, NO write)
 *   .rodata  — read-only data: strings, constants (mapped Read only)
 *   .data    — initialized mutable data (mapped Read + Write)
 *   .bss     — zero-initialized mutable data (mapped Read + Write)
 *
 * Each section is page-aligned (4096 bytes) so we can set different
 * page table permissions for each one. This prevents code injection
 * attacks — even if an attacker corrupts .data, they can't execute it
 * because .data pages have the NX (No Execute) bit set.
 *
 * =========================================================================== */

/* Kernel entry point — Rust function annotated with #[no_mangle] */
ENTRY(kmain)

/* Higher-half virtual address base */
KERNEL_VIRT_BASE = 0xFFFFFFFF80000000;

SECTIONS
{
    /* -----------------------------------------------------------------------
     * Start placing sections at the higher-half base address.
     * The Limine bootloader maps the kernel here.
     * We add a 2MB offset to avoid conflicting with any bootloader structures.
     * ----------------------------------------------------------------------- */
    . = KERNEL_VIRT_BASE + 2M;

    /* -----------------------------------------------------------------------
     * .text — Executable code
     * Permissions: Read + Execute (no write — W^X policy)
     * 
     * The `KEEP` directive ensures the linker doesn't discard sections even
     * if they appear unused (important for interrupt handler stubs that are
     * only referenced via the IDT, not by direct function calls).
     * ----------------------------------------------------------------------- */
    _kernel_start = .;
    
    .text ALIGN(4K) : {
        _text_start = .;
        KEEP(*(.text.kmain))    /* Entry point first — for predictable layout */
        *(.text .text.*)        /* All other code */
        _text_end = .;
    }

    /* -----------------------------------------------------------------------
     * .rodata — Read-only data
     * Permissions: Read only (no write, no execute)
     *
     * Contains: string literals, const arrays, vtables, RTTI
     * Also contains Limine request structures (they're static consts)
     * ----------------------------------------------------------------------- */
    .rodata ALIGN(4K) : {
        _rodata_start = .;

        /* Limine requests — the bootloader scans for these magic patterns
         * in the kernel binary. They must be in a loadable section. */
        KEEP(*(.limine_requests))

        *(.rodata .rodata.*)
        _rodata_end = .;
    }

    /* -----------------------------------------------------------------------
     * .data — Initialized mutable data
     * Permissions: Read + Write (no execute — NX bit set)
     *
     * Contains: mutable statics, initialized global variables
     * Relatively small in a well-designed kernel.
     * ----------------------------------------------------------------------- */
    .data ALIGN(4K) : {
        _data_start = .;
        *(.data .data.*)
        _data_end = .;
    }

    /* -----------------------------------------------------------------------
     * .bss — Zero-initialized mutable data
     * Permissions: Read + Write (no execute — NX bit set)
     *
     * Contains: uninitialized statics (Rust initializes them to zero)
     * The bootloader zeros this region before jumping to the kernel.
     *
     * This is where large statically-allocated structures go:
     *   - Physical memory bitmap (~256KB for 8GB RAM)
     *   - Initial kernel stack
     *   - Per-CPU data arrays
     *
     * BSS doesn't take space in the binary file — only in memory.
     * ----------------------------------------------------------------------- */
    .bss ALIGN(4K) : {
        _bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        _bss_end = .;
    }

    _kernel_end = .;

    /* -----------------------------------------------------------------------
     * Discard unnecessary sections
     * 
     * These are standard ELF sections we don't need in a kernel:
     *   .comment  — compiler version strings
     *   .note.*   — ELF notes (GNU build ID, etc.)
     *   .eh_frame — C++ exception handling (we don't unwind)
     * ----------------------------------------------------------------------- */
    /DISCARD/ : {
        *(.comment)
        *(.note.*)
        *(.eh_frame*)
        *(.gnu.hash)
    }
}

/* ===========================================================================
 * Symbols exported for use in Rust code
 * ===========================================================================
 *
 * The following symbols are available in Rust via:
 *   extern "C" { static _kernel_start: u8; }
 *   let addr = &_kernel_start as *const u8 as usize;
 *
 *   _kernel_start  — first byte of kernel in virtual memory
 *   _kernel_end    — one past the last byte of kernel
 *   _text_start/end    — bounds of executable code
 *   _rodata_start/end  — bounds of read-only data
 *   _data_start/end    — bounds of initialized mutable data
 *   _bss_start/end     — bounds of zero-initialized data
 *
 * These are used by the memory manager to:
 *   1. Know which physical frames the kernel occupies (don't free them!)
 *   2. Set correct page permissions per section (W^X enforcement)
 *   3. Calculate kernel size for boot logging
 * =========================================================================== */
