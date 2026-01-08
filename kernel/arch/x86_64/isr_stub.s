/* 64-bit ISR stubs for x86_64 */
.section .text

/* Macro for ISR that does NOT push error code */
.macro ISR_NOERRCODE num
.global isr\num
.type isr\num, @function
isr\num:
    push $0                     /* Push dummy error code */
    push $\num                  /* Push interrupt number */
    jmp isr_common_stub
.size isr\num, . - isr\num
.endm

/* Macro for ISR that DOES push error code */
.macro ISR_ERRCODE num
.global isr\num
.type isr\num, @function
isr\num:
    push $\num                  /* Push interrupt number */
    jmp isr_common_stub
.size isr\num, . - isr\num
.endm

/* Define all 32 CPU exception handlers */
ISR_NOERRCODE 0     /* Division By Zero */
ISR_NOERRCODE 1     /* Debug */
ISR_NOERRCODE 2     /* Non Maskable Interrupt */
ISR_NOERRCODE 3     /* Breakpoint */
ISR_NOERRCODE 4     /* Into Detected Overflow */
ISR_NOERRCODE 5     /* Out of Bounds */
ISR_NOERRCODE 6     /* Invalid Opcode */
ISR_NOERRCODE 7     /* No Coprocessor */
ISR_ERRCODE   8     /* Double Fault (error code) */
ISR_NOERRCODE 9     /* Coprocessor Segment Overrun */
ISR_ERRCODE   10    /* Bad TSS (error code) */
ISR_ERRCODE   11    /* Segment Not Present (error code) */
ISR_ERRCODE   12    /* Stack Fault (error code) */
ISR_ERRCODE   13    /* General Protection Fault (error code) */
ISR_ERRCODE   14    /* Page Fault (error code) */
ISR_NOERRCODE 15    /* Reserved */
ISR_NOERRCODE 16    /* x87 Floating Point Exception */
ISR_ERRCODE   17    /* Alignment Check (error code) */
ISR_NOERRCODE 18    /* Machine Check */
ISR_NOERRCODE 19    /* SIMD Floating Point Exception */
ISR_NOERRCODE 20    /* Virtualization Exception */
ISR_ERRCODE   21    /* Control Protection Exception */
ISR_NOERRCODE 22    /* Reserved */
ISR_NOERRCODE 23    /* Reserved */
ISR_NOERRCODE 24    /* Reserved */
ISR_NOERRCODE 25    /* Reserved */
ISR_NOERRCODE 26    /* Reserved */
ISR_NOERRCODE 27    /* Reserved */
ISR_NOERRCODE 28    /* Hypervisor Injection Exception */
ISR_ERRCODE   29    /* VMM Communication Exception */
ISR_ERRCODE   30    /* Security Exception */
ISR_NOERRCODE 31    /* Reserved */

/* Syscall handler (int 0x80) */
ISR_NOERRCODE 128

/* Common ISR stub - saves all registers and calls C handler */
.global isr_common_stub
.type isr_common_stub, @function
isr_common_stub:
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
    
    /* Align stack to 16 bytes (ABI requirement) */
    and $~0xF, %rsp
    
    /* Call C handler */
    call isr_handler
    
    /* Restore stack pointer */
    mov %rdi, %rsp
    
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

.size isr_common_stub, . - isr_common_stub
