#ifndef IDT_H
#define IDT_H

#include "../../include/types.h"

// IDT Entry structure
struct idt_entry {
    u16 offset_low;
    u16 selector;
    u8  ist;           // Interrupt Stack Table offset
    u8  type_attr;
    u16 offset_mid;
    u32 offset_high;
    u32 zero;
} __attribute__((packed));

// IDT Pointer structure
struct idt_ptr {
    u16 limit;
    u64 base;
} __attribute__((packed));

// CPU registers structure (pushed on interrupt/exception)
struct registers {
    u64 r15, r14, r13, r12, r11, r10, r9, r8;
    u64 rbp, rdi, rsi, rdx, rcx, rbx, rax;
    u64 int_no, err_code;
    u64 rip, cs, rflags, rsp, ss;
} __attribute__((packed));

typedef void (*interrupt_handler_t)(struct registers* regs);

void idt_init(void);
void idt_set_gate(u8 num, u64 handler, u16 selector, u8 flags);
void register_interrupt_handler(u8 num, interrupt_handler_t handler);

#endif // IDT_H
