/* 64-bit IRQ stubs for x86_64 */
.section .text

/* Macro for IRQ handlers */
.macro IRQ num, int_num
.global irq\num
.type irq\num, @function
irq\num:
    push $0                     /* Push dummy error code */
    push $\int_num              /* Push interrupt number */
    jmp irq_common_stub
.size irq\num, . - irq\num
.endm

/* Define all 16 hardware IRQ handlers (remapped to INT 32-47) */
IRQ 0, 32       /* Timer */
IRQ 1, 33       /* Keyboard */
IRQ 2, 34       /* Cascade for 8259A slave */
IRQ 3, 35       /* COM2 */
IRQ 4, 36       /* COM1 */
IRQ 5, 37       /* LPT2 */
IRQ 6, 38       /* Floppy */
IRQ 7, 39       /* LPT1 / Spurious */
IRQ 8, 40       /* RTC */
IRQ 9, 41       /* Free */
IRQ 10, 42      /* Free */
IRQ 11, 43      /* Free */
IRQ 12, 44      /* PS/2 Mouse */
IRQ 13, 45      /* FPU */
IRQ 14, 46      /* Primary ATA */
IRQ 15, 47      /* Secondary ATA */

/* Common IRQ stub - saves all registers and calls C handler */
.global irq_common_stub
.type irq_common_stub, @function
irq_common_stub:
    /* Save all general purpose registers */
    push %rax
    push %rbx
    push %rcx
    push %rdx
    push %rsi
    push %rdi
    push %rbp
    push %r8
    push %r9
    push %r10
    push %r11
    push %r12
    push %r13
    push %r14
    push %r15
    
    /* Pass pointer to registers struct as first argument */
    mov %rsp, %rdi
    
    /* Save stack pointer for later restoration */
    mov %rsp, %rbx
    
    /* Align stack to 16 bytes (ABI requirement) */
    and $~0xF, %rsp
    
    /* Call C handler */
    call irq_handler
    
    /* Restore stack pointer */
    mov %rbx, %rsp
    
    /* Restore all registers */
    pop %r15
    pop %r14
    pop %r13
    pop %r12
    pop %r11
    pop %r10
    pop %r9
    pop %r8
    pop %rbp
    pop %rdi
    pop %rsi
    pop %rdx
    pop %rcx
    pop %rbx
    pop %rax
    
    /* Remove interrupt number and error code from stack */
    add $16, %rsp
    
    /* Return from interrupt */
    iretq

.size irq_common_stub, . - irq_common_stub
