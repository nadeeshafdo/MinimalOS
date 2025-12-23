/* ISR stub macros and handlers */
.section .text

/* Macro for ISRs without error code */
.macro ISR_NOERRCODE num
.global isr\num
isr\num:
    cli
    push $0          /* Dummy error code */
    push $\num       /* Interrupt number */
    jmp isr_common_stub
.endm

/* Macro for ISRs with error code */
.macro ISR_ERRCODE num
.global isr\num
isr\num:
    cli
    push $\num       /* Interrupt number */
    jmp isr_common_stub
.endm

/* Define all 32 ISRs */
ISR_NOERRCODE 0
ISR_NOERRCODE 1
ISR_NOERRCODE 2
ISR_NOERRCODE 3
ISR_NOERRCODE 4
ISR_NOERRCODE 5
ISR_NOERRCODE 6
ISR_NOERRCODE 7
ISR_ERRCODE 8
ISR_NOERRCODE 9
ISR_ERRCODE 10
ISR_ERRCODE 11
ISR_ERRCODE 12
ISR_ERRCODE 13
ISR_ERRCODE 14
ISR_NOERRCODE 15
ISR_NOERRCODE 16
ISR_ERRCODE 17
ISR_NOERRCODE 18
ISR_NOERRCODE 19
ISR_NOERRCODE 20
ISR_NOERRCODE 21
ISR_NOERRCODE 22
ISR_NOERRCODE 23
ISR_NOERRCODE 24
ISR_NOERRCODE 25
ISR_NOERRCODE 26
ISR_NOERRCODE 27
ISR_NOERRCODE 28
ISR_NOERRCODE 29
ISR_ERRCODE 30
ISR_NOERRCODE 31
ISR_NOERRCODE 128

/* Common ISR stub - saves state and calls C handler */
.extern isr_handler
isr_common_stub:
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
    call isr_handler
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
