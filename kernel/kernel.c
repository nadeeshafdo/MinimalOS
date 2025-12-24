/* MinimalOS 64-bit Kernel */

#include <stdint.h>
#include "idt.h"
#include "pic.h"
#include "timer.h"
#include "keyboard.h"
#include "multiboot2.h"
#include "pmm.h"
#include "kheap.h"

/* VGA text mode */
#define VGA_BUFFER ((volatile uint16_t*)0xB8000)
#define VGA_WIDTH 80
#define VGA_HEIGHT 25
#define VGA_COLOR(fg, bg) ((bg << 4) | fg)
#define VGA_ENTRY(c, color) ((uint16_t)(c) | ((uint16_t)(color) << 8))

static int cursor_x = 0;
static int cursor_y = 0;
static uint8_t color = VGA_COLOR(15, 0);

/* Multiboot info (saved for later use) */
static uint64_t saved_mb_info = 0;

void clear_screen(void) {
    for (int i = 0; i < VGA_WIDTH * VGA_HEIGHT; i++) {
        VGA_BUFFER[i] = VGA_ENTRY(' ', color);
    }
    cursor_x = 0;
    cursor_y = 0;
}

static void scroll(void) {
    for (int i = 0; i < VGA_WIDTH * (VGA_HEIGHT - 1); i++) {
        VGA_BUFFER[i] = VGA_BUFFER[i + VGA_WIDTH];
    }
    for (int i = 0; i < VGA_WIDTH; i++) {
        VGA_BUFFER[(VGA_HEIGHT - 1) * VGA_WIDTH + i] = VGA_ENTRY(' ', color);
    }
    cursor_y = VGA_HEIGHT - 1;
}

void putchar(char c) {
    if (c == '\n') { cursor_x = 0; cursor_y++; }
    else if (c == '\r') { cursor_x = 0; }
    else if (c == '\t') { cursor_x = (cursor_x + 8) & ~7; }
    else if (c == '\b') {
        if (cursor_x > 0) { cursor_x--; VGA_BUFFER[cursor_y * VGA_WIDTH + cursor_x] = VGA_ENTRY(' ', color); }
    } else {
        VGA_BUFFER[cursor_y * VGA_WIDTH + cursor_x] = VGA_ENTRY(c, color);
        cursor_x++;
    }
    if (cursor_x >= VGA_WIDTH) { cursor_x = 0; cursor_y++; }
    if (cursor_y >= VGA_HEIGHT) scroll();
}

void puts(const char *s) { while (*s) putchar(*s++); }

void print_dec(uint64_t n) {
    if (n == 0) { putchar('0'); return; }
    char buf[21]; int i = 0;
    while (n) { buf[i++] = '0' + (n % 10); n /= 10; }
    while (i--) putchar(buf[i]);
}

void print_hex(uint64_t n) {
    const char *hex = "0123456789ABCDEF";
    puts("0x");
    int started = 0;
    for (int i = 60; i >= 0; i -= 4) {
        int d = (n >> i) & 0xF;
        if (d || started || i == 0) { putchar(hex[d]); started = 1; }
    }
}

void set_color(uint8_t c) { color = c; }

/* String comparison */
static int strcmp(const char *s1, const char *s2) {
    while (*s1 && *s1 == *s2) { s1++; s2++; }
    return *s1 - *s2;
}

/* Exception handler */
static const char *exceptions[] = {
    "Divide by Zero", "Debug", "NMI", "Breakpoint", "Overflow",
    "Bound Range", "Invalid Opcode", "Device N/A", "Double Fault",
    "Coproc Seg", "Invalid TSS", "Seg Not Present", "Stack Fault",
    "General Protection", "Page Fault", "Reserved", "x87 FPU",
    "Alignment Check", "Machine Check", "SIMD Exception"
};

void exception_handler(uint64_t num, uint64_t err) {
    set_color(VGA_COLOR(15, 4));
    puts("\n*** EXCEPTION: ");
    if (num < 20) puts(exceptions[num]);
    puts(" ***\n");
    set_color(VGA_COLOR(15, 0));
    puts("INT: "); print_dec(num);
    puts(" ERR: "); print_hex(err);
    puts("\nSystem halted.");
    while (1) __asm__ volatile ("hlt");
}

/* Simple shell */
#define CMD_BUFFER_SIZE 256
static char cmd_buffer[CMD_BUFFER_SIZE];
static int cmd_pos = 0;

void shell_prompt(void) {
    set_color(VGA_COLOR(10, 0)); puts("\nminimal");
    set_color(VGA_COLOR(15, 0)); puts("> ");
}

