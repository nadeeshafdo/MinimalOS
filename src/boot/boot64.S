; src/boot/boot64.S
; 64-bit Entry Point (NASM Syntax)

global long_mode_entry
extern kernel_main
extern stack_top
extern multiboot_info_ptr

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

    ; Pass Multiboot2 info pointer to kernel_main
    ; value is stored in multiboot_info_ptr
    ; access it using RIP-relative addressing (default in NASM for 64-bit if simple symbol?)
    ; Actually NASM 64-bit absolute addressing is default for simple symbols unless 'default rel' used?
    ; 'mcmodel=kernel' in GCC assumes lower 2GB or negative 2GB.
    ; We are in negative 2GB (0xFFFFFFFF...).
    ; So absolute 32-bit signed displacement works?
    ; Or RIP relative.
    ; Let's force RIP relative or just use `mov edi, [multiboot_info_ptr]`
    
    mov edi, [multiboot_info_ptr]
    ; RDI now holds the physical address of Multiboot Info

    call kernel_main

    cli
.loop:
    hlt
    jmp .loop
