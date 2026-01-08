/* Usermode transition for x86_64 */
.section .text
.global jump_usermode
.type jump_usermode, @function

/* void jump_usermode(uint64_t entry, uint64_t user_stack) */
/* rdi = entry point, rsi = user stack pointer */
jump_usermode:
    /* Disable interrupts while setting up */
    cli
    
    /* Set up the stack frame for iretq:
       SS     (rsp + 32)
       RSP    (rsp + 24)
       RFLAGS (rsp + 16)
       CS     (rsp + 8)
       RIP    (rsp + 0) */
    
    /* User data segment selector (0x20 | 3 for RPL 3) */
    mov $0x23, %rax
    push %rax           /* SS */
    
    /* User stack pointer */
    push %rsi           /* RSP */
    
    /* RFLAGS with interrupts enabled */
    pushfq
    pop %rax
    or $0x200, %rax     /* Set IF (interrupt flag) */
    push %rax           /* RFLAGS */
    
    /* User code segment selector (0x18 | 3 for RPL 3) */
    mov $0x1B, %rax
    push %rax           /* CS */
    
    /* Entry point */
    push %rdi           /* RIP */
    
    /* Clear registers */
    xor %rax, %rax
    xor %rbx, %rbx
    xor %rcx, %rcx
    xor %rdx, %rdx
    xor %rsi, %rsi
    xor %rdi, %rdi
    xor %rbp, %rbp
    xor %r8, %r8
    xor %r9, %r9
    xor %r10, %r10
    xor %r11, %r11
    xor %r12, %r12
    xor %r13, %r13
    xor %r14, %r14
    xor %r15, %r15
    
    /* Jump to user mode */
    iretq

.size jump_usermode, . - jump_usermode
