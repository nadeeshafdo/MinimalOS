/**
 * MinimalOS - Virtual Memory Manager
 * Page table manipulation for 4-level paging
 */

#ifndef MM_VMM_H
#define MM_VMM_H

#include <minimalos/types.h>

/* Page table entry flags */
#define VMM_PRESENT (1UL << 0)  /* Page is present */
#define VMM_WRITABLE (1UL << 1) /* Page is writable */
#define VMM_USER (1UL << 2)     /* User accessible */
#define VMM_PWT (1UL << 3)      /* Page write-through */
#define VMM_PCD (1UL << 4)      /* Page cache disable */
#define VMM_ACCESSED (1UL << 5) /* Page was accessed */
#define VMM_DIRTY (1UL << 6)    /* Page was written */
#define VMM_HUGE (1UL << 7)     /* Huge page (2MB/1GB) */
#define VMM_GLOBAL (1UL << 8)   /* Global page */
#define VMM_NX (1UL << 63)      /* No execute */

/* Common flag combinations */
#define VMM_KERNEL_RW (VMM_PRESENT | VMM_WRITABLE)
#define VMM_KERNEL_RO (VMM_PRESENT)
#define VMM_KERNEL_MMIO (VMM_PRESENT | VMM_WRITABLE | VMM_PWT | VMM_PCD)
#define VMM_USER_RW (VMM_PRESENT | VMM_WRITABLE | VMM_USER)
#define VMM_USER_RO (VMM_PRESENT | VMM_USER)

/* Page table index extraction macros */
#define PML4_INDEX(addr) (((addr) >> 39) & 0x1FF)
#define PDPT_INDEX(addr) (((addr) >> 30) & 0x1FF)
#define PD_INDEX(addr) (((addr) >> 21) & 0x1FF)
#define PT_INDEX(addr) (((addr) >> 12) & 0x1FF)

/* Page table entry address mask */
#define PTE_ADDR_MASK 0x000FFFFFFFFFF000UL

/* LAPIC physical and virtual addresses */
#define LAPIC_PHYS_BASE 0xFEE00000UL
#define LAPIC_VIRT_BASE (KERNEL_VMA + LAPIC_PHYS_BASE)

/**
 * Initialize the VMM
 * Sets up additional page mappings needed by the kernel
 */
void vmm_init(void);

/**
 * Map a single 4KB page
 * @param virt Virtual address (must be page-aligned)
 * @param phys Physical address (must be page-aligned)
 * @param flags Page flags (VMM_PRESENT, VMM_WRITABLE, etc.)
 * @return 0 on success, -1 on failure
 */
int vmm_map_page(uint64_t virt, uint64_t phys, uint64_t flags);

/**
 * Unmap a single 4KB page
 * @param virt Virtual address to unmap
 */
void vmm_unmap_page(uint64_t virt);

/**
 * Map a contiguous region of memory
 * @param virt Virtual base address
 * @param phys Physical base address
 * @param size Size in bytes (will be rounded up to page size)
 * @param flags Page flags
 * @return 0 on success, -1 on failure
 */
int vmm_map_region(uint64_t virt, uint64_t phys, size_t size, uint64_t flags);

/**
 * Flush TLB for a specific address
 */
static inline void vmm_flush_tlb(uint64_t addr) {
  __asm__ volatile("invlpg (%0)" : : "r"(addr) : "memory");
}

/**
 * Flush entire TLB by reloading CR3
 */
static inline void vmm_flush_tlb_all(void) {
  uint64_t cr3;
  __asm__ volatile("mov %%cr3, %0" : "=r"(cr3));
  __asm__ volatile("mov %0, %%cr3" : : "r"(cr3) : "memory");
}

/**
 * Get physical address for a virtual address
 * @param virt Virtual address to translate
 * @return Physical address, or 0 if not mapped
 */
uint64_t vmm_virt_to_phys(uint64_t virt);

#endif /* MM_VMM_H */
