#include <stdint.h>
#include <stddef.h>
#include <kernel/pmm.h>

/* Memory constants */
#define BLOCKS_PER_BYTE 8
#define BLOCK_SIZE PAGE_SIZE

/* Bitmap for tracking free/used pages */
static uint32_t *memory_bitmap = 0;
static uint32_t total_blocks = 0;
static uint32_t used_blocks = 0;
static uint32_t bitmap_size = 0;
static uint32_t total_memory = 0;

/* Set a bit in the bitmap (mark block as used) */
static inline void bitmap_set(uint32_t bit) {
    memory_bitmap[bit / 32] |= (1 << (bit % 32));
}

/* Clear a bit in the bitmap (mark block as free) */
static inline void bitmap_clear(uint32_t bit) {
    memory_bitmap[bit / 32] &= ~(1 << (bit % 32));
}

/* Test a bit in the bitmap */
static inline uint32_t bitmap_test(uint32_t bit) {
    return memory_bitmap[bit / 32] & (1 << (bit % 32));
}

/* Find first free block */
static int32_t bitmap_first_free(void) {
    for (uint32_t i = 0; i < total_blocks / 32; i++) {
        if (memory_bitmap[i] != 0xFFFFFFFF) {
            for (uint32_t j = 0; j < 32; j++) {
                uint32_t bit = 1 << j;
                if (!(memory_bitmap[i] & bit)) {
                    return i * 32 + j;
                }
            }
        }
    }
    return -1;  /* No free blocks */
}

void pmm_init(uint32_t mem_size, uint32_t *bitmap_addr) {
    total_memory = mem_size;
    memory_bitmap = bitmap_addr;
    total_blocks = mem_size / BLOCK_SIZE;
    bitmap_size = total_blocks / BLOCKS_PER_BYTE;
    
    if (total_blocks % BLOCKS_PER_BYTE) {
        bitmap_size++;
    }
    
    /* Mark all blocks as used initially */
    for (uint32_t i = 0; i < bitmap_size / 4; i++) {
        memory_bitmap[i] = 0xFFFFFFFF;
    }
    used_blocks = total_blocks;
}

void *pmm_alloc_frame(void) {
    if (used_blocks >= total_blocks) {
        return 0;  /* Out of memory */
    }
    
    int32_t frame = bitmap_first_free();
    if (frame == -1) {
        return 0;
    }
    
    bitmap_set(frame);
    used_blocks++;
    
    return (void*)(frame * BLOCK_SIZE);
}

void pmm_free_frame(void *frame) {
    uint32_t addr = (uint32_t)frame;
    uint32_t block = addr / BLOCK_SIZE;
    
    if (bitmap_test(block)) {
        bitmap_clear(block);
        used_blocks--;
    }
}

void pmm_mark_region_used(uint32_t base, size_t size) {
    uint32_t align = base / BLOCK_SIZE;
    uint32_t blocks = size / BLOCK_SIZE;
    
    if (size % BLOCK_SIZE) {
        blocks++;
    }
    
    for (uint32_t i = 0; i < blocks; i++) {
        if (!bitmap_test(align + i)) {
            bitmap_set(align + i);
            used_blocks++;
        }
    }
}

void pmm_mark_region_free(uint32_t base, size_t size) {
    uint32_t align = base / BLOCK_SIZE;
    uint32_t blocks = size / BLOCK_SIZE;
    
    if (size % BLOCK_SIZE) {
        blocks++;
    }
    
    for (uint32_t i = 0; i < blocks; i++) {
        if (bitmap_test(align + i)) {
            bitmap_clear(align + i);
            used_blocks--;
        }
    }
}

uint32_t pmm_get_total_memory(void) {
    return total_memory;
}

uint32_t pmm_get_free_memory(void) {
    return (total_blocks - used_blocks) * BLOCK_SIZE;
}
