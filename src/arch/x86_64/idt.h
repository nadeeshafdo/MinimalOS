/**
 * MinimalOS - IDT Header
 */

#ifndef ARCH_X86_64_IDT_H
#define ARCH_X86_64_IDT_H

#include <minimalos/types.h>

/* IDT gate types */
#define IDT_GATE_INTERRUPT 0x8E /* Interrupt gate (IF cleared) */
#define IDT_GATE_TRAP 0x8F      /* Trap gate (IF not modified) */
#define IDT_GATE_USER 0x60      /* Ring 3 accessible */

/* Number of IDT entries */
#define IDT_ENTRIES 256

/* Exception vectors */
#define EXCEPTION_DE 0  /* Divide Error */
#define EXCEPTION_DB 1  /* Debug */
#define EXCEPTION_NMI 2 /* Non-Maskable Interrupt */
#define EXCEPTION_BP 3  /* Breakpoint */
#define EXCEPTION_OF 4  /* Overflow */
#define EXCEPTION_BR 5  /* Bound Range Exceeded */
#define EXCEPTION_UD 6  /* Invalid Opcode */
#define EXCEPTION_NM 7  /* Device Not Available */
#define EXCEPTION_DF 8  /* Double Fault */
#define EXCEPTION_TS 10 /* Invalid TSS */
#define EXCEPTION_NP 11 /* Segment Not Present */
#define EXCEPTION_SS 12 /* Stack Segment Fault */
#define EXCEPTION_GP 13 /* General Protection */
#define EXCEPTION_PF 14 /* Page Fault */
#define EXCEPTION_MF 16 /* x87 FPU Error */
#define EXCEPTION_AC 17 /* Alignment Check */
#define EXCEPTION_MC 18 /* Machine Check */
#define EXCEPTION_XM 19 /* SIMD Exception */
#define EXCEPTION_VE 20 /* Virtualization Exception */

/* IRQ vectors (remapped) */
#define IRQ_BASE 32
#define IRQ_TIMER (IRQ_BASE + 0)
#define IRQ_KEYBOARD (IRQ_BASE + 1)
#define IRQ_SPURIOUS 255

/* IDT entry structure (16 bytes for 64-bit) */
struct __attribute__((packed)) idt_entry {
  uint16_t offset_low;  /* Offset bits 0..15 */
  uint16_t selector;    /* Code segment selector */
  uint8_t ist;          /* IST index (0 = legacy stack) */
  uint8_t type_attr;    /* Type and attributes */
  uint16_t offset_mid;  /* Offset bits 16..31 */
  uint32_t offset_high; /* Offset bits 32..63 */
  uint32_t reserved;    /* Reserved (zero) */
};

/* IDT pointer structure */
struct __attribute__((packed)) idt_ptr {
  uint16_t limit;
  uint64_t base;
};

/* Saved CPU state during interrupt */
struct __attribute__((packed)) interrupt_frame {
  /* Pushed by ISR stub */
  uint64_t r15, r14, r13, r12, r11, r10, r9, r8;
  uint64_t rbp, rdi, rsi, rdx, rcx, rbx, rax;
  uint64_t int_no, error_code;

  /* Pushed by CPU */
  uint64_t rip;
  uint64_t cs;
  uint64_t rflags;
  uint64_t rsp;
  uint64_t ss;
};

/* Function prototypes */
void idt_init(void);
void idt_set_gate(uint8_t vector, void *handler, uint8_t ist,
                  uint8_t type_attr);
void idt_load(void);

#endif /* ARCH_X86_64_IDT_H */
