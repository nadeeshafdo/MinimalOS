#include <stdint.h>
#include <stddef.h>
#include <kernel/shell.h>
#include <kernel/tty.h>
#include <kernel/keyboard.h>
#include <kernel/timer.h>
#include <kernel/pmm.h>

/* Command buffer */
#define CMD_BUFFER_SIZE 256
static char cmd_buffer[CMD_BUFFER_SIZE];
static size_t cmd_pos = 0;

/* String comparison */
static int strcmp(const char *s1, const char *s2) {
    while (*s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return *(unsigned char*)s1 - *(unsigned char*)s2;
}

/* String starts with */
static int strncmp(const char *s1, const char *s2, size_t n) {
    while (n && *s1 && (*s1 == *s2)) {
        s1++;
        s2++;
        n--;
    }
    if (n == 0) return 0;
    return *(unsigned char*)s1 - *(unsigned char*)s2;
}

/* Print hex number (currently unused but kept for future use) */
static void print_hex(uint32_t value) __attribute__((unused));
static void print_hex(uint32_t value) {
    char hex[11] = "0x00000000";
    const char* digits = "0123456789ABCDEF";
    
    for (int i = 9; i >= 2; i--) {
        hex[i] = digits[value & 0xF];
        value >>= 4;
    }
    
    terminal_writestring(hex);
}

/* Print decimal number */
static void print_dec(uint32_t value) {
    char buf[12];
    int i = 10;
    buf[11] = '\0';
    
    if (value == 0) {
        terminal_writestring("0");
        return;
    }
    
    while (value > 0) {
        buf[i--] = '0' + (value % 10);
        value /= 10;
    }
    
    terminal_writestring(&buf[i + 1]);
}

/* Command: help */
static void cmd_help(void) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
    terminal_writestring("\nAvailable commands:\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
    terminal_writestring("  help     - Show this help message\n");
    terminal_writestring("  clear    - Clear the screen\n");
    terminal_writestring("  info     - Show system information\n");
    terminal_writestring("  mem      - Show memory usage\n");
    terminal_writestring("  uptime   - Show system uptime\n");
    terminal_writestring("  echo     - Echo text (echo hello)\n");
    terminal_writestring("  reboot   - Reboot the system\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
}

/* Command: clear */
static void cmd_clear(void) {
    terminal_clear();
}

/* Command: info */
static void cmd_info(void) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("\n=== MinimalOS System Info ===\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring("Version:     0.1 Alpha\n");
    terminal_writestring("Arch:        x86 (32-bit)\n");
    terminal_writestring("Features:\n");
    terminal_writestring("  - VGA text mode\n");
    terminal_writestring("  - GDT/IDT/ISR/IRQ\n");
    terminal_writestring("  - PIT Timer\n");
    terminal_writestring("  - PS/2 Keyboard\n");
    terminal_writestring("  - Paging (Virtual Memory)\n");
    terminal_writestring("  - Kernel Heap\n");
    terminal_writestring("  - Multitasking Scheduler\n");
    terminal_writestring("  - System Calls\n");
}

/* Command: mem */
static void cmd_mem(void) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
    terminal_writestring("\n=== Memory Info ===\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring("Free memory: ");
    print_dec(pmm_get_free_memory() / 1024);
    terminal_writestring(" KB\n");
}

/* Command: uptime */
static void cmd_uptime(void) {
    uint32_t ticks = timer_get_ticks();
    uint32_t seconds = ticks / 100;  /* 100 Hz timer */
    uint32_t minutes = seconds / 60;
    seconds = seconds % 60;
    
    terminal_writestring("\nUptime: ");
    print_dec(minutes);
    terminal_writestring(" min ");
    print_dec(seconds);
    terminal_writestring(" sec\n");
}

/* Command: echo */
static void cmd_echo(const char *args) {
    terminal_writestring("\n");
    if (args && *args) {
        terminal_writestring(args);
    }
    terminal_writestring("\n");
}

/* Command: reboot */
static void cmd_reboot(void) {
    terminal_writestring("\nRebooting...\n");
    /* Triple fault reboot */
    __asm__ volatile (
        "lidt 0\n"
        "int $0x03"
    );
}

/* Execute command */
static void execute_command(void) {
    /* Null terminate */
    cmd_buffer[cmd_pos] = '\0';
    
    /* Skip empty commands */
    if (cmd_pos == 0) {
        terminal_writestring("\n> ");
        return;
    }
    
    /* Parse and execute */
    if (strcmp(cmd_buffer, "help") == 0) {
        cmd_help();
    } else if (strcmp(cmd_buffer, "clear") == 0) {
        cmd_clear();
    } else if (strcmp(cmd_buffer, "info") == 0) {
        cmd_info();
    } else if (strcmp(cmd_buffer, "mem") == 0) {
        cmd_mem();
    } else if (strcmp(cmd_buffer, "uptime") == 0) {
        cmd_uptime();
    } else if (strncmp(cmd_buffer, "echo ", 5) == 0) {
        cmd_echo(cmd_buffer + 5);
    } else if (strcmp(cmd_buffer, "echo") == 0) {
        cmd_echo("");
    } else if (strcmp(cmd_buffer, "reboot") == 0) {
        cmd_reboot();
    } else {
        terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_RED, VGA_COLOR_BLACK));
        terminal_writestring("\nUnknown command: ");
        terminal_writestring(cmd_buffer);
        terminal_writestring("\nType 'help' for available commands.\n");
        terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    }
    
    /* Reset buffer and show prompt */
    cmd_pos = 0;
    terminal_writestring("> ");
}

void shell_input(char c) {
    if (c == '\n') {
        /* Execute command on Enter */
        execute_command();
    } else if (c == '\b') {
        /* Handle backspace */
        if (cmd_pos > 0) {
            cmd_pos--;
            terminal_putchar('\b');
        }
    } else if (c >= 32 && c < 127) {
        /* Regular printable character */
        if (cmd_pos < CMD_BUFFER_SIZE - 1) {
            cmd_buffer[cmd_pos++] = c;
            terminal_putchar(c);
        }
    }
}

void shell_init(void) {
    cmd_pos = 0;
    terminal_writestring("> ");
}

void shell_run(void) {
    /* Shell runs via keyboard interrupts */
    /* Just idle here */
    while (1) {
        __asm__ volatile ("hlt");
    }
}
