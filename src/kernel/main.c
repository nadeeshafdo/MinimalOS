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
    vga_print("MinimalOS Kernel Started!\n");

    setup_paging();  // Set up kernel paging
    vga_print("Paging initialized.\n");
    
    setup_gdt();     // Load GDT with user segments
    vga_print("GDT loaded.\n");
    
    setup_tss();     // Set up TSS for ring switches
    vga_print("TSS configured.\n");
    
    setup_idt();     // Interrupts
    vga_print("Interrupts enabled.\n");
    
    setup_keyboard(); // Keyboard IRQ
    vga_print("Keyboard ready.\n");
    
    setup_syscalls(); // Syscall MSR
    vga_print("Syscalls initialized.\n");

    vga_print("Kernel initialization complete.\n");
    vga_print("MinimalOS> ");

    // Simple keyboard loop
    while (1) {
        char ch = kb_read();
        vga_putchar(ch);
        if (ch == '\n') {
            vga_print("MinimalOS> ");
        }
    }
}
