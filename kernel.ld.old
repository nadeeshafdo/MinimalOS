ENTRY(_kernel_entry)
SECTIONS
{
    . = 0x8000;  /* Kernel loaded at 32KB physical (where bootloader puts it) */
    .text ALIGN(4K) : { *(.text) }
    .rodata ALIGN(4K) : { *(.rodata) }
    .data ALIGN(4K) : { *(.data) }
    .bss ALIGN(4K) : { *(COMMON) *(.bss) }
}