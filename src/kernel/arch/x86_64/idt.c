#include "idt.h"
#include "keyboard.h"
#include "../../stdint.h"

// Port I/O functions
static inline void outb(uint16_t port, uint8_t val) {
    asm volatile ("outb %0, %1" : : "a"(val), "Nd"(port));
}

#define IDT_ENTRIES 256

struct idt_entry {
    uint16_t base_low;
    uint16_t sel;
    uint8_t zero;
    uint8_t flags;
    uint16_t base_mid;
    uint32_t base_high;
    uint32_t zero2;
} __attribute__((packed));

static struct idt_entry idt[IDT_ENTRIES];

struct idt_ptr {
    uint16_t limit;
    uint64_t base;
} __attribute__((packed));

static struct idt_ptr idt_ptr;

// Simple ISR stub that just returns
extern void generic_isr(void);
asm(
    ".global generic_isr\n"
    "generic_isr:\n"
    "    iretq\n"
);

// Keyboard ISR wrapper
extern void keyboard_isr_wrapper(void);
asm(
    ".global keyboard_isr_wrapper\n"
    "keyboard_isr_wrapper:\n"
    "    push %rax\n"
    "    push %rbx\n"
    "    push %rcx\n"
    "    push %rdx\n"
    "    push %rsi\n"
    "    push %rdi\n"
    "    push %r8\n"
    "    push %r9\n"
    "    push %r10\n"
    "    push %r11\n"
    "    call keyboard_isr\n"
    "    pop %r11\n"
    "    pop %r10\n"
    "    pop %r9\n"
    "    pop %r8\n"
    "    pop %rdi\n"
    "    pop %rsi\n"
    "    pop %rdx\n"
    "    pop %rcx\n"
    "    pop %rbx\n"
    "    pop %rax\n"
    "    iretq\n"
);

void set_idt_entry(int num, uint64_t base, uint16_t sel, uint8_t flags) {
    idt[num].base_low = base & 0xFFFF;
    idt[num].sel = sel;
    idt[num].zero = 0;
    idt[num].flags = flags;
    idt[num].base_mid = (base >> 16) & 0xFFFF;
    idt[num].base_high = (base >> 32) & 0xFFFFFFFF;
    idt[num].zero2 = 0;
}

void setup_idt() {
    idt_ptr.limit = sizeof(idt) - 1;
    idt_ptr.base = (uint64_t)&idt;

    // Set up exception handlers (0-31) with generic handler
    for (int i = 0; i < 32; i++) {
        set_idt_entry(i, (uint64_t)generic_isr, 0x08, 0x8E);
    }

    // Remap PIC
    outb(0x20, 0x11);  // Initialize PIC1
    outb(0xA0, 0x11);  // Initialize PIC2
    outb(0x21, 0x20);  // PIC1 offset to 32
    outb(0xA1, 0x28);  // PIC2 offset to 40
    outb(0x21, 0x04);  // PIC1 cascade to IRQ2
    outb(0xA1, 0x02);  // PIC2 cascade identity
    outb(0x21, 0x01);  // 8086 mode
    outb(0xA1, 0x01);  // 8086 mode
    outb(0x21, 0xFE);  // Mask all IRQs except IRQ1 (keyboard)
    outb(0xA1, 0xFF);  // Mask all IRQs on PIC2

    // Set IRQ1 (keyboard) to entry 33
    set_idt_entry(33, (uint64_t)keyboard_isr_wrapper, 0x08, 0x8E);

    // Load IDT and enable interrupts
    asm volatile ("lidt (%0)" :: "r"(&idt_ptr));
    asm volatile ("sti");
}