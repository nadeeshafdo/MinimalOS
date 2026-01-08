/* Linker script for x86_64 kernel with Limine bootloader */
OUTPUT_FORMAT(elf64-x86-64)

/* Entry point - Limine will call kmain directly */
ENTRY(kmain)

/* Define program headers for proper MMU permissions */
PHDRS
{
    limine_requests PT_LOAD;
    text    PT_LOAD;
    rodata  PT_LOAD;
    data    PT_LOAD;
}

SECTIONS
{
    /* Kernel loaded in higher half at 0xffffffff80000000 (top 2GB) */
    /* This is required by the Limine boot protocol */
    . = 0xffffffff80000000;
    
    /* Store kernel start address */
    __kernel_start = .;
    
    /* Limine requests section - must be in its own segment */
    .limine_requests : {
        KEEP(*(.limine_requests_start))
        KEEP(*(.limine_requests))
        KEEP(*(.limine_requests_end))
    } :limine_requests
    
    /* Code section - page aligned */
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .text : {
        *(.text .text.*)
    } :text
    
    /* Read-only data - page aligned */
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .rodata : {
        *(.rodata .rodata.*)
    } :rodata
    
    /* Build ID if present */
    .note.gnu.build-id : {
        *(.note.gnu.build-id)
    } :rodata
    
    /* Initialized data - page aligned */
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .data : {
        *(.data .data.*)
    } :data
    
    /* Uninitialized data (BSS) - must be last in :data */
    .bss : {
        *(COMMON)
        *(.bss .bss.*)
    } :data
    
    /* Store kernel end address */
    __kernel_end = .;
    
    /* Discard unwanted sections */
    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
    }
}
