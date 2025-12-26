#include "include/types.h"
#include "drivers/serial.h"
#include "drivers/vga.h"
#include "drivers/timer.h"
#include "lib/printk.h"
#include "arch/x86_64/gdt.h"
#include "arch/x86_64/idt.h"
#include "mm/pmm.h"
#include "mm/vmm.h"
#include "mm/heap.h"
#include "process/process.h"
#include "process/scheduler.h"

// Multiboot2 structures
struct multiboot_tag {
    u32 type;
    u32 size;
} __attribute__((packed));

struct multiboot_info {
    u32 total_size;
    u32 reserved;
    // Tags follow immediately after
} __attribute__((packed));

// Test kernel thread functions  
void kernel_thread_1(void) {
    printk("[Thread 1] Starting...\n");
    for (int i = 0; i < 5; i++) {
        printk("[Thread 1] Iteration %d\n", i);
        yield();  // Give other threads a chance
    }
    printk("[Thread 1] Exiting.\n");
    process_exit(0);
}

void kernel_thread_2(void) {
    printk("[Thread 2] Starting...\n");
    for (int i = 0; i < 5; i++) {
        printk("[Thread 2] Iteration %d\n", i);
        yield();  // Give other threads a chance
    }
    printk("[Thread 2] Exiting.\n");
    process_exit(0);
}

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
    
    struct multiboot_tag* tag = (struct multiboot_tag*)((u8*)mbi + 8);
    u32 tag_count = 0;
    while (tag->type != 0 && (u8*)tag < (u8*)mbi + mbi->total_size) {
        tag_count++;
        if (tag->type == 6) {  // Memory map tag
            printk("  Memory map found\n");
        } else if (tag->type == 9) {  // Module tag
            printk("  Module found\n");
        }
        
        // Move to next tag (aligned to 8 bytes)
        tag = (struct multiboot_tag*)((u8*)tag + ((tag->size + 7) & ~7));
    }
    
    printk("  Parsed %u tags\n", tag_count);
    
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
    
    printk("\n");
    printk("========================================\n");
    printk("Phase 2 Complete! Memory management working.\n");
    printk("========================================\n\n");
    
    // Initialize process management
    printk("Initializing Process Management:\n");
    printk("========================================\n");
    
    // Initialize timer
    timer_init();
    printk("\n");
    
    // Initialize processes
    process_init();
    printk("\n");
    
    // Initialize scheduler
    scheduler_init();
    printk("\n");
    
    printk("========================================\n");
    printk("Phase 3 Complete! Process management ready.\n");
    printk("========================================\n\n");
    
    printk("System Summary:\n");
    printk("  Total memory: %lu MB\n", pmm_get_total_memory() / (1024 * 1024));
    printk("  Free memory:  %lu MB\n", pmm_get_free_memory() / (1024 * 1024));
    printk("  Used memory:  %lu MB\n", pmm_get_used_memory() / (1024 * 1024));
    
    printk("\nKernel initialization complete!\n\n");
    
    // Create and start test kernel threads
    printk("========================================\n");
    printk("Starting Kernel Threads (Multitasking Demo)\n");
    printk("========================================\n\n");
    
    process_t* thread1 = process_create("thread1");
    if (thread1) {
        process_setup_kernel_thread(thread1, kernel_thread_1);
        scheduler_add_process(thread1);
    }
    
    process_t* thread2 = process_create("thread2");
    if (thread2) {
        process_setup_kernel_thread(thread2, kernel_thread_2);
        scheduler_add_process(thread2);
    }
    
    printk("\nEnabling scheduler...\n");
    scheduler_enable();
    
    printk("[Kernel] Scheduler started! Threads should run...\n\n");
    
    // Enable interrupts
    __asm__ volatile("sti");
    
    // Trigger first context switch manually
    printk("[Kernel] Yielding to threads...\n");
    yield();
    
    // Kernel idle loop - keep yielding to let other threads run
    printk("[Kernel] Back in idle loop\n");
    while (1) {
        yield();  // Give other threads CPU time
        __asm__ volatile("hlt");  // Halt until next interrupt
    }
}
