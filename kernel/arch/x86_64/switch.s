/* Context switch routine for x86_64 */
.section .text
.global context_switch
.type context_switch, @function

/* void context_switch(cpu_context_t *old, cpu_context_t *new) */
/* rdi = old context pointer, rsi = new context pointer (System V AMD64 ABI) */
context_switch:
    /* Save callee-saved registers to old context */
    /* Offset layout in cpu_context_t struct (see process.h):
       0x00: r15, 0x08: r14, 0x10: r13, 0x18: r12
       0x20: rbx, 0x28: rbp, 0x30: rsp, 0x38: rip
       0x40: rflags */
    
    mov %r15, 0x00(%rdi)
    mov %r14, 0x08(%rdi)
    mov %r13, 0x10(%rdi)
    mov %r12, 0x18(%rdi)
    mov %rbx, 0x20(%rdi)
    mov %rbp, 0x28(%rdi)
    mov %rsp, 0x30(%rdi)
    
    /* Save return address as RIP */
    mov (%rsp), %rax
    mov %rax, 0x38(%rdi)
    
    /* Save RFLAGS */
    pushfq
    pop %rax
    mov %rax, 0x40(%rdi)
    
    /* Load new context */
    mov 0x00(%rsi), %r15
    mov 0x08(%rsi), %r14
    mov 0x10(%rsi), %r13
    mov 0x18(%rsi), %r12
    mov 0x20(%rsi), %rbx
    mov 0x28(%rsi), %rbp
    
    /* Load new stack */
    mov 0x30(%rsi), %rsp
    
    /* Load RFLAGS */
    mov 0x40(%rsi), %rax
    push %rax
    popfq
    
    /* Push new RIP for return */
    mov 0x38(%rsi), %rax
    push %rax
    
    ret

.size context_switch, . - context_switch