void shell_execute(void) {
    cmd_buffer[cmd_pos] = '\0';
    if (cmd_pos == 0) { shell_prompt(); return; }
    
    if (strcmp(cmd_buffer, "help") == 0) {
        puts("\n");
        set_color(VGA_COLOR(11, 0)); puts("Available commands:\n");
        set_color(VGA_COLOR(15, 0));
        puts("  help     - Show this help\n");
        puts("  clear    - Clear screen\n");
        puts("  uptime   - Show system uptime\n");
        puts("  mem      - Show memory info\n");
        puts("  alloc    - Test memory allocation\n");
        puts("  reboot   - Reboot system\n");
        puts("  halt     - Halt CPU\n");
    }
    else if (strcmp(cmd_buffer, "clear") == 0) {
        clear_screen();
    }
    else if (strcmp(cmd_buffer, "uptime") == 0) {
        puts("\nUptime: ");
        print_dec(timer_get_uptime());
        puts(" seconds (");
        print_dec(timer_get_ticks());
        puts(" ticks)\n");
    }
    else if (strcmp(cmd_buffer, "mem") == 0) {
        puts("\n");
        set_color(VGA_COLOR(11, 0)); puts("Memory Information:\n");
        set_color(VGA_COLOR(15, 0));
        
        puts("  Physical Memory:\n");
        puts("    Total:  ");
        print_dec(pmm_get_total_memory() / 1024 / 1024);
        puts(" MB\n");
        puts("    Used:   ");
        print_dec(pmm_get_used_memory() / 1024 / 1024);
        puts(" MB\n");
        puts("    Free:   ");
        print_dec(pmm_get_free_memory() / 1024 / 1024);
        puts(" MB\n\n");
        
        puts("  Kernel Heap:\n");
        puts("    Used:   ");
        print_dec(kheap_get_used() / 1024);
        puts(" KB\n");
        puts("    Free:   ");
        print_dec(kheap_get_free() / 1024);
        puts(" KB\n");
    }
    else if (strcmp(cmd_buffer, "alloc") == 0) {
        puts("\nTesting memory allocation...\n");
        
        void *p1 = kmalloc(64);
        puts("  kmalloc(64) = "); print_hex((uint64_t)p1); puts("\n");
        
        void *p2 = kmalloc(128);
        puts("  kmalloc(128) = "); print_hex((uint64_t)p2); puts("\n");
        
        void *p3 = kmalloc(256);
        puts("  kmalloc(256) = "); print_hex((uint64_t)p3); puts("\n");
        
        puts("  Heap used: "); print_dec(kheap_get_used()); puts(" bytes\n");
        
        kfree(p2);
        puts("  kfree(p2)\n");
        
        void *p4 = kmalloc(100);
        puts("  kmalloc(100) = "); print_hex((uint64_t)p4); puts("\n");
        
        kfree(p1); kfree(p3); kfree(p4);
        puts("  Freed all. Heap used: "); print_dec(kheap_get_used()); puts(" bytes\n");
        
        set_color(VGA_COLOR(10, 0)); puts("Memory allocation test passed!\n");
        set_color(VGA_COLOR(15, 0));
    }
    else if (strcmp(cmd_buffer, "reboot") == 0) {
        puts("\nRebooting...\n");
        __asm__ volatile ("lidt 0\nint $0x03");
    }
    else if (strcmp(cmd_buffer, "halt") == 0) {
        puts("\nSystem halted.\n");
        __asm__ volatile ("cli; hlt");
    }
    else {
        set_color(VGA_COLOR(12, 0)); puts("\nUnknown: ");
        set_color(VGA_COLOR(15, 0)); puts(cmd_buffer);
        puts("\nType 'help' for commands.");
    }
    
    cmd_pos = 0;
    shell_prompt();
}

void shell_input(char c) {
    if (c == '\n') shell_execute();
    else if (c == '\b') { if (cmd_pos > 0) { cmd_pos--; putchar('\b'); } }
    else if (cmd_pos < CMD_BUFFER_SIZE - 1) { cmd_buffer[cmd_pos++] = c; putchar(c); }
}

/* Kernel main */
void kernel_main(uint64_t multiboot_info, uint64_t magic) {
    (void)magic;
    saved_mb_info = multiboot_info;
    
    clear_screen();
    
    set_color(VGA_COLOR(11, 0));
    puts("========================================\n");
    puts("  MinimalOS 64-bit - Long Mode Active!\n");
    puts("========================================\n\n");
    set_color(VGA_COLOR(15, 0));
    
    /* Initialize PIC */
    puts("Initializing PIC... ");
    pic_init();
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    /* Initialize IDT */
    puts("Initializing IDT... ");
    idt_init();
    for (int i = 0; i < 32; i++) register_interrupt_handler(i, exception_handler);
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    /* Initialize PMM */
    puts("Initializing PMM... ");
    pmm_init(multiboot_info);
    set_color(VGA_COLOR(10, 0)); puts("[OK] ");
    set_color(VGA_COLOR(7, 0));
    print_dec(pmm_get_total_memory() / 1024 / 1024);
    puts(" MB detected\n");
    set_color(VGA_COLOR(15, 0));
    
    /* Initialize kernel heap */
    puts("Initializing heap... ");
    kheap_init();
    set_color(VGA_COLOR(10, 0)); puts("[OK] ");
    set_color(VGA_COLOR(7, 0));
    print_dec(kheap_get_free() / 1024);
    puts(" KB available\n");
    set_color(VGA_COLOR(15, 0));
    
    /* Initialize timer */
    puts("Initializing timer... ");
    timer_init(100);
    pic_enable_irq(0);
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    /* Initialize keyboard */
    puts("Initializing keyboard... ");
    keyboard_init();
    pic_enable_irq(1);
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n"); set_color(VGA_COLOR(15, 0));
    
    /* Enable interrupts */
    puts("Enabling interrupts... ");
    __asm__ volatile ("sti");
    set_color(VGA_COLOR(10, 0)); puts("[OK]\n\n"); set_color(VGA_COLOR(15, 0));
    
    set_color(VGA_COLOR(14, 0));
    puts("Welcome to MinimalOS! Type 'help' for commands.\n");
    set_color(VGA_COLOR(15, 0));
    
    shell_prompt();
    
    while (1) {
        char c = keyboard_getchar();
        shell_input(c);
    }
}
