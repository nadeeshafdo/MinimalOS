/* Basic shell commands: help, clear, echo, reboot, halt */
#include <kernel/commands.h>
#include <kernel/tty.h>

void cmd_help(void) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
    terminal_writestring("\n=== MinimalOS Shell Commands ===\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
    terminal_writestring("\nBasic:\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
    terminal_writestring("  help           Show this help\n");
    terminal_writestring("  clear          Clear screen\n");
    terminal_writestring("  echo <text>    Print text\n");
    terminal_writestring("  reboot         Reboot system\n");
    terminal_writestring("  halt           Halt CPU\n");
    terminal_writestring("  poweroff       Power off system\n");
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
    terminal_writestring("\nSystem Info:\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
    terminal_writestring("  info           System information\n");
    terminal_writestring("  mem            Memory usage\n");
    terminal_writestring("  uptime         System uptime\n");
    terminal_writestring("  ps             List processes\n");
    terminal_writestring("  cpuid          CPU information\n");
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
    terminal_writestring("\nMemory Tools:\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
    terminal_writestring("  peek <addr>    Read memory (hex)\n");
    terminal_writestring("  poke <a> <v>   Write memory\n");
    terminal_writestring("  hexdump <addr> Dump 64 bytes\n");
    terminal_writestring("  alloc <size>   Allocate memory\n");
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
    terminal_writestring("\nDisplay:\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
    terminal_writestring("  color <fg> <bg> Set colors (0-15)\n");
    terminal_writestring("  banner         Show ASCII banner\n");
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
    terminal_writestring("\nTests:\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
    terminal_writestring("  test           Run system tests\n");
    terminal_writestring("  cpufreq        Estimate CPU speed\n");
    terminal_writestring("  panic          Trigger kernel panic\n");
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
}

void cmd_clear(void) {
    terminal_clear();
}

void cmd_echo(const char *args) {
    terminal_writestring("\n");
    if (args && *args) terminal_writestring(args);
    terminal_writestring("\n");
}

void cmd_reboot(void) {
    terminal_writestring("\nRebooting...\n");
    __asm__ volatile ("lidt 0\n" "int $0x03");
}

void cmd_halt(void) {
    terminal_writestring("\nSystem halted. Press reset to restart.\n");
    __asm__ volatile ("cli; hlt");
}

/* I/O port operations */
static inline void outw(uint16_t port, uint16_t value) {
    __asm__ volatile ("outw %0, %1" : : "a"(value), "Nd"(port));
}

static inline void outb_basic(uint16_t port, uint8_t value) {
    __asm__ volatile ("outb %0, %1" : : "a"(value), "Nd"(port));
}

void cmd_poweroff(void) {
    terminal_writestring("\nShutting down...\n");
    terminal_writestring("Attempting ACPI power off...\n");
    
    /* Method 1: QEMU shutdown (via isa-debug-exit or newer QEMU) */
    outw(0x604, 0x2000);  /* QEMU ACPI shutdown */
    
    /* Method 2: Bochs/older QEMU */
    outw(0xB004, 0x2000);
    
    /* Method 3: VirtualBox ACPI */
    outw(0x4004, 0x3400);
    
    /* Note: Real hardware ACPI power off requires parsing ACPI tables */
    /* to find the PM1a_CNT register. Without that, we can only halt. */
    
    /* If we're still here, ACPI didn't work - just halt safely */
    terminal_writestring("\nACPI power off not available.\n");
    terminal_writestring("System halted - you can safely turn off power.\n");
    __asm__ volatile ("cli");
    while (1) {
        __asm__ volatile ("hlt");
    }
}

