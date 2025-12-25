#ifndef PMM_H
#define PMM_H

#include "../include/types.h"

#define PAGE_SIZE 4096

// Multiboot2 memory map structures
struct multiboot_mmap_entry {
    u64 addr;
    u64 len;
    u32 type;
    u32 zero;
} __attribute__((packed));

#define MULTIBOOT_MEMORY_AVAILABLE        1
#define MULTIBOOT_MEMORY_RESERVED         2
#define MULTIBOOT_MEMORY_ACPI_RECLAIMABLE 3
#define MULTIBOOT_MEMORY_NVS              4
#define MULTIBOOT_MEMORY_BADRAM           5

/**
 * Initialize the physical memory manager
 * Parses multiboot2 memory map and sets up frame bitmap
 */
void pmm_init(void* mbi_ptr);

/**
 * Allocate a single 4KB physical frame
 * Returns physical address of frame, or 0 on failure
 */
uintptr pmm_alloc_frame(void);

/**
 * Free a single 4KB physical frame
 */
void pmm_free_frame(uintptr frame);

/**
 * Allocate multiple contiguous frames
 * Returns physical address of first frame, or 0 on failure
 */
uintptr pmm_alloc_frames(size_t count);

/**
 * Free multiple contiguous frames
 */
void pmm_free_frames(uintptr frame, size_t count);

/**
 * Get total memory in bytes
 */
u64 pmm_get_total_memory(void);

/**
 * Get free memory in bytes
 */
u64 pmm_get_free_memory(void);

/**
 * Get used memory in bytes
 */
u64 pmm_get_used_memory(void);

#endif // PMM_H
