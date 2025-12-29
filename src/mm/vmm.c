/**
 * MinimalOS - Virtual Memory Manager Implementation
 * Manipulates 4-level page tables for x86-64
 */

#include "vmm.h"
#include "pmm.h"

extern void printk(const char *fmt, ...);

/* External: current PML4 from boot code */
extern uint64_t boot_pml4[];

/* Get current CR3 (PML4 physical address) */
static inline uint64_t read_cr3(void) {
  uint64_t cr3;
  __asm__ volatile("mov %%cr3, %0" : "=r"(cr3));
  return cr3;
}

/**
 * Get pointer to page table entry, creating intermediate tables if needed
 * @param virt Virtual address
 * @param create If true, create missing page tables
 * @return Pointer to PTE, or NULL if not found and create is false
 */
static uint64_t *get_pte(uint64_t virt, bool create) {
  uint64_t cr3 = read_cr3();

  /* Get PML4 virtual address */
  uint64_t *pml4 = (uint64_t *)PHYS_TO_VIRT(cr3 & PTE_ADDR_MASK);

  uint64_t pml4_idx = PML4_INDEX(virt);
  uint64_t pdpt_idx = PDPT_INDEX(virt);
  uint64_t pd_idx = PD_INDEX(virt);
  uint64_t pt_idx = PT_INDEX(virt);

  /* Walk PML4 -> PDPT */
  if (!(pml4[pml4_idx] & VMM_PRESENT)) {
    if (!create)
      return NULL;

    void *new_page = pmm_alloc_frame();
    if (!new_page)
      return NULL;

    /* Zero the new page table */
    uint64_t *new_pt = (uint64_t *)PHYS_TO_VIRT((uint64_t)new_page);
    for (int i = 0; i < 512; i++)
      new_pt[i] = 0;

    pml4[pml4_idx] = (uint64_t)new_page | VMM_PRESENT | VMM_WRITABLE;
  }

  uint64_t *pdpt = (uint64_t *)PHYS_TO_VIRT(pml4[pml4_idx] & PTE_ADDR_MASK);

  /* Walk PDPT -> PD */
  if (!(pdpt[pdpt_idx] & VMM_PRESENT)) {
    if (!create)
      return NULL;

    void *new_page = pmm_alloc_frame();
    if (!new_page)
      return NULL;

    uint64_t *new_pt = (uint64_t *)PHYS_TO_VIRT((uint64_t)new_page);
    for (int i = 0; i < 512; i++)
      new_pt[i] = 0;

    pdpt[pdpt_idx] = (uint64_t)new_page | VMM_PRESENT | VMM_WRITABLE;
  }

  /* Check for 1GB huge page */
  if (pdpt[pdpt_idx] & VMM_HUGE) {
    return NULL; /* Cannot map 4KB page within 1GB huge page */
  }

  uint64_t *pd = (uint64_t *)PHYS_TO_VIRT(pdpt[pdpt_idx] & PTE_ADDR_MASK);

  /* Walk PD -> PT */
  if (!(pd[pd_idx] & VMM_PRESENT)) {
    if (!create)
      return NULL;

    void *new_page = pmm_alloc_frame();
    if (!new_page)
      return NULL;

    uint64_t *new_pt = (uint64_t *)PHYS_TO_VIRT((uint64_t)new_page);
    for (int i = 0; i < 512; i++)
      new_pt[i] = 0;

    pd[pd_idx] = (uint64_t)new_page | VMM_PRESENT | VMM_WRITABLE;
  }

  /* Check for 2MB huge page */
  if (pd[pd_idx] & VMM_HUGE) {
    return NULL; /* Cannot map 4KB page within 2MB huge page */
  }

  uint64_t *pt = (uint64_t *)PHYS_TO_VIRT(pd[pd_idx] & PTE_ADDR_MASK);

  return &pt[pt_idx];
}

/**
 * Map a single 4KB page
 */
int vmm_map_page(uint64_t virt, uint64_t phys, uint64_t flags) {
  /* Align addresses to page boundary */
  virt &= ~(PAGE_SIZE - 1);
  phys &= ~(PAGE_SIZE - 1);

  uint64_t *pte = get_pte(virt, true);
  if (!pte) {
    printk("VMM: Failed to get PTE for 0x%lx\n", virt);
    return -1;
  }

  *pte = phys | flags;
  vmm_flush_tlb(virt);

  return 0;
}

/**
 * Unmap a single 4KB page
 */
void vmm_unmap_page(uint64_t virt) {
  virt &= ~(PAGE_SIZE - 1);

  uint64_t *pte = get_pte(virt, false);
  if (pte && (*pte & VMM_PRESENT)) {
    *pte = 0;
    vmm_flush_tlb(virt);
  }
}

/**
 * Map a contiguous region of memory
 */
int vmm_map_region(uint64_t virt, uint64_t phys, size_t size, uint64_t flags) {
  size_t pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;

  for (size_t i = 0; i < pages; i++) {
    if (vmm_map_page(virt + i * PAGE_SIZE, phys + i * PAGE_SIZE, flags) != 0) {
      /* Rollback on failure */
      for (size_t j = 0; j < i; j++) {
        vmm_unmap_page(virt + j * PAGE_SIZE);
      }
      return -1;
    }
  }

  return 0;
}

/**
 * Get physical address for a virtual address
 */
uint64_t vmm_virt_to_phys(uint64_t virt) {
  uint64_t *pte = get_pte(virt, false);
  if (!pte || !(*pte & VMM_PRESENT)) {
    return 0;
  }

  return (*pte & PTE_ADDR_MASK) | (virt & (PAGE_SIZE - 1));
}

/**
 * Initialize VMM and set up required mappings
 */
void vmm_init(void) {
  printk("  Mapping LAPIC region...\n");

  /* Map LAPIC MMIO region (4KB at 0xFEE00000) */
  if (vmm_map_page(LAPIC_VIRT_BASE, LAPIC_PHYS_BASE, VMM_KERNEL_MMIO) != 0) {
    printk("  ERROR: Failed to map LAPIC!\n");
    return;
  }

  printk("  LAPIC mapped: 0x%lx -> 0x%lx\n", LAPIC_VIRT_BASE, LAPIC_PHYS_BASE);
}
