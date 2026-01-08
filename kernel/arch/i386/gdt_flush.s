.section .text
.global gdt_flush
.type gdt_flush, @function

/* Load GDT and update segment registers */
gdt_flush:
    mov 4(%esp), %eax    /* Get GDT pointer from stack */
    lgdt (%eax)          /* Load GDT */
    
    /* Update segment registers */
    mov $0x10, %ax       /* Kernel data segment (offset 0x10) */
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    mov %ax, %ss
    
    /* Far jump to update CS */
    ljmp $0x08, $flush_done   /* Kernel code segment (offset 0x08) */
    
flush_done:
    ret

.global tss_flush
tss_flush:
    mov $0x28, %ax      /* 0x28 is the offset in the GDT to our TSS (index 5 * 8) */
    ltr %ax             /* Load Task Register */
    ret
