/* Linker Script for MinimalOS Kernel */

ENTRY(_start)

SECTIONS
{
    /* Kernel loaded at 1MB by GRUB */
    . = 1M;
    
    _kernel_start = .;
    
    /* Boot code with multiboot header FIRST */
    .text.boot : {
        *(.multiboot)
        *(.text.boot)
        *(.text.boot64)
    }
    
    /* Main code section */
    .text : {
        *(.text)
    }
    
    /* Read-only data */
    .rodata : {
        *(.rodata*)
    }
    
    /* Initialized data */
    .data : {
        *(.data)
    }
    
    /* Uninitialized data */
    .bss : {
        *(COMMON)
        *(.bss)
    }
    
    /* Export kernel end address */
    _kernel_end = .;
}
