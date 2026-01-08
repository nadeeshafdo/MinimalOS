/* Context switch routine */
.section .text
.global context_switch
.type context_switch, @function

/* void context_switch(cpu_context_t *old, cpu_context_t *new) */
context_switch:
    /* Get pointers from stack */
    mov 4(%esp), %eax    /* old context pointer */
    mov 8(%esp), %edx    /* new context pointer */
    
    /* Save current context to old */
    mov %edi, 0(%eax)
    mov %esi, 4(%eax)
    mov %ebp, 8(%eax)
    mov %esp, 12(%eax)
    mov %ebx, 16(%eax)
    /* edx, ecx, eax will be overwritten - save them */
    push %edx
    mov 12(%esp), %ecx   /* Get original edx from stack (before push) */
    mov %ecx, 20(%eax)   /* Save edx */
    mov 8(%esp), %ecx    /* Get original ecx */
    mov %ecx, 24(%eax)
    mov 4(%esp), %ecx    /* Get original eax */
    mov %ecx, 28(%eax)
    pop %edx
    
    /* Save return address as EIP */
    mov (%esp), %ecx     /* Return address */
    mov %ecx, 32(%eax)   /* Save EIP */
    
    /* Save CS (current code segment) */
    mov %cs, %cx
    movzx %cx, %ecx
    mov %ecx, 36(%eax)
    
    /* Save EFLAGS */
    pushf
    pop %ecx
    mov %ecx, 40(%eax)
    
    /* Load new context from new */
    mov 0(%edx), %edi
    mov 4(%edx), %esi
    mov 8(%edx), %ebp
    mov 16(%edx), %ebx
    mov 24(%edx), %ecx
    mov 28(%edx), %eax
    
    /* Load new stack */
    mov 12(%edx), %esp
    
    /* Push new EIP for return */
    push 32(%edx)
    
    /* Load EFLAGS */
    push 40(%edx)
    popf
    
    /* Load remaining registers and return */
    mov 20(%edx), %edx   /* Load edx last since we used it */
    
    ret
