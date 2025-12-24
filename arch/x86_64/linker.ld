/* Linker script for x86_64 higher-half kernel */

ENTRY(_start)

/* Higher-half virtual address */
KERNEL_VMA = 0xFFFFFFFF80000000;

SECTIONS
{
    /* Physical load address (GRUB loads here) */
    . = 1M;

    /* Multiboot header must be early */
    .multiboot2 ALIGN(8) :
    {
        *(.multiboot2)
    }

    /* Text section - mapped to higher half virtually */
    .text ALIGN(4K) : AT(ADDR(.text))
    {
        *(.text)
        *(.text.*)
    }

    .rodata ALIGN(4K) : AT(ADDR(.rodata))
    {
        *(.rodata)
        *(.rodata.*)
    }

    .data ALIGN(4K) : AT(ADDR(.data))
    {
        *(.data)
        *(.data.*)
    }

    .bss ALIGN(4K) : AT(ADDR(.bss))
    {
        __bss_start = .;
        *(.bss)
        *(COMMON)
        __bss_end = .;
    }

    __kernel_end = .;
}
