/*
 * linker.ld
 * Linker Script for MinimalOS (Higher Half)
 */

ENTRY(_start)

/* Virtual Base Address */
KERNEL_VMA = 0xFFFFFFFF80000000;

/* Physical Load Address (1MB) */
KERNEL_PHYS = 0x00100000;

SECTIONS
{
    /* Start at 1MB offset in higher half */
    . = KERNEL_VMA + KERNEL_PHYS;

    _kernel_start = .;

    /* 
     * Multiboot2 header must be within the first 32KB of the kernel image.
     * We put it at the start.
     * AT(...) specifies the physical address (Load Memory Address).
     */
    .multiboot2 : AT(ADDR(.multiboot2) - KERNEL_VMA)
    {
        KEEP(*(.multiboot2))
    }

    .text : AT(ADDR(.text) - KERNEL_VMA)
    {
        *(.text)
    }

    .rodata : AT(ADDR(.rodata) - KERNEL_VMA)
    {
        *(.rodata)
    }

    .data : AT(ADDR(.data) - KERNEL_VMA)
    {
        *(.data)
    }

    .bss : AT(ADDR(.bss) - KERNEL_VMA)
    {
        _bss_start = .;
        *(.bss)
        *(COMMON)
        _bss_end = .;
    }

    _kernel_end = .;
}
