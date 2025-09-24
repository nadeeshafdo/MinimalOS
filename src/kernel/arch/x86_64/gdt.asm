[bits 64]
section .data
global gdt_ptr
global gdt
gdt:
    dq 0  ; Null
    dq 0x00AF9B000000FFFF  ; Kernel code
    dq 0x00AF93000000FFFF  ; Kernel data
    dq 0x00AFFB000000FFFF  ; User code (DPL=3)
    dq 0x00AFF3000000FFFF  ; User data (DPL=3)
    ; TSS entry (16 bytes)
    tss_descriptor:
        dd 0
        dd 0
gdt_ptr:
    dw gdt_ptr - gdt - 1
    dq gdt

section .text
global setup_gdt
setup_gdt:
    lgdt [gdt_ptr]
    ret