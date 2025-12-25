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
    
    # Setup kernel stack (higher-half address)
    mov $kernel_stack_top, %rsp
    
    # Clear frame pointer for stack traces
    xor %rbp, %rbp
    
    # Save multiboot info pointer (still in RDI from 32-bit code)
    # RDI is the first argument in x86_64 calling convention
    
    # Call kernel main (multiboot info pointer is in RDI)
    call kernel_main
    
    # If kernel_main returns, halt
halt_loop:
    cli
    hlt
    jmp halt_loop

# Kernel stack (64KB) in higher-half
.section .bss
.align 16
kernel_stack_bottom:
    .skip 65536
kernel_stack_top:
