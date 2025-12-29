/* MinimalOS Linker Script
 * Higher-half kernel at 0xFFFFFFFF80000000
 * Physical load at 1MB
 */

ENTRY(_start)

/* Virtual address where kernel will be linked */
KERNEL_VMA = 0xFFFFFFFF80000000;

/* Physical address where bootloader loads us */
KERNEL_LMA = 0x100000;

SECTIONS
{
    /* Physical location for multiboot header (must be in first 32KB) */
    . = KERNEL_LMA;

    /* Multiboot2 header - must come first */
    .multiboot2 ALIGN(8) : 
    {
        KEEP(*(.multiboot2))
    }

    /* 32-bit boot code (runs before long mode transition) - physical addresses */
    .boot32 ALIGN(4K) :
    {
        *(.boot32)
        *(.boot32.*)
    }

    /* Boot-time BSS - page tables etc in low memory (physical addresses only) */
    .boot_bss ALIGN(4K) (NOLOAD) :
    {
        _boot_bss_start = .;
        *(.boot_bss)
        _boot_bss_end = .;
    }

    /* Record where 32-bit boot sections end */
    _boot_end_phys = .;

    /* Switch to higher-half virtual addresses */
    . += KERNEL_VMA;

    /* 64-bit kernel code */
    .text ALIGN(4K) : AT(ADDR(.text) - KERNEL_VMA)
    {
        _text_start = .;
        *(.text)
        *(.text.*)
        _text_end = .;
    }

    /* Read-only data */
    .rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_VMA)
    {
        _rodata_start = .;
        *(.rodata)
        *(.rodata.*)
        _rodata_end = .;
    }

    /* Initialized data */
    .data ALIGN(4K) : AT(ADDR(.data) - KERNEL_VMA)
    {
        _data_start = .;
        *(.data)
        *(.data.*)
        _data_end = .;
    }

    /* Uninitialized data (BSS) - at higher-half addresses */
    .bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_VMA)
    {
        _bss_start = .;
        *(COMMON)
        *(.bss)
        *(.bss.*)
        _bss_end = .;
    }

    /* Kernel end marker */
    . = ALIGN(4K);
    _kernel_end = .;
    _kernel_end_phys = . - KERNEL_VMA;

    /* Discard unnecessary sections */
    /DISCARD/ :
    {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
    }
}

/* Export symbols for kernel use */
PROVIDE(_kernel_start = KERNEL_VMA + KERNEL_LMA);
PROVIDE(_kernel_start_phys = KERNEL_LMA);
