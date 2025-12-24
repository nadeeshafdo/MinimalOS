; User mode entry for x86_64
; Uses iretq to switch from Ring 0 to Ring 3

section .text
bits 64

; User segment selectors (with RPL=3)
USER_CODE_SEG equ 0x18 | 3   ; 0x1B
USER_DATA_SEG equ 0x20 | 3   ; 0x23

; void user_mode_enter(uint64_t entry, uint64_t user_stack)
; RDI = user entry point
; RSI = user stack pointer
global user_mode_enter
user_mode_enter:
    ; Disable interrupts during transition
    cli
    
    ; Set up stack frame for iretq
    ; Stack must contain (bottom to top):
    ;   SS, RSP, RFLAGS, CS, RIP
    
    mov rax, USER_DATA_SEG
    push rax                ; SS
    push rsi                ; RSP (user stack)
    pushfq                  ; RFLAGS
    pop rax
    or rax, 0x200           ; Enable interrupts in user mode
    push rax
    mov rax, USER_CODE_SEG
    push rax                ; CS
    push rdi                ; RIP (entry point)
    
    ; Clear general purpose registers for security
    xor rax, rax
    xor rbx, rbx
    xor rcx, rcx
    xor rdx, rdx
    xor rsi, rsi
    xor rdi, rdi
    xor rbp, rbp
    xor r8, r8
    xor r9, r9
    xor r10, r10
    xor r11, r11
    xor r12, r12
    xor r13, r13
    xor r14, r14
    xor r15, r15
    
    ; Switch to user mode
    iretq
