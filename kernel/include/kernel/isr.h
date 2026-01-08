/* ISR header for x86_64 */
#ifndef KERNEL_ISR_H
#define KERNEL_ISR_H

#include <kernel/tty.h> /* For VGA colors */
#include <stdint.h>

/* 64-bit interrupt stack frame */
struct registers {
  /* Pushed by our ISR stub */
  uint64_t r15, r14, r13, r12, r11, r10, r9, r8;
  uint64_t rbp, rdi, rsi, rdx, rcx, rbx, rax;

  /* Interrupt number and error code */
  uint64_t int_no, err_code;

  /* Pushed by CPU automatically */
  uint64_t rip, cs, rflags, rsp, ss;
} __attribute__((packed));

/* ISR handler function pointer type */
typedef void (*isr_handler_t)(struct registers *);

/* Register and call handlers */
void isr_register_handler(uint8_t num, isr_handler_t handler);
void isr_handler(struct registers *regs);

/* Initialize ISRs */
void isr_init(void);

/* ISR stubs (assembly) */
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

/* Syscall ISR */
extern void isr128(void);

#endif
