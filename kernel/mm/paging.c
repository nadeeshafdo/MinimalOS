/* Paging for x86_64 - Limine already sets up page tables */
#include <kernel/isr.h>
#include <kernel/paging.h>
#include <kernel/pmm.h>
#include <kernel/tty.h>
#include <stdint.h>

/*
 * Limine already sets up 4-level paging for us:
 * - Kernel is mapped in higher half at 0xFFFFFFFF80000000
 * - Physical memory is identity-mapped via HHDM
 *
 * For now, we just use what Limine provides.
 * More sophisticated paging can be added later.
 */

/* Current page directory (PML4) */
static page_directory_t *current_directory = NULL;
static page_directory_t *kernel_directory = NULL;

/* Get CR3 (PML4 physical address) */
static inline uint64_t read_cr3(void) {
  uint64_t cr3;
  __asm__ volatile("mov %%cr3, %0" : "=r"(cr3));
  return cr3;
}

/* Load page directory into CR3 */
static inline void load_page_directory(page_directory_t *dir) {
  __asm__ volatile("mov %0, %%cr3" : : "r"((uint64_t)dir) : "memory");
}

/* HHDM offset for physical-to-virtual conversion */
extern uint64_t get_hhdm_offset(void);

/* Page fault handler */
void page_fault_handler(struct registers *regs) {
  uint64_t faulting_address;
  __asm__ volatile("mov %%cr2, %0" : "=r"(faulting_address));

  int present = !(regs->err_code & 0x1);
  int rw = regs->err_code & 0x2;
  int us = regs->err_code & 0x4;
  int reserved = regs->err_code & 0x8;

  terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_RED));
  terminal_writestring("\n*** PAGE FAULT ***\n");
  terminal_writestring("Faulting address: 0x");

  /* Print hex address */
  char hex[17];
  const char *digits = "0123456789ABCDEF";
  uint64_t addr_copy = faulting_address;
  for (int i = 15; i >= 0; i--) {
    hex[i] = digits[addr_copy & 0xF];
    addr_copy >>= 4;
  }
  hex[16] = '\0';
  terminal_writestring(hex);
  terminal_writestring("\n");

  if (present)
    terminal_writestring("  - Page not present\n");
  if (rw)
    terminal_writestring("  - Write operation\n");
  if (us)
    terminal_writestring("  - User mode\n");
  if (reserved)
    terminal_writestring("  - Reserved bits set\n");

  terminal_writestring("RIP: 0x");
  addr_copy = regs->rip;
  for (int i = 15; i >= 0; i--) {
    hex[i] = digits[addr_copy & 0xF];
    addr_copy >>= 4;
  }
  terminal_writestring(hex);
  terminal_writestring("\n");

  terminal_writestring("System halted.\n");
  while (1) {
    __asm__ volatile("cli; hlt");
  }
}

void paging_init(void) {
  /* Limine already set up paging for us */
  /* Just save the current page directory and register page fault handler */

  uint64_t cr3 = read_cr3();
  uint64_t hhdm = get_hhdm_offset();

  /* Convert physical CR3 to virtual address via HHDM */
  kernel_directory = (page_directory_t *)(cr3 + hhdm);
  current_directory = kernel_directory;

  /* Register page fault handler */
  isr_register_handler(14, page_fault_handler);
}

void paging_map(uint64_t virtual_addr, uint64_t physical_addr, uint64_t flags) {
  /* TODO: Implement proper page mapping for x86_64 */
  /* For now, we rely on Limine's initial mapping */
  (void)virtual_addr;
  (void)physical_addr;
  (void)flags;
}

void paging_unmap(uint64_t virtual_addr) {
  /* TODO: Implement proper page unmapping */
  (void)virtual_addr;
}

uint64_t paging_get_physical(uint64_t virtual_addr) {
  /* TODO: Walk page tables to get physical address */
  /* For now, if it's in HHDM range, subtract offset */
  uint64_t hhdm = get_hhdm_offset();
  if (virtual_addr >= hhdm) {
    return virtual_addr - hhdm;
  }
  return 0;
}

void paging_switch_directory(page_directory_t *dir) {
  current_directory = dir;
  /* Convert virtual to physical and load */
  uint64_t phys = paging_get_physical((uint64_t)dir);
  if (phys) {
    load_page_directory((page_directory_t *)phys);
  }
}

page_directory_t *paging_get_directory(void) { return current_directory; }
