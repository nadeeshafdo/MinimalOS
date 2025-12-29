; MinimalOS - Context Switch Assembly
; Saves/restores CPU context for task switching

section .text
bits 64

; void context_switch(struct cpu_context **old_context, struct cpu_context *new_context)
; 
; Arguments:
;   rdi = pointer to save location for old context (old task's RSP storage)
;   rsi = new context (new task's saved RSP)
;
; This function:
; 1. Saves callee-saved registers to current stack
; 2. Saves current RSP to *old_context
; 3. Loads new RSP from new_context
; 4. Restores callee-saved registers from new stack
; 5. Returns (RIP was saved on stack, so ret goes to new task)

global context_switch
context_switch:
    ; Save callee-saved registers (System V ABI)
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15
    
    ; Save current RSP to *old_context
    ; old_context is a pointer to the task's context field
    mov [rdi], rsp
    
    ; Load new RSP from new_context
    mov rsp, rsi
    
    ; Restore callee-saved registers
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    
    ; Return to new task
    ; The 'ret' instruction pops RIP from stack
    ret
