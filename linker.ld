/* Linker Script for MinimalOS Kernel */

ENTRY(_start)

SECTIONS
{
    /* Kernel loaded at 1MB by GRUB */
    . = 1M;
    
    /* Multiboot header must be in first 32KB */
    .multiboot ALIGN(4K) : {
        *(.multiboot)
    }
    
    /* Boot code (32-bit and 64-bit) */
    .text.boot ALIGN(4K) : {
        *(.text.boot)
        *(.text.boot64)
    }
    
    /* Main code section */
    .text ALIGN(4K) : {
        *(.text)
    }
    
    /* Read-only data */
    .rodata ALIGN(4K) : {
        *(.rodata*)
    }
    
    /* Initialized data */
    .data ALIGN(4K) : {
        *(.data)
    }
    
    /* Uninitialized data */
    .bss ALIGN(4K) : {
        *(COMMON)
        *(.bss)
    }
    
    /* Export kernel end address */
    _kernel_end = .;
}
