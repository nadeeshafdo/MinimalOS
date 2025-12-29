/**
 * MinimalOS - Kernel Panic Handler
 */

#include <minimalos/types.h>

extern void printk(const char *fmt, ...);

/* Registers saved during exception */
struct panic_regs {
  uint64_t r15, r14, r13, r12, r11, r10, r9, r8;
  uint64_t rbp, rdi, rsi, rdx, rcx, rbx, rax;
  uint64_t int_no, error_code;
  uint64_t rip, cs, rflags, rsp, ss;
};

/**
 * Kernel panic - unrecoverable error
 * Prints diagnostic information and halts
 */
__noreturn void panic(const char *message) {
  /* Disable interrupts */
  __asm__ volatile("cli");

  printk("\n");
  printk("!!! KERNEL PANIC !!!\n");
  printk("====================\n");
  printk("%s\n", message);
  printk("\n");
  printk("System halted.\n");

  /* Halt forever */
  for (;;) {
    __asm__ volatile("hlt");
  }
}

/**
 * Panic with register dump
 * Called from exception handlers
 */
__noreturn void panic_with_regs(const char *message, struct panic_regs *regs) {
  /* Disable interrupts */
  __asm__ volatile("cli");

  printk("\n");
  printk("!!! KERNEL PANIC !!!\n");
  printk("====================\n");
  printk("Exception: %s\n", message);
  printk("Interrupt: %lu  Error Code: 0x%lx\n", regs->int_no, regs->error_code);
  printk("\n");
  printk("Register Dump:\n");
  printk("  RAX=%016lx  RBX=%016lx  RCX=%016lx\n", regs->rax, regs->rbx,
         regs->rcx);
  printk("  RDX=%016lx  RSI=%016lx  RDI=%016lx\n", regs->rdx, regs->rsi,
         regs->rdi);
  printk("  RBP=%016lx  RSP=%016lx  RIP=%016lx\n", regs->rbp, regs->rsp,
         regs->rip);
  printk("  R8 =%016lx  R9 =%016lx  R10=%016lx\n", regs->r8, regs->r9,
         regs->r10);
  printk("  R11=%016lx  R12=%016lx  R13=%016lx\n", regs->r11, regs->r12,
         regs->r13);
  printk("  R14=%016lx  R15=%016lx\n", regs->r14, regs->r15);
  printk("  CS=%04lx  SS=%04lx  RFLAGS=%016lx\n", regs->cs, regs->ss,
         regs->rflags);
  printk("\n");
  printk("System halted.\n");

  /* Halt forever */
  for (;;) {
    __asm__ volatile("hlt");
  }
}
