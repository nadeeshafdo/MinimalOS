/* IRQ header for x86_64 */
#ifndef KERNEL_IRQ_H
#define KERNEL_IRQ_H

#include <kernel/isr.h>
#include <stdint.h>

/* IRQ handler function pointer type */
typedef void (*irq_handler_t)(struct registers *);

/* Functions */
void irq_init(void);
void irq_register_handler(uint8_t irq, irq_handler_t handler);

/* IRQ stubs (assembly) */
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

#endif
