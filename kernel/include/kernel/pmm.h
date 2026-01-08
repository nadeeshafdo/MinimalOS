/* Physical Memory Manager header for x86_64 */
#ifndef KERNEL_PMM_H
#define KERNEL_PMM_H

#include <stddef.h>
#include <stdint.h>

/* Forward declaration */
struct limine_memmap_response;

/* Page size */
#define PAGE_SIZE 4096

/* Initialize PMM with Limine memory map */
void pmm_init_limine(struct limine_memmap_response *memmap);

/* Allocate/free physical frames */
void *pmm_alloc_frame(void);
void pmm_free_frame(void *frame);

/* Mark regions as used/free */
void pmm_mark_region_used(uint64_t base, size_t size);
void pmm_mark_region_free(uint64_t base, size_t size);

/* Memory info */
uint64_t pmm_get_total_memory(void);
uint64_t pmm_get_free_memory(void);

#endif
