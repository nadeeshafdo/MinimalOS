[bits 32]

; Multiboot header constants
MULTIBOOT_MAGIC     equ 0x1BADB002
MULTIBOOT_FLAGS     equ 0x00000003  ; Page align + memory info
MULTIBOOT_CHECKSUM  equ -(MULTIBOOT_MAGIC + MULTIBOOT_FLAGS)

section .multiboot
align 4
    dd MULTIBOOT_MAGIC
    dd MULTIBOOT_FLAGS
    dd MULTIBOOT_CHECKSUM

section .bss
align 16
stack_bottom:
    resb 16384  ; 16 KB stack
stack_top:

section .text
global _start
_start:
    ; Set up stack
    mov esp, stack_top
    
    ; Call kernel main
    extern kernel_main
    call kernel_main
    
    ; Hang if kernel returns
    cli
.hang:
    hlt
    jmp .hang
