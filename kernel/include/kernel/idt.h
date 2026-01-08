/* IDT header for x86_64 */
#ifndef KERNEL_IDT_H
#define KERNEL_IDT_H

#include <stdint.h>

/* IDT entry structure for x86_64 (16 bytes) */
struct idt_entry {
  uint16_t base_low; /* Lower 16 bits of handler address */
  uint16_t selector; /* Code segment selector */
  uint8_t ist;   /* Interrupt Stack Table offset (bits 0-2), rest reserved */
  uint8_t flags; /* Type and attributes */
  uint16_t base_mid;  /* Middle 16 bits of handler address */
  uint32_t base_high; /* Upper 32 bits of handler address */
  uint32_t reserved;  /* Always zero */
} __attribute__((packed));

/* IDT pointer structure */
struct idt_ptr {
  uint16_t limit;
  uint64_t base;
} __attribute__((packed));

/* IDT flags */
#define IDT_PRESENT 0x80
#define IDT_DPL_RING0 0x00
#define IDT_DPL_RING3 0x60
#define IDT_GATE_INT 0x0E  /* 64-bit interrupt gate */
#define IDT_GATE_TRAP 0x0F /* 64-bit trap gate */

/* Functions */
void idt_init(void);
void idt_set_gate(uint8_t num, uint64_t base, uint16_t selector, uint8_t flags);

#endif
