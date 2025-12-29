; MinimalOS - Interrupt Service Routine Assembly Stubs
; These stubs save registers and call C handlers

%include "include/asm/defines.inc"

section .text
bits 64

; External C handlers
extern isr_handler
extern irq_handler

; Macro for ISR without error code
%macro ISR_NOERRCODE 1
global isr%1
isr%1:
    push qword 0                ; Dummy error code
    push qword %1               ; Interrupt number
    jmp isr_common_stub
%endmacro

; Macro for ISR with error code (pushed by CPU)
%macro ISR_ERRCODE 1
global isr%1
isr%1:
    push qword %1               ; Interrupt number (error code already on stack)
    jmp isr_common_stub
%endmacro

; Exception handlers (0-31)
ISR_NOERRCODE 0     ; Divide Error
ISR_NOERRCODE 1     ; Debug
ISR_NOERRCODE 2     ; NMI
ISR_NOERRCODE 3     ; Breakpoint
ISR_NOERRCODE 4     ; Overflow
ISR_NOERRCODE 5     ; Bound Range Exceeded
ISR_NOERRCODE 6     ; Invalid Opcode
ISR_NOERRCODE 7     ; Device Not Available
ISR_ERRCODE   8     ; Double Fault (has error code)
ISR_NOERRCODE 9     ; Coprocessor Segment Overrun
ISR_ERRCODE   10    ; Invalid TSS (has error code)
ISR_ERRCODE   11    ; Segment Not Present (has error code)
ISR_ERRCODE   12    ; Stack-Segment Fault (has error code)
ISR_ERRCODE   13    ; General Protection Fault (has error code)
ISR_ERRCODE   14    ; Page Fault (has error code)
ISR_NOERRCODE 15    ; Reserved
ISR_NOERRCODE 16    ; x87 FPU Error
ISR_ERRCODE   17    ; Alignment Check (has error code)
ISR_NOERRCODE 18    ; Machine Check
ISR_NOERRCODE 19    ; SIMD Exception
ISR_NOERRCODE 20    ; Virtualization Exception
ISR_ERRCODE   21    ; Control Protection (has error code)
ISR_NOERRCODE 22    ; Reserved
ISR_NOERRCODE 23    ; Reserved
ISR_NOERRCODE 24    ; Reserved
ISR_NOERRCODE 25    ; Reserved
ISR_NOERRCODE 26    ; Reserved
ISR_NOERRCODE 27    ; Reserved
ISR_NOERRCODE 28    ; Reserved
ISR_NOERRCODE 29    ; Reserved
ISR_ERRCODE   30    ; Security Exception (has error code)
ISR_NOERRCODE 31    ; Reserved

; IRQ handlers (32+)
ISR_NOERRCODE 32    ; Timer
ISR_NOERRCODE 33    ; Keyboard

; Spurious interrupt handler
global isr_spurious
isr_spurious:
    iretq               ; Just return, don't send EOI

; Common ISR stub - saves context and calls C handler
isr_common_stub:
    ; Save all general purpose registers
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
    
    ; Save segment registers (for debugging)
    mov ax, ds
    push rax
    
    ; Load kernel data segment
    mov ax, DATA64_SEL
    mov ds, ax
    mov es, ax
    
    ; Call C handler with pointer to stack frame
    mov rdi, rsp                ; First argument: pointer to saved registers
    call isr_handler
    
    ; Restore segment registers
    pop rax
    mov ds, ax
    mov es, ax
    
    ; Restore all general purpose registers
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
    
    ; Remove interrupt number and error code from stack
    add rsp, 16
    
    ; Return from interrupt
    iretq
