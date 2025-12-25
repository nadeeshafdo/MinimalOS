#include "vmm.h"
#include "pmm.h"
#include "../lib/string.h"
#include "../lib/printk.h"

// 4-level paging structures
typedef struct {
    u64 entries[512];
} page_table_t;

// Page directory is PML4 in x86_64
struct page_directory {
    page_table_t* pml4;
    uintptr pml4_phys;
};

static page_directory_t kernel_directory;
static page_directory_t* current_directory = NULL;

// Helper to get table index from virtual address
#define PML4_INDEX(addr) (((addr) >> 39) & 0x1FF)
#define PDPT_INDEX(addr) (((addr) >> 30) & 0x1FF)
#define PD_INDEX(addr)   (((addr) >> 21) & 0x1FF)
#define PT_INDEX(addr)   (((addr) >> 12) & 0x1FF)

// Helper to extract address from entry
#define ENTRY_ADDR(entry) ((entry) & 0x000FFFFFFFFFF000ULL)
#define ENTRY_FLAGS(entry) ((entry) & 0xFFF)

static page_table_t* get_or_create_table(u64* entry, u32 flags) {
    if (*entry & PAGE_PRESENT) {
        // Table exists, return it
        return (page_table_t*)ENTRY_ADDR(*entry);
    }
    
    // Allocate new table
    uintptr phys = pmm_alloc_frame();
    if (phys == 0) {
        return NULL;
    }
    
    // Zero the table
    page_table_t* table = (page_table_t*)phys;
    memset(table, 0, sizeof(page_table_t));
    
    // Set entry
    *entry = phys | flags | PAGE_PRESENT | PAGE_WRITE;
    
    return table;
}

void vmm_init(void) {
    printk("[VMM] Initializing virtual memory manager...\n");
    
    // Allocate kernel PML4
    kernel_directory.pml4_phys = pmm_alloc_frame();
    kernel_directory.pml4 = (page_table_t*)kernel_directory.pml4_phys;
    
    memset(kernel_directory.pml4, 0, sizeof(page_table_t));
    
    printk("[VMM] Kernel PML4 at: %p\n", (void*)kernel_directory.pml4_phys);
    
    // Identity map first 4MB (for kernel code/data)
    // We already have paging from boot.S, so we're setting up proper kernel tables
    for (uintptr addr = 0; addr < 0x400000; addr += PAGE_SIZE) {
        vmm_map_page(&kernel_directory, addr, addr, 
                     PAGE_PRESENT | PAGE_WRITE);
    }
    
    // Map kernel to higher half (0xFFFFFFFF80000000)
    // Map first 16MB of physical memory
    for (uintptr addr = 0; addr < 0x1000000; addr += PAGE_SIZE) {
        vmm_map_page(&kernel_directory, 0xFFFFFFFF80000000ULL + addr, addr,
                     PAGE_PRESENT | PAGE_WRITE);
    }
    
    current_directory = &kernel_directory;
    
    printk("[VMM] Mapped first 4MB identity and 16MB at higher-half\n");
    printk("[VMM] Initialization complete!\n");
}

page_directory_t* vmm_create_address_space(void) {
    page_directory_t* pd = (page_directory_t*)pmm_alloc_frame();
    if (pd == NULL) {
        return NULL;
    }
    
    pd->pml4_phys = pmm_alloc_frame();
    pd->pml4 = (page_table_t*)pd->pml4_phys;
    
    if (pd->pml4 == NULL) {
        pmm_free_frame((uintptr)pd);
        return NULL;
    }
    
    memset(pd->pml4, 0, sizeof(page_table_t));
    
    // Copy kernel mappings to new address space
    // Copy upper half (kernel space)
    for (size_t i = 256; i < 512; i++) {
        pd->pml4->entries[i] = kernel_directory.pml4->entries[i];
    }
    
    return pd;
}

void vmm_destroy_address_space(page_directory_t* pd) {
    if (pd == NULL || pd == &kernel_directory) {
        return;
    }
    
    // Free user-space page tables (lower half only)
    for (size_t i = 0; i < 256; i++) {
        if (pd->pml4->entries[i] & PAGE_PRESENT) {
            page_table_t* pdpt = (page_table_t*)ENTRY_ADDR(pd->pml4->entries[i]);
            
            for (size_t j = 0; j < 512; j++) {
                if (pdpt->entries[j] & PAGE_PRESENT) {
                    page_table_t* pdir = (page_table_t*)ENTRY_ADDR(pdpt->entries[j]);
                    
                    for (size_t k = 0; k < 512; k++) {
                        if (pdir->entries[k] & PAGE_PRESENT) {
                            page_table_t* pt = (page_table_t*)ENTRY_ADDR(pdir->entries[k]);
                            pmm_free_frame((uintptr)pt);
                        }
                    }
                    
                    pmm_free_frame((uintptr)pdir);
                }
            }
            
            pmm_free_frame((uintptr)pdpt);
        }
    }
    
    pmm_free_frame(pd->pml4_phys);
    pmm_free_frame((uintptr)pd);
}

