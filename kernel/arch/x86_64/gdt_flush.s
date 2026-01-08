/* GDT flush routine for x86_64 */
.section .text
.global gdt_flush
.type gdt_flush, @function

/* void gdt_flush(uint64_t gdt_ptr_address) */
gdt_flush:
    /* Load GDT - address in rdi (System V AMD64 ABI) */
    lgdt (%rdi)
    
    /* Reload data segment registers */
    mov $0x10, %ax      /* Kernel data segment selector */
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    mov %ax, %ss
    
    /* Far return to reload CS with kernel code segment */
    /* In 64-bit mode, we use a far return through the stack */
    pop %rdi            /* Save return address */
    push $0x08          /* Kernel code segment selector */
    push %rdi           /* Return address */
    retfq               /* Far return (64-bit) */

.size gdt_flush, . - gdt_flush

.global tss_flush
.type tss_flush, @function

tss_flush:
    /* Load TSS selector (0x28) into task register */
    mov $0x28, %ax
    ltr %ax
    ret

.size tss_flush, . - tss_flush
