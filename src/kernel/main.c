#include "arch/x86_64/vga.h"
#include "arch/x86_64/idt.h"
#include "arch/x86_64/keyboard.h"
#include "arch/x86_64/paging.h"
#include "arch/x86_64/tss.h"
#include "syscall.h"
#include "stddef.h"

// Function declarations
void setup_gdt(void);

// Simple memory copy function
void *memcpy(void *dest, const void *src, size_t n) {
    char *d = dest;
    const char *s = src;
    while (n--) *d++ = *s++;
    return dest;
}

// User space address for shell (matches user.ld)
#define USER_CODE_ADDR 0x400000
#define USER_STACK_ADDR 0x500000
#define USER_STACK_SIZE 0x1000

// Halt function
void hlt() {
    asm volatile("hlt");
}

void kernel_main() {
    vga_init();
    vga_print("Kernel loaded in long mode.\n");

    setup_paging();  // Set up kernel paging
    setup_gdt();     // Load GDT with user segments
    setup_tss();     // Set up TSS for ring switches
    setup_idt();     // Interrupts
    setup_keyboard(); // Keyboard IRQ
    setup_syscalls(); // Syscall MSR

    vga_print("Setting up user space...\n");

    // Load user shell from sectors 21+ (we'll read it from disk)
    // For now, we'll create a simple placeholder
    
    // Set up user stack (map a page for it)
    map_user_stack(USER_STACK_ADDR, USER_STACK_SIZE);

    vga_print("Kernel initialization complete. Starting user shell...\n");

    // For now, just stay in kernel mode and provide a simple interface
    while (1) {
        vga_print("MinimalOS> ");
        char ch = kb_read();
        vga_putchar(ch);
        if (ch == '\n') {
            vga_print("Command received!\n");
        }
    }
}