# 64-bit Long Mode Entry Point
# This code runs after the transition to 64-bit mode

.code64
.section .text.boot64

.extern kernel_main
.global long_mode_start

long_mode_start:
    # We're now in 64-bit long mode!
    
    # Reload segment selectors
    mov $0x10, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    mov %ax, %ss
    
    # Setup kernel stack
    mov $kernel_stack_top, %rsp
    
    # Clear frame pointer for stack traces
    xor %rbp, %rbp
    
    # Multiboot info pointer is in EDI (from 32-bit code)
    # Zero-extend EDI to RDI for 64-bit (clear upper 32 bits)
    mov %edi, %edi
    # Now RDI contains the multiboot info pointer as first argument
    
    # Call kernel main (multiboot info pointer is in RDI)
    call kernel_main
    
    # If kernel_main returns, halt
halt_loop:
    cli
    hlt
    jmp halt_loop

# Kernel stack (64KB)
.section .bss
.align 16
kernel_stack_bottom:
    .skip 65536
kernel_stack_top:
