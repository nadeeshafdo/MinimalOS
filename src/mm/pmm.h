/**
 * MinimalOS - Physical Memory Manager Header
 */

#ifndef MM_PMM_H
#define MM_PMM_H

#include <minimalos/types.h>

/* Page size constants */
#define PAGE_SIZE 4096UL
#define PAGE_SHIFT 12

/* Align address up to page boundary */
#define PAGE_ALIGN_UP(addr) (((addr) + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1))
#define PAGE_ALIGN_DOWN(addr) ((addr) & ~(PAGE_SIZE - 1))

/* Convert between address and page frame number */
#define ADDR_TO_PFN(addr) ((addr) >> PAGE_SHIFT)
#define PFN_TO_ADDR(pfn) ((pfn) << PAGE_SHIFT)

/* Function prototypes */
void pmm_init(void);
void *pmm_alloc_frame(void);
void *pmm_alloc_frames(size_t count);
void pmm_free_frame(void *addr);
void pmm_free_frames(void *addr, size_t count);
size_t pmm_get_free_frames(void);
size_t pmm_get_total_frames(void);
void pmm_mark_used(uint64_t addr);
void pmm_mark_range_used(uint64_t start, uint64_t end);
void pmm_mark_range_free(uint64_t start, uint64_t end);

#endif /* MM_PMM_H */
