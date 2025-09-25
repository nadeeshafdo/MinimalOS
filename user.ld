ENTRY(_user_start)
SECTIONS
{
    . = 0x400000;  /* User space at 4MB virtual */
    .text : { *(.text) }
    .rodata : { *(.rodata) }
    .data : { *(.data) }
    .bss : { *(COMMON) *(.bss) }
}