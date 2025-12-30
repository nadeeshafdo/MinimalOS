; MinimalOS - GDT Setup
; Defines both 32-bit and 64-bit segment descriptors
; GDT must be in low memory for 32-bit boot code to access it

%include "include/asm/defines.inc"

; Place GDT in boot32 section which stays at low physical addresses
section .boot32

align 16

; Global Descriptor Table
global gdt_start
global gdt_ptr
global gdt64_ptr
global gdt_end

gdt_start:
    ; Null descriptor (0x00)
    dq 0x0000000000000000

    ; 32-bit code segment (0x08)
    ; Base=0, Limit=0xFFFFF, Access=0x9A (present, ring 0, code), Flags=0xC (4KB granularity, 32-bit)
    dw 0xFFFF                       ; Limit 0:15
    dw 0x0000                       ; Base 0:15
    db 0x00                         ; Base 16:23
    db 10011010b                    ; Access: P=1, DPL=0, S=1, Type=1010 (exec/read)
    db 11001111b                    ; Flags: G=1, D=1, L=0, Limit 16:19=0xF
    db 0x00                         ; Base 24:31

    ; 32-bit data segment (0x10)
    ; Base=0, Limit=0xFFFFF, Access=0x92 (present, ring 0, data), Flags=0xC
    dw 0xFFFF                       ; Limit 0:15
    dw 0x0000                       ; Base 0:15
    db 0x00                         ; Base 16:23
    db 10010010b                    ; Access: P=1, DPL=0, S=1, Type=0010 (read/write)
    db 11001111b                    ; Flags: G=1, D=1, L=0, Limit 16:19=0xF
    db 0x00                         ; Base 24:31

    ; 64-bit code segment (0x18)
    ; In long mode, base and limit are ignored. L=1, D=0 for 64-bit.
    dw 0x0000                       ; Limit 0:15 (ignored)
    dw 0x0000                       ; Base 0:15 (ignored)
    db 0x00                         ; Base 16:23 (ignored)
    db 10011010b                    ; Access: P=1, DPL=0, S=1, Type=1010 (exec/read)
    db 00100000b                    ; Flags: G=0, D=0, L=1, Limit 16:19=0
    db 0x00                         ; Base 24:31 (ignored)

    ; 64-bit data segment (0x20)
    dw 0x0000                       ; Limit 0:15 (ignored)
    dw 0x0000                       ; Base 0:15 (ignored)
    db 0x00                         ; Base 16:23 (ignored)
    db 10010010b                    ; Access: P=1, DPL=0, S=1, Type=0010 (read/write)
    db 00000000b                    ; Flags: G=0, D=0, L=0
    db 0x00                         ; Base 24:31 (ignored)

    ; 64-bit user code segment (0x28) - Ring 3
    dw 0x0000
    dw 0x0000
    db 0x00
    db 11111010b                    ; Access: P=1, DPL=3, S=1, Type=1010
    db 00100000b                    ; Flags: L=1
    db 0x00

    ; 64-bit user data segment (0x30) - Ring 3
    dw 0x0000
    dw 0x0000
    db 0x00
    db 11110010b                    ; Access: P=1, DPL=3, S=1, Type=0010
    db 00000000b
    db 0x00

    ; TSS descriptor placeholder (0x38) - 16 bytes in 64-bit mode
    ; Will be filled in at runtime
    dq 0x0000000000000000
    dq 0x0000000000000000
gdt_end:

; GDT pointer for 32-bit mode (6 bytes: 2-byte limit + 4-byte base)
gdt_ptr:
    dw gdt_end - gdt_start - 1      ; Limit = 71
    dd gdt_start                     ; Base (32-bit physical address)

; GDT pointer for 64-bit mode (10 bytes: 2-byte limit + 8-byte base)
; Note: After transition to 64-bit, we'll need to reload with virtual address
gdt64_ptr:
    dw gdt_end - gdt_start - 1      ; Limit
    dq gdt_start                     ; Base (will work after paging maps identity)

bits 32

; Load the 32-bit GDT
global gdt_load32
gdt_load32:
    lgdt [gdt_ptr]
    ; Reload segment registers
    mov ax, DATA32_SEL
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    ret

bits 64

; Load the 64-bit GDT (called after transition to long mode)
global gdt_load64
gdt_load64:
    lgdt [gdt64_ptr]
    ret
