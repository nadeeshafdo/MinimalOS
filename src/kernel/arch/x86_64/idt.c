#include "idt.h"
#include "../../include/types.h"
#include "../../lib/printk.h"

#define IDT_ENTRIES 256

static struct idt_entry idt[IDT_ENTRIES];
static struct idt_ptr idt_pointer;
static interrupt_handler_t interrupt_handlers[IDT_ENTRIES];

// External assembly ISR handlers
extern void isr0(void);
extern void isr1(void);
extern void isr2(void);
extern void isr3(void);
extern void isr4(void);
extern void isr5(void);
extern void isr6(void);
extern void isr7(void);
extern void isr8(void);
extern void isr9(void);
extern void isr10(void);
extern void isr11(void);
extern void isr12(void);
extern void isr13(void);
extern void isr14(void);
extern void isr15(void);
extern void isr16(void);
extern void isr17(void);
extern void isr18(void);
extern void isr19(void);
extern void isr20(void);
extern void isr21(void);
extern void isr22(void);
extern void isr23(void);
extern void isr24(void);
extern void isr25(void);
extern void isr26(void);
extern void isr27(void);
extern void isr28(void);
extern void isr29(void);
extern void isr30(void);
extern void isr31(void);

// IRQ handlers (32-47)
extern void irq0(void);
extern void irq1(void);
extern void irq2(void);
extern void irq3(void);
extern void irq4(void);
extern void irq5(void);
extern void irq6(void);
extern void irq7(void);
extern void irq8(void);
extern void irq9(void);
extern void irq10(void);
extern void irq11(void);
extern void irq12(void);
extern void irq13(void);
extern void irq14(void);
extern void irq15(void);

extern void idt_flush(u64 idt_ptr_addr);

void idt_set_gate(u8 num, u64 handler, u16 selector, u8 flags) {
    idt[num].offset_low = handler & 0xFFFF;
    idt[num].offset_mid = (handler >> 16) & 0xFFFF;
    idt[num].offset_high = (handler >> 32) & 0xFFFFFFFF;
    idt[num].selector = selector;
    idt[num].ist = 0;
    idt[num].type_attr = flags;
    idt[num].zero = 0;
}

