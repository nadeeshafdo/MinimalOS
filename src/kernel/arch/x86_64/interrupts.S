# Interrupt Service Routines (ISRs)
# These are assembly stubs that save state and call the C interrupt handler

.section .text

# Macro for ISRs without error code
.macro ISR_NOERRCODE num
.global isr\num
isr\num:
    push $0                # Dummy error code
    push $\num             # Interrupt number
    jmp isr_common_stub
.endm

# Macro for ISRs with error code
.macro ISR_ERRCODE num
.global isr\num
isr\num:
    push $\num             # Interrupt number (error code already pushed by CPU)
    jmp isr_common_stub
.endm

# CPU Exceptions (0-31)
ISR_NOERRCODE 0    # Divide by zero
ISR_NOERRCODE 1    # Debug
ISR_NOERRCODE 2    # Non-maskable interrupt
ISR_NOERRCODE 3    # Breakpoint
ISR_NOERRCODE 4    # Overflow
ISR_NOERRCODE 5    # Bound range exceeded
ISR_NOERRCODE 6    # Invalid opcode
ISR_NOERRCODE 7    # Device not available
ISR_ERRCODE   8    # Double fault
ISR_NOERRCODE 9    # Coprocessor segment overrun
ISR_ERRCODE   10   # Invalid TSS
ISR_ERRCODE   11   # Segment not present
ISR_ERRCODE   12   # Stack segment fault
ISR_ERRCODE   13   # General protection fault
ISR_ERRCODE   14   # Page fault
ISR_NOERRCODE 15   # Reserved
ISR_NOERRCODE 16   # x87 floating point exception
ISR_ERRCODE   17   # Alignment check
ISR_NOERRCODE 18   # Machine check
ISR_NOERRCODE 19   # SIMD floating point exception
ISR_NOERRCODE 20   # Virtualization exception
ISR_NOERRCODE 21   # Reserved
ISR_NOERRCODE 22   # Reserved
ISR_NOERRCODE 23   # Reserved
ISR_NOERRCODE 24   # Reserved
ISR_NOERRCODE 25   # Reserved
ISR_NOERRCODE 26   # Reserved
ISR_NOERRCODE 27   # Reserved
ISR_NOERRCODE 28   # Reserved
ISR_NOERRCODE 29   # Reserved
ISR_ERRCODE   30   # Security exception
ISR_NOERRCODE 31   # Reserved

# IRQ handlers (32-47)
.macro IRQ num, irq_num
.global irq\irq_num
irq\irq_num:
    push $0
    push $\num
    jmp isr_common_stub
.endm

IRQ 32, 0
IRQ 33, 1
IRQ 34, 2
IRQ 35, 3
IRQ 36, 4
IRQ 37, 5
IRQ 38, 6
IRQ 39, 7
IRQ 40, 8
IRQ 41, 9
IRQ 42, 10
IRQ 43, 11
IRQ 44, 12
IRQ 45, 13
IRQ 46, 14
IRQ 47, 15

# Common ISR stub
.extern interrupt_handler

isr_common_stub:
    # Save all registers
    push %rax
    push %rbx
    push %rcx
    push %rdx
    push %rsi
    push %rdi
    push %rbp
    push %r8
    push %r9
    push %r10
    push %r11
    push %r12
    push %r13
    push %r14
    push %r15
    
    # Call C interrupt handler
    mov %rsp, %rdi         # First argument: pointer to registers struct
    call interrupt_handler
    
    # Restore all registers
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
    pop %rax
    
    # Clean up error code and interrupt number
    add $16, %rsp
    
    # Return from interrupt
    iretq

# IDT flush function
.global idt_flush
idt_flush:
    lidt (%rdi)
    ret
