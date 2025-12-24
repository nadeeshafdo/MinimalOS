#ifndef _IDT_H
#define _IDT_H

#include <stdint.h>

/* 64-bit IDT gate descriptor (16 bytes) */
typedef struct {
    uint16_t offset_low;      /* Offset bits 0-15 */
    uint16_t selector;        /* Code segment selector */
    uint8_t  ist;             /* Interrupt Stack Table offset (0 = none) */
    uint8_t  type_attr;       /* Type and attributes */
    uint16_t offset_mid;      /* Offset bits 16-31 */
    uint32_t offset_high;     /* Offset bits 32-63 */
    uint32_t reserved;        /* Must be zero */
} __attribute__((packed)) idt_entry_t;

/* IDT pointer for LIDT instruction */
typedef struct {
    uint16_t limit;
    uint64_t base;
} __attribute__((packed)) idt_ptr_t;

/* Gate types */
#define IDT_INTERRUPT_GATE  0x8E  /* Present, DPL=0, 64-bit interrupt gate */
#define IDT_TRAP_GATE       0x8F  /* Present, DPL=0, 64-bit trap gate */
#define IDT_USER_GATE       0xEE  /* Present, DPL=3, 64-bit interrupt gate */

/* Initialize IDT */
void idt_init(void);

/* Set an IDT gate */
void idt_set_gate(uint8_t num, uint64_t handler, uint16_t selector, uint8_t type);

/* Register interrupt handler callback */
typedef void (*interrupt_handler_t)(uint64_t int_num, uint64_t error_code);
void register_interrupt_handler(uint8_t num, interrupt_handler_t handler);

#endif /* _IDT_H */
