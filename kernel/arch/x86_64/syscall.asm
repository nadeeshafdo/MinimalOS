; syscall/sysret entry point for x86_64
; Uses the SYSCALL instruction (fast system call)

section .text
bits 64

extern syscall_handler

; MSR addresses
MSR_EFER    equ 0xC0000080
MSR_STAR    equ 0xC0000081
MSR_LSTAR   equ 0xC0000082
MSR_SFMASK  equ 0xC0000084

; Initialize SYSCALL/SYSRET
global syscall_init_asm
syscall_init_asm:
    ; Enable SYSCALL/SYSRET in EFER
    mov ecx, MSR_EFER
    rdmsr
    or eax, 1           ; Set SCE bit (SYSCALL Enable)
    wrmsr
    
    ; Set up STAR MSR: segments for SYSCALL and SYSRET
    ; STAR[31:0] = unused, STAR[47:32] = Kernel CS, STAR[63:48] = User CS base
    mov ecx, MSR_STAR
    xor eax, eax
    mov edx, 0x00180008 ; Kernel CS=0x08, User base=0x18
    wrmsr
    
    ; Set LSTAR to syscall entry point
    mov ecx, MSR_LSTAR
    lea rax, [syscall_entry]
    mov rdx, rax
    shr rdx, 32
    wrmsr
    
    ; Set SFMASK - flags to clear on SYSCALL
    mov ecx, MSR_SFMASK
    mov eax, 0x200      ; Clear IF
    xor edx, edx
    wrmsr
    
    ret

; SYSCALL entry point
; On entry: RCX = user RIP, R11 = user RFLAGS
; Syscall number in RAX, args in RDI, RSI, RDX
global syscall_entry
syscall_entry:
    ; Save return address and flags
    push rcx            ; User RIP (return address)
    push r11            ; User RFLAGS
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15
    
    ; Call C handler: syscall_handler(num, arg1, arg2, arg3)
    ; Current: RAX=num, RDI=arg1, RSI=arg2, RDX=arg3
    ; Need:    RDI=num, RSI=arg1, RDX=arg2, RCX=arg3
    mov r10, rdx        ; Save arg3
    mov rcx, r10        ; arg3 -> RCX
    mov r10, rsi        ; Save arg2
    mov rdx, r10        ; arg2 -> RDX
    mov rsi, rdi        ; arg1 -> RSI
    mov rdi, rax        ; num -> RDI
    
    call syscall_handler
    
    ; Result in RAX
    
    ; Restore registers
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    pop r11             ; User RFLAGS
    pop rcx             ; User RIP
    
    ; Re-enable interrupts before returning
    sti
    
    ; Since we're in Ring 0 only, use simple return via RCX
    ; Push return address and use ret
    push rcx
    ret