void idt_init(void) {
    idt_pointer.limit = (sizeof(struct idt_entry) * IDT_ENTRIES) - 1;
    idt_pointer.base = (u64)&idt;
    
    // Clear handlers
    for (i32 i = 0; i < IDT_ENTRIES; i++) {
        interrupt_handlers[i] = NULL;
    }
    
    // Remap PIC (Programmable Interrupt Controller)
    // ICW1: Initialize
    __asm__ volatile("outb %%al, %%dx" : : "a"(0x11), "d"(0x20));
    __asm__ volatile("outb %%al, %%dx" : : "a"(0x11), "d"(0xA0));
    
    // ICW2: Remap IRQ0-7 to 32-39, IRQ8-15 to 40-47
    __asm__ volatile("outb %%al, %%dx" : : "a"(0x20), "d"(0x21));
    __asm__ volatile("outb %%al, %%dx" : : "a"(0x28), "d"(0xA1));
    
    // ICW3: Setup cascade
    __asm__ volatile("outb %%al, %%dx" : : "a"(0x04), "d"(0x21));
    __asm__ volatile("outb %%al, %%dx" : : "a"(0x02), "d"(0xA1));
    
    // ICW4: 8086 mode
    __asm__ volatile("outb %%al, %%dx" : : "a"(0x01), "d"(0x21));
    __asm__ volatile("outb %%al, %%dx" : : "a"(0x01), "d"(0xA1));
    
    // Mask all interrupts initially
    __asm__ volatile("outb %%al, %%dx" : : "a"(0xFF), "d"(0x21));
    __asm__ volatile("outb %%al, %%dx" : : "a"(0xFF), "d"(0xA1));
    
    // Install ISRs (exceptions 0-31)
    idt_set_gate(0, (u64)isr0, 0x08, 0x8E);
    idt_set_gate(1, (u64)isr1, 0x08, 0x8E);
    idt_set_gate(2, (u64)isr2, 0x08, 0x8E);
    idt_set_gate(3, (u64)isr3, 0x08, 0x8E);
    idt_set_gate(4, (u64)isr4, 0x08, 0x8E);
    idt_set_gate(5, (u64)isr5, 0x08, 0x8E);
    idt_set_gate(6, (u64)isr6, 0x08, 0x8E);
    idt_set_gate(7, (u64)isr7, 0x08, 0x8E);
    idt_set_gate(8, (u64)isr8, 0x08, 0x8E);
    idt_set_gate(9, (u64)isr9, 0x08, 0x8E);
    idt_set_gate(10, (u64)isr10, 0x08, 0x8E);
    idt_set_gate(11, (u64)isr11, 0x08, 0x8E);
    idt_set_gate(12, (u64)isr12, 0x08, 0x8E);
    idt_set_gate(13, (u64)isr13, 0x08, 0x8E);
    idt_set_gate(14, (u64)isr14, 0x08, 0x8E);
    idt_set_gate(15, (u64)isr15, 0x08, 0x8E);
    idt_set_gate(16, (u64)isr16, 0x08, 0x8E);
    idt_set_gate(17, (u64)isr17, 0x08, 0x8E);
    idt_set_gate(18, (u64)isr18, 0x08, 0x8E);
    idt_set_gate(19, (u64)isr19, 0x08, 0x8E);
    idt_set_gate(20, (u64)isr20, 0x08, 0x8E);
    idt_set_gate(21, (u64)isr21, 0x08, 0x8E);
    idt_set_gate(22, (u64)isr22, 0x08, 0x8E);
    idt_set_gate(23, (u64)isr23, 0x08, 0x8E);
    idt_set_gate(24, (u64)isr24, 0x08, 0x8E);
    idt_set_gate(25, (u64)isr25, 0x08, 0x8E);
    idt_set_gate(26, (u64)isr26, 0x08, 0x8E);
    idt_set_gate(27, (u64)isr27, 0x08, 0x8E);
    idt_set_gate(28, (u64)isr28, 0x08, 0x8E);
    idt_set_gate(29, (u64)isr29, 0x08, 0x8E);
    idt_set_gate(30, (u64)isr30, 0x08, 0x8E);
    idt_set_gate(31, (u64)isr31, 0x08, 0x8E);
    
    // Install IRQ handlers (32-47)
    idt_set_gate(32, (u64)irq0, 0x08, 0x8E);
    idt_set_gate(33, (u64)irq1, 0x08, 0x8E);
    idt_set_gate(34, (u64)irq2, 0x08, 0x8E);
    idt_set_gate(35, (u64)irq3, 0x08, 0x8E);
    idt_set_gate(36, (u64)irq4, 0x08, 0x8E);
    idt_set_gate(37, (u64)irq5, 0x08, 0x8E);
    idt_set_gate(38, (u64)irq6, 0x08, 0x8E);
    idt_set_gate(39, (u64)irq7, 0x08, 0x8E);
    idt_set_gate(40, (u64)irq8, 0x08, 0x8E);
    idt_set_gate(41, (u64)irq9, 0x08, 0x8E);
    idt_set_gate(42, (u64)irq10, 0x08, 0x8E);
    idt_set_gate(43, (u64)irq11, 0x08, 0x8E);
    idt_set_gate(44, (u64)irq12, 0x08, 0x8E);
    idt_set_gate(45, (u64)irq13, 0x08, 0x8E);
    idt_set_gate(46, (u64)irq14, 0x08, 0x8E);
    idt_set_gate(47, (u64)irq15, 0x08, 0x8E);
    
    // Load IDT
    idt_flush((u64)&idt_pointer);
}

void register_interrupt_handler(u8 num, interrupt_handler_t handler) {
    interrupt_handlers[num] = handler;
}

// Common interrupt handler (called from assembly)
void interrupt_handler(struct registers* regs) {
    if (interrupt_handlers[regs->int_no] != NULL) {
        interrupt_handler_t handler = interrupt_handlers[regs->int_no];
        handler(regs);
    } else if (regs->int_no < 32) {
        // Unhandled exception!
        printk("\n[CPU] EXCEPTION %lu taking place!\n", regs->int_no);
        printk("      Error Code: %lu\n", regs->err_code);
        printk("      RIP: %lx  CS: %lx  RFLAGS: %lx\n", regs->rip, regs->cs, regs->rflags);
        printk("      RSP: %lx  SS: %lx\n", regs->rsp, regs->ss);
        
        if (regs->int_no == 14) {
            u64 cr2;
            __asm__ volatile("mov %%cr2, %0" : "=r"(cr2));
            printk("      CR2 (Fault Addr): %lx\n", cr2);
        }
        
        // Halt
        printk("[CPU] System Halted.\n");
        while(1) { __asm__ volatile("hlt"); }
    }
    
    // Send EOI to PIC if IRQ
    if (regs->int_no >= 32 && regs->int_no < 48) {
        if (regs->int_no >= 40) {
            __asm__ volatile("outb %%al, %%dx" : : "a"(0x20), "d"(0xA0));
        }
        __asm__ volatile("outb %%al, %%dx" : : "a"(0x20), "d"(0x20));
    }
}
