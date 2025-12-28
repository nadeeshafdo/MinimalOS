; src/boot/boot.S
; 32-bit Boot Trampoline (NASM Syntax)

global _start
global pml4_table
global pdpt_table
global pd_table
global stack_top
global multiboot_info_ptr
global gdt64_pointer

extern long_mode_entry

KERNEL_VIRTUAL_BASE equ 0xFFFFFFFF80000000

section .text
bits 32

_start:
    cli

    mov esp, (stack_top - KERNEL_VIRTUAL_BASE)

    ; Save Multiboot2 info pointer
    mov edi, (multiboot_info_ptr - KERNEL_VIRTUAL_BASE)
    mov [edi], ebx  ; Store ebx (pointer) into variable

    ; Save Multiboot2 magic number
    mov edi, (multiboot_magic - KERNEL_VIRTUAL_BASE)
    mov [edi], eax  ; Store eax (magic) into variable

    ; 1. Map PD entries (Huge pages)
    ; PD[0] maps 0-2MB
    mov eax, 0x83           ; Present | Write | Huge
    mov [(pd_table - KERNEL_VIRTUAL_BASE)], eax

    ; PD[1] maps 2MB-4MB
    add eax, 0x200000
    mov [(pd_table - KERNEL_VIRTUAL_BASE) + 8], eax

    ; 2. Map PDPT entries
    ; PDPT[0] -> PD (For identity map)
    mov eax, (pd_table - KERNEL_VIRTUAL_BASE)
    or eax, 0x3             ; Present | Write
    mov [(pdpt_table - KERNEL_VIRTUAL_BASE)], eax

    ; PDPT[510] -> PD (For higher half map)
    mov [(pdpt_table - KERNEL_VIRTUAL_BASE) + 4080], eax

    ; 3. Map PML4 entries
    ; PML4[0] -> PDPT
    mov eax, (pdpt_table - KERNEL_VIRTUAL_BASE)
    or eax, 0x3
    mov [(pml4_table - KERNEL_VIRTUAL_BASE)], eax

    ; PML4[511] -> PDPT
    mov [(pml4_table - KERNEL_VIRTUAL_BASE) + 4088], eax

    ; 4. Enable PAE
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax

    ; 5. Load CR3
    mov eax, (pml4_table - KERNEL_VIRTUAL_BASE)
    mov cr3, eax

    ; 6. Enable Long Mode in EFER MSR
    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x100
    wrmsr

    ; 7. Enable Paging
    mov eax, cr0
    or eax, 0x80000000
    mov cr0, eax

    ; 8. Load 64-bit GDT
    lgdt [(gdt64_pointer - KERNEL_VIRTUAL_BASE)]

    ; 9. Jump to 64-bit code
    jmp 0x08:(long_mode_entry - KERNEL_VIRTUAL_BASE)

section .bss
align 4096
pml4_table:
    resb 4096
pdpt_table:
    resb 4096
pd_table:
    resb 4096
stack_bottom:
    resb 16384
stack_top:

section .data
align 4096
multiboot_info_ptr:
    dd 0
multiboot_magic:
    dd 0

section .rodata
align 8
global multiboot_magic
gdt64:
    dq 0 ; Null
gdt64_code:
    dq 0x00209A0000000000
gdt64_data:
    dq 0x0000920000000000
gdt64_pointer:
    dw $ - gdt64 - 1
    dq (gdt64 - KERNEL_VIRTUAL_BASE)
