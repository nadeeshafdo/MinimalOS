/* Physical Memory Manager - Bitmap allocator */

#include <stdint.h>
#include "pmm.h"
#include "multiboot2.h"

/* Bitmap storage - statically allocated for simplicity */
/* Support up to 4GB = 1M pages = 128KB bitmap */
#define MAX_PAGES (1024 * 1024)
static uint64_t bitmap[MAX_PAGES / 64];

/* Memory statistics */
static uint64_t total_pages = 0;
static uint64_t used_pages = 0;

/* Kernel end address (from linker) */
extern char __kernel_end[];

/* Bitmap operations */
static inline void bitmap_set(uint64_t bit) {
    bitmap[bit / 64] |= (1ULL << (bit % 64));
}

static inline void bitmap_clear(uint64_t bit) {
    bitmap[bit / 64] &= ~(1ULL << (bit % 64));
}

static inline int bitmap_test(uint64_t bit) {
    return (bitmap[bit / 64] >> (bit % 64)) & 1;
}

void pmm_init(uint64_t mb_info_addr) {
    /* First, mark all pages as used */
    for (uint64_t i = 0; i < MAX_PAGES / 64; i++) {
        bitmap[i] = 0xFFFFFFFFFFFFFFFFULL;
    }
    
    /* Get memory map from multiboot2 */
    multiboot2_tag_mmap_t *mmap = (multiboot2_tag_mmap_t *)multiboot2_find_tag(mb_info_addr, MULTIBOOT2_TAG_MMAP);
    
    if (!mmap) {
        /* Fallback: assume 16MB available starting at 1MB */
        total_pages = 16 * 256;  /* 16MB / 4KB */
        for (uint64_t i = 256; i < 256 + total_pages; i++) {
            bitmap_clear(i);
        }
        return;
    }
    
    /* Parse memory map and mark available regions */
    multiboot2_mmap_entry_t *entry = (multiboot2_mmap_entry_t *)((uint64_t)mmap + sizeof(multiboot2_tag_mmap_t));
    
    while ((uint64_t)entry < (uint64_t)mmap + mmap->size) {
        if (entry->type == MULTIBOOT2_MMAP_AVAILABLE) {
            uint64_t start_page = (entry->base_addr + PAGE_SIZE - 1) / PAGE_SIZE;
            uint64_t end_page = (entry->base_addr + entry->length) / PAGE_SIZE;
            
            if (end_page > MAX_PAGES) end_page = MAX_PAGES;
            
            for (uint64_t i = start_page; i < end_page; i++) {
                bitmap_clear(i);
                total_pages++;
            }
        }
        entry = (multiboot2_mmap_entry_t *)((uint64_t)entry + mmap->entry_size);
    }
    
    /* Reserve first 1MB (legacy BIOS area) */
    for (uint64_t i = 0; i < 256; i++) {
        if (!bitmap_test(i)) {
            bitmap_set(i);
            used_pages++;
        }
    }
    
    /* Reserve kernel memory (up to __kernel_end) */
    uint64_t kernel_end_page = ((uint64_t)__kernel_end + PAGE_SIZE - 1) / PAGE_SIZE;
    for (uint64_t i = 256; i <= kernel_end_page; i++) {
        if (!bitmap_test(i)) {
            bitmap_set(i);
            used_pages++;
        }
    }
    
    /* Reserve bitmap itself */
    uint64_t bitmap_start = (uint64_t)bitmap / PAGE_SIZE;
    uint64_t bitmap_end = ((uint64_t)bitmap + sizeof(bitmap) + PAGE_SIZE - 1) / PAGE_SIZE;
    for (uint64_t i = bitmap_start; i <= bitmap_end; i++) {
        if (i < MAX_PAGES && !bitmap_test(i)) {
            bitmap_set(i);
            used_pages++;
        }
    }
}

uint64_t pmm_alloc_page(void) {
    for (uint64_t i = 0; i < MAX_PAGES / 64; i++) {
        if (bitmap[i] != 0xFFFFFFFFFFFFFFFFULL) {
            /* Found a free bit */
            for (int j = 0; j < 64; j++) {
                uint64_t bit = i * 64 + j;
                if (!bitmap_test(bit)) {
                    bitmap_set(bit);
                    used_pages++;
                    return bit * PAGE_SIZE;
                }
            }
        }
    }
    return 0;  /* Out of memory */
}

uint64_t pmm_alloc_pages(uint64_t count) {
    uint64_t consecutive = 0;
    uint64_t start = 0;
    
    for (uint64_t i = 0; i < MAX_PAGES; i++) {
        if (!bitmap_test(i)) {
            if (consecutive == 0) start = i;
            consecutive++;
            if (consecutive == count) {
                /* Allocate all pages */
                for (uint64_t j = start; j < start + count; j++) {
                    bitmap_set(j);
                    used_pages++;
                }
                return start * PAGE_SIZE;
            }
        } else {
            consecutive = 0;
        }
    }
    return 0;  /* Not enough contiguous memory */
}

void pmm_free_page(uint64_t phys_addr) {
    uint64_t page = phys_addr / PAGE_SIZE;
    if (page < MAX_PAGES && bitmap_test(page)) {
        bitmap_clear(page);
        used_pages--;
    }
}

void pmm_free_pages(uint64_t phys_addr, uint64_t count) {
    for (uint64_t i = 0; i < count; i++) {
        pmm_free_page(phys_addr + i * PAGE_SIZE);
    }
}

uint64_t pmm_get_total_memory(void) {
    return total_pages * PAGE_SIZE;
}

uint64_t pmm_get_free_memory(void) {
    return (total_pages - used_pages) * PAGE_SIZE;
}

uint64_t pmm_get_used_memory(void) {
    return used_pages * PAGE_SIZE;
}
