/* Test commands: test, panic, cpufreq for x86_64 */
#include <kernel/commands.h>
#include <kernel/kheap.h>
#include <kernel/process.h>
#include <kernel/timer.h>
#include <kernel/tty.h>
#include <stdint.h>

void cmd_test(void) {
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
  terminal_writestring("\n=== System Tests ===\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Memory allocation test */
  terminal_writestring("Memory alloc... ");
  void *p = kmalloc(64);
  if (p) {
    kfree(p);
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[PASS]\n");
  } else {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_RED, VGA_COLOR_BLACK));
    terminal_writestring("[FAIL]\n");
  }
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Timer test - just check if ticks are advancing */
  terminal_writestring("Timer running... ");
  uint32_t t1 = timer_get_ticks();
  if (t1 > 0) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[PASS] (");
    print_dec64(t1);
    terminal_writestring(" ticks)\n");
  } else {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_RED, VGA_COLOR_BLACK));
    terminal_writestring("[FAIL]\n");
  }
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  /* Process exists test */
  terminal_writestring("Processes... ");
  if (process_get(0)) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[PASS]\n");
  } else {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_RED, VGA_COLOR_BLACK));
    terminal_writestring("[FAIL]\n");
  }
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));

  terminal_writestring("All tests complete.\n");
}

void cmd_panic(void) {
  terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_RED));
  terminal_writestring("\n\n*** KERNEL PANIC ***\n");
  terminal_writestring("User-triggered panic for testing.\n");
  terminal_writestring("System halted.\n");
  __asm__ volatile("cli; hlt");
}

/* CPU frequency estimation - simple loop count method */
void cmd_cpufreq(void) {
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
  terminal_writestring("\n=== CPU Speed Estimation ===\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
  terminal_writestring("Running calibration loop...\n");

  /* Simple loop count in fixed iterations */
  volatile uint32_t count = 0;
  uint32_t start = timer_get_ticks();

  /* Count for a fixed number of iterations */
  for (volatile uint32_t i = 0; i < 10000000; i++) {
    count++;
  }

  uint32_t end = timer_get_ticks();
  uint32_t elapsed = end - start;

  terminal_writestring("Iterations: 10,000,000\n");
  terminal_writestring("Timer ticks: ");
  print_dec64(elapsed);
  terminal_writestring(" (");
  if (elapsed > 0) {
    uint32_t ms = elapsed * 10; /* 100Hz = 10ms per tick */
    print_dec64(ms);
    terminal_writestring(" ms)\n");

    /* Estimate: 10M iterations in X ms */
    if (ms > 0) {
      uint32_t loops_per_sec = (10000000UL * 1000UL) / ms;
      uint32_t est_mhz = loops_per_sec / 100000;

      terminal_writestring("\nEstimated speed: ~");
      print_dec64(est_mhz);
      terminal_writestring(" MHz equivalent\n");
    }
  } else {
    terminal_writestring("timer error)\n");
  }

  terminal_writestring("\nNote: This is a rough loop-based estimate.\n");
}
