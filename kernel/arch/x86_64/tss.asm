; TSS loading for x86_64
; Adds TSS descriptor to GDT and loads Task Register

section .data

; Extended GDT with TSS descriptor
align 16
gdt64_with_tss:
    dq 0                        ; 0x00: Null descriptor
    dq 0x00AF9A000000FFFF       ; 0x08: Kernel Code (64-bit)
    dq 0x00CF92000000FFFF       ; 0x10: Kernel Data
    dq 0x00AFFA000000FFFF       ; 0x18: User Code (64-bit, DPL=3)
    dq 0x00CFF2000000FFFF       ; 0x20: User Data (DPL=3)
.tss_low:
    dq 0                        ; 0x28: TSS low (filled at runtime)
.tss_high:
    dq 0                        ; 0x30: TSS high (filled at runtime)
gdt64_with_tss_end:

gdt64_ptr_tss:
    dw gdt64_with_tss_end - gdt64_with_tss - 1
    dq gdt64_with_tss

section .text
bits 64

; void gdt_load_tss(uint64_t tss_addr)
global gdt_load_tss
gdt_load_tss:
    ; RDI = TSS address
    
    ; Build TSS descriptor (16 bytes for 64-bit TSS)
    ; Low 8 bytes: limit[15:0], base[23:0], type, DPL, P, limit[19:16], base[31:24]
    ; High 8 bytes: base[63:32], reserved
    
    mov rax, rdi            ; TSS base address
    
    ; TSS size = 104 bytes (0x67 limit)
    mov rcx, 0x67           ; Limit
    
    ; Build low descriptor
    ; Bits 0-15: limit[15:0]
    ; Bits 16-39: base[23:0]
    ; Bits 40-43: type (0x9 = Available 64-bit TSS)
    ; Bit 44: 0
    ; Bits 45-46: DPL (0)
    ; Bit 47: Present (1)
    ; Bits 48-51: limit[19:16]
    ; Bits 52-55: 0
    ; Bits 56-63: base[31:24]
    
    mov rdx, rcx            ; limit[15:0]
    and rdx, 0xFFFF
    
    mov r8, rax             ; base[15:0]
    and r8, 0xFFFF
    shl r8, 16
    or rdx, r8
    
    mov r8, rax             ; base[23:16]
    shr r8, 16
    and r8, 0xFF
    shl r8, 32
    or rdx, r8
    
    mov r8, 0x89            ; Type=0x9 (TSS), Present=1
    shl r8, 40
    or rdx, r8
    
    mov r8, rax             ; base[31:24]
    shr r8, 24
    and r8, 0xFF
    shl r8, 56
    or rdx, r8
    
    ; Store low descriptor
    mov [gdt64_with_tss.tss_low], rdx
    
    ; Build high descriptor (base[63:32] and reserved)
    mov rdx, rax
    shr rdx, 32             ; base[63:32]
    mov [gdt64_with_tss.tss_high], rdx
    
    ; Load new GDT
    lgdt [gdt64_ptr_tss]
    
    ; Load Task Register with TSS selector (0x28)
    mov ax, 0x28
    ltr ax
    
    ret
