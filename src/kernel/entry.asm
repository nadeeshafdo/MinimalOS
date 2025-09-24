[bits 64]
section .text
global _kernel_entry
_kernel_entry:
    extern kernel_main
    call kernel_main
    hlt