#include "include/types.h"
#include "drivers/serial.h"
#include "drivers/vga.h"
#include "drivers/timer.h"
#include "drivers/keyboard.h"
#include "lib/printk.h"
#include "lib/string.h"
#include "arch/x86_64/gdt.h"
#include "arch/x86_64/idt.h"
#include "mm/pmm.h"
#include "mm/vmm.h"
#include "mm/heap.h"
#include "process/process.h"
#include "process/scheduler.h"
#include "loader/elf.h"
#include "arch/x86_64/syscall.h"
#include "ipc/ipc.h"
#include "fs/vfs.h"
#include "fs/initrd.h"

// Multiboot2 structures
struct multiboot_tag {
    u32 type;
    u32 size;
} __attribute__((packed));

struct multiboot_tag_module {
    u32 type;
    u32 size;
    u32 mod_start;
    u32 mod_end;
    char cmdline[];
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
    
    // Send IPC Message to User Process (PID 3)
    printk("[Thread 1] Sending IPC message to PID 3...\n");
    ipc_message_t msg;
    msg.type = 1;
    msg.length = 18;
    // msg.data should be set safely
    const char* txt = "Hello from Kernel";
    memcpy(msg.data, txt, 18); // Includes null terminator
    
    int result = ipc_send_message(3, &msg);
    if (result == 0) {
        printk("[Thread 1] IPC Send Success! Waking up PID 3.\n");
    } else {
        printk("[Thread 1] IPC Send Failed: %d\n", result);
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
    
    // Initialize VFS and mount initrd
    printk("Initializing Filesystem:\n");
    printk("========================================\n");
    
    vfs_init();
    printk("\n");
    
    // Find and parse multiboot module (initrd)
    uintptr initrd_addr = 0;
    size_t initrd_size = 0;
    
    tag = (struct multiboot_tag*)((u8*)mbi + 8);
    while (tag->type != 0 && (u8*)tag < (u8*)mbi + mbi->total_size) {
        if (tag->type == 3) {  // Module tag
            struct multiboot_tag_module* mod = (struct multiboot_tag_module*)tag;
            initrd_addr = mod->mod_start;
            initrd_size = mod->mod_end - mod->mod_start;
            printk("[VFS] Found initrd module at 0x%lx (size: %lu bytes)\n", 
                   initrd_addr, initrd_size);
            break;
        }
        tag = (struct multiboot_tag*)((u8*)tag + ((tag->size + 7) & ~7));
    }
    
    if (initrd_addr && initrd_size > 0) {
        vfs_node_t* initrd_root = initrd_init(initrd_addr, initrd_size);
        if (initrd_root) {
            vfs_mount("/", initrd_root);
            printk("[VFS] Initrd mounted successfully\n");
        } else {
            printk("[VFS] Failed to initialize initrd\n");
        }
    } else {
        printk("[VFS] No initrd module found\n");
    }
    
    printk("\n");
    printk("========================================\n");
    printk("Phase 5 Complete! Filesystem ready.\n");
    printk("========================================\n\n");
    
    // Initialize process management
    printk("Initializing Process Management:\n");
    printk("========================================\n");
    
    // Initialize timer
    timer_init();
    printk("\n");
    
    // Initialize keyboard
    keyboard_init();
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
    
    // Initialize system calls
    syscall_init();
    
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
    
    // Test ELF loader with embedded binary
    printk("========================================\n");
    printk("Testing ELF Loader\n");
    printk("========================================\n\n");
    
    // Symbols created by objcopy for embedded binary
    extern u8 _binary_userspace_test_elf_start[];
    extern u8 _binary_userspace_test_elf_end[];
    
    size_t elf_size = _binary_userspace_test_elf_end - _binary_userspace_test_elf_start;
    printk("[Kernel] Embedded test ELF: %p, size: %lu bytes\n", 
           _binary_userspace_test_elf_start, elf_size);
    
    // Validate the ELF
    if (elf_validate(_binary_userspace_test_elf_start)) {
        printk("[Kernel] ELF validation passed!\n");
        
        // Get entry point
        u64 entry = elf_get_entry(_binary_userspace_test_elf_start);
        printk("[Kernel] Entry point: 0x%lx\n", entry);
        
        // Create user process
        process_t* user_proc = process_create("user_test");
        if (user_proc) {
            // Load ELF
            if (elf_load(user_proc, _binary_userspace_test_elf_start, elf_size) == 0) {
                printk("[Kernel] ELF loaded into process '%s' (PID %u)\n", 
                       user_proc->name, user_proc->pid);
                scheduler_add_process(user_proc);
            } else {
                printk("[Kernel] Failed to load ELF!\n");
            }
        }
    } else {
        printk("[Kernel] ELF validation failed!\n");
    }
    
    printk("\n");
    
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
