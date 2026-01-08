/* Linker script for i686 kernel */
ENTRY(_start)

SECTIONS
{
    /* Kernel loaded at 2M physical address (UEFI-safe) */
    . = 2M;
    
    /* Store kernel start address */
    __kernel_start = .;
    
    /* Multiboot header must be early in the binary */
    .text BLOCK(4K) : ALIGN(4K)
    {
        *(.multiboot)
        *(.text)
    }
    
    /* Read-only data */
    .rodata BLOCK(4K) : ALIGN(4K)
    {
        *(.rodata)
    }
    
    /* Initialized data */
    .data BLOCK(4K) : ALIGN(4K)
    {
        *(.data)
    }
    
    /* Uninitialized data and stack */
    .bss BLOCK(4K) : ALIGN(4K)
    {
        *(COMMON)
        *(.bss)
    }
    
    /* Store kernel end address */
    __kernel_end = .;
}
