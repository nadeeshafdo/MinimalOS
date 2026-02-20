/* Linker script for display_server user-mode binary.
   Placed at 0x600000 to avoid collision with init (0x400000) and shell (0x500000). */

ENTRY(_start)

SECTIONS
{
	. = 0x600000;

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
