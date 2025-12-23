#ifndef _KERNEL_PAGING_H
#define _KERNEL_PAGING_H

#include <stdint.h>
#include <kernel/isr.h>

/* Page directory/table entry flags */
#define PAGE_PRESENT    0x001
#define PAGE_WRITE      0x002
#define PAGE_USER       0x004
#define PAGE_ACCESSED   0x020
#define PAGE_DIRTY      0x040

/* Page directory entry */
typedef uint32_t page_directory_entry_t;

/* Page table entry */
typedef uint32_t page_table_entry_t;

/* Page directory structure */
typedef struct {
    page_directory_entry_t entries[1024];
} __attribute__((aligned(4096))) page_directory_t;

/* Page table structure */
typedef struct {
    page_table_entry_t entries[1024];
} __attribute__((aligned(4096))) page_table_t;

/* Initialize paging */
void paging_init(void);

/* Map virtual address to physical address */
void paging_map(uint32_t virtual_addr, uint32_t physical_addr, uint32_t flags);

/* Unmap virtual address */
void paging_unmap(uint32_t virtual_addr);

/* Get physical address from virtual address */
uint32_t paging_get_physical(uint32_t virtual_addr);

/* Page fault handler */
void page_fault_handler(struct registers *regs);

/* Flush TLB for a specific address */
static inline void paging_flush_tlb(uint32_t addr) {
    __asm__ volatile ("invlpg (%0)" : : "r"(addr) : "memory");
}

/* Switch page directory */
void paging_switch_directory(page_directory_t *dir);

/* Get current page directory */
page_directory_t *paging_get_directory(void);

#endif /* _KERNEL_PAGING_H */
