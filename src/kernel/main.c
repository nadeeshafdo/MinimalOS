// MinimalOS v2.0 - Interactive Shell OS
// 32-bit kernel with VGA output, keyboard input, and command shell

#include "stdint.h"
#include "stddef.h"

// VGA text mode
#define VGA_WIDTH 80
#define VGA_HEIGHT 25
#define VGA_MEMORY 0xB8000

// Keyboard
#define KEYBOARD_DATA_PORT 0x60
#define KEYBOARD_STATUS_PORT 0x64

// PIC (Programmable Interrupt Controller)
#define PIC1_COMMAND 0x20
#define PIC1_DATA 0x21
#define PIC2_COMMAND 0xA0
#define PIC2_DATA 0xA1

// Color definitions
#define COLOR_BLACK 0
#define COLOR_BLUE 1
#define COLOR_GREEN 2
#define COLOR_CYAN 3
#define COLOR_RED 4
#define COLOR_MAGENTA 5
#define COLOR_BROWN 6
#define COLOR_LIGHT_GREY 7
#define COLOR_DARK_GREY 8
#define COLOR_LIGHT_BLUE 9
#define COLOR_LIGHT_GREEN 10
#define COLOR_LIGHT_CYAN 11
#define COLOR_LIGHT_RED 12
#define COLOR_LIGHT_MAGENTA 13
#define COLOR_LIGHT_BROWN 14
#define COLOR_WHITE 15

// VGA globals
static volatile uint16_t* vga = (volatile uint16_t*)VGA_MEMORY;
static int vga_row = 0;
static int vga_col = 0;
static uint8_t vga_color = 0x0F; // White on black

// Keyboard globals
static char keyboard_buffer[256];
static int kb_read_pos = 0;
static int kb_write_pos = 0;

// Command input buffer
#define CMD_BUFFER_SIZE 256
static char cmd_buffer[CMD_BUFFER_SIZE];
static int cmd_pos = 0;

// Port I/O functions
static inline void outb(uint16_t port, uint8_t value) {
    asm volatile("outb %0, %1" : : "a"(value), "Nd"(port));
}

static inline uint8_t inb(uint16_t port) {
    uint8_t value;
    asm volatile("inb %1, %0" : "=a"(value) : "Nd"(port));
    return value;
}

// String functions
static size_t strlen(const char* str) {
    size_t len = 0;
    while (str[len]) len++;
    return len;
}

