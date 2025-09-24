#include "arch/x86_64/vga.h"
#include "arch/x86_64/idt.h"
#include "arch/x86_64/keyboard.h"
#include "arch/x86_64/paging.h"
#include "syscall.h"
#include "stddef.h"

// Simple memory management
static char kernel_heap[0x100000]; // 1MB heap
static size_t heap_offset = 0;

void* kmalloc(size_t size) {
    if (heap_offset + size >= sizeof(kernel_heap)) {
        return NULL; // Out of memory
    }
    void* ptr = &kernel_heap[heap_offset];
    heap_offset += size;
    return ptr;
}

// Simple memory copy function
void* memcpy(void* dest, const void* src, size_t n) {
    char* d = (char*)dest;
    const char* s = (const char*)src;
    while (n--) {
        *d++ = *s++;
    }
    return dest;
}

void kernel_main() {
    // Initialize VGA
    vga_init();
    vga_set_color(VGA_COLOR_LIGHT_GREEN);
    vga_print("MinimalOS Kernel Starting...\n");
    vga_set_color(VGA_COLOR_WHITE);
    
    // Set up interrupt handling
    setup_idt();
    vga_print("IDT initialized\n");
    
    // Initialize keyboard
    setup_keyboard();
    vga_print("Keyboard initialized\n");
    
    // Set up syscalls
    setup_syscalls();
    vga_print("Syscalls initialized\n");
    
    // Set up basic memory management
    setup_paging();
    vga_print("Memory management initialized\n");
    
    vga_set_color(VGA_COLOR_LIGHT_GREEN);
    vga_print("Kernel initialization complete!\n");
    vga_set_color(VGA_COLOR_LIGHT_BROWN);
    vga_print("Starting interactive shell...\n\n");
    vga_set_color(VGA_COLOR_WHITE);
    
    // Switch to user mode and run shell
    // For now, we'll run the shell in kernel mode for simplicity
    // In a real OS, this would involve setting up user space properly
    extern void user_shell_main();
    user_shell_main();
    
    // Should not reach here
    vga_set_color(VGA_COLOR_LIGHT_RED);
    vga_print("ERROR: Kernel returned from user shell!\n");
    vga_set_color(VGA_COLOR_WHITE);
    while (1) {
        asm volatile("hlt");
    }
}
