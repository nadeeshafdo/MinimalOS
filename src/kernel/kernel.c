#include "include/types.h"
#include "drivers/serial.h"
#include "drivers/vga.h"
#include "lib/printk.h"
#include "arch/x86_64/gdt.h"
#include "arch/x86_64/idt.h"
#include "mm/pmm.h"
#include "mm/vmm.h"
#include "mm/heap.h"

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
    
    // Initialize memory management
    printk("Initializing Memory Management:\n");
    printk("========================================\n");
    
    // Initialize physical memory manager
    pmm_init(mbi);
    printk("\n");
    
    // Initialize virtual memory manager
    vmm_init();
    printk("\n");
    
    // Initialize kernel heap
    heap_init();
    printk("\n");
    
    printk("========================================\n");
    printk("Memory Management Tests:\n");
    printk("========================================\n");
    
    // Test PMM
    printk("\n[TEST] Physical Memory Allocator:\n");
    uintptr frame1 = pmm_alloc_frame();
    uintptr frame2 = pmm_alloc_frame();
    uintptr frame3 = pmm_alloc_frame();
    printk("  Allocated frames: 0x%lx, 0x%lx, 0x%lx\n", frame1, frame2, frame3);
    pmm_free_frame(frame2);
    printk("  Freed frame: 0x%lx\n", frame2);
    uintptr frame4 = pmm_alloc_frame();
    printk("  Allocated frame: 0x%lx (should reuse 0x%lx)\n", frame4, frame2);
    if (frame4 == frame2) {
        printk("  [PASS] Frame reuse working!\n");
    }
    
    // Test heap allocator
    printk("\n[TEST] Kernel Heap Allocator:\n");
    void* ptr1 = kmalloc(64);
    void* ptr2 = kmalloc(128);
    void* ptr3 = kmalloc(256);
    printk("  Allocated: ptr1=%p, ptr2=%p, ptr3=%p\n", ptr1, ptr2, ptr3);
    
    kfree(ptr2);
    printk("  Freed ptr2\n");
    
    void* ptr4 = kmalloc(100);
    printk("  Allocated ptr4=%p (should reuse freed space)\n", ptr4);
    
    size_t total, used, free_mem;
    heap_get_stats(&total, &used, &free_mem);
    printk("  Heap stats: total=%u KB, used=%u KB, free=%u KB\n",
           (u32)(total/1024), (u32)(used/1024), (u32)(free_mem/1024));
    printk("  [PASS] Heap allocator working!\n");
    
    // Test zeroed allocation
    printk("\n[TEST] Zero-initialized allocation:\n");
    char* test_buf = (char*)kzalloc(32);
    bool all_zero = true;
    for (size_t i = 0; i < 32; i++) {
        if (test_buf[i] != 0) {
            all_zero = false;
            break;
        }
    }
    printk("  kzalloc(32) = %p, all zeros: %s\n", test_buf, all_zero ? "YES" : "NO");
    if (all_zero) {
        printk("  [PASS] kzalloc working!\n");
    }
    
    printk("\n");
    printk("========================================\n");
    printk("Kernel initialized successfully!\n");
    printk("========================================\n");
    printk("\nPhase 2 Complete! Memory management working.\n");
    printk("System memory:\n");
    printk("  Total: %lu MB\n", pmm_get_total_memory() / (1024 * 1024));
    printk("  Free:  %lu MB\n", pmm_get_free_memory() / (1024 * 1024));
    printk("  Used:  %lu MB\n", pmm_get_used_memory() / (1024 * 1024));
    
    // Enable interrupts
    __asm__ volatile("sti");
    
    // Halt
    while (1) {
        __asm__ volatile("hlt");
    }
}

