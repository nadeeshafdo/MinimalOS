[bits 64]
section .text
global _kernel_entry
_kernel_entry:
    ; Absolute minimal kernel entry - just halt
    ; If we reach here, bootloader-to-kernel jump works
    cli                     ; Disable interrupts
    hlt                     ; Halt
    jmp $-1                 ; Loop on halt