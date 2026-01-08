/* System info shell commands for x86_64 */
#include <kernel/commands.h>
#include <kernel/kheap.h>
#include <kernel/pmm.h>
#include <kernel/process.h>
#include <kernel/timer.h>
#include <kernel/tty.h>

void cmd_info(void) {
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
  terminal_writestring("\n=== System Information ===\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));

  terminal_writestring("OS:          MinimalOS v0.2\n");
  terminal_writestring("Arch:        x86_64 (64-bit)\n");
  terminal_writestring("Bootloader:  Limine\n");
  terminal_writestring("Timer:       PIT @ 100 Hz\n");

  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
}

void cmd_mem(void) {
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
  terminal_writestring("\n=== Memory Information ===\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));

  terminal_writestring("Total memory:  ");
  print_dec64(pmm_get_total_memory() / 1024 / 1024);
  terminal_writestring(" MB\n");

  terminal_writestring("Free memory:   ");
  print_dec64(pmm_get_free_memory() / 1024 / 1024);
  terminal_writestring(" MB\n");

  terminal_writestring("Heap used:     ");
  print_dec64(kheap_get_used() / 1024);
  terminal_writestring(" KB\n");

  terminal_writestring("Heap free:     ");
  print_dec64(kheap_get_free() / 1024);
  terminal_writestring(" KB\n");

  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
}

void cmd_uptime(void) {
  uint32_t ticks = timer_get_ticks();
  uint32_t seconds = ticks / 100; /* 100 Hz timer */
  uint32_t minutes = seconds / 60;
  uint32_t hours = minutes / 60;

  terminal_writestring("\nUptime: ");
  print_dec64(hours);
  terminal_writestring("h ");
  print_dec64(minutes % 60);
  terminal_writestring("m ");
  print_dec64(seconds % 60);
  terminal_writestring("s (");
  print_dec64(ticks);
  terminal_writestring(" ticks)\n");
}

void cmd_ps(void) {
  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
  terminal_writestring("\n=== Process List ===\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
  terminal_writestring("PID  State    Name\n");
  terminal_writestring("---  -----    ----\n");

  for (uint32_t i = 0; i < MAX_PROCESSES; i++) {
    process_t *proc = process_get(i);
    if (proc) {
      print_dec64(proc->pid);
      terminal_writestring("    ");

      switch (proc->state) {
      case PROCESS_STATE_RUNNING:
        terminal_setcolor(
            vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
        terminal_writestring("RUN  ");
        break;
      case PROCESS_STATE_READY:
        terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
        terminal_writestring("RDY  ");
        break;
      case PROCESS_STATE_BLOCKED:
        terminal_setcolor(
            vga_entry_color(VGA_COLOR_LIGHT_RED, VGA_COLOR_BLACK));
        terminal_writestring("BLK  ");
        break;
      default:
        terminal_writestring("???  ");
      }
      terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
      terminal_writestring("    ");
      terminal_writestring(proc->name);
      terminal_writestring("\n");
    }
  }

  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
}

void cmd_cpuid(void) {
  uint32_t eax, ebx, ecx, edx;
  char vendor[13];

  /* Get vendor string */
  __asm__ volatile("cpuid"
                   : "=a"(eax), "=b"(ebx), "=c"(ecx), "=d"(edx)
                   : "a"(0));

  *(uint32_t *)&vendor[0] = ebx;
  *(uint32_t *)&vendor[4] = edx;
  *(uint32_t *)&vendor[8] = ecx;
  vendor[12] = '\0';

  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
  terminal_writestring("\n=== CPU Information ===\n");
  terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));

  terminal_writestring("Vendor: ");
  terminal_writestring(vendor);
  terminal_writestring("\n");

  /* Get processor info */
  __asm__ volatile("cpuid"
                   : "=a"(eax), "=b"(ebx), "=c"(ecx), "=d"(edx)
                   : "a"(1));

  terminal_writestring("Family: ");
  print_dec64((eax >> 8) & 0xF);
  terminal_writestring("\n");

  terminal_writestring("Model:  ");
  print_dec64((eax >> 4) & 0xF);
  terminal_writestring("\n");

  terminal_writestring("Features: ");
  if (edx & (1 << 0))
    terminal_writestring("FPU ");
  if (edx & (1 << 4))
    terminal_writestring("TSC ");
  if (edx & (1 << 5))
    terminal_writestring("MSR ");
  if (edx & (1 << 9))
    terminal_writestring("APIC ");
  if (edx & (1 << 25))
    terminal_writestring("SSE ");
  if (edx & (1 << 26))
    terminal_writestring("SSE2 ");
  if (ecx & (1 << 0))
    terminal_writestring("SSE3 ");
  terminal_writestring("\n");

  terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
}
