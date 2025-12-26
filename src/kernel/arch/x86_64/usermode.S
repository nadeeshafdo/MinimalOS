// Assembly helper to enter user mode
// Uses iretq to change privilege level from Ring 0 to Ring 3

.section .text
.global enter_userspace

# void enter_userspace(u64 entry, u64 stack)
# RDI = entry point (RIP)
# RSI = stack top (RSP)

enter_userspace:
    # Disable interrupts during transition
    cli
    
    # Setup user data segments (0x23 = GDT entry 4, RPL=3)
    mov $0x23, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    
    # Build iretq frame on kernel stack
    # iretq expects: SS, RSP, RFLAGS, CS, RIP
    
    push $0x23          # SS (user data segment)
    push %rsi           # RSP (user stack)
    pushfq              # RFLAGS (current flags)
    pop %rax
    or $0x200, %rax     # Enable interrupts (IF flag)
    push %rax           # RFLAGS with IF set
    push $0x1B          # CS (user code segment, GDT entry 3, RPL=3)
    push %rdi           # RIP (entry point)
    
    # Zero out all general-purpose registers for clean entry
    xor %rax, %rax
    xor %rbx, %rbx
    xor %rcx, %rcx
    xor %rdx, %rdx
    xor %rsi, %rsi
    xor %rdi, %rdi
    xor %rbp, %rbp
    xor %r8, %r8
    xor %r9, %r9
    xor %r10, %r10
    xor %r11, %r11
    xor %r12, %r12
    xor %r13, %r13
    xor %r14, %r14
    xor %r15, %r15
    
    # Return to user mode (Ring 3)
    # This pops CS:RIP, RFLAGS, SS:RSP from stack
    iretq
