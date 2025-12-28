; Multiboot2 Header for MinimalOS (NASM Syntax)

section .multiboot2
align 8
header_start:
    dd 0xE85250D6                ; Magic number
    dd 0                         ; Architecture (0 = i386 protected mode)
    dd header_end - header_start ; Header length
    dd -(0xE85250D6 + 0 + (header_end - header_start)) ; Checksum

    ; End Tag
    align 8
    dw 0    ; Type
    dw 0    ; Flags
    dd 8    ; Size
header_end:
