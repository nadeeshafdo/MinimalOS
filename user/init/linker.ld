/* =============================================================================
 * init — Linker Script
 * =============================================================================
 *
 * Places the init binary at virtual address 0x400000 (4 MiB).
 * The kernel's ELF loader maps PT_LOAD segments at the addresses specified
 * in the ELF program headers. This linker script ensures consistent placement.
 *
 * Same base address as serial_drv — each process has isolated page tables
 * so there's no conflict.
 * =============================================================================
 */

ENTRY(_start)

SECTIONS {
    . = 0x400000;

    .text ALIGN(16) : {
        *(.text.entry)
        *(.text .text.*)
    }

    .rodata ALIGN(16) : {
        *(.rodata .rodata.*)
    }

    .data ALIGN(16) : {
        *(.data .data.*)
    }

    .bss ALIGN(16) : {
        *(.bss .bss.*)
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.note.*)
        *(.comment)
        *(.debug_*)
    }
}
