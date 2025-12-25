#include "include/types.h"
#include "drivers/serial.h"
#include "drivers/vga.h"
#include "lib/printk.h"
#include "arch/x86_64/gdt.h"
#include "arch/x86_64/idt.h"

// Multiboot2 structures
struct multiboot_tag {
    u32 type;
    u32 size;
};

struct multiboot_info {
    u32 total_size;
    u32 reserved;
    struct multiboot_tag tags[];
};

void kernel_main(struct multiboot_info* mbi) {
    // Initialize serial port for debugging
    serial_init();
    
    // Initialize VGA text mode
    vga_init();
    
    // Print boot message
    printk("\n");
    printk("========================================\n");
    printk("MinimalOS - Booting...\n");
    printk("========================================\n");
    printk("x86_64 Multitasking Operating System\n");
    printk("========================================\n\n");
    
    printk("[OK] Serial port initialized\n");
    printk("[OK] VGA text mode initialized\n");
    
    // Initialize GDT
    gdt_init();
    printk("[OK] GDT initialized\n");
    
    // Initialize IDT
    idt_init();
    printk("[OK] IDT initialized\n");
    
    // Parse multiboot2 information
    printk("\nMultiboot2 Information:\n");
    printk("  Total size: %u bytes\n", mbi->total_size);
    
    struct multiboot_tag* tag = mbi->tags;
    while (tag->type != 0) {
        if (tag->type == 6) {  // Memory map tag
            printk("  Memory map found\n");
        } else if (tag->type == 9) {  // Module tag
            printk("  Module found\n");
        }
        
        // Move to next tag (aligned to 8 bytes)
        tag = (struct multiboot_tag*)((u8*)tag + ((tag->size + 7) & ~7));
    }
    
    printk("\n");
    printk("========================================\n");
    printk("Kernel initialized successfully!\n");
    printk("========================================\n");
    printk("\nSystem halted. Next: implement memory management...\n");
    
    // Enable interrupts
    __asm__ volatile("sti");
    
    // Halt
    while (1) {
        __asm__ volatile("hlt");
    }
}