static int strcmp(const char* s1, const char* s2) {
    while (*s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return *(unsigned char*)s1 - *(unsigned char*)s2;
}

static int strncmp(const char* s1, const char* s2, size_t n) {
    while (n && *s1 && (*s1 == *s2)) {
        s1++;
        s2++;
        n--;
    }
    if (n == 0) return 0;
    return *(unsigned char*)s1 - *(unsigned char*)s2;
}

static void* memset(void* dest, int val, size_t n) {
    uint8_t* d = (uint8_t*)dest;
    while (n--) *d++ = (uint8_t)val;
    return dest;
}

// VGA functions
static void vga_set_color(uint8_t fg, uint8_t bg) {
    vga_color = (bg << 4) | (fg & 0x0F);
}

static void vga_scroll() {
    // Move all lines up by one
    for (int y = 0; y < VGA_HEIGHT - 1; y++) {
        for (int x = 0; x < VGA_WIDTH; x++) {
            vga[y * VGA_WIDTH + x] = vga[(y + 1) * VGA_WIDTH + x];
        }
    }
    // Clear last line
    for (int x = 0; x < VGA_WIDTH; x++) {
        vga[(VGA_HEIGHT - 1) * VGA_WIDTH + x] = ((uint16_t)vga_color << 8) | ' ';
    }
    vga_row = VGA_HEIGHT - 1;
}

static void vga_putchar(char c) {
    if (c == '\n') {
        vga_col = 0;
        vga_row++;
    } else if (c == '\b') {
        if (vga_col > 0) {
            vga_col--;
            vga[vga_row * VGA_WIDTH + vga_col] = ((uint16_t)vga_color << 8) | ' ';
        }
        return;
    } else {
        vga[vga_row * VGA_WIDTH + vga_col] = ((uint16_t)vga_color << 8) | c;
        vga_col++;
        if (vga_col >= VGA_WIDTH) {
            vga_col = 0;
            vga_row++;
        }
    }
    
    if (vga_row >= VGA_HEIGHT) {
        vga_scroll();
    }
}

static void vga_print(const char* str) {
    while (*str) {
        vga_putchar(*str++);
    }
}

static void vga_clear() {
    for (int i = 0; i < VGA_WIDTH * VGA_HEIGHT; i++) {
        vga[i] = ((uint16_t)vga_color << 8) | ' ';
    }
    vga_row = 0;
    vga_col = 0;
}

static void vga_print_hex(uint32_t value) {
    char hex[] = "0123456789ABCDEF";
    vga_print("0x");
    for (int i = 7; i >= 0; i--) {
        vga_putchar(hex[(value >> (i * 4)) & 0xF]);
    }
}

// Keyboard scancode to ASCII mapping (US layout)
static const char scancode_to_ascii[] = {
    0, 0, '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\b',
    '\t', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n',
    0, 'a', 's', 'd', 'f', 'g', 'h', '  j', 'k', 'l', ';', '\'', '`', 0,
    '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', 0, '*', 0, ' '
};

// Keyboard interrupt handler (called from IDT)
void keyboard_handler() {
    uint8_t scancode = inb(KEYBOARD_DATA_PORT);
    
    // Only handle key press events (bit 7 clear)
    if (!(scancode & 0x80)) {
        if (scancode < sizeof(scancode_to_ascii)) {
            char ascii = scancode_to_ascii[scancode];
            if (ascii) {
                keyboard_buffer[kb_write_pos] = ascii;
                kb_write_pos = (kb_write_pos + 1) % 256;
            }
        }
    }
    
    // Send End of Interrupt to PIC
    outb(PIC1_COMMAND, 0x20);
}

// Get character from keyboard buffer
static char kb_getchar() {
    while (kb_read_pos == kb_write_pos) {
        asm volatile("hlt"); // Wait for interrupt
    }
    char c = keyboard_buffer[kb_read_pos];
    kb_read_pos = (kb_read_pos + 1) % 256;
    return c;
}

// IDT (Interrupt Descriptor Table) setup
struct idt_entry {
    uint16_t offset_low;
    uint16_t selector;
    uint8_t zero;
    uint8_t type_attr;
    uint16_t offset_high;
} __attribute__((packed));

struct idt_ptr {
    uint16_t limit;
    uint32_t base;
} __attribute__((packed));

static struct idt_entry idt[256];
static struct idt_ptr idtp;

// Assembly stub for keyboard interrupt
extern void keyboard_interrupt_stub();
asm(
    ".global keyboard_interrupt_stub\n"
    "keyboard_interrupt_stub:\n"
    "   pusha\n"
    "   call keyboard_handler\n"
    "   popa\n"
    "   iret\n"
);

static void idt_set_gate(uint8_t num, uint32_t handler) {
    idt[num].offset_low = handler & 0xFFFF;
    idt[num].selector = 0x08; // Kernel code segment
    idt[num].zero = 0;
    idt[num].type_attr = 0x8E; // Present, DPL=0, 32-bit interrupt gate
    idt[num].offset_high = (handler >> 16) & 0xFFFF;
}

static void init_idt() {
    idtp.limit = sizeof(idt) - 1;
    idtp.base = (uint32_t)&idt;
    
    memset(&idt, 0, sizeof(idt));
    
    // Set keyboard interrupt (IRQ1 = INT 0x21)
    idt_set_gate(0x21, (uint32_t)keyboard_interrupt_stub);
    
    // Load IDT
    asm volatile("lidt %0" : : "m"(idtp));
    
    // Remap PIC
    outb(PIC1_COMMAND, 0x11);
    outb(PIC2_COMMAND, 0x11);
    outb(PIC1_DATA, 0x20);
    outb(PIC2_DATA, 0x28);
    outb(PIC1_DATA, 0x04);
    outb(PIC2_DATA, 0x02);
    outb(PIC1_DATA, 0x01);
    outb(PIC2_DATA, 0x01);
    outb(PIC1_DATA, 0xFD); // Enable only keyboard (IRQ1)
    outb(PIC2_DATA, 0xFF);
    
    // Enable interrupts
    asm volatile("sti");
}

// Shell command handlers
static void cmd_help() {
    vga_set_color(COLOR_LIGHT_CYAN, COLOR_BLACK);
    vga_print("Available Commands:\n");
    vga_set_color(COLOR_WHITE, COLOR_BLACK);
    vga_print("  help      - Show this help message\n");
    vga_print("  clear     - Clear the screen\n");
    vga_print("  echo      - Echo arguments\n");
    vga_print("  version   - Show OS version\n");
    vga_print("  info      - Display system information\n");
    vga_print("  mem       - Show memory information\n");
    vga_print("  reboot    - Reboot the system\n");
    vga_print("  shutdown  - Halt the system\n");
}

static void cmd_clear() {
    vga_clear();
}

static void cmd_echo(const char* args) {
    vga_print(args);
    vga_print("\n");
}

static void cmd_version() {
    vga_set_color(COLOR_LIGHT_GREEN, COLOR_BLACK);
    vga_print("MinimalOS v2.0 - Production Shell OS\n");
    vga_set_color(COLOR_WHITE, COLOR_BLACK);
    vga_print("Build: Multiboot/GRUB Edition\n");
    vga_print("Architecture: i386 (32-bit Protected Mode)\n");
    vga_print("Copyright (c) 2025\n");
}

static void cmd_info() {
    vga_set_color(COLOR_LIGHT_CYAN, COLOR_BLACK);
    vga_print("System Information:\n");
    vga_set_color(COLOR_WHITE, COLOR_BLACK);
    vga_print("  OS: MinimalOS v2.0\n");
    vga_print("  Kernel: 32-bit protected mode\n");
    vga_print("  VGA: Text mode 80x25\n");
    vga_print("  Keyboard: PS/2 with interrupt support\n");
    vga_print("  Bootloader: Multiboot-compliant\n");
}

static void cmd_mem() {
    vga_set_color(COLOR_LIGHT_CYAN, COLOR_BLACK);
    vga_print("Memory Information:\n");
    vga_set_color(COLOR_WHITE, COLOR_BLACK);
    vga_print("  VGA Buffer: ");
    vga_print_hex(VGA_MEMORY);
    vga_print("\n  Kernel: ~9.5 KB\n");
    vga_print("  Stack: 16 KB\n");
}

static void cmd_reboot() {
    vga_set_color(COLOR_LIGHT_RED, COLOR_BLACK);
    vga_print("Rebooting system...\n");
    // Pulse reset line
    uint8_t temp = inb(0x64);
    while (temp & 0x02) temp = inb(0x64);
    outb(0x64, 0xFE);
    while (1) asm volatile("hlt");
}

static void cmd_shutdown() {
    vga_set_color(COLOR_LIGHT_BROWN, COLOR_BLACK);
    vga_print("System halted. You can now power off.\n");
    while (1) asm volatile("cli; hlt");
}

// Command parser and executor
static void execute_command(const char* cmd) {
    // Skip leading spaces
    while (*cmd == ' ') cmd++;
    if (*cmd == '\0') return;
    
    // Find command end and args start
    const char* args = cmd;
    while (*args && *args != ' ') args++;
    size_t cmd_len = args - cmd;
    while (*args == ' ') args++;
    
    if (strncmp(cmd, "help", cmd_len) == 0 && cmd_len == 4) {
        cmd_help();
    } else if (strncmp(cmd, "clear", cmd_len) == 0 && cmd_len == 5) {
        cmd_clear();
    } else if (strncmp(cmd, "echo", cmd_len) == 0 && cmd_len == 4) {
        cmd_echo(args);
    } else if (strncmp(cmd, "version", cmd_len) == 0 && cmd_len == 7) {
        cmd_version();
    } else if (strncmp(cmd, "info", cmd_len) == 0 && cmd_len == 4) {
        cmd_info();
    } else if (strncmp(cmd, "mem", cmd_len) == 0 && cmd_len == 3) {
        cmd_mem();
    } else if (strncmp(cmd, "reboot", cmd_len) == 0 && cmd_len == 6) {
        cmd_reboot();
    } else if (strncmp(cmd, "shutdown", cmd_len) == 0 && cmd_len == 8) {
        cmd_shutdown();
    } else {
        vga_set_color(COLOR_LIGHT_RED, COLOR_BLACK);
        vga_print("Unknown command: ");
        while (cmd_len--) vga_putchar(*cmd++);
        vga_print("\nType 'help' for available commands.\n");
        vga_set_color(COLOR_WHITE, COLOR_BLACK);
    }
}

// Shell main loop
static void shell_main() {
    vga_clear();
    
    // Welcome message
    vga_set_color(COLOR_LIGHT_CYAN, COLOR_BLACK);
    vga_print("======================================\n");
    vga_print("  MinimalOS v2.0 - Interactive Shell  \n");
    vga_print("======================================\n\n");
    vga_set_color(COLOR_WHITE, COLOR_BLACK);
    vga_print("Welcome to MinimalOS! Type 'help' for commands.\n\n");
    
    while (1) {
        // Show prompt
        vga_set_color(COLOR_LIGHT_GREEN, COLOR_BLACK);
        vga_print("shell> ");
        vga_set_color(COLOR_WHITE, COLOR_BLACK);
        
        // Read command
        cmd_pos = 0;
        memset(cmd_buffer, 0, CMD_BUFFER_SIZE);
        
        while (1) {
            char c = kb_getchar();
            
            if (c == '\n') {
                vga_putchar('\n');
                cmd_buffer[cmd_pos] = '\0';
                break;
            } else if (c == '\b') {
                if (cmd_pos > 0) {
                    cmd_pos--;
                    cmd_buffer[cmd_pos] = '\0';
                    vga_putchar('\b');
                }
            } else if (cmd_pos < CMD_BUFFER_SIZE - 1) {
                cmd_buffer[cmd_pos++] = c;
                vga_putchar(c);
            }
        }
        
        // Execute command
        execute_command(cmd_buffer);
    }
}

// Kernel entry point
void kernel_main(void) {
    // Initialize IDT and keyboard
    init_idt();
    
    // Start shell
    shell_main();
    
    // Should never reach here
    while (1) asm volatile("hlt");
}
