/* Linker script for flat user-mode binaries.
   Places _start at the very beginning so the kernel can jump to offset 0. */

ENTRY(_start)

SECTIONS
{
    /* The kernel will load us at a user-space virtual address (e.g. 0x400000).
       We use 0 as the base here because we produce a flat binary — the kernel
       adds the actual load address. */
    . = 0x400000;

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
    }

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note*)
    }
}
