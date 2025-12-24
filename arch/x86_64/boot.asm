; MinimalOS 64-bit - Multiboot2 Header and Boot Code
; Transitions from 32-bit protected mode (GRUB) to 64-bit long mode

; Constants
MULTIBOOT2_MAGIC equ 0xE85250D6
MULTIBOOT2_ARCH  equ 0              ; i386

section .multiboot2
align 8

; Multiboot2 header
mb2_header_start:
    dd MULTIBOOT2_MAGIC             ; Magic
    dd MULTIBOOT2_ARCH              ; Architecture
    dd mb2_header_end - mb2_header_start  ; Header length
    dd -(MULTIBOOT2_MAGIC + MULTIBOOT2_ARCH + (mb2_header_end - mb2_header_start)) ; Checksum

    ; End tag (required)
    align 8
    dw 0                            ; Type: end
    dw 0                            ; Flags
    dd 8                            ; Size
mb2_header_end:

section .bss
align 4096

; Page tables (must be page-aligned)
pml4:
    resb 4096
pdpt:
    resb 4096
pd:
    resb 4096

; Stack
align 16
stack_bottom:
    resb 65536                      ; 64KB stack
stack_top:

section .data
align 16

; 64-bit GDT
gdt64:
    dq 0                            ; Null descriptor
.code equ $ - gdt64
    ; Code segment: Present, Executable, 64-bit
    dq 0x00AF9A000000FFFF           
.data equ $ - gdt64
    ; Data segment: Present, Writable
    dq 0x00CF92000000FFFF           
.end:

gdt64_ptr:
    dw gdt64.end - gdt64 - 1        ; Limit
    dq gdt64                        ; Base (will be fixed at runtime for 64-bit)

section .text
bits 32

global _start
extern kernel_main

_start:
    ; Disable interrupts immediately
    cli

    ; Save multiboot info
    mov edi, ebx                    ; Multiboot info pointer
    mov esi, eax                    ; Multiboot magic

    ; Debug: Write 'A' to screen (confirms we got here)
    mov byte [0xB8000], 'A'
    mov byte [0xB8001], 0x0F

    ; Check CPUID availability
    pushfd
    pop eax
    mov ecx, eax
    xor eax, 0x200000               ; Flip ID bit (bit 21)
    push eax
    popfd
    pushfd
    pop eax
    push ecx
    popfd
    cmp eax, ecx
    je .no_cpuid

    ; Debug: Write 'B'
    mov byte [0xB8002], 'B'
    mov byte [0xB8003], 0x0F

    ; Check extended CPUID
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb .no_long_mode

    ; Debug: Write 'C'
    mov byte [0xB8004], 'C'
    mov byte [0xB8005], 0x0F

    ; Check long mode support
    mov eax, 0x80000001
    cpuid
    test edx, (1 << 29)             ; Long mode bit
    jz .no_long_mode

    ; Debug: Write 'D'
    mov byte [0xB8006], 'D'
    mov byte [0xB8007], 0x0F

    ; === Set up identity-mapped paging ===
    
    ; Zero out page tables
    mov edi, pml4
    xor eax, eax
    mov ecx, 0x1000                 ; 4096 bytes / 4 = 1024 dwords per table * 3 tables
    rep stosd

    mov edi, pml4
    xor eax, eax
    mov ecx, 0x1000
    rep stosd

    mov edi, pdpt
    xor eax, eax
    mov ecx, 0x1000
    rep stosd

    mov edi, pd
    xor eax, eax
    mov ecx, 0x1000
    rep stosd

    ; Debug: Write 'E'
    mov byte [0xB8008], 'E'
    mov byte [0xB8009], 0x0F

    ; PML4[0] -> PDPT
    mov eax, pdpt
    or eax, 0x03                    ; Present + Writable
    mov [pml4], eax

    ; PDPT[0] -> PD
    mov eax, pd
    or eax, 0x03                    ; Present + Writable
    mov [pdpt], eax

    ; Map first 2MB using 2MB pages (PD entries)
    ; PD[0] = 0x0 | 0x83 (Present + Writable + 2MB page)
    mov dword [pd], 0x00000083      ; First 2MB
    mov dword [pd + 4], 0x00000000  ; High 32 bits = 0

    ; Debug: Write 'F'
    mov byte [0xB800A], 'F'
    mov byte [0xB800B], 0x0F

    ; Set CR3 to PML4
    mov eax, pml4
    mov cr3, eax

    ; Debug: Write 'G'
    mov byte [0xB800C], 'G'
    mov byte [0xB800D], 0x0F

    ; Enable PAE (CR4.PAE = bit 5)
    mov eax, cr4
    or eax, (1 << 5)
    mov cr4, eax

    ; Debug: Write 'H'
    mov byte [0xB800E], 'H'
    mov byte [0xB800F], 0x0F

    ; Enable Long Mode (EFER.LME = bit 8)
    mov ecx, 0xC0000080             ; EFER MSR
    rdmsr
    or eax, (1 << 8)
    wrmsr

    ; Debug: Write 'I'
    mov byte [0xB8010], 'I'
    mov byte [0xB8011], 0x0F

    ; Enable paging (CR0.PG = bit 31)
    mov eax, cr0
    or eax, (1 << 31)
    mov cr0, eax

    ; Debug: Write 'J' - if we see this, paging worked!
    mov byte [0xB8012], 'J'
    mov byte [0xB8013], 0x0F

    ; Load 64-bit GDT
    lgdt [gdt64_ptr]

    ; Far jump to 64-bit code segment
    jmp gdt64.code:long_mode_start

.no_cpuid:
    mov byte [0xB8000], 'C'
    mov byte [0xB8001], 0x4F
    jmp .halt

.no_long_mode:
    mov byte [0xB8000], 'L'
    mov byte [0xB8001], 0x4F
    jmp .halt

.halt:
    hlt
    jmp .halt

; ============================================================
; 64-bit Long Mode Code
; ============================================================
bits 64

long_mode_start:
    ; Clear all data segment registers
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Set up 64-bit stack
    mov rsp, stack_top

    ; Debug: Write 'K' in 64-bit mode
    mov byte [0xB8014], 'K'
    mov byte [0xB8015], 0x0F

    ; Zero-extend multiboot pointers (already in EDI/ESI)
    mov rdi, rdi                    ; Clear upper bits
    mov rsi, rsi

    ; Call C kernel
    call kernel_main

    ; Halt if kernel returns
.halt64:
    cli
    hlt
    jmp .halt64
