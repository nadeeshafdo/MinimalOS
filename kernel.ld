ENTRY(_kernel_entry)
SECTIONS
{
    . = 0x100000;  /* Kernel loaded at 1MB physical */
    .text ALIGN(4K) : { *(.text) }
    .rodata ALIGN(4K) : { *(.rodata) }
    .data ALIGN(4K) : { *(.data) }
    .bss ALIGN(4K) : { *(COMMON) *(.bss) }
}