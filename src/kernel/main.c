/**
 * MinimalOS - Kernel Main Entry Point
 */

#include <minimalos/multiboot2.h>
#include <minimalos/types.h>

/* External function declarations */
extern void serial_init(void);
extern void printk(const char *fmt, ...);
extern void vga_init(void);
extern void cpu_init(void);
extern void idt_init(void);
extern void pmm_init(void);
extern void vmm_init(void);
extern void heap_init(void);
extern void pic_init(void);
extern void apic_init(void);
extern void pic_unmask_irq(uint8_t irq);

/* Kernel version */
#define KERNEL_VERSION "0.1.0"

/* Timer tick counter */
static volatile uint64_t timer_ticks = 0;

/**
 * Called from ISR on timer interrupt
 */
void timer_tick(void) { timer_ticks++; }

/**
 * Kernel main entry point
 * Called from long_mode.asm after transition to 64-bit mode
 *
 * @param multiboot_info Physical address of Multiboot2 information structure
 */
void kernel_main(uint64_t multiboot_info) {
  /* Initialize serial port first for early debug output */
  serial_init();

  printk("\n");
  printk("===========================================\n");
  printk("  MinimalOS v%s - Booting...\n", KERNEL_VERSION);
  printk("===========================================\n");
  printk("\n");

  /* Initialize VGA text mode */
  vga_init();
  printk("[OK] VGA text mode initialized\n");

  /* Parse Multiboot2 information */
  printk("[..] Parsing Multiboot2 info at 0x%lx\n", multiboot_info);
  multiboot2_parse(multiboot_info);
  printk("[OK] Multiboot2 info parsed\n");

  /* Initialize CPU (CPUID, enable features) */
  printk("[..] Initializing CPU\n");
  cpu_init();
  printk("[OK] CPU initialized\n");

  /* Initialize IDT (exception handlers) */
  printk("[..] Initializing IDT\n");
  idt_init();
  printk("[OK] IDT initialized\n");

  /* Initialize physical memory manager */
  printk("[..] Initializing physical memory manager\n");
  pmm_init();
  printk("[OK] Physical memory manager initialized\n");

  /* Initialize virtual memory manager (page mapping) */
  printk("[..] Initializing virtual memory manager\n");
  vmm_init();
  printk("[OK] Virtual memory manager initialized\n");

  /* Initialize kernel heap */
  printk("[..] Initializing kernel heap\n");
  heap_init();
  printk("[OK] Kernel heap initialized\n");

  /* Initialize legacy PIC (always, for fallback) */
  printk("[..] Initializing PIC\n");
  pic_init();
  printk("[OK] PIC initialized\n");

  /* Initialize APIC (now that LAPIC is mapped) */
  printk("[..] Initializing APIC\n");
  apic_init();
  printk("[OK] APIC initialized\n");

  printk("\n");
  printk("===========================================\n");
  printk("  MinimalOS kernel initialized!\n");
  printk("===========================================\n");
  printk("\n");

  /* Unmask timer IRQ for testing */
  pic_unmask_irq(0); /* IRQ0 = Timer */

  /* Enable interrupts */
  printk("Enabling interrupts...\n");
  __asm__ volatile("sti");

  /* Halt loop - kernel idle with timer */
  printk("Entering idle loop...\n");
  uint64_t last_ticks = 0;

  for (;;) {
    __asm__ volatile("hlt");

    /* Print tick count every 100 ticks */
    if (timer_ticks >= last_ticks + 100) {
      printk("Timer: %lu ticks\n", timer_ticks);
      last_ticks = timer_ticks;
    }
  }
}
