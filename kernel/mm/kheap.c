/* Kernel Heap - Simple block allocator */

#include <stdint.h>
#include <stddef.h>
#include "kheap.h"
#include "pmm.h"

/* Heap block header */
typedef struct heap_block {
    size_t size;           /* Size of data (excluding header) */
    int free;              /* 1 if free, 0 if allocated */
    struct heap_block *next;
    struct heap_block *prev;
} heap_block_t;

/* Heap state */
static heap_block_t *heap_start = (void*)0;
static heap_block_t *heap_end = (void*)0;
static size_t heap_used = 0;
static size_t heap_total = 0;

/* Initial heap size: 1MB */
#define INITIAL_HEAP_SIZE (1024 * 1024)
#define BLOCK_HEADER_SIZE sizeof(heap_block_t)
#define MIN_BLOCK_SIZE 16

/* Memory operations */
static void *memset_heap(void *s, int c, size_t n) {
    uint8_t *p = (uint8_t *)s;
    while (n--) *p++ = (uint8_t)c;
    return s;
}

void kheap_init(void) {
    /* Allocate initial heap pages */
    uint64_t pages_needed = (INITIAL_HEAP_SIZE + PAGE_SIZE - 1) / PAGE_SIZE;
    uint64_t heap_phys = pmm_alloc_pages(pages_needed);
    
    if (!heap_phys) {
        /* Failed to allocate heap - panic */
        return;
    }
    
    heap_start = (heap_block_t *)heap_phys;
    heap_total = pages_needed * PAGE_SIZE;
    
    /* Initialize first free block */
    heap_start->size = heap_total - BLOCK_HEADER_SIZE;
    heap_start->free = 1;
    heap_start->next = (void*)0;
    heap_start->prev = (void*)0;
    
    heap_end = heap_start;
}

/* Find a free block of at least 'size' bytes */
static heap_block_t *find_free_block(size_t size) {
    heap_block_t *block = heap_start;
    
    while (block) {
        if (block->free && block->size >= size) {
            return block;
        }
        block = block->next;
    }
    
    return (void*)0;
}

/* Split a block if it's larger than needed */
static void split_block(heap_block_t *block, size_t size) {
    if (block->size >= size + BLOCK_HEADER_SIZE + MIN_BLOCK_SIZE) {
        heap_block_t *new_block = (heap_block_t *)((uint8_t *)block + BLOCK_HEADER_SIZE + size);
        new_block->size = block->size - size - BLOCK_HEADER_SIZE;
        new_block->free = 1;
        new_block->next = block->next;
        new_block->prev = block;
        
        if (block->next) {
            block->next->prev = new_block;
        } else {
            heap_end = new_block;
        }
        
        block->next = new_block;
        block->size = size;
    }
}

/* Merge adjacent free blocks */
static void merge_blocks(heap_block_t *block) {
    /* Merge with next block if free */
    if (block->next && block->next->free) {
        block->size += BLOCK_HEADER_SIZE + block->next->size;
        block->next = block->next->next;
        if (block->next) {
            block->next->prev = block;
        } else {
            heap_end = block;
        }
    }
    
    /* Merge with previous block if free */
    if (block->prev && block->prev->free) {
        block->prev->size += BLOCK_HEADER_SIZE + block->size;
        block->prev->next = block->next;
        if (block->next) {
            block->next->prev = block->prev;
        } else {
            heap_end = block->prev;
        }
    }
}

void *kmalloc(size_t size) {
    if (!heap_start || size == 0) return (void*)0;
    
    /* Align size to 8 bytes */
    size = (size + 7) & ~7;
    
    heap_block_t *block = find_free_block(size);
    
    if (!block) {
        /* TODO: Expand heap by allocating more pages */
        return (void*)0;
    }
    
    split_block(block, size);
    block->free = 0;
    heap_used += block->size + BLOCK_HEADER_SIZE;
    
    return (void *)((uint8_t *)block + BLOCK_HEADER_SIZE);
}

void *kzalloc(size_t size) {
    void *ptr = kmalloc(size);
    if (ptr) {
        memset_heap(ptr, 0, size);
    }
    return ptr;
}

void *kmalloc_aligned(size_t size, size_t alignment) {
    /* Simple implementation: over-allocate and align */
    if (alignment <= 8) return kmalloc(size);
    
    void *ptr = kmalloc(size + alignment);
    if (!ptr) return (void*)0;
    
    uint64_t addr = (uint64_t)ptr;
    uint64_t aligned = (addr + alignment - 1) & ~(alignment - 1);
    
    /* Store original pointer just before aligned address */
    /* Note: This is a simplified implementation */
    return (void *)aligned;
}

void kfree(void *ptr) {
    if (!ptr) return;
    
    heap_block_t *block = (heap_block_t *)((uint8_t *)ptr - BLOCK_HEADER_SIZE);
    
    /* Sanity check */
    if (block < heap_start || (uint64_t)block >= (uint64_t)heap_start + heap_total) {
        return;  /* Invalid pointer */
    }
    
    if (block->free) return;  /* Already free */
    
    block->free = 1;
    heap_used -= block->size + BLOCK_HEADER_SIZE;
    
    merge_blocks(block);
}

size_t kheap_get_used(void) {
    return heap_used;
}

size_t kheap_get_free(void) {
    return heap_total - heap_used;
}
