/**
 * MinimalOS - System Call Interface Implementation
 */

#include "syscall.h"
#include "apic.h" /* For printing */
#include "cpu.h"

extern void printk(const char *fmt, ...);
extern void syscall_entry(void);

#define MSR_STAR 0xC0000081
#define MSR_LSTAR 0xC0000082
#define MSR_FMASK 0xC0000084

#define EFER_SCE (1 << 0)

/**
 * Initialize system call interface
 */
void syscall_init(void) {
  /* 1. Enable System Call Extensions (SCE) */
  uint64_t efer = rdmsr(MSR_IA32_EFER);
  efer |= EFER_SCE;
  wrmsr(MSR_IA32_EFER, efer);

  /* 2. Setup STAR MSR */
  /* Bits 63:48 - SYSRET CS/SS selector base (User) */
  /*   SYSRET CS = Base + 16 = 0x20 + 0x10 = 0x30 (User Code) */
  /*   SYSRET SS = Base + 8  = 0x20 + 0x08 = 0x28 (User Data) */
  /* Bits 47:32 - SYSCALL CS/SS selector base (Kernel) */
  /*   SYSCALL CS = Base = 0x18 (Kernel Code) */
  /*   SYSCALL SS = Base + 8 = 0x20 (Kernel Data) */
  uint64_t star_user = 0x20;
  uint64_t star_kernel = 0x18;
  uint64_t star = (star_user << 48) | (star_kernel << 32);
  wrmsr(MSR_STAR, star);

  /* 3. Setup LSTAR MSR - Target RIP */
  wrmsr(MSR_LSTAR, (uint64_t)syscall_entry);

  /* 4. Setup FMASK MSR - RFLAGS mask */
  /* Mask Interrupt Flag (IF) to disable interrupts on entry */
  wrmsr(MSR_FMASK, 0x200); /* Clear IF (bit 9) */

  printk("  SYSCALL interface enabled (MSRs configured)\n");
}

/**
 * System Call Dispatcher
 * Called from syscall_entry.asm
 */
long syscall_handler(long sys_num, long a1, long a2, long a3, long a4, long a5,
                     long a6) {
  (void)a3;
  (void)a4;
  (void)a5;
  (void)a6;

  switch (sys_num) {
  case SYS_EXIT:
    printk("[SYSCALL] exit(%ld)\n", a1);
    /* TODO: Implement process termination */
    return 0;

  case SYS_WRITE:
    printk("[SYSCALL] write(fd=%ld, buf=%lx)\n", a1, a2);
    /* TODO: Implement write */
    return a2; /* return length */

  case SYS_SLEEP:
    printk("[SYSCALL] sleep(%ld)\n", a1);
    /* TODO: Call timer_sleep_ms */
    return 0;

  case SYS_GETPID:
    return 1; /* Dummy PID */

  default:
    printk("[SYSCALL] Unknown syscall: %ld\n", sys_num);
    return -1;
  }
}
