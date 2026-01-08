#ifndef _KERNEL_PMM_H
#define _KERNEL_PMM_H

#include <stdint.h>
#include <stddef.h>

/* Page size is 4KB */
#define PAGE_SIZE 4096

/* Initialize physical memory manager */
void pmm_init(uint32_t mem_size, uint32_t *bitmap_addr);

/* Allocate a physical page frame */
void *pmm_alloc_frame(void);

/* Free a physical page frame */
void pmm_free_frame(void *frame);

/* Mark a region as used */
void pmm_mark_region_used(uint32_t base, size_t size);

/* Mark a region as free */
void pmm_mark_region_free(uint32_t base, size_t size);

/* Get total memory size */
uint32_t pmm_get_total_memory(void);

/* Get free memory size */
uint32_t pmm_get_free_memory(void);

#endif /* _KERNEL_PMM_H */
