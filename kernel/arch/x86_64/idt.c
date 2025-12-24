/* 64-bit IDT (Interrupt Descriptor Table) implementation */

#include <stdint.h>
#include "idt.h"

/* External assembly functions */
extern void idt_load(idt_ptr_t *ptr);

/* ISR stubs from assembly */
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

/* IRQs (remapped to 32-47) */
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

/* IDT with 256 entries */
static idt_entry_t idt[256];
static idt_ptr_t idt_ptr;

/* Interrupt handlers */
static interrupt_handler_t handlers[256] = {0};

/* Kernel code segment selector (from GDT) */
#define KERNEL_CS 0x08

void idt_set_gate(uint8_t num, uint64_t handler, uint16_t selector, uint8_t type) {
    idt[num].offset_low = handler & 0xFFFF;
    idt[num].selector = selector;
    idt[num].ist = 0;
    idt[num].type_attr = type;
    idt[num].offset_mid = (handler >> 16) & 0xFFFF;
    idt[num].offset_high = (handler >> 32) & 0xFFFFFFFF;
    idt[num].reserved = 0;
}

void register_interrupt_handler(uint8_t num, interrupt_handler_t handler) {
    handlers[num] = handler;
}

/* Called from assembly ISR stubs */
void isr_handler(uint64_t int_num, uint64_t error_code) {
    if (handlers[int_num]) {
        handlers[int_num](int_num, error_code);
    } else {
        /* Default handler - print to VGA */
        volatile uint16_t *vga = (volatile uint16_t *)0xB8000;
        const char *msg = "INT:";
        int i = 0;
        while (msg[i]) {
            vga[i] = (uint16_t)msg[i] | 0x4F00;  /* White on red */
            i++;
        }
        /* Print interrupt number */
        vga[i++] = (uint16_t)('0' + (int_num / 10)) | 0x4F00;
        vga[i++] = (uint16_t)('0' + (int_num % 10)) | 0x4F00;
    }
}

/* Called from assembly IRQ stubs */
void irq_handler(uint64_t irq_num) {
    /* Call registered handler */
    if (handlers[irq_num + 32]) {
        handlers[irq_num + 32](irq_num + 32, 0);
    }
    
    /* Send EOI to PIC */
    if (irq_num >= 8) {
        /* Send EOI to slave PIC */
        __asm__ volatile ("outb %0, %1" : : "a"((uint8_t)0x20), "Nd"((uint16_t)0xA0));
    }
    /* Send EOI to master PIC */
    __asm__ volatile ("outb %0, %1" : : "a"((uint8_t)0x20), "Nd"((uint16_t)0x20));
}

void idt_init(void) {
    /* Set up IDT pointer */
    idt_ptr.limit = sizeof(idt) - 1;
    idt_ptr.base = (uint64_t)&idt;
    
    /* Clear all entries */
    for (int i = 0; i < 256; i++) {
        idt_set_gate(i, 0, 0, 0);
    }
    
    /* CPU exceptions (ISR 0-31) */
    idt_set_gate(0, (uint64_t)isr0, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(1, (uint64_t)isr1, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(2, (uint64_t)isr2, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(3, (uint64_t)isr3, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(4, (uint64_t)isr4, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(5, (uint64_t)isr5, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(6, (uint64_t)isr6, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(7, (uint64_t)isr7, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(8, (uint64_t)isr8, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(9, (uint64_t)isr9, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(10, (uint64_t)isr10, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(11, (uint64_t)isr11, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(12, (uint64_t)isr12, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(13, (uint64_t)isr13, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(14, (uint64_t)isr14, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(15, (uint64_t)isr15, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(16, (uint64_t)isr16, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(17, (uint64_t)isr17, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(18, (uint64_t)isr18, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(19, (uint64_t)isr19, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(20, (uint64_t)isr20, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(21, (uint64_t)isr21, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(22, (uint64_t)isr22, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(23, (uint64_t)isr23, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(24, (uint64_t)isr24, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(25, (uint64_t)isr25, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(26, (uint64_t)isr26, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(27, (uint64_t)isr27, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(28, (uint64_t)isr28, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(29, (uint64_t)isr29, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(30, (uint64_t)isr30, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(31, (uint64_t)isr31, KERNEL_CS, IDT_INTERRUPT_GATE);
    
    /* Hardware IRQs (remapped to 32-47) */
    idt_set_gate(32, (uint64_t)irq0, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(33, (uint64_t)irq1, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(34, (uint64_t)irq2, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(35, (uint64_t)irq3, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(36, (uint64_t)irq4, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(37, (uint64_t)irq5, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(38, (uint64_t)irq6, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(39, (uint64_t)irq7, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(40, (uint64_t)irq8, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(41, (uint64_t)irq9, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(42, (uint64_t)irq10, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(43, (uint64_t)irq11, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(44, (uint64_t)irq12, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(45, (uint64_t)irq13, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(46, (uint64_t)irq14, KERNEL_CS, IDT_INTERRUPT_GATE);
    idt_set_gate(47, (uint64_t)irq15, KERNEL_CS, IDT_INTERRUPT_GATE);
    
    /* Load IDT */
    idt_load(&idt_ptr);
}
