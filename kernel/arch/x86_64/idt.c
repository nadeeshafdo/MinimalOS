/* 64-bit Interrupt Descriptor Table for x86_64 */
#include <kernel/idt.h>
#include <stdint.h>

#define IDT_ENTRIES 256

static struct idt_entry idt[IDT_ENTRIES];
static struct idt_ptr idt_p;

/* External assembly function to load IDT */
extern void idt_flush(uint64_t);

void idt_set_gate(uint8_t num, uint64_t base, uint16_t selector,
                  uint8_t flags) {
  idt[num].base_low = base & 0xFFFF;
  idt[num].base_mid = (base >> 16) & 0xFFFF;
  idt[num].base_high = (base >> 32) & 0xFFFFFFFF;

  idt[num].selector = selector;
  idt[num].ist = 0; /* No IST for now */
  idt[num].flags = flags;
  idt[num].reserved = 0;
}

void idt_init(void) {
  idt_p.limit = (sizeof(struct idt_entry) * IDT_ENTRIES) - 1;
  idt_p.base = (uint64_t)&idt;

  /* Clear all IDT entries */
  for (int i = 0; i < IDT_ENTRIES; i++) {
    idt_set_gate(i, 0, 0, 0);
  }

  /* Load IDT */
  idt_flush((uint64_t)&idt_p);
}
