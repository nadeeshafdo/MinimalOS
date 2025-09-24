// Kernel-space shell for simplicity
// In a real OS, this would be user-space with proper syscalls

#include "../kernel/arch/x86_64/vga.h"
#include "../kernel/arch/x86_64/keyboard.h"

// Simple string functions
int strlen(const char *str) {
    int len = 0;
    while (str[len]) len++;
    return len;
}

int strcmp(const char *s1, const char *s2) {
    while (*s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return *s1 - *s2;
}

int strncmp(const char *s1, const char *s2, int n) {
    while (n-- && *s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return n < 0 ? 0 : *s1 - *s2;
}

void strcpy(char *dest, const char *src) {
    while (*src) {
        *dest++ = *src++;
    }
    *dest = '\0';
}

// Built-in commands
void cmd_help() {
    vga_print("MinimalOS Shell - Available Commands:\n");
    vga_print("  help    - Show this help message\n");
    vga_print("  echo    - Echo text to screen\n");
    vga_print("  clear   - Clear the screen\n");
    vga_print("  info    - Show system information\n");
    vga_print("  reboot  - Restart the system\n");
}

void cmd_echo(const char *args) {
    if (*args) {
        vga_print(args);
    }
    vga_print("\n");
}

void cmd_clear() {
    vga_init();
    vga_print("MinimalOS v1.0 - Interactive Shell\n");
    vga_print("Type 'help' for available commands.\n\n");
}

void cmd_info() {
    vga_print("MinimalOS v1.0\n");
    vga_print("Architecture: x86-64\n");
    vga_print("Boot method: BIOS\n");
    vga_print("Features: VGA output, keyboard input, basic shell\n");
    vga_print("Memory: Basic paging enabled\n");
    vga_print("Interrupts: Enabled (keyboard)\n");
}

void cmd_reboot() {
    vga_print("Rebooting system...\n");
    // Simple reboot via keyboard controller
    asm volatile("outb %0, %1" :: "a"((uint8_t)0xFE), "Nd"((uint16_t)0x64));
    while(1); // Should not reach here
}

void user_shell_main() {
    char buffer[256];
    int pos = 0;
    
    // Clear screen and show welcome
    cmd_clear();
    
    while (1) {
        // Show prompt
        vga_set_color(VGA_COLOR_LIGHT_CYAN);
        vga_print("shell> ");
        vga_set_color(VGA_COLOR_WHITE);
        
        // Read command
        pos = 0;
        while (1) {
            char ch = kb_read();
            
            if (ch == '\n') {
                vga_print("\n");
                break;
            } else if (ch == '\b') {
                if (pos > 0) {
                    pos--;
                    vga_print("\b");
                }
            } else if (ch >= 32 && ch <= 126 && pos < 255) {  // Printable characters
                buffer[pos++] = ch;
                vga_putchar(ch);
            }
        }
        
        buffer[pos] = '\0';
        
        // Skip empty commands
        if (pos == 0) continue;
        
        // Parse and execute commands
        if (strcmp(buffer, "help") == 0) {
            cmd_help();
        } else if (strcmp(buffer, "clear") == 0) {
            cmd_clear();
        } else if (strcmp(buffer, "info") == 0) {
            cmd_info();
        } else if (strcmp(buffer, "reboot") == 0) {
            cmd_reboot();
        } else if (strncmp(buffer, "echo ", 5) == 0) {
            cmd_echo(buffer + 5);
        } else if (strncmp(buffer, "echo", 4) == 0 && (buffer[4] == '\0' || buffer[4] == ' ')) {
            cmd_echo(buffer + 4);
        } else {
            vga_set_color(VGA_COLOR_LIGHT_RED);
            vga_print("Unknown command: ");
            vga_print(buffer);
            vga_print("\nType 'help' for available commands.\n");
            vga_set_color(VGA_COLOR_WHITE);
        }
    }
}