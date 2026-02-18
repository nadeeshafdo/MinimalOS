/* Linker script for shell user-mode binary.
   Placed at a different base address than init to allow shared address space. */

ENTRY(_start)

SECTIONS
{
    . = 0x500000;

    .text : {
        *(.text._start)   /* _start first */
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.note .note.*)
        *(.comment)
    }
}
