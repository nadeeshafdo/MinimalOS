// Kernel-space shell for MinimalOS
// Provides interactive command-line interface

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

// Built-in shell commands
void cmd_help() {
    vga_set_color(VGA_COLOR_LIGHT_CYAN);
    vga_print("===== MinimalOS Shell - Command Reference =====\n\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("Available Commands:\n");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("  help       - Display this help message\n");
    vga_print("  echo [...]  - Echo text to the screen\n");
    vga_print("  clear      - Clear the screen\n");
    vga_print("  info       - Show system information\n");
    vga_print("  version    - Display OS version and build info\n");
    vga_print("  mem        - Display memory layout\n");
    vga_print("  cpu        - Show CPU information\n");
    vga_print("  reboot     - Restart the system\n");
    vga_print("  shutdown   - Halt the system\n\n");
}

void cmd_echo(const char *args) {
    if (*args) {
        vga_print(args);
    }
    vga_print("\n");
}

void cmd_clear() {
    vga_init();
    vga_set_color(VGA_COLOR_LIGHT_GREEN);
    vga_print("MinimalOS v2.0 - Interactive Shell\n");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("Type 'help' for available commands.\n\n");
}

void cmd_info() {
    vga_set_color(VGA_COLOR_LIGHT_CYAN);
    vga_print("===== System Information =====\n\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("OS Name:        ");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("MinimalOS\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("Version:        ");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("2.0 (Production)\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("Architecture:   ");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("x86-64 (64-bit)\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("Boot Method:    ");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("BIOS Legacy Boot\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("CPU Mode:       ");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("Long Mode (64-bit)\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("Features:       ");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("VGA, Keyboard, Interrupts, Syscalls\n\n");
}

void cmd_version() {
    vga_set_color(VGA_COLOR_LIGHT_GREEN);
    vga_print("MinimalOS Version 2.0 (Production Build)\n");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("Copyright (c) 2025 Nadeesha Fernando\n");
    vga_print("Licensed under MIT License\n\n");
    vga_print("Build Date:     December 2025\n");
    vga_print("Compiler:       GCC (freestanding)\n");
    vga_print("Assembler:      NASM\n");
    vga_print("Target:         x86-64\n\n");
}

void cmd_mem() {
    vga_set_color(VGA_COLOR_LIGHT_CYAN);
    vga_print("===== Memory Layout =====\n\n");
    
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("0x00007C00  Bootloader (512 bytes)\n");
    vga_print("0x00008000  Kernel temporary load\n");
    vga_print("0x00070000  Page tables (PML4/PDP/PD)\n");
    vga_print("0x00090000  Kernel stack\n");
    vga_print("0x000B8000  VGA text buffer\n");
    vga_print("0x00100000  Kernel code & data (1MB)\n");
    vga_print("0x00200000  TSS kernel stack\n\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("Memory Management: ");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("Identity paging (4MB)\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("Page Size:         ");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("2MB (huge pages)\n\n");
}

void cmd_cpu() {
    vga_set_color(VGA_COLOR_LIGHT_CYAN);
    vga_print("===== CPU Information =====\n\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("CPU Mode:       ");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print("x86-64 Long Mode\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("Privilege Level:");
    vga_set_color(VGA_COLOR_LIGHT_GREY);
    vga_print(" Ring 0 (Kernel)\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("Paging:         ");
    vga_set_color(VGA_COLOR_LIGHT_GREEN);
    vga_print("Enabled (PAE + Long Mode)\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("Interrupts:     ");
    vga_set_color(VGA_COLOR_LIGHT_GREEN);
    vga_print("Enabled (IDT configured)\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("GDT:            ");
    vga_set_color(VGA_COLOR_LIGHT_GREEN);
    vga_print("Configured\n");
    
    vga_set_color(VGA_COLOR_WHITE);
    vga_print("TSS:            ");
    vga_set_color(VGA_COLOR_LIGHT_GREEN);
    vga_print("Configured\n\n");
}

void cmd_reboot() {
    vga_set_color(VGA_COLOR_LIGHT_RED);
    vga_print("Rebooting system...\n");
    // Trigger reset via keyboard controller
    asm volatile("outb %0, %1" :: "a"((unsigned char)0xFE), "Nd"((unsigned short)0x64));
    while(1); // Should not reach here
}

void cmd_shutdown() {
    vga_set_color(VGA_COLOR_LIGHT_BROWN); // Light brown/yellow
    vga_print("System halted. You can now power off.\n");
    vga_print("Press the reset button to restart.\n");
    while (1) {
        asm volatile("cli; hlt");
    }
}

// Main shell loop
void user_shell_main() {
    char buffer[256];
    int pos = 0;
    
    // Display welcome message
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
        vga_set_color(VGA_COLOR_LIGHT_GREY);
        
        if (strcmp(buffer, "help") == 0) {
            cmd_help();
        } else if (strcmp(buffer, "clear") == 0) {
            cmd_clear();
        } else if (strcmp(buffer, "info") == 0) {
            cmd_info();
        } else if (strcmp(buffer, "version") == 0) {
            cmd_version();
        } else if (strcmp(buffer, "mem") == 0) {
            cmd_mem();
        } else if (strcmp(buffer, "cpu") == 0) {
            cmd_cpu();
        } else if (strcmp(buffer, "reboot") == 0) {
            cmd_reboot();
        } else if (strcmp(buffer, "shutdown") == 0) {
            cmd_shutdown();
        } else if (strncmp(buffer, "echo ", 5) == 0) {
            cmd_echo(buffer + 5);
        } else if (strcmp(buffer, "echo") == 0) {
            cmd_echo("");
        } else {
            vga_set_color(VGA_COLOR_LIGHT_RED);
            vga_print("Error: Unknown command '");
            vga_print(buffer);
            vga_print("'\n");
            vga_set_color(VGA_COLOR_LIGHT_GREY);
            vga_print("Type 'help' for available commands.\n");
        }
    }
}