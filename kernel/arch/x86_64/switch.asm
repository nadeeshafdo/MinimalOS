; Context switch for x86_64
; void switch_context(uint64_t *old_rsp, uint64_t new_rsp)
;   rdi = pointer to old process's RSP save location
;   rsi = new process's RSP

section .text
bits 64

global switch_context

switch_context:
    ; Save callee-saved registers on current stack
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15
    
    ; Save current RSP to old process
    mov [rdi], rsp
    
    ; Load new process's RSP
    mov rsp, rsi
    
    ; Restore callee-saved registers from new stack
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    
    ; Return to new process
    ret