void vmm_map_page(page_directory_t* pd, uintptr virt, uintptr phys, u32 flags) {
    if (pd == NULL) {
        pd = current_directory;
    }
    
    size_t pml4_idx = PML4_INDEX(virt);
    size_t pdpt_idx = PDPT_INDEX(virt);
    size_t pd_idx = PD_INDEX(virt);
    size_t pt_idx = PT_INDEX(virt);
    
    // Get or create PDPT
    page_table_t* pdpt = get_or_create_table(&pd->pml4->entries[pml4_idx], flags);
    if (pdpt == NULL) return;
    
    // Get or create PD
    page_table_t* pdir = get_or_create_table(&pdpt->entries[pdpt_idx], flags);
    if (pdir == NULL) return;
    
    // Get or create PT
    page_table_t* pt = get_or_create_table(&pdir->entries[pd_idx], flags);
    if (pt == NULL) return;
    
    // Map the page
    pt->entries[pt_idx] = (phys & 0x000FFFFFFFFFF000ULL) | (flags & 0xFFF) | PAGE_PRESENT;
}

void vmm_unmap_page(page_directory_t* pd, uintptr virt) {
    if (pd == NULL) {
        pd = current_directory;
    }
    
    size_t pml4_idx = PML4_INDEX(virt);
    size_t pdpt_idx = PDPT_INDEX(virt);
    size_t pd_idx = PD_INDEX(virt);
    size_t pt_idx = PT_INDEX(virt);
    
    if (!(pd->pml4->entries[pml4_idx] & PAGE_PRESENT)) return;
    page_table_t* pdpt = (page_table_t*)ENTRY_ADDR(pd->pml4->entries[pml4_idx]);
    
    if (!(pdpt->entries[pdpt_idx] & PAGE_PRESENT)) return;
    page_table_t* pdir = (page_table_t*)ENTRY_ADDR(pdpt->entries[pdpt_idx]);
    
    if (!(pdir->entries[pd_idx] & PAGE_PRESENT)) return;
    page_table_t* pt = (page_table_t*)ENTRY_ADDR(pdir->entries[pd_idx]);
    
    // Unmap the page
    pt->entries[pt_idx] = 0;
    
    // Invalidate TLB
    __asm__ volatile("invlpg (%0)" : : "r"(virt) : "memory");
}

uintptr vmm_get_physical(page_directory_t* pd, uintptr virt) {
    if (pd == NULL) {
        pd = current_directory;
    }
    
    size_t pml4_idx = PML4_INDEX(virt);
    size_t pdpt_idx = PDPT_INDEX(virt);
    size_t pd_idx = PD_INDEX(virt);
    size_t pt_idx = PT_INDEX(virt);
    
    if (!(pd->pml4->entries[pml4_idx] & PAGE_PRESENT)) return 0;
    page_table_t* pdpt = (page_table_t*)ENTRY_ADDR(pd->pml4->entries[pml4_idx]);
    
    if (!(pdpt->entries[pdpt_idx] & PAGE_PRESENT)) return 0;
    page_table_t* pdir = (page_table_t*)ENTRY_ADDR(pdpt->entries[pdpt_idx]);
    
    if (!(pdir->entries[pd_idx] & PAGE_PRESENT)) return 0;
    page_table_t* pt = (page_table_t*)ENTRY_ADDR(pdir->entries[pd_idx]);
    
    if (!(pt->entries[pt_idx] & PAGE_PRESENT)) return 0;
    
    return ENTRY_ADDR(pt->entries[pt_idx]) | (virt & 0xFFF);
}

void vmm_switch_directory(page_directory_t* pd) {
    if (pd == NULL) return;
    
    current_directory = pd;
    
    // Load CR3 with new PML4
    __asm__ volatile("mov %0, %%cr3" : : "r"(pd->pml4_phys) : "memory");
}

page_directory_t* vmm_get_current_directory(void) {
    return current_directory;
}
