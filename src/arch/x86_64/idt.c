/**
 * MinimalOS - IDT Initialization and Management
 */

#include "idt.h"

extern void printk(const char *fmt, ...);

/* External ISR stubs defined in isr.asm */
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
extern void isr32(void); /* Timer */
extern void isr33(void); /* Keyboard */
extern void isr_spurious(void);

/* IDT table and pointer */
static struct idt_entry idt[IDT_ENTRIES] __aligned(16);
static struct idt_ptr idtr;

/* Code segment selector (from GDT) */
#define KERNEL_CS 0x18

/**
 * Set an IDT gate
 */
void idt_set_gate(uint8_t vector, void *handler, uint8_t ist,
                  uint8_t type_attr) {
  uintptr_t addr = (uintptr_t)handler;

  idt[vector].offset_low = addr & 0xFFFF;
  idt[vector].selector = KERNEL_CS;
  idt[vector].ist = ist & 0x7;
  idt[vector].type_attr = type_attr;
  idt[vector].offset_mid = (addr >> 16) & 0xFFFF;
  idt[vector].offset_high = (addr >> 32) & 0xFFFFFFFF;
  idt[vector].reserved = 0;
}

/**
 * Load IDT into IDTR
 */
void idt_load(void) { __asm__ volatile("lidt %0" : : "m"(idtr)); }

/**
 * Initialize IDT with exception and interrupt handlers
 */
void idt_init(void) {
  /* Set up IDT pointer */
  idtr.limit = sizeof(idt) - 1;
  idtr.base = (uint64_t)&idt;

  /* Clear IDT */
  for (int i = 0; i < IDT_ENTRIES; i++) {
    idt[i].offset_low = 0;
    idt[i].selector = 0;
    idt[i].ist = 0;
    idt[i].type_attr = 0;
    idt[i].offset_mid = 0;
    idt[i].offset_high = 0;
    idt[i].reserved = 0;
  }

  /* Set up exception handlers (vectors 0-31) */
  idt_set_gate(0, isr0, 0, IDT_GATE_INTERRUPT);            /* Divide Error */
  idt_set_gate(1, isr1, 0, IDT_GATE_INTERRUPT);            /* Debug */
  idt_set_gate(2, isr2, 1, IDT_GATE_INTERRUPT);            /* NMI (use IST1) */
  idt_set_gate(3, isr3, 0, IDT_GATE_TRAP | IDT_GATE_USER); /* Breakpoint */
  idt_set_gate(4, isr4, 0, IDT_GATE_TRAP);                 /* Overflow */
  idt_set_gate(5, isr5, 0, IDT_GATE_INTERRUPT);            /* Bound Range */
  idt_set_gate(6, isr6, 0, IDT_GATE_INTERRUPT);            /* Invalid Opcode */
  idt_set_gate(7, isr7, 0, IDT_GATE_INTERRUPT);   /* Device Not Available */
  idt_set_gate(8, isr8, 2, IDT_GATE_INTERRUPT);   /* Double Fault (use IST2) */
  idt_set_gate(9, isr9, 0, IDT_GATE_INTERRUPT);   /* Coprocessor Segment */
  idt_set_gate(10, isr10, 0, IDT_GATE_INTERRUPT); /* Invalid TSS */
  idt_set_gate(11, isr11, 0, IDT_GATE_INTERRUPT); /* Segment Not Present */
  idt_set_gate(12, isr12, 0, IDT_GATE_INTERRUPT); /* Stack Segment */
  idt_set_gate(13, isr13, 0, IDT_GATE_INTERRUPT); /* General Protection */
  idt_set_gate(14, isr14, 0, IDT_GATE_INTERRUPT); /* Page Fault */
  idt_set_gate(15, isr15, 0, IDT_GATE_INTERRUPT); /* Reserved */
  idt_set_gate(16, isr16, 0, IDT_GATE_INTERRUPT); /* x87 FPU Error */
  idt_set_gate(17, isr17, 0, IDT_GATE_INTERRUPT); /* Alignment Check */
  idt_set_gate(18, isr18, 0, IDT_GATE_INTERRUPT); /* Machine Check */
  idt_set_gate(19, isr19, 0, IDT_GATE_INTERRUPT); /* SIMD Exception */
  idt_set_gate(20, isr20, 0, IDT_GATE_INTERRUPT); /* Virtualization */
  idt_set_gate(21, isr21, 0, IDT_GATE_INTERRUPT); /* Reserved */
  idt_set_gate(22, isr22, 0, IDT_GATE_INTERRUPT); /* Reserved */
  idt_set_gate(23, isr23, 0, IDT_GATE_INTERRUPT); /* Reserved */
  idt_set_gate(24, isr24, 0, IDT_GATE_INTERRUPT); /* Reserved */
  idt_set_gate(25, isr25, 0, IDT_GATE_INTERRUPT); /* Reserved */
  idt_set_gate(26, isr26, 0, IDT_GATE_INTERRUPT); /* Reserved */
  idt_set_gate(27, isr27, 0, IDT_GATE_INTERRUPT); /* Reserved */
  idt_set_gate(28, isr28, 0, IDT_GATE_INTERRUPT); /* Reserved */
  idt_set_gate(29, isr29, 0, IDT_GATE_INTERRUPT); /* Reserved */
  idt_set_gate(30, isr30, 0, IDT_GATE_INTERRUPT); /* Security Exception */
  idt_set_gate(31, isr31, 0, IDT_GATE_INTERRUPT); /* Reserved */

  /* Set up hardware interrupt handlers (vectors 32+) */
  idt_set_gate(32, isr32, 0, IDT_GATE_INTERRUPT); /* Timer */
  idt_set_gate(33, isr33, 0, IDT_GATE_INTERRUPT); /* Keyboard */

  /* Spurious interrupt handler */
  idt_set_gate(255, isr_spurious, 0, IDT_GATE_INTERRUPT);

  /* Load IDT */
  idt_load();

  printk("  IDT loaded: %u entries at 0x%lx\n", IDT_ENTRIES, idtr.base);
}
