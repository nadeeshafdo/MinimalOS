; MinimalOS - Multiboot2 Header and 32-bit Entry Point
; This file is loaded by GRUB and starts in 32-bit protected mode

%include "include/asm/defines.inc"

; Multiboot2 header - must be in first 32KB of kernel image
section .multiboot2
align 8

; Calculate header length and checksum locally
%define MB2_HEADER_LEN  (multiboot2_header_end - multiboot2_header_start)
%define MB2_CHECKSUM    (-(MULTIBOOT2_MAGIC + MULTIBOOT2_ARCH_I386 + MB2_HEADER_LEN))

multiboot2_header_start:
    dd MULTIBOOT2_MAGIC             ; Magic number
    dd MULTIBOOT2_ARCH_I386         ; Architecture (i386 32-bit)
    dd MB2_HEADER_LEN               ; Header length
    dd MB2_CHECKSUM                 ; Checksum

    ; Information request tag
    align 8
    dw 1                            ; Type: information request
    dw 0                            ; Flags
    dd 24                           ; Size
    dd 6                            ; Request memory map
    dd 14                           ; Request ACPI old RSDP
    dd 15                           ; Request ACPI new RSDP
    dd 0                            ; Padding

    ; Framebuffer tag (optional, request text mode)
    align 8
    dw 5                            ; Type: framebuffer
    dw 0                            ; Flags (not required)
    dd 20                           ; Size
    dd 80                           ; Width
    dd 25                           ; Height
    dd 0                            ; Depth (text mode)

    ; Module alignment tag
    align 8
    dw 6                            ; Type: module alignment
    dw 0                            ; Flags
    dd 8                            ; Size

    ; End tag
    align 8
    dw 0                            ; Type: end
    dw 0                            ; Flags
    dd 8                            ; Size
multiboot2_header_end:

; 32-bit boot code section (runs before long mode transition)
section .boot32
bits 32

global _start
extern long_mode_init

_start:
    ; Disable interrupts
    cli

    ; Save multiboot2 info pointer and magic
    mov edi, eax                    ; Magic in EDI
    mov esi, ebx                    ; Info pointer in ESI

    ; Verify multiboot2 magic
    cmp edi, MULTIBOOT2_BOOTLOADER_MAGIC
    jne .no_multiboot

    ; Setup temporary stack
    mov esp, boot_stack_top

    ; Clear direction flag
    cld

    ; Jump to long mode initialization
    jmp long_mode_init

.no_multiboot:
    ; Display error and halt
    mov dword [0xB8000], 0x4F524F45     ; "ER"
    mov dword [0xB8004], 0x4F3A4F52     ; "R:"
    mov dword [0xB8008], 0x4F424F4D     ; "MB"
    hlt
    jmp $

; Boot stack (used during 32-bit initialization) - must be in low memory
section .boot_bss nobits alloc write
align 16
boot_stack_bottom:
    resb KERNEL_STACK_SIZE
boot_stack_top:

; Export stack symbols for use in long_mode.asm
global boot_stack_top
