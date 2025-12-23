#include <stdint.h>
#include <kernel/irq.h>
#include <kernel/isr.h>
#include <kernel/idt.h>

/* I/O port operations */
static inline void outb(uint16_t port, uint8_t value) {
    __asm__ volatile ("outb %0, %1" : : "a"(value), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    __asm__ volatile ("inb %1, %0" : "=a"(ret) : "Nd"(port));
    return ret;
}

/* PIC constants */
#define PIC1_COMMAND 0x20
#define PIC1_DATA    0x21
#define PIC2_COMMAND 0xA0
#define PIC2_DATA    0xA1
#define PIC_EOI      0x20

/* IRQ handler array */
static irq_handler_t irq_handlers[16];

/* Register IRQ handler */
void irq_register_handler(uint8_t irq, irq_handler_t handler) {
    irq_handlers[irq] = handler;
}

/* Remap PIC to avoid conflicts with CPU exceptions */
static void pic_remap(void) {
    /* Start initialization (ICW1) */
    outb(PIC1_COMMAND, 0x11);
    outb(PIC2_COMMAND, 0x11);
    
    /* Set vector offsets (ICW2) */
    outb(PIC1_DATA, 0x20);  /* IRQ0-7 -> INT 32-39 */
    outb(PIC2_DATA, 0x28);  /* IRQ8-15 -> INT 40-47 */
    
    /* Set up cascading (ICW3) */
    outb(PIC1_DATA, 0x04);  /* IRQ2 has slave */
    outb(PIC2_DATA, 0x02);  /* Slave ID 2 */
    
    /* Set 8086 mode (ICW4) */
    outb(PIC1_DATA, 0x01);
    outb(PIC2_DATA, 0x01);
    
    /* Unmask IRQ0 (timer) and IRQ1 (keyboard), mask others on PIC1 */
    outb(PIC1_DATA, 0xFC);  /* 11111100 - enable IRQ0 and IRQ1 */
    
    /* Mask all IRQs on PIC2 for now */
    outb(PIC2_DATA, 0xFF);
}

/* Common IRQ handler */
void irq_handler(struct registers* regs) {
    /* Call custom handler if registered */
    if (regs->int_no >= 32 && regs->int_no <= 47) {
        uint8_t irq_num = regs->int_no - 32;
        
        if (irq_handlers[irq_num] != 0) {
            irq_handler_t handler = irq_handlers[irq_num];
            handler(regs);
        }
    }
    
    /* Send EOI to PIC */
    if (regs->int_no >= 40) {
        outb(PIC2_COMMAND, PIC_EOI);  /* Send EOI to slave */
    }
    outb(PIC1_COMMAND, PIC_EOI);      /* Send EOI to master */
}

void irq_init(void) {
    /* Remap PIC */
    pic_remap();
    
    /* Set IRQ gates in IDT */
    idt_set_gate(32, (uint32_t)irq0, 0x08, 0x8E);
    idt_set_gate(33, (uint32_t)irq1, 0x08, 0x8E);
    idt_set_gate(34, (uint32_t)irq2, 0x08, 0x8E);
    idt_set_gate(35, (uint32_t)irq3, 0x08, 0x8E);
    idt_set_gate(36, (uint32_t)irq4, 0x08, 0x8E);
    idt_set_gate(37, (uint32_t)irq5, 0x08, 0x8E);
    idt_set_gate(38, (uint32_t)irq6, 0x08, 0x8E);
    idt_set_gate(39, (uint32_t)irq7, 0x08, 0x8E);
    idt_set_gate(40, (uint32_t)irq8, 0x08, 0x8E);
    idt_set_gate(41, (uint32_t)irq9, 0x08, 0x8E);
    idt_set_gate(42, (uint32_t)irq10, 0x08, 0x8E);
    idt_set_gate(43, (uint32_t)irq11, 0x08, 0x8E);
    idt_set_gate(44, (uint32_t)irq12, 0x08, 0x8E);
    idt_set_gate(45, (uint32_t)irq13, 0x08, 0x8E);
    idt_set_gate(46, (uint32_t)irq14, 0x08, 0x8E);
    idt_set_gate(47, (uint32_t)irq15, 0x08, 0x8E);
    
    /* Enable interrupts */
    __asm__ volatile ("sti");
}
