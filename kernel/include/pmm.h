#ifndef _PMM_H
#define _PMM_H

#include <stdint.h>

/* Page size (4KB) */
#define PAGE_SIZE 4096

/* Initialize PMM with multiboot2 memory map */
void pmm_init(uint64_t mb_info_addr);

/* Allocate a single physical page, returns physical address or 0 on failure */
uint64_t pmm_alloc_page(void);

/* Allocate contiguous pages, returns physical address or 0 on failure */
uint64_t pmm_alloc_pages(uint64_t count);

/* Free a physical page */
void pmm_free_page(uint64_t phys_addr);

/* Free contiguous pages */
void pmm_free_pages(uint64_t phys_addr, uint64_t count);

/* Get total physical memory (bytes) */
uint64_t pmm_get_total_memory(void);

/* Get free physical memory (bytes) */
uint64_t pmm_get_free_memory(void);

/* Get used physical memory (bytes) */
uint64_t pmm_get_used_memory(void);

#endif /* _PMM_H */
