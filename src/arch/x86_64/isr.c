/**
 * MinimalOS - Interrupt Service Routine C Handlers
 */

#include "apic.h"
#include "cpu.h"
#include "idt.h"

extern void printk(const char *fmt, ...);
extern void panic(const char *message);

/* Forward declaration - panic_with_frame takes interrupt_frame */
static void panic_with_frame(const char *message,
                             struct interrupt_frame *frame);

/* Exception names for debugging */
static const char *exception_names[] = {"Divide Error",
                                        "Debug",
                                        "Non-Maskable Interrupt",
                                        "Breakpoint",
                                        "Overflow",
                                        "Bound Range Exceeded",
                                        "Invalid Opcode",
                                        "Device Not Available",
                                        "Double Fault",
                                        "Coprocessor Segment Overrun",
                                        "Invalid TSS",
                                        "Segment Not Present",
                                        "Stack-Segment Fault",
                                        "General Protection Fault",
                                        "Page Fault",
                                        "Reserved",
                                        "x87 FPU Error",
                                        "Alignment Check",
                                        "Machine Check",
                                        "SIMD Floating-Point Exception",
                                        "Virtualization Exception",
                                        "Control Protection Exception",
                                        "Reserved",
                                        "Reserved",
                                        "Reserved",
                                        "Reserved",
                                        "Reserved",
                                        "Reserved",
                                        "Reserved",
                                        "Reserved",
                                        "Security Exception",
                                        "Reserved"};

/**
 * Handle page fault specially
 */
static void handle_page_fault(struct interrupt_frame *frame) {
  uint64_t fault_addr = read_cr2();

  printk("\n!!! PAGE FAULT !!!\n");
  printk("Faulting address: 0x%lx\n", fault_addr);
  printk("Error code: 0x%lx\n", frame->error_code);
  printk("  %s\n", (frame->error_code & 1) ? "Page-level protection violation"
                                           : "Non-present page");
  printk("  %s\n", (frame->error_code & 2) ? "Write access" : "Read access");
  printk("  %s\n", (frame->error_code & 4) ? "User mode" : "Supervisor mode");
  printk("  %s\n", (frame->error_code & 8) ? "Reserved bit violation" : "");
  printk("  %s\n", (frame->error_code & 16) ? "Instruction fetch" : "");
  printk("RIP: 0x%lx\n", frame->rip);
  printk("RSP: 0x%lx\n", frame->rsp);

  /* For now, panic on page faults */
  panic_with_frame("Page Fault", frame);
}

/**
 * Panic with register dump (local implementation)
 */
static void panic_with_frame(const char *message,
                             struct interrupt_frame *frame) {
  __asm__ volatile("cli");

  printk("\n!!! KERNEL PANIC !!!\n");
  printk("====================\n");
  printk("Exception: %s\n", message);
  printk("Interrupt: %lu  Error Code: 0x%lx\n", frame->int_no,
         frame->error_code);
  printk("\nRegister Dump:\n");
  printk("  RAX=%016lx  RBX=%016lx  RCX=%016lx\n", frame->rax, frame->rbx,
         frame->rcx);
  printk("  RDX=%016lx  RSI=%016lx  RDI=%016lx\n", frame->rdx, frame->rsi,
         frame->rdi);
  printk("  RBP=%016lx  RSP=%016lx  RIP=%016lx\n", frame->rbp, frame->rsp,
         frame->rip);
  printk("  R8 =%016lx  R9 =%016lx  R10=%016lx\n", frame->r8, frame->r9,
         frame->r10);
  printk("  R11=%016lx  R12=%016lx  R13=%016lx\n", frame->r11, frame->r12,
         frame->r13);
  printk("  R14=%016lx  R15=%016lx\n", frame->r14, frame->r15);
  printk("  CS=%04lx  SS=%04lx  RFLAGS=%016lx\n", frame->cs, frame->ss,
         frame->rflags);
  printk("\nSystem halted.\n");

  for (;;) {
    __asm__ volatile("hlt");
  }
}

/**
 * Main ISR handler - called from assembly stub
 */
void isr_handler(struct interrupt_frame *frame) {
  uint64_t int_no = frame->int_no;

  /* Handle exceptions (0-31) */
  if (int_no < 32) {
    /* Page fault gets special handling */
    if (int_no == EXCEPTION_PF) {
      handle_page_fault(frame);
      return;
    }

    /* All other exceptions are fatal for now */
    const char *name = exception_names[int_no];
    panic_with_frame(name, frame);
    return;
  }

  /* Handle hardware interrupts (32+) */
  if (int_no >= IRQ_BASE) {
    uint64_t irq = int_no - IRQ_BASE;

    switch (irq) {
    case 0: /* Timer */
      /* Call timer tick handler */
      {
        extern void timer_tick(void);
        timer_tick();
      }
      break;

    case 1: /* Keyboard */
      /* Read scancode to acknowledge */
      {
        uint8_t scancode;
        __asm__ volatile("inb $0x60, %0" : "=a"(scancode));
        printk("Key: 0x%x\n", scancode);
      }
      break;

    default:
      printk("Unhandled IRQ %lu\n", irq);
      break;
    }

    /* Send End of Interrupt - use PIC for IRQs 0-15, APIC for others */
    if (irq < 16) {
      extern void pic_eoi(uint8_t irq);
      pic_eoi((uint8_t)irq);
    } else {
      apic_eoi();
    }
  }
}

/**
 * IRQ handler (alternative entry for IRQs)
 */
void irq_handler(struct interrupt_frame *frame) { isr_handler(frame); }
