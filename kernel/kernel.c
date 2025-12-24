#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <kernel/tty.h>
#include <kernel/gdt.h>
#include <kernel/idt.h>
#include <kernel/isr.h>
#include <kernel/irq.h>
#include <kernel/timer.h>
#include <kernel/keyboard.h>
#include <kernel/pmm.h>
#include <kernel/paging.h>
#include <kernel/kheap.h>
#include <kernel/process.h>
#include <kernel/scheduler.h>
#include <kernel/syscall.h>
#include <kernel/shell.h>
#include <kernel/framebuffer.h>

/* Verify target architecture */
#if !defined(__i386__) && !defined(__x86_64__)
#error "This kernel requires x86 architecture"
#endif

/* Shell task - just idles and lets keyboard interrupts handle input */
void shell_task(void) {
    /* The keyboard handler prints characters directly */
    /* This task just idles, waiting for keyboard interrupts */
    while (1) {
        __asm__ volatile ("hlt");  /* Wait for interrupt */
    }
}

/* Multiboot info structure (simplified) */
struct multiboot_info {
    uint32_t flags;
    uint32_t mem_lower;
    uint32_t mem_upper;
    /* ... more fields exist but we don't use them yet */
} __attribute__((packed));

/* Kernel panic function */
void kernel_panic(const char* message) {
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_RED));
    terminal_writestring("\n\n*** KERNEL PANIC ***\n");
    terminal_writestring(message);
    terminal_writestring("\nSystem halted.\n");
    
    /* Halt the system */
    while (1) {
        __asm__ volatile ("cli; hlt");
    }
}

/* Print a number in hexadecimal */
static void print_hex(uint32_t value) {
    char hex[11] = "0x00000000";
    const char* digits = "0123456789ABCDEF";
    
    for (int i = 9; i >= 2; i--) {
        hex[i] = digits[value & 0xF];
        value >>= 4;
    }
    
    terminal_writestring(hex);
}

/* Kernel main function - entry point after boot.s */
void kernel_main(uint32_t multiboot_magic, struct multiboot_info* multiboot_info) {
    /* Initialize terminal */
    terminal_initialize();
    
    /* Display welcome banner */
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
    terminal_writestring("======================================\n");
    terminal_writestring("       MinimalOS v0.1 Alpha\n");
    terminal_writestring("======================================\n\n");
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    /* Verify multiboot compliance */
    if (multiboot_magic != 0x2BADB002) {
        kernel_panic("Invalid multiboot magic number!");
    }
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring(" Multiboot compliant bootloader detected\n");
    
    /* Initialize framebuffer if available */
    if (fb_init(multiboot_info)) {
        framebuffer_info_t *fb = fb_get_info();
        terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
        terminal_writestring("[OK]");
        terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
        terminal_writestring(" Framebuffer: ");
        print_hex(fb->width);
        terminal_writestring("x");
        print_hex(fb->height);
        terminal_writestring("x");
        print_hex(fb->bpp);
        terminal_writestring("\n");
    } else {
        terminal_writestring("     Using VGA text mode (no framebuffer)\n");
    }
    
    /* Display memory information */
    if (multiboot_info->flags & 0x01) {
        terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
        terminal_writestring("[OK]");
        terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
        terminal_writestring(" Memory info available\n");
        
        terminal_writestring("     Lower memory: ");
        print_hex(multiboot_info->mem_lower);
        terminal_writestring(" KB\n");
        
        terminal_writestring("     Upper memory: ");
        print_hex(multiboot_info->mem_upper);
        terminal_writestring(" KB\n");
    }
    
    /* Initialize GDT */
    terminal_writestring("Initializing GDT... ");
    gdt_init();
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    /* Initialize IDT */
    terminal_writestring("Initializing IDT... ");
    idt_init();
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    /* Initialize ISRs */
    terminal_writestring("Initializing ISRs... ");
    isr_init();
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    /* Initialize IRQs */
    terminal_writestring("Initializing IRQs and PIC... ");
    irq_init();
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    /* Initialize timer (100 Hz) */
    terminal_writestring("Initializing timer (100 Hz)... ");
    timer_init(100);
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    /* Initialize keyboard */
    terminal_writestring("Initializing keyboard... ");
    keyboard_init();
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    /* Initialize physical memory manager */
    terminal_writestring("Initializing PMM... ");
    uint32_t total_mem = (multiboot_info->mem_upper + 1024) * 1024;  /* Total memory in bytes */
    static uint32_t pmm_bitmap[32768];  /* Supports up to 4GB */
    pmm_init(total_mem, pmm_bitmap);
    /* Mark first 16MB as used (kernel space) */
    pmm_mark_region_used(0, 0x1000000);
    /* Mark usable memory as free (above 16MB) */
    if (total_mem > 0x1000000) {
        pmm_mark_region_free(0x1000000, total_mem - 0x1000000);
    }
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring("     Free memory: ");
    print_hex(pmm_get_free_memory() / 1024);
    terminal_writestring(" KB\n");
    
    /* Initialize paging */
    terminal_writestring("Initializing paging... ");
    paging_init();
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    /* Initialize kernel heap */
    terminal_writestring("Initializing kernel heap... ");
    kheap_init();
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    /* Initialize process management */
    terminal_writestring("Initializing process management... ");
    process_init();
    scheduler_init();
    syscall_init();
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREEN, VGA_COLOR_BLACK));
    terminal_writestring("[OK]\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    terminal_writestring("\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
    terminal_writestring("*** System Initialization Complete ***\n\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_WHITE, VGA_COLOR_BLACK));
    terminal_writestring("Welcome to MinimalOS!\n");
    terminal_writestring("This is a functional operating system built from scratch.\n\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_CYAN, VGA_COLOR_BLACK));
    terminal_writestring("Features:\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    terminal_writestring("  * VGA text mode terminal with scrolling\n");
    terminal_writestring("  * GDT with kernel and user segments\n");
    terminal_writestring("  * IDT with CPU exception handlers\n");
    terminal_writestring("  * Hardware interrupts (PIC remapping)\n");
    terminal_writestring("  * Programmable Interval Timer (PIT)\n");
    terminal_writestring("  * PS/2 keyboard driver with live input\n");
    terminal_writestring("  * Physical memory manager (PMM)\n");
    terminal_writestring("  * Virtual memory with paging\n");
    terminal_writestring("  * Kernel heap (kmalloc/kfree)\n");
    terminal_writestring("  * Multitasking scheduler (Round-Robin)\n");
    
    terminal_writestring("\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_YELLOW, VGA_COLOR_BLACK));
    terminal_writestring("Type 'help' for available commands.\n\n");
    terminal_setcolor(vga_entry_color(VGA_COLOR_LIGHT_GREY, VGA_COLOR_BLACK));
    
    /* Initialize and start shell */
    shell_init();
    
    /* Create shell task */
    process_t *shell = process_create("Shell", shell_run);
    scheduler_add(shell);
    
    /* Start scheduler */
    scheduler_start();
    
    /* Should not be reached */
    while (1) {
        __asm__ volatile ("hlt");
    }
}
