/* Test commands: test, panic */
#include <stdint.h>
#include <kernel/commands.h>
#include <kernel/tty.h>
#include <kernel/timer.h>
#include <kernel/kheap.h>
#include <kernel/process.h>

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
    
    /* Timer test */
    terminal_writestring("Timer running... ");
    uint32_t t1 = timer_get_ticks();
    for (volatile int i = 0; i < 1000000; i++);
    uint32_t t2 = timer_get_ticks();
    if (t2 > t1) {
        terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
        terminal_writestring("[PASS]\n");
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
    __asm__ volatile ("cli; hlt");
}
