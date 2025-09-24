[bits 16]
[org 0x7C00]

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00

    ; Enable A20 line
    in al, 0x92
    or al, 2
    out 0x92, al

    ; Load kernel from sector 2 to 0x1000 (19 sectors)
    mov ah, 0x02
    mov al, 19  ; Sectors to read
    mov ch, 0
    mov cl, 2   ; Start sector
    mov dh, 0
    mov bx, 0x1000
    int 0x13
    jc load_error

    ; Set up paging for long mode (identity map first 2MB)
    mov edi, 0x2000   ; PML4 at 0x2000
    mov cr3, edi
    xor eax, eax
    mov ecx, 4096 * 4  ; Clear 4 page tables
    rep stosd
    mov edi, 0x2000

    mov dword [edi], 0x3003   ; PML4[0] = PDP at 0x3000, present+rw
    add edi, 0x1000           ; PDP at 0x3000
    mov dword [edi], 0x4003   ; PDP[0] = PD at 0x4000, present+rw
    add edi, 0x1000           ; PD at 0x4000
    mov dword [edi], 0x00000083  ; PD[0] = 2MB page, present+rw+big

    ; Enable PAE and PGE
    mov eax, cr4
    or eax, 1 << 5 | 1 << 7
    mov cr4, eax

    ; Set LM bit in EFER MSR
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    ; Enable paging and protected mode
    mov eax, cr0
    or eax, 1 << 31 | 1 << 0
    mov cr0, eax

    ; Load GDT for long mode
    lgdt [gdt64_ptr]

    ; Far jump to long mode
    jmp 0x08:long_mode

load_error:
    mov ah, 0x0E
    mov al, 'E'
    int 0x10
    jmp $

[bits 64]
long_mode:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Jump to kernel entry at 0x100000
    jmp 0x100000

gdt64:
    dq 0  ; Null
    dq 0x00AF9B000000FFFF  ; Code: long mode, present, DPL=0
    dq 0x00AF93000000FFFF  ; Data: present, DPL=0
gdt64_ptr:
    dw gdt64_ptr - gdt64 - 1
    dq gdt64

times 510 - ($ - $$) db 0
dw 0xAA55