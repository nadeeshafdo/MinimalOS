# Context Switching for x86_64
# Saves and restores CPU state between processes

.section .text
.global context_switch

# void context_switch(cpu_context_t** old_ctx, cpu_context_t* new_ctx)
# RDI = pointer to old context pointer
# RSI = new context pointer

context_switch:
    # Save current context if old_ctx is not NULL
    test %rdi, %rdi
    jz load_new_context
    
    # Get pointer to where we should save context
    mov (%rdi), %rax
    test %rax, %rax
    jz load_new_context
    
    # Save all general-purpose registers
    mov %r15, 0(%rax)
    mov %r14, 8(%rax)
    mov %r13, 16(%rax)
    mov %r12, 24(%rax)
    mov %r11, 32(%rax)
    mov %r10, 40(%rax)
    mov %r9,  48(%rax)
    mov %r8,  56(%rax)
    mov %rbp, 64(%rax)
    mov %rdi, 72(%rax)
    
    # Save RSI (we need to grab it from stack since we used it)
    mov %rsi, 80(%rax)
    
    mov %rdx, 88(%rax)
    mov %rcx, 96(%rax)
    mov %rbx, 104(%rax)
    
    # Save RAX last (we've been using it)
    mov %rax, %r11
    mov %r11, 112(%rax)
    
    # Save RIP (return address)  
    mov (%rsp), %r11
    mov %r11, 120(%rax)
    
    # Save CS
    mov $0x08, %r11
    mov %r11, 128(%rax)
    
    # Save RFLAGS
    pushfq
    pop %r11
    mov %r11, 136(%rax)
    
    # Save RSP (stack pointer before return address)
    lea 8(%rsp), %r11
    mov %r11, 144(%rax)
    
    # Save SS
    mov $0x10, %r11
    mov %r11, 152(%rax)
    
load_new_context:
    # Load new context from RSI
    test %rsi, %rsi
    jz done_context_switch
    
    # Restore general-purpose registers
    mov 0(%rsi),   %r15
    mov 8(%rsi),   %r14
    mov 16(%rsi),  %r13
    mov 24(%rsi),  %r12
    mov 32(%rsi),  %r11
    mov 40(%rsi),  %r10
    mov 48(%rsi),  %r9
    mov 56(%rsi),  %r8
    mov 64(%rsi),  %rbp
    mov 72(%rsi),  %rdi
    mov 88(%rsi),  %rdx
    mov 96(%rsi),  %rcx
    mov 104(%rsi), %rbx
    mov 112(%rsi), %rax
    
    # Restore RFLAGS
    mov 136(%rsi), %r11
    push %r11
    popfq
    
    # Restore RSP
    mov 144(%rsi), %rsp
    
    # Restore RSI last
    mov 80(%rsi), %rsi
    
done_context_switch:
    ret
