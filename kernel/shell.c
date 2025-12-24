/* Shell - command input and dispatch */
#include <stdint.h>
#include <stddef.h>
#include <kernel/shell.h>
#include <kernel/tty.h>
#include <kernel/commands.h>

/* Command buffer */
#define CMD_BUFFER_SIZE 256
static char cmd_buffer[CMD_BUFFER_SIZE];
static size_t cmd_pos = 0;

/* String comparison */
static int strcmp(const char *s1, const char *s2) {
    while (*s1 && (*s1 == *s2)) { s1++; s2++; }
    return *(unsigned char*)s1 - *(unsigned char*)s2;
}

static int strncmp(const char *s1, const char *s2, size_t n) {
    while (n && *s1 && (*s1 == *s2)) { s1++; s2++; n--; }
    if (n == 0) return 0;
    return *(unsigned char*)s1 - *(unsigned char*)s2;
}

/* Command dispatcher */
static void execute_command(void) {
    cmd_buffer[cmd_pos] = '\0';
    
    if (cmd_pos == 0) {
        terminal_writestring("\n> ");
        return;
    }
    
    /* Dispatch commands */
    if (strcmp(cmd_buffer, "help") == 0) cmd_help();
    else if (strcmp(cmd_buffer, "clear") == 0) cmd_clear();
    else if (strcmp(cmd_buffer, "info") == 0) cmd_info();
    else if (strcmp(cmd_buffer, "mem") == 0) cmd_mem();
    else if (strcmp(cmd_buffer, "uptime") == 0) cmd_uptime();
    else if (strncmp(cmd_buffer, "echo ", 5) == 0) cmd_echo(cmd_buffer + 5);
    else if (strcmp(cmd_buffer, "echo") == 0) cmd_echo("");
    else if (strcmp(cmd_buffer, "reboot") == 0) cmd_reboot();
    else if (strcmp(cmd_buffer, "halt") == 0) cmd_halt();
    else if (strcmp(cmd_buffer, "ps") == 0) cmd_ps();
    else if (strcmp(cmd_buffer, "cpuid") == 0) cmd_cpuid();
    else if (strncmp(cmd_buffer, "peek ", 5) == 0) cmd_peek(cmd_buffer + 5);
    else if (strncmp(cmd_buffer, "poke ", 5) == 0) cmd_poke(cmd_buffer + 5);
    else if (strncmp(cmd_buffer, "hexdump ", 8) == 0) cmd_hexdump(cmd_buffer + 8);
    else if (strncmp(cmd_buffer, "alloc ", 6) == 0) cmd_alloc(cmd_buffer + 6);
    else if (strncmp(cmd_buffer, "color ", 6) == 0) cmd_color(cmd_buffer + 6);
    else if (strcmp(cmd_buffer, "color") == 0) cmd_color("");
    else if (strcmp(cmd_buffer, "banner") == 0) cmd_banner();
    else if (strcmp(cmd_buffer, "test") == 0) cmd_test();
    else if (strcmp(cmd_buffer, "panic") == 0) cmd_panic();
    else {
        terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_RED, VGA_COLOR_BLACK));
        terminal_writestring("\nUnknown: ");
        terminal_writestring(cmd_buffer);
        terminal_writestring(" (try 'help')\n");
        terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    }
    
    cmd_pos = 0;
    terminal_writestring("> ");
}

void shell_input(char c) {
    if (c == '\n') {
        execute_command();
    } else if (c == '\b') {
        if (cmd_pos > 0) {
            cmd_pos--;
            terminal_putchar('\b');
        }
    } else if (c >= 32 && c < 127) {
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
    while (1) {
        __asm__ volatile ("hlt");
    }
}
