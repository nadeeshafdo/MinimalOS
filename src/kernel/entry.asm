[bits 64]
section .text
global _kernel_entry
_kernel_entry:
    ; Set up a stack for the kernel - using 64-bit register
    mov rax, 0x90000  ; Load stack address into 64-bit register first
    mov rsp, rax      ; Then move to stack pointer
    
    ; Ensure segments are correct
    mov ax, 0x10      ; Data segment (segment registers are still 16-bit)
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    
    ; Clear direction flag
    cld
    
    ; Call kernel main
    extern kernel_main
    call kernel_main
    
    ; If kernel_main returns, halt
.halt:
    hlt
    jmp .halt