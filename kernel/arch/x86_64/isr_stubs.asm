; 64-bit IDT assembly stubs
; ISR and IRQ handlers with proper register saving

section .text
bits 64

; External C handlers
extern isr_handler
extern irq_handler

; Load IDT
global idt_load
idt_load:
    lidt [rdi]
    ret

; Macro for ISR without error code
%macro ISR_NOERRCODE 1
global isr%1
isr%1:
    push 0              ; Dummy error code
    push %1             ; Interrupt number
    jmp isr_common
%endmacro

; Macro for ISR with error code
%macro ISR_ERRCODE 1
global isr%1
isr%1:
    ; Error code already pushed by CPU
    push %1             ; Interrupt number
    jmp isr_common
%endmacro

; Macro for IRQ
%macro IRQ 2
global irq%1
irq%1:
    push 0              ; Dummy error code
    push %2             ; IRQ number
    jmp irq_common
%endmacro

; CPU Exceptions (ISR 0-31)
ISR_NOERRCODE 0         ; Divide by zero
ISR_NOERRCODE 1         ; Debug
ISR_NOERRCODE 2         ; NMI
ISR_NOERRCODE 3         ; Breakpoint
ISR_NOERRCODE 4         ; Overflow
ISR_NOERRCODE 5         ; Bound range exceeded
ISR_NOERRCODE 6         ; Invalid opcode
ISR_NOERRCODE 7         ; Device not available
ISR_ERRCODE   8         ; Double fault
ISR_NOERRCODE 9         ; Coprocessor segment overrun
ISR_ERRCODE   10        ; Invalid TSS
ISR_ERRCODE   11        ; Segment not present
ISR_ERRCODE   12        ; Stack-segment fault
ISR_ERRCODE   13        ; General protection fault
ISR_ERRCODE   14        ; Page fault
ISR_NOERRCODE 15        ; Reserved
ISR_NOERRCODE 16        ; x87 FPU error
ISR_ERRCODE   17        ; Alignment check
ISR_NOERRCODE 18        ; Machine check
ISR_NOERRCODE 19        ; SIMD floating-point
ISR_NOERRCODE 20        ; Virtualization
ISR_NOERRCODE 21        ; Reserved
ISR_NOERRCODE 22        ; Reserved
ISR_NOERRCODE 23        ; Reserved
ISR_NOERRCODE 24        ; Reserved
ISR_NOERRCODE 25        ; Reserved
ISR_NOERRCODE 26        ; Reserved
ISR_NOERRCODE 27        ; Reserved
ISR_NOERRCODE 28        ; Reserved
ISR_NOERRCODE 29        ; Reserved
ISR_ERRCODE   30        ; Security exception
ISR_NOERRCODE 31        ; Reserved

; Hardware IRQs (remapped to 32-47)
IRQ 0, 0                ; Timer
IRQ 1, 1                ; Keyboard
IRQ 2, 2                ; Cascade
IRQ 3, 3                ; COM2
IRQ 4, 4                ; COM1
IRQ 5, 5                ; LPT2
IRQ 6, 6                ; Floppy
IRQ 7, 7                ; LPT1 / Spurious
IRQ 8, 8                ; RTC
IRQ 9, 9                ; Free
IRQ 10, 10              ; Free
IRQ 11, 11              ; Free
IRQ 12, 12              ; PS/2 Mouse
IRQ 13, 13              ; FPU
IRQ 14, 14              ; Primary ATA
IRQ 15, 15              ; Secondary ATA

; Common ISR handler
isr_common:
    ; Save all general-purpose registers
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    ; Get interrupt number and error code from stack
    mov rdi, [rsp + 120]    ; Interrupt number (after 15 pushes = 120 bytes)
    mov rsi, [rsp + 128]    ; Error code

    ; Call C handler
    call isr_handler

    ; Restore registers
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax

    ; Remove error code and interrupt number
    add rsp, 16

    ; Return from interrupt
    iretq

; Common IRQ handler
irq_common:
    ; Save all general-purpose registers
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    ; Get IRQ number from stack
    mov rdi, [rsp + 120]    ; IRQ number

    ; Call C handler
    call irq_handler

    ; Restore registers
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax

    ; Remove error code and IRQ number
    add rsp, 16

    ; Return from interrupt
    iretq
