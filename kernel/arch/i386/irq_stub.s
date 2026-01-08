/* IRQ stub macros and handlers */
.section .text

/* Macro for IRQ handlers */
.macro IRQ num, isr_num
.global irq\num
irq\num:
    cli
    push $0          /* Dummy error code */
    push $\isr_num   /* Interrupt number */
    jmp irq_common_stub
.endm

/* Define all 16 IRQs */
IRQ 0, 32
IRQ 1, 33
IRQ 2, 34
IRQ 3, 35
IRQ 4, 36
IRQ 5, 37
IRQ 6, 38
IRQ 7, 39
IRQ 8, 40
IRQ 9, 41
IRQ 10, 42
IRQ 11, 43
IRQ 12, 44
IRQ 13, 45
IRQ 14, 46
IRQ 15, 47

/* Common IRQ stub - saves state and calls C handler */
.extern irq_handler
irq_common_stub:
    /* Save all registers */
    pusha
    
    /* Save segment registers */
    push %ds
    push %es
    push %fs
    push %gs
    
    /* Load kernel data segment */
    mov $0x10, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    
    /* Call C handler */
    push %esp
    call irq_handler
    add $4, %esp
    
    /* Restore segment registers */
    pop %gs
    pop %fs
    pop %es
    pop %ds
    
    /* Restore general purpose registers */
    popa
    
    /* Clean up error code and interrupt number */
    add $8, %esp
    
    /* Return from interrupt */
    iret
