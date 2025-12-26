.section .text
.global syscall_entry
.extern syscall_handler_c

# Syscall Entry Point (reached via LSTAR)
# No stack switch happens automatically!
# RCX = return RIP
# R11 = saved RFLAGS

syscall_entry:
    # DEBUG: Print 'S' to serial COM1 (0x3F8)
    push %rax
    push %rdx
    mov $0x3F8, %dx
    mov $'S', %al
    out %al, %dx
    pop %rdx
    pop %rax
    
    # 1. Switch directly to kernel stack
    swapgs                      # Switch to kernel GS
    mov %rsp, %gs:0            # Save user RSP to scratch_rsp (offset 0)
    mov %gs:8, %rsp            # Load kernel RSP from kernel_stack (offset 8)
    
    # 2. Build interrupt-like frame or just save regs
    # SysV ABI: RDI, RSI, RDX, RCX, R8, R9
    # We want to preserve everything or at least C callee-saved
    
    pushq %gs:0                # Push USER RSP (restored from scratch)
    pushq %r11                 # Push USER RFLAGS
    pushq %rcx                 # Push USER RIP
    
    # Save standard registers
    push %rax
    push %rbx
    push %rcx # Saved RIP again? No this is arg4
    push %rdx
    push %rsi
    push %rdi
    push %rbp
    push %r8
    push %r9
    push %r10
    push %r11 # Saved RFLAGS again? No arg
    push %r12
    push %r13
    push %r14
    push %r15
    
    # 3. Call C handler
    # ABI: RDI=arg1, RSI=arg2, RDX=arg3, RCX=arg4, R8=arg5, R9=arg6
    # Syscall convention (Linux): RAX=sys_num, RDI=arg1, RSI=arg2, RDX=arg3, R10=arg4...
    # NOTE: RCX is used by `syscall` instruction for RIP. Linux uses R10 for 4th arg.
    # We will map standard regs to C function args.
    
    # syscall_handler_c(num, arg1, arg2, arg3)
    mov %rax, %rdi      # Arg1: syscall num
    mov %rdi, %rsi      # Arg2: arg1 (user RDI is on stack but we can use register)
    # Wait, simple mapping:
    # RDI (Handler Arg1) = RAX (Syscall Num)
    # RSI (Handler Arg2) = RDI (Syscall Arg1)
    # RDX (Handler Arg3) = RSI (Syscall Arg2)
    # RCX (Handler Arg4) = RDX (Syscall Arg3)
    
    # Reload syscall arguments from stack (Register order: RAX, RBX... R15 at RSP)
    # RSP points to R15
    # Offsets:
    # R15(0), R14(8), R13(16), R12(24), R11(32), R10(40), R9(48), R8(56), RBP(64)
    # RDI(72), RSI(80), RDX(88), RCX(96), RBX(104), RAX(112)
    
    mov 112(%rsp), %rdi # Arg1: Syscall Num (RAX)
    mov 72(%rsp), %rsi  # Arg2: a1 (User RDI)
    mov 80(%rsp), %rdx  # Arg3: a2 (User RSI)
    mov 88(%rsp), %rcx  # Arg4: a3 (User RDX)
    
    call syscall_handler_c
    
    # 4. Restore everything
    # Assume syscall_handler_c returned result in RAX? (We don't support return val yet)
    
    pop %r15
    pop %r14
    pop %r13
    pop %r12
    pop %r11
    pop %r10
    pop %r9
    pop %r8
    pop %rbp
    pop %rdi
    pop %rsi
    pop %rdx
    pop %rcx
    pop %rbx
    # pop %rax  # Don't restore RAX, return value!
    add $8, %rsp # Skip RAX slot
    
    # The stack now has: RIP, RFLAGS, RSP (pushed at entry: push RSP, push RFLAGS, push RIP)
    # Stack Top -> RIP -> RFLAGS -> RSP
    
    pop %rcx    # Pop User RIP
    pop %r11    # Pop User RFLAGS
    pop %r12    # Pop User RSP
    
    # Construct iretq frame
    push $0x23          # SS (User Data)
    push %r12           # RSP (User RSP)
    push %r11           # RFLAGS
    push $0x1B          # CS (User Code)
    push %rcx           # RIP
    
    # We are on kernel stack. iretq will pop these and switch stack.
    # Restore User GS base
    swapgs
    
    iretq
