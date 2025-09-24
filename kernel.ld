ENTRY(_kernel_entry)
SECTIONS
{
    . = 0x10000;  /* Kernel loaded at 64KB physical instead of 1MB */
    .text ALIGN(4K) : { *(.text) }
    .rodata ALIGN(4K) : { *(.rodata) }
    .data ALIGN(4K) : { *(.data) }
    .bss ALIGN(4K) : { *(COMMON) *(.bss) }
}