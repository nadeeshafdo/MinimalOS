; MinimalOS - System Call Entry Point
; Handles transition from Ring 3 to Ring 0 via SYSCALL instruction

global syscall_entry
extern syscall_handler

section .text
bits 64

; Per-CPU data offsets (must match struct per_cpu_data in cpu.h)
%define OFF_USER_RSP     24
%define OFF_KERNEL_STACK 16

syscall_entry:
    ; 1. Swap GS base to access per-cpu kernel data
    ;    User GS base -> Saved in MSR_KERNEL_GS_BASE
    ;    Kernel GS base (bsp_cpu_data) -> Loaded into MSR_GS_BASE
    swapgs
    
    ; 2. Save user stack pointer to per-cpu data
    mov [gs:OFF_USER_RSP], rsp
    
    ; 3. Load kernel stack pointer from per-cpu data
    mov rsp, [gs:OFF_KERNEL_STACK]
    
    ; 4. Establish kernel interrupt stack frame (legacy/convention)
    ;    or just save registers.
    ;    SysV ABI: RDI, RSI, RDX, RCX, R8, R9 are args.
    ;    SYSCALL: RCX=RIP, R11=RFLAGS.
    
    ; We need to preserve RCX and R11 because they hold return info.
    push r11    ; User RFLAGS
    push rcx    ; User RIP
    push rbp
    
    ; Save other callee-preserved registers (as per C ABI)
    push rbx
    push r12
    push r13
    push r14
    push r15
    
    ; 5. Setup arguments for C handler
    ;    User: RDI, RSI, RDX, R10, R8, R9
    ;    Kernel: RDI, RSI, RDX, RCX, R8, R9
    ;    Note: User arg4 is in R10, Kernel arg4 expects RCX.
    ;    RAX holds syscall number.
    
    ; Save user arg4 (R10) to RCX (arg4 for C)
    mov rcx, r10
    
    ; The rest match (RDI, RSI, RDX, R8, R9)
    ; Pass system call number as 1st argument?
    ; Or C handler takes (sys_num, arg1..arg6)?
    ; Let's assume C handler: long syscall_handler(long sys_num, long a1, long a2, long a3, long a4, long a5, long a6)
    
    ; Shift args for sys_num
    push r9      ; Save arg6 (stack arg)
    mov r9, r8   ; arg5
    mov r8, r10  ; arg4 (was R10)
    mov rcx, rdx ; arg3
    mov rdx, rsi ; arg2
    mov rsi, rdi ; arg1
    mov rdi, rax ; sys_num
    
    ; 6. Call C handler
    ;    Align stack if needed (16-byte)
    call syscall_handler
    
    ; RAX holds return value
    
    ; 7. Restore registers
    pop r9      ; Remove stack arg
    
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    pop rcx     ; User RIP
    pop r11     ; User RFLAGS
    
    ; 8. Restore user stack
    mov rsp, [gs:OFF_USER_RSP]
    
    ; 9. Restore user GS
    swapgs
    
    ; 10. Return to user mode
    o64 sysret
