; MinimalOS - 32-bit to 64-bit Long Mode Transition
; Sets up paging and transitions to 64-bit mode

%include "include/asm/defines.inc"

; For 32-bit code, we use position-independent addressing or ensure
; page tables are in low memory
section .boot32
bits 32

extern gdt_load32
extern kernel_main
extern boot_stack_top
global long_mode_init

; GDT symbols from gdt.asm
extern gdt_start

long_mode_init:
    ; ESI contains multiboot2 info pointer (saved from _start)
    ; Save it for later
    push esi

    ; Load 32-bit GDT first
    call gdt_load32

    ; Check for CPUID support
    call check_cpuid
    test eax, eax
    jz .no_cpuid

    ; Check for long mode support
    call check_long_mode
    test eax, eax
    jz .no_long_mode

    ; Setup initial page tables (in low memory section)
    call setup_page_tables

    ; Enable PAE (Physical Address Extension)
    mov eax, cr4
    or eax, CR4_PAE
    mov cr4, eax

    ; Load PML4 address into CR3 (pml4 is in .boot_bss at low address)
    mov eax, boot_pml4
    mov cr3, eax

    ; Enable long mode by setting LME bit in EFER MSR
    mov ecx, IA32_EFER
    rdmsr
    or eax, IA32_EFER_LME | IA32_EFER_NXE   ; Enable long mode and NX bit
    wrmsr

    ; Enable paging (this activates long mode)
    mov eax, cr0
    or eax, CR0_PG | CR0_WP                 ; Enable paging and write protect
    mov cr0, eax

    ; Retrieve multiboot info pointer
    pop esi

    ; Load 64-bit GDT and far jump to 64-bit trampoline (in same low section)
    lgdt [gdt64_ptr_phys]
    jmp CODE64_SEL:trampoline_64

.no_cpuid:
    mov dword [0xB8000], 0x4F434F4E         ; "NC" - No CPUID
    hlt
    jmp $

.no_long_mode:
    mov dword [0xB8000], 0x4F4C4F4E         ; "NL" - No Long mode
    hlt
    jmp $

; Check CPUID instruction support by flipping ID bit in EFLAGS
check_cpuid:
    pushfd
    pop eax
    mov ecx, eax
    xor eax, 0x200000                       ; Flip ID bit
    push eax
    popfd
    pushfd
    pop eax
    push ecx
    popfd
    xor eax, ecx
    ret                                      ; EAX != 0 if CPUID supported

; Check for long mode (64-bit) support
check_long_mode:
    ; Check if extended CPUID is available
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb .no_long_mode_check

    ; Check if long mode is available
    mov eax, 0x80000001
    cpuid
    test edx, (1 << 29)                     ; LM bit
    jz .no_long_mode_check

    mov eax, 1
    ret

.no_long_mode_check:
    xor eax, eax
    ret

; Setup initial identity and higher-half page tables
; Uses 2MB huge pages for simplicity
; Page tables are in .boot_bss section which is in low memory
setup_page_tables:
    ; Clear page table area
    mov edi, boot_pml4
    mov ecx, 4096 * 4                       ; 4 pages
    xor eax, eax
    rep stosb

    ; PML4[0] -> PDPT_low (identity map)
    mov eax, boot_pdpt_low
    or eax, PTE_PRESENT | PTE_WRITABLE
    mov [boot_pml4], eax

    ; PML4[511] -> PDPT_high (higher half: 0xFFFF8000_00000000 and 0xFFFFFFFF_80000000)
    mov eax, boot_pdpt_high
    or eax, PTE_PRESENT | PTE_WRITABLE
    mov [boot_pml4 + 511 * 8], eax

    ; PDPT_low[0] -> PD (first 1GB identity mapped)
    mov eax, boot_pd
    or eax, PTE_PRESENT | PTE_WRITABLE
    mov [boot_pdpt_low], eax

    ; PDPT_high[510] -> PD (kernel at -2GB = 0xFFFFFFFF_80000000)
    ; The address 0xFFFFFFFF_80000000 has PML4 index 511, PDPT index 510
    mov eax, boot_pd
    or eax, PTE_PRESENT | PTE_WRITABLE
    mov [boot_pdpt_high + 510 * 8], eax

    ; Fill PD with 2MB huge pages (512 entries = 1GB)
    mov edi, boot_pd
    mov eax, PTE_PRESENT | PTE_WRITABLE | PTE_HUGE
    mov ecx, 512
.fill_pd:
    mov [edi], eax
    add eax, 0x200000                       ; Next 2MB page
    add edi, 8
    loop .fill_pd

    ret

; 64-bit GDT pointer (at physical/low address for transition)
; GDT has 72 bytes (see gdt.asm), limit = 71
gdt64_ptr_phys:
    dw 0x47                         ; GDT limit (72 bytes - 1)
    dq gdt_start                    ; Base address

; =============================================================================
; 64-bit trampoline - in boot32 section so it's at low address
; This code then jumps to the high address kernel code
; =============================================================================
bits 64

trampoline_64:
    ; We're now in 64-bit long mode at a low physical address
    ; Setup segment registers
    mov ax, DATA64_SEL
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Zero-extend multiboot info pointer
    ; ESI still has the value from 32-bit code
    mov edi, esi
    xor rsi, rsi
    mov esi, edi

    ; Now jump to the kernel code at high address using indirect jump
    ; We need to use a 64-bit absolute jump
    mov rax, long_mode_high
    jmp rax

; Boot-time page tables in low memory
; This section stays at low physical addresses
section .boot_bss nobits alloc write
align 4096

boot_pml4:
    resb 4096

boot_pdpt_low:
    resb 4096

boot_pdpt_high:
    resb 4096

boot_pd:
    resb 4096

; =============================================================================
; High address kernel code - in .text section at higher half
; =============================================================================
section .text
bits 64

global long_mode_high
long_mode_high:
    ; We're now at the high virtual address

    ; Setup kernel stack
    mov rsp, kernel_stack_top

    ; Clear BSS section
    extern _bss_start
    extern _bss_end
    mov rdi, _bss_start
    mov rcx, _bss_end
    sub rcx, rdi
    shr rcx, 3                      ; Divide by 8 for qword count
    xor eax, eax
    rep stosq

    ; Call kernel main (RDI has multiboot info address from RSI)
    ; ESI was saved in trampoline, but we need to re-get it from the lower bits
    mov rdi, rsi                    ; Multiboot info physical address
    and rdi, 0xFFFFFFFF             ; Ensure it's a valid physical address
    call kernel_main

    ; Should never return, but halt if it does
.halt:
    cli
    hlt
    jmp .halt

; Kernel stack in BSS (at high address)
section .bss
align 16
kernel_stack_bottom:
    resb KERNEL_STACK_SIZE
kernel_stack_top:
