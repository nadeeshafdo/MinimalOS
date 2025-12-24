/* MinimalOS 64-bit Kernel Entry Point */

#include <stdint.h>

/* VGA text mode buffer */
#define VGA_BUFFER ((volatile uint16_t*)0xB8000)
#define VGA_WIDTH 80
#define VGA_HEIGHT 25

/* VGA colors */
#define VGA_COLOR(fg, bg) ((bg << 4) | fg)
#define VGA_ENTRY(c, color) ((uint16_t)(c) | ((uint16_t)(color) << 8))

static int cursor_x = 0;
static int cursor_y = 0;
static uint8_t color = VGA_COLOR(15, 0); /* White on black */

/* Clear screen */
void clear_screen(void) {
    for (int i = 0; i < VGA_WIDTH * VGA_HEIGHT; i++) {
        VGA_BUFFER[i] = VGA_ENTRY(' ', color);
    }
    cursor_x = 0;
    cursor_y = 0;
}

/* Print a single character */
void putchar(char c) {
    if (c == '\n') {
        cursor_x = 0;
        cursor_y++;
    } else {
        VGA_BUFFER[cursor_y * VGA_WIDTH + cursor_x] = VGA_ENTRY(c, color);
        cursor_x++;
        if (cursor_x >= VGA_WIDTH) {
            cursor_x = 0;
            cursor_y++;
        }
    }
    
    /* Simple scroll */
    if (cursor_y >= VGA_HEIGHT) {
        for (int i = 0; i < VGA_WIDTH * (VGA_HEIGHT - 1); i++) {
            VGA_BUFFER[i] = VGA_BUFFER[i + VGA_WIDTH];
        }
        for (int i = 0; i < VGA_WIDTH; i++) {
            VGA_BUFFER[(VGA_HEIGHT - 1) * VGA_WIDTH + i] = VGA_ENTRY(' ', color);
        }
        cursor_y = VGA_HEIGHT - 1;
    }
}

/* Print string */
void puts(const char *s) {
    while (*s) {
        putchar(*s++);
    }
}

/* Print hex number */
void print_hex(uint64_t n) {
    const char *hex = "0123456789ABCDEF";
    puts("0x");
    for (int i = 60; i >= 0; i -= 4) {
        putchar(hex[(n >> i) & 0xF]);
    }
}

/* Kernel main - 64-bit entry point */
void kernel_main(uint64_t multiboot_info, uint64_t magic) {
    clear_screen();
    
    color = VGA_COLOR(11, 0); /* Light cyan */
    puts("========================================\n");
    puts("  MinimalOS 64-bit - Long Mode Active!\n");
    puts("========================================\n\n");
    
    color = VGA_COLOR(15, 0); /* White */
    
    puts("Multiboot2 magic: ");
    print_hex(magic);
    puts("\n");
    
    puts("Multiboot2 info:  ");
    print_hex(multiboot_info);
    puts("\n\n");
    
    color = VGA_COLOR(10, 0); /* Light green */
    puts("[OK] ");
    color = VGA_COLOR(15, 0);
    puts("Successfully transitioned to 64-bit long mode!\n");
    
    color = VGA_COLOR(10, 0);
    puts("[OK] ");
    color = VGA_COLOR(15, 0);
    puts("4-level paging active (identity + higher-half)\n");
    
    color = VGA_COLOR(10, 0);
    puts("[OK] ");
    color = VGA_COLOR(15, 0);
    puts("64-bit GDT loaded\n\n");
    
    color = VGA_COLOR(14, 0); /* Yellow */
    puts("Next steps:\n");
    color = VGA_COLOR(7, 0); /* Gray */
    puts("  - Parse multiboot2 memory map\n");
    puts("  - Set up IDT (64-bit interrupt gates)\n");
    puts("  - Initialize physical memory manager\n");
    puts("  - Set up proper higher-half paging\n\n");
    
    color = VGA_COLOR(15, 0);
    puts("System halted.");
    
    /* Halt */
    while (1) {
        __asm__ volatile ("hlt");
    }
}
