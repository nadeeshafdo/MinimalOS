#ifndef _KERNEL_ISR_H
#define _KERNEL_ISR_H

#include <stdint.h>

/* CPU register state - must match the stack layout from isr/irq stubs */
struct registers {
    /* Pushed last, popped first - segment registers */
    uint32_t gs, fs, es, ds;
    /* Pushed by pusha - order is: eax, ecx, edx, ebx, esp, ebp, esi, edi */
    uint32_t edi, esi, ebp, esp, ebx, edx, ecx, eax;
    /* Pushed by our stub - interrupt number and error code */
    uint32_t int_no, err_code;
    /* Pushed by CPU on interrupt - instruction pointer, code segment, flags */
    uint32_t eip, cs, eflags, useresp, ss;
};

/* ISR handler function type */
typedef void (*isr_handler_t)(struct registers*);

/* Initialize ISRs */
void isr_init(void);

/* Register custom ISR handler */
void isr_register_handler(uint8_t num, isr_handler_t handler);

/* External ISR declarations (0-31 for CPU exceptions) */
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

#endif /* _KERNEL_ISR_H */
