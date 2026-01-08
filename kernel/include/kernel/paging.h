/* Paging header for x86_64 */
#ifndef KERNEL_PAGING_H
#define KERNEL_PAGING_H

#include <stdint.h>

/* Page size */
#define PAGE_SIZE 4096

/* Page flags */
#define PAGE_PRESENT 0x001
#define PAGE_WRITE 0x002
#define PAGE_USER 0x004
#define PAGE_PWT 0x008
#define PAGE_PCD 0x010
#define PAGE_ACCESSED 0x020
#define PAGE_DIRTY 0x040
#define PAGE_HUGE 0x080
#define PAGE_GLOBAL 0x100
#define PAGE_NX (1ULL << 63)

/* Page table structures for x86_64 4-level paging */
/* Each level has 512 entries (9 bits per level) */
typedef struct {
  uint64_t entries[512];
} page_table_t;

/* Page directory (actually PML4 in x86_64) */
typedef page_table_t page_directory_t;

/* Functions */
void paging_init(void);
void paging_map(uint64_t virtual_addr, uint64_t physical_addr, uint64_t flags);
void paging_unmap(uint64_t virtual_addr);
uint64_t paging_get_physical(uint64_t virtual_addr);
void paging_switch_directory(page_directory_t *dir);
page_directory_t *paging_get_directory(void);

/* Flush TLB for a specific address */
static inline void paging_flush_tlb(uint64_t addr) {
  __asm__ volatile("invlpg (%0)" : : "r"(addr) : "memory");
}

#endif
