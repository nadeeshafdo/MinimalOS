/* System info commands: info, mem, uptime, ps, cpuid */
#include <stdint.h>
#include <kernel/commands.h>
#include <kernel/tty.h>
#include <kernel/timer.h>
#include <kernel/pmm.h>
#include <kernel/process.h>

void cmd_info(void) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("\n=== MinimalOS System Info ===\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring("Version:     0.1 Alpha\n");
    terminal_writestring("Arch:        x86 (32-bit)\n");
    terminal_writestring("Timer:       100 Hz PIT\n");
    terminal_writestring("Page Size:   4 KB\n");
    terminal_writestring("Scheduler:   Round-Robin\n");
}

void cmd_mem(void) {
    uint32_t free_mem = pmm_get_free_memory();
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
    terminal_writestring("\n=== Memory Info ===\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring("Free:  ");
    cmd_print_dec(free_mem / 1024);
    terminal_writestring(" KB (");
    cmd_print_dec(free_mem / 1048576);
    terminal_writestring(" MB)\n");
    terminal_writestring("Pages: ");
    cmd_print_dec(free_mem / 4096);
    terminal_writestring(" free frames\n");
}

void cmd_uptime(void) {
    uint32_t ticks = timer_get_ticks();
    uint32_t total_secs = ticks / 100;
    uint32_t hours = total_secs / 3600;
    uint32_t mins = (total_secs % 3600) / 60;
    uint32_t secs = total_secs % 60;
    
    terminal_writestring("\nUptime: ");
    cmd_print_dec(hours);
    terminal_writestring("h ");
    cmd_print_dec(mins);
    terminal_writestring("m ");
    cmd_print_dec(secs);
    terminal_writestring("s (");
    cmd_print_dec(ticks);
    terminal_writestring(" ticks)\n");
}

void cmd_ps(void) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
    terminal_writestring("\n=== Process List ===\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
    terminal_writestring("PID  STATE    NAME\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    for (uint32_t i = 0; i < 16; i++) {
        process_t *p = process_get(i);
        if (p) {
            cmd_print_dec(p->pid);
            terminal_writestring("    ");
            switch (p->state) {
                case PROCESS_STATE_READY:   terminal_writestring("READY   "); break;
                case PROCESS_STATE_RUNNING: terminal_writestring("RUNNING "); break;
                case PROCESS_STATE_BLOCKED: terminal_writestring("BLOCKED "); break;
                case PROCESS_STATE_ZOMBIE:  terminal_writestring("ZOMBIE  "); break;
                default: terminal_writestring("UNKNOWN "); break;
            }
            terminal_writestring(p->name);
            terminal_writestring("\n");
        }
    }
}

void cmd_cpuid(void) {
    uint32_t eax, ebx, ecx, edx;
    char vendor[13] = {0};
    
    __asm__ volatile ("cpuid" : "=a"(eax), "=b"(ebx), "=c"(ecx), "=d"(edx) : "a"(0));
    *(uint32_t*)&vendor[0] = ebx;
    *(uint32_t*)&vendor[4] = edx;
    *(uint32_t*)&vendor[8] = ecx;
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
    terminal_writestring("\n=== CPU Info ===\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring("Vendor: ");
    terminal_writestring(vendor);
    terminal_writestring("\n");
}
