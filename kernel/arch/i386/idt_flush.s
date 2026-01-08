.section .text
.global idt_flush
.type idt_flush, @function

/* Load IDT */
idt_flush:
    mov 4(%esp), %eax    /* Get IDT pointer from stack */
    lidt (%eax)          /* Load IDT */
    ret
