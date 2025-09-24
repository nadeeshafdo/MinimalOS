[bits 16]
[org 0x7C00]

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00

    ; Print loading message
    mov si, loading_msg
    call print_string

    ; Enable A20 line
    in al, 0x92
    or al, 2
    out 0x92, al

    ; Load kernel from sector 2 to 0x8000 (19 sectors)
    mov ah, 0x02
    mov al, 19  ; Sectors to read
    mov ch, 0
    mov cl, 2   ; Start sector
    mov dh, 0
    mov bx, 0x8000   ; Load to 0x8000 (32KB) 
    int 0x13
    jc load_error

    ; Print loaded message
    mov si, loaded_msg
    call print_string

    ; Check for long mode support
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb no_long_mode
    
    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29  ; Test LM bit
    jz no_long_mode

    ; Print success message - last BIOS call before long mode
    mov si, success_msg
    call print_string

    ; Clear 4 pages for paging structures
    mov edi, 0x1000   ; Use 0x1000 instead of 0x2000 to avoid conflicts
    xor eax, eax
    mov ecx, 4096
    rep stosd

    ; Set up page tables (identity map first 2MB)
    mov edi, 0x1000   ; PML4 at 0x1000
    mov dword [edi], 0x2003   ; PML4[0] = PDP at 0x2000, present+rw

    mov edi, 0x2000   ; PDP at 0x2000  
    mov dword [edi], 0x3003   ; PDP[0] = PD at 0x3000, present+rw

    mov edi, 0x3000   ; PD at 0x3000
    mov dword [edi], 0x00000083  ; PD[0] = 2MB page, present+rw+big

    ; Enable PAE
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax

    ; Set page table in CR3
    mov eax, 0x1000
    mov cr3, eax

    ; Set long mode bit in EFER MSR
    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x100
    wrmsr

    ; Enable paging
    mov eax, cr0
    or eax, 0x80000001
    mov cr0, eax

    ; Load GDT
    lgdt [gdt64_ptr]

    ; Far jump to 64-bit mode
    jmp 0x08:long_mode

print_string:
    push ax
.next_char:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0E
    int 0x10
    jmp .next_char
.done:
    pop ax
    ret

load_error:
    mov si, error_msg
    call print_string
    jmp $

no_long_mode:
    mov si, no_longmode_msg
    call print_string
    jmp $

[bits 64]
long_mode:
    ; Now we're in 64-bit mode - set up segments
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Write to VGA to prove we're in 64-bit mode
    mov rax, 0xB8000
    mov byte [rax], '6'
    mov byte [rax+1], 0x2F    ; Green on white
    mov byte [rax+2], '4'
    mov byte [rax+3], 0x2F

    ; Copy kernel from 0x8000 to 0x100000 (1MB) to avoid conflicts
    mov rsi, 0x8000      ; Source (loaded at 0x8000 by disk read)
    mov rdi, 0x100000    ; Destination (1MB)
    mov rcx, 19 * 512 / 8 ; Size in qwords
    rep movsq

    ; Jump to kernel at new location
    jmp 0x100000

; String messages
loading_msg db 'MinimalOS Loading...', 13, 10, 0
loaded_msg db 'Kernel loaded successfully.', 13, 10, 0
success_msg db 'Entering long mode...', 13, 10, 0
error_msg db 'Error: Could not load kernel!', 13, 10, 0
no_longmode_msg db 'Error: Long mode not supported!', 13, 10, 0

; 64-bit GDT
align 8
gdt64:
    dq 0x0000000000000000    ; Null descriptor
    dq 0x00AF9A000000FFFF    ; Code segment (64-bit)
    dq 0x00AF92000000FFFF    ; Data segment (64-bit)
gdt64_ptr:
    dw $ - gdt64 - 1
    dq gdt64

times 510 - ($ - $$) db 0
dw 0xAA55