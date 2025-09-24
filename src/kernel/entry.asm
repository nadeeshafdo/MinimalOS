[bits 64]
section .text
global _kernel_entry
_kernel_entry:
    ; Set up a stack for the kernel
    mov rsp, 0x90000  ; Set stack pointer to a safe location
    
    ; Ensure segments are correct
    mov ax, 0x10      ; Data segment
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