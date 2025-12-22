// Simplified kernel for GRUB Multiboot
// VGA text mode driver
#define VGA_WIDTH 80
#define VGA_HEIGHT 25
#define VGA_MEMORY 0xB8000

volatile unsigned short* vga = (volatile unsigned short*)VGA_MEMORY;
int vga_row = 0;
int vga_col = 0;

void terminal_clear() {
    for (int i = 0; i < VGA_WIDTH * VGA_HEIGHT; i++) {
        vga[i] = 0x0F00 | ' '; // White on black, space
    }
    vga_row = 0;
    vga_col = 0;
}

void terminal_putchar(char c) {
    if (c == '\n') {
        vga_col = 0;
        vga_row++;
    } else {
        vga[vga_row * VGA_WIDTH + vga_col] = 0x0F00 | c; // White on black
        vga_col++;
        if (vga_col >= VGA_WIDTH) {
            vga_col = 0;
            vga_row++;
        }
    }
    if (vga_row >= VGA_HEIGHT) {
        vga_row = 0; // Simple wrap
    }
}

void terminal_writestring(const char* str) {
    while (*str) {
        terminal_putchar(*str);
        str++;
    }
}

void kernel_main(void) {
    terminal_clear();
    
    terminal_writestring("MinimalOS v2.0 - GRUB Edition\n");
    terminal_writestring("============================\n\n");
    terminal_writestring("Kernel booted successfully!\n");
    terminal_writestring("GRUB handled all bootloader complexity.\n\n");
    terminal_writestring("System is running in 32-bit protected mode.\n");
    terminal_writestring("VGA text mode active at 0xB8000.\n");
    
    // Infinite loop
    while(1) {
        asm volatile("hlt");
    }
}
