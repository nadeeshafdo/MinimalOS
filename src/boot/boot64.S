; src/boot/boot64.S
; 64-bit Entry Point (NASM Syntax)

global long_mode_entry
extern kernel_main
extern stack_top
extern multiboot_info_ptr
extern multiboot_magic

section .text
bits 64

long_mode_entry:
    ; Reload data segment selectors
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Update stack pointer to higher-half virtual address
    mov rsp, stack_top

    ; Jump to higher half
    mov rax, higher_half_start
    jmp rax

higher_half_start:
    ; Now running at higher-half address

    ; Pass Multiboot2 info pointer to kernel_main (1st arg, RDI)
    mov edi, [multiboot_info_ptr]
    
    ; Pass Multiboot2 magic number to kernel_main (2nd arg, RSI)
    mov esi, [multiboot_magic]

    call kernel_main

    cli
.loop:
    hlt
    jmp .loop
