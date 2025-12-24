/* MinimalOS 64-bit Kernel */

#include <stdint.h>
#include "idt.h"
#include "pic.h"

/* VGA text mode buffer */
#define VGA_BUFFER ((volatile uint16_t*)0xB8000)
#define VGA_WIDTH 80
#define VGA_HEIGHT 25

/* VGA colors */
#define VGA_COLOR(fg, bg) ((bg << 4) | fg)
#define VGA_ENTRY(c, color) ((uint16_t)(c) | ((uint16_t)(color) << 8))

/* Terminal state */
static int cursor_x = 0;
static int cursor_y = 0;
static uint8_t color = VGA_COLOR(15, 0);

/* Clear screen */
void clear_screen(void) {
    for (int i = 0; i < VGA_WIDTH * VGA_HEIGHT; i++) {
        VGA_BUFFER[i] = VGA_ENTRY(' ', color);
    }
    cursor_x = 0;
    cursor_y = 0;
}

/* Scroll screen up */
static void scroll(void) {
    for (int i = 0; i < VGA_WIDTH * (VGA_HEIGHT - 1); i++) {
        VGA_BUFFER[i] = VGA_BUFFER[i + VGA_WIDTH];
    }
    for (int i = 0; i < VGA_WIDTH; i++) {
        VGA_BUFFER[(VGA_HEIGHT - 1) * VGA_WIDTH + i] = VGA_ENTRY(' ', color);
    }
    cursor_y = VGA_HEIGHT - 1;
}

/* Print character */
void putchar(char c) {
    if (c == '\n') {
        cursor_x = 0;
        cursor_y++;
    } else if (c == '\r') {
        cursor_x = 0;
    } else if (c == '\t') {
        cursor_x = (cursor_x + 8) & ~7;
    } else {
        VGA_BUFFER[cursor_y * VGA_WIDTH + cursor_x] = VGA_ENTRY(c, color);
        cursor_x++;
    }
    
    if (cursor_x >= VGA_WIDTH) {
        cursor_x = 0;
        cursor_y++;
    }
    if (cursor_y >= VGA_HEIGHT) {
        scroll();
    }
}

/* Print string */
void puts(const char *s) {
    while (*s) putchar(*s++);
}

/* Print hex */
void print_hex(uint64_t n) {
    const char *hex = "0123456789ABCDEF";
    puts("0x");
    int started = 0;
    for (int i = 60; i >= 0; i -= 4) {
        int digit = (n >> i) & 0xF;
        if (digit || started || i == 0) {
            putchar(hex[digit]);
            started = 1;
        }
    }
}

/* Print decimal */
void print_dec(uint64_t n) {
    if (n == 0) {
        putchar('0');
        return;
    }
    char buf[21];
    int i = 0;
    while (n) {
        buf[i++] = '0' + (n % 10);
        n /= 10;
    }
    while (i--) putchar(buf[i]);
}

/* Set terminal color */
void set_color(uint8_t c) {
    color = c;
}

/* Exception names */
static const char *exception_names[] = {
    "Divide by Zero",          // 0
    "Debug",                   // 1
    "NMI",                     // 2
    "Breakpoint",              // 3
    "Overflow",                // 4
    "Bound Range Exceeded",    // 5
    "Invalid Opcode",          // 6
    "Device Not Available",    // 7
    "Double Fault",            // 8
    "Coprocessor Segment",     // 9
    "Invalid TSS",             // 10
    "Segment Not Present",     // 11
    "Stack Segment Fault",     // 12
    "General Protection Fault",// 13
    "Page Fault",              // 14
    "Reserved",                // 15
    "x87 FPU Error",           // 16
    "Alignment Check",         // 17
    "Machine Check",           // 18
    "SIMD Exception",          // 19
};

/* Exception handler */
void exception_handler(uint64_t int_num, uint64_t error_code) {
    set_color(VGA_COLOR(15, 4));  /* White on red */
    puts("\n\n*** EXCEPTION: ");
    if (int_num < 20) {
        puts(exception_names[int_num]);
    } else {
        puts("Unknown");
    }
    puts(" ***\n");
    
    set_color(VGA_COLOR(15, 0));
    puts("Interrupt: ");
    print_dec(int_num);
    puts("\nError code: ");
    print_hex(error_code);
    puts("\n\nSystem halted.");
    
    while (1) __asm__ volatile ("hlt");
}

/* Timer tick counter */
static volatile uint64_t timer_ticks = 0;

/* Timer IRQ handler */
void timer_handler(uint64_t int_num, uint64_t error_code) {
    (void)int_num;
    (void)error_code;
    timer_ticks++;
}

/* Keyboard IRQ handler */
void keyboard_handler(uint64_t int_num, uint64_t error_code) {
    (void)int_num;
    (void)error_code;
    
    /* Read scancode */
    uint8_t scancode;
    __asm__ volatile ("inb %1, %0" : "=a"(scancode) : "Nd"((uint16_t)0x60));
    
    /* Simple: just print scancode for now */
    if (!(scancode & 0x80)) {  /* Key press, not release */
        set_color(VGA_COLOR(14, 0));
        puts("Key: ");
        print_hex(scancode);
        puts(" ");
        set_color(VGA_COLOR(15, 0));
    }
}

/* Kernel main */
void kernel_main(uint64_t multiboot_info, uint64_t magic) {
    (void)multiboot_info;
    (void)magic;
    
    clear_screen();
    
    set_color(VGA_COLOR(11, 0));
    puts("========================================\n");
    puts("  MinimalOS 64-bit - Long Mode Active!\n");
    puts("========================================\n\n");
    
    set_color(VGA_COLOR(15, 0));
    
    /* Initialize PIC */
    puts("Initializing PIC... ");
    pic_init();
    set_color(VGA_COLOR(10, 0));
    puts("[OK]\n");
    set_color(VGA_COLOR(15, 0));
    
    /* Initialize IDT */
    puts("Initializing IDT... ");
    idt_init();
    set_color(VGA_COLOR(10, 0));
    puts("[OK]\n");
    set_color(VGA_COLOR(15, 0));
    
    /* Register exception handlers */
    for (int i = 0; i < 32; i++) {
        register_interrupt_handler(i, exception_handler);
    }
    
    /* Register timer handler (IRQ0 = INT 32) */
    register_interrupt_handler(32, timer_handler);
    
    /* Register keyboard handler (IRQ1 = INT 33) */
    register_interrupt_handler(33, keyboard_handler);
    
    /* Enable timer and keyboard IRQs */
    pic_enable_irq(0);  /* Timer */
    pic_enable_irq(1);  /* Keyboard */
    
    /* Enable interrupts */
    puts("Enabling interrupts... ");
    __asm__ volatile ("sti");
    set_color(VGA_COLOR(10, 0));
    puts("[OK]\n\n");
    set_color(VGA_COLOR(15, 0));
    
    puts("Press any key to test keyboard interrupt!\n\n");
    
    /* Main loop - show timer ticks */
    uint64_t last_ticks = 0;
    while (1) {
        if (timer_ticks != last_ticks && (timer_ticks % 100) == 0) {
            last_ticks = timer_ticks;
            set_color(VGA_COLOR(8, 0));
            puts("Uptime: ");
            print_dec(timer_ticks / 100);
            puts("s ");
        }
        __asm__ volatile ("hlt");
    }
}
