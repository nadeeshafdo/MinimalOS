#ifndef VMM_H
#define VMM_H

#include "../include/types.h"

#define PAGE_SIZE 4096

// Page table flags
#define PAGE_PRESENT    (1 << 0)
#define PAGE_WRITE      (1 << 1)
#define PAGE_USER       (1 << 2)
#define PAGE_WRITETHROUGH (1 << 3)
#define PAGE_CACHE_DISABLE (1 << 4)
#define PAGE_ACCESSED   (1 << 5)
#define PAGE_DIRTY      (1 << 6)
#define PAGE_HUGE       (1 << 7)
#define PAGE_GLOBAL     (1 << 8)

// Page directory (PML4 for x86_64) - opaque pointer
typedef struct page_directory page_directory_t;

/**
 * Initialize the virtual memory manager
 * Sets up kernel page tables
 */
void vmm_init(void);

/**
 * Create a new address space (for processes)
 * Returns new page directory
 */
page_directory_t* vmm_create_address_space(void);

/**
 * Destroy an address space
 */
void vmm_destroy_address_space(page_directory_t* pd);

/**
 * Map a virtual address to a physical frame
 * @param pd Page directory (NULL for kernel directory)
 * @param virt Virtual address
 * @param phys Physical address
 * @param flags Page flags (PAGE_PRESENT | PAGE_WRITE | etc)
 */
void vmm_map_page(page_directory_t* pd, uintptr virt, uintptr phys, u32 flags);

/**
 * Unmap a virtual address
 */
void vmm_unmap_page(page_directory_t* pd, uintptr virt);

/**
 * Get physical address for a virtual address
 * Returns 0 if not mapped
 */
uintptr vmm_get_physical(page_directory_t* pd, uintptr virt);

/**
 * Switch to a different page directory
 */
void vmm_switch_directory(page_directory_t* pd);

/**
 * Get current page directory
 */
page_directory_t* vmm_get_current_directory(void);

#endif // VMM_H
