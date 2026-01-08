/* ISR C handlers for x86_64 */
#include <kernel/idt.h>
#include <kernel/isr.h>
#include <kernel/tty.h>
#include <stdint.h>

/* Exception messages */
static const char *exception_messages[] = {"Division By Zero",
                                           "Debug",
                                           "Non Maskable Interrupt",
                                           "Breakpoint",
                                           "Into Detected Overflow",
                                           "Out of Bounds",
                                           "Invalid Opcode",
                                           "No Coprocessor",
                                           "Double Fault",
                                           "Coprocessor Segment Overrun",
                                           "Bad TSS",
                                           "Segment Not Present",
                                           "Stack Fault",
                                           "General Protection Fault",
                                           "Page Fault",
                                           "Unknown Interrupt",
                                           "Coprocessor Fault",
                                           "Alignment Check",
                                           "Machine Check",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved",
                                           "Reserved"};

/* ISR handler array */
static isr_handler_t isr_handlers[256];

/* Register a custom ISR handler */
void isr_register_handler(uint8_t num, isr_handler_t handler) {
  isr_handlers[num] = handler;
}

/* Print hex value */
static void print_hex(uint64_t value) {
  char hex[19] = "0x0000000000000000";
  const char *digits = "0123456789ABCDEF";

  for (int i = 17; i >= 2; i--) {
    hex[i] = digits[value & 0xF];
    value >>= 4;
  }

  terminal_writestring(hex);
}

/* Common ISR handler */
void isr_handler(struct registers *regs) {
  /* Call custom handler if registered */
  if (isr_handlers[regs->int_no] != 0) {
    isr_handler_t handler = isr_handlers[regs->int_no];
    handler(regs);
  } else {
    /* Unhandled exception */
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_RED));
    terminal_writestring("\n\n*** CPU EXCEPTION ***\n");

    if (regs->int_no < 32) {
      terminal_writestring("Exception: ");
      terminal_writestring(exception_messages[regs->int_no]);
      terminal_writestring("\n");
    }

    terminal_writestring("RIP: ");
    print_hex(regs->rip);
    terminal_writestring("\n");

    terminal_writestring("Error code: ");
    print_hex(regs->err_code);
    terminal_writestring("\n");

    terminal_writestring("System halted.\n");

    /* Halt the system */
    while (1) {
      __asm__ volatile("cli; hlt");
    }
  }
}

void isr_init(void) {
  /* Set ISR gates in IDT - using kernel code segment 0x08 */
  idt_set_gate(0, (uint64_t)isr0, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(1, (uint64_t)isr1, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(2, (uint64_t)isr2, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(3, (uint64_t)isr3, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(4, (uint64_t)isr4, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(5, (uint64_t)isr5, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(6, (uint64_t)isr6, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(7, (uint64_t)isr7, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(8, (uint64_t)isr8, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(9, (uint64_t)isr9, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(10, (uint64_t)isr10, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(11, (uint64_t)isr11, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(12, (uint64_t)isr12, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(13, (uint64_t)isr13, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(14, (uint64_t)isr14, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(15, (uint64_t)isr15, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(16, (uint64_t)isr16, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(17, (uint64_t)isr17, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(18, (uint64_t)isr18, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(19, (uint64_t)isr19, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(20, (uint64_t)isr20, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(21, (uint64_t)isr21, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(22, (uint64_t)isr22, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(23, (uint64_t)isr23, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(24, (uint64_t)isr24, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(25, (uint64_t)isr25, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(26, (uint64_t)isr26, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(27, (uint64_t)isr27, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(28, (uint64_t)isr28, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(29, (uint64_t)isr29, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(30, (uint64_t)isr30, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);
  idt_set_gate(31, (uint64_t)isr31, 0x08,
               IDT_PRESENT | IDT_DPL_RING0 | IDT_GATE_INT);

  /* Syscall ISR (0x80 = 128) - DPL 3 for User Mode Access */
  idt_set_gate(128, (uint64_t)isr128, 0x08,
               IDT_PRESENT | IDT_DPL_RING3 | IDT_GATE_INT);
}
