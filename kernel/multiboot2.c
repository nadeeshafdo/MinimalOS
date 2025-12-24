/* Multiboot2 parsing */

#include <stdint.h>
#include "multiboot2.h"

multiboot2_tag_t *multiboot2_find_tag(uint64_t mb_info_addr, uint32_t tag_type) {
    multiboot2_info_t *info = (multiboot2_info_t *)mb_info_addr;
    multiboot2_tag_t *tag = (multiboot2_tag_t *)(mb_info_addr + 8);
    
    while ((uint64_t)tag < mb_info_addr + info->total_size) {
        if (tag->type == tag_type) {
            return tag;
        }
        if (tag->type == MULTIBOOT2_TAG_END) {
            break;
        }
        /* Move to next tag (8-byte aligned) */
        tag = (multiboot2_tag_t *)((uint64_t)tag + ((tag->size + 7) & ~7));
    }
    
    return (void*)0;
}

uint64_t multiboot2_get_memory_size(uint64_t mb_info_addr) {
    multiboot2_tag_mmap_t *mmap = (multiboot2_tag_mmap_t *)multiboot2_find_tag(mb_info_addr, MULTIBOOT2_TAG_MMAP);
    
    if (!mmap) {
        /* Try basic meminfo */
        multiboot2_tag_basic_meminfo_t *basic = (multiboot2_tag_basic_meminfo_t *)multiboot2_find_tag(mb_info_addr, MULTIBOOT2_TAG_BASIC_MEMINFO);
        if (basic) {
            return ((uint64_t)basic->mem_upper + 1024) * 1024;  /* Convert KB to bytes */
        }
        return 0;
    }
    
    uint64_t total = 0;
    multiboot2_mmap_entry_t *entry = (multiboot2_mmap_entry_t *)((uint64_t)mmap + sizeof(multiboot2_tag_mmap_t));
    
    while ((uint64_t)entry < (uint64_t)mmap + mmap->size) {
        if (entry->type == MULTIBOOT2_MMAP_AVAILABLE) {
            total += entry->length;
        }
        entry = (multiboot2_mmap_entry_t *)((uint64_t)entry + mmap->entry_size);
    }
    
    return total;
}
