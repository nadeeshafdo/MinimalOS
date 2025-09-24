[bits 16]
[org 0x7C00]

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00

    ; Save boot drive number (passed by BIOS in DL)
    mov [boot_drive], dl

    ; Print loading message
    mov si, loading_msg
    call print_string

    ; Enhanced A20 line enabling
    in al, 0x92
    test al, 2
    jnz .a20_done
    or al, 2
    out 0x92, al
.a20_done:

    ; Load kernel using detected drive number
    mov ah, 0x02
    mov al, 19      ; Sectors to read
    mov ch, 0       ; Cylinder 0
    mov cl, 2       ; Start sector 2
    mov dh, 0       ; Head 0
    mov dl, [boot_drive]  ; Use detected boot drive
    mov bx, 0x8000  ; Load to 0x8000
    int 0x13
    jc disk_error

    mov si, loaded_msg
    call print_string

    ; Check long mode support
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb no_lm
    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz no_lm

    mov si, pmode_msg
    call print_string

    ; ===== STEP 1: 16-bit Real Mode → 32-bit Protected Mode =====
    ; Load 32-bit GDT
    lgdt [gdt32_ptr]
    
    ; Enter 32-bit protected mode
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    
    ; Far jump to 32-bit protected mode
    jmp 0x08:protected_mode

print_string:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0E
    int 0x10
    jmp print_string
.done:
    ret

disk_error:
    mov si, loading_msg  ; Reuse message to save space
    call print_string
    jmp $

no_lm:
    mov si, loading_msg  ; Reuse message to save space  
    call print_string
    jmp $

; ===== STEP 2: 32-bit Protected Mode =====
[bits 32]
protected_mode:
    ; Set up 32-bit data segments
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000

    ; Setup paging for long mode at safe address
    mov edi, 0x70000    ; Use 0x70000 instead of 0x1000 to avoid conflicts
    xor eax, eax
    mov ecx, 4096 * 4   ; Clear 4 pages worth of memory (16KB total)
    rep stosd
    
    ; PML4 at 0x70000
    mov edi, 0x70000
    mov dword [edi], 0x71003      ; Point to PDP at 0x71000, present+writable
    mov dword [edi + 4], 0        ; Clear upper 32 bits
    
    ; PDP at 0x71000  
    mov edi, 0x71000
    mov dword [edi], 0x72003      ; Point to PD at 0x72000, present+writable
    mov dword [edi + 4], 0        ; Clear upper 32 bits
    
    ; PD at 0x72000 - Identity map first 2MB with 2MB pages
    mov edi, 0x72000
    mov dword [edi], 0x00000083   ; 2MB page, present+writable+page size
    mov dword [edi + 4], 0        ; Clear upper 32 bits
    
    ; Map second 2MB page (covers VGA memory at 0xB8000) 
    mov dword [edi + 8], 0x00200083   ; 2MB page starting at 2MB
    mov dword [edi + 12], 0           ; Clear upper 32 bits

    ; ===== STEP 3: 32-bit Protected Mode → 64-bit Long Mode =====
    ; Enable PAE and PGE
    mov eax, cr4
    or eax, 0xA0        ; Enable PAE (bit 5) and PGE (bit 7)
    mov cr4, eax
    
    ; Set CR3 to page tables
    mov eax, 0x70000
    mov cr3, eax
    
    ; Enable long mode in EFER MSR
    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x100       ; Set LME bit (bit 8)
    wrmsr
    
    ; Enable paging to activate long mode
    mov eax, cr0
    or eax, 0x80000000  ; Set PG bit (bit 31)
    mov cr0, eax

    ; Add a small delay after enabling paging
    nop
    nop
    nop

    ; Load 64-bit GDT and jump to long mode
    lgdt [gdt64_ptr]
    
    ; Add more delay before far jump
    nop
    nop
    
    jmp 0x08:long_mode

; Add error handler right after protected mode section
paging_error:
    jmp $  ; Infinite loop to catch paging errors

[bits 64]
long_mode:
    ; Set up 64-bit segments
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Copy kernel from 0x8000 to 0x100000
    mov rsi, 0x8000
    mov rdi, 0x100000
    mov rcx, 19 * 512 / 8  ; Copy in 8-byte chunks
    rep movsq

    ; Jump to kernel entry point (should match kernel.ld)
    mov rax, 0x100000
    jmp rax

; Data
boot_drive db 0

; Messages (shortened to save space)
loading_msg db 'MinimalOS Loading...', 13, 10, 0
loaded_msg db 'Kernel loaded.', 13, 10, 0
pmode_msg db 'Protected mode...', 13, 10, 0

; 32-bit GDT for protected mode transition
align 8
gdt32:
    dq 0x0000000000000000    ; Null descriptor
    dq 0x00CF9A000000FFFF    ; 32-bit code segment
    dq 0x00CF92000000FFFF    ; 32-bit data segment
gdt32_ptr:
    dw $ - gdt32 - 1
    dd gdt32

; 64-bit GDT for long mode
align 8
gdt64:
    dq 0x0000000000000000    ; Null descriptor
    dq 0x00209A0000000000    ; 64-bit code segment (proper long mode descriptor)
    dq 0x0000920000000000    ; 64-bit data segment
gdt64_ptr:
    dw $ - gdt64 - 1
    dq gdt64

; Bootloader signature (pad to 512 bytes total)
times 510 - ($ - $$) db 0
dw 0xAA55