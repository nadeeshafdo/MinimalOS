/* IDT flush routine for x86_64 */
.section .text
.global idt_flush
.type idt_flush, @function

/* void idt_flush(uint64_t idt_ptr_address) */
idt_flush:
    /* Load IDT - address in rdi (System V AMD64 ABI) */
    lidt (%rdi)
    ret

.size idt_flush, . - idt_flush
