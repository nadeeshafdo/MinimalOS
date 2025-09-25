[bits 64]
section .data
global tss
align 16
tss:
    times 104 db 0  ; Basic TSS structure (IO map not used)
    ; Set RSP0 in TSS (kernel stack for syscalls)
    ; Assume kernel stack at 0x200000
    dd 0x200000  ; RSP0 low
    dd 0         ; RSP0 high

section .text
global setup_tss
extern gdt
setup_tss:
    mov rax, tss
    mov [gdt + 40], ax   ; TSS base low
    shr rax, 16
    mov [gdt + 42], al   ; TSS base mid
    mov [gdt + 43], ah   ; TSS base high
    shr rax, 8
    mov [gdt + 47], eax  ; TSS base upper
    mov ax, 0x28         ; TSS selector
    ltr ax
    ret