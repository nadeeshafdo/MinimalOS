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
extern void timer_init(void);
extern void sched_init(void);
extern uint64_t timer_get_ticks(void);

/* Task functions */
struct task;
extern struct task *task_create(void (*entry)(void), const char *name);
extern struct task *current_task;

/* Kernel version */
#define KERNEL_VERSION "0.1.0"

/* Task functions */
extern void task_yield(void);

/**
 * Test task 1 - prints periodically and yields
 */
static void test_task1(void) {
  uint64_t count = 0;
  for (;;) {
    printk("[Task1] count=%lu\n", count++);
    /* Yield to other tasks */
    task_yield();
  }
}

/**
 * Test task 2 - prints periodically and yields
 */
static void test_task2(void) {
  uint64_t count = 0;
  for (;;) {
    printk("[Task2] count=%lu\n", count++);
    /* Yield to other tasks */
    task_yield();
  }
}

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

  /* Initialize legacy PIC */
  printk("[..] Initializing PIC\n");
  pic_init();
  printk("[OK] PIC initialized\n");

  /* Initialize APIC (now that LAPIC is mapped) */
  printk("[..] Initializing APIC\n");
  apic_init();
  printk("[OK] APIC initialized\n");

  /* Initialize and calibrate timer */
  printk("[..] Initializing timer\n");
  timer_init();
  printk("[OK] Timer initialized\n");

  /* Initialize system call interface */
  printk("[..] Initializing SYSCALL interface\n");
  extern void syscall_init(void);
  syscall_init();
  printk("[OK] SYSCALL interface initialized\n");

  /* Initialize scheduler */
  printk("[..] Initializing scheduler\n");
  sched_init();
  printk("[OK] Scheduler initialized\n");

  printk("\n");
  printk("===========================================\n");
  printk("  MinimalOS kernel initialized!\n");
  printk("===========================================\n");
  printk("\n");

  /* Create test tasks */
  printk("Creating test tasks...\n");
  task_create(test_task1, "task1");
  task_create(test_task2, "task2");

  /* Enable interrupts */
  printk("Enabling interrupts...\n");
  __asm__ volatile("sti");

  /* Start first real task (cooperative switch) */
  printk("Starting scheduler...\n");
  extern void schedule(void);
  schedule();

  /* Main kernel loop - just idle, scheduler handles the rest */
  uint64_t last_ticks = 0;
  for (;;) {
    __asm__ volatile("hlt");

    /* Print timer tick every 500 ticks (5 seconds) */
    uint64_t ticks = timer_get_ticks();
    if (ticks >= last_ticks + 500) {
      printk("--- %lu seconds uptime ---\n", ticks / 100);
      last_ticks = ticks;
    }
  }
}
