/* =============================================================================
 * serial_drv — Linker Script
 * =============================================================================
 *
 * Places the serial driver binary at virtual address 0x400000 (4 MiB).
 * The kernel maps pages at this address with USER flag before spawning
 * the driver thread.
 *
 * After `objcopy -O binary`, the first byte of the flat binary is the
 * first instruction of _start. The kernel loads this flat binary at 0x400000.
 * =============================================================================
 */

ENTRY(_start)

SECTIONS {
    . = 0x400000;

    .text ALIGN(4096) : {
        *(.text.entry)
        *(.text .text.*)
    }

    .rodata ALIGN(4096) : {
        *(.rodata .rodata.*)
    }

    .data ALIGN(4096) : {
        *(.data .data.*)
    }

    .bss ALIGN(4096) : {
        *(.bss .bss.*)
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.note.*)
        *(.comment)
        *(.debug_*)
    }
}
