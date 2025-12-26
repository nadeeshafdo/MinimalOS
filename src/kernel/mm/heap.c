#include "heap.h"
#include "pmm.h"
#include "../lib/string.h"
#include "../lib/printk.h"

// Simple block header for tracking allocations
typedef struct block_header {
    size_t size;
    bool is_free;
    struct block_header* next;
    u32 magic;  // For debugging
} block_header_t;

#define BLOCK_MAGIC 0xDEADBEEF
#define HEAP_START 0x00200000  // 2MB (after kernel, within 8MB identity map)
#define HEAP_SIZE  0x00400000  // 4MB initial heap

static block_header_t* heap_start = NULL;
static size_t total_heap = 0;
static size_t used_heap = 0;

void heap_init(void) {
    printk("[HEAP] Initializing kernel heap allocator...\n");
    
    heap_start = (block_header_t*)HEAP_START;
    total_heap = HEAP_SIZE;
    
    printk("[HEAP] Heap at: %p (size: %u MB)\n", heap_start, (u32)(HEAP_SIZE / (1024 * 1024)));
    
    // Initialize first block
    heap_start->size = HEAP_SIZE - sizeof(block_header_t);
    heap_start->is_free = true;
    heap_start->next = NULL;
    heap_start->magic = BLOCK_MAGIC;
    
    printk("[HEAP] Initialization complete!\n");
}

static block_header_t* find_free_block(size_t size) {
    block_header_t* current = heap_start;
    
    while (current != NULL) {
        if (current->magic != BLOCK_MAGIC) {
            printk("[HEAP] ERROR: Corrupted heap block at %p\n", current);
            return NULL;
        }
        
        if (current->is_free && current->size >= size) {
            return current;
        }
        
        current = current->next;
    }
    
    return NULL;
}

static void split_block(block_header_t* block, size_t size) {
    // Only split if there's enough space for a new block header + some data
    if (block->size >= size + sizeof(block_header_t) + 64) {
        block_header_t* new_block = (block_header_t*)((u8*)block + sizeof(block_header_t) + size);
        new_block->size = block->size - size - sizeof(block_header_t);
        new_block->is_free = true;
        new_block->next = block->next;
        new_block->magic = BLOCK_MAGIC;
        
        block->size = size;
        block->next = new_block;
    }
}

static void merge_free_blocks(void) {
    block_header_t* current = heap_start;
    
    while (current != NULL && current->next != NULL) {
        if (current->is_free && current->next->is_free) {
            // Merge with next block
            current->size += sizeof(block_header_t) + current->next->size;
            current->next = current->next->next;
        } else {
            current = current->next;
        }
    }
}

void* kmalloc(size_t size) {
    if (size == 0) {
        return NULL;
    }
    
    // Align size to 8 bytes
    size = (size + 7) & ~7;
    
    block_header_t* block = find_free_block(size);
    if (block == NULL) {
        printk("[HEAP] ERROR: Out of memory! (requested: %u bytes)\n", (u32)size);
        return NULL;
    }
    
    split_block(block, size);
    block->is_free = false;
    
    used_heap += size + sizeof(block_header_t);
    
    return (void*)((u8*)block + sizeof(block_header_t));
}

void* kzalloc(size_t size) {
    void* ptr = kmalloc(size);
    if (ptr != NULL) {
        memset(ptr, 0, size);
    }
    return ptr;
}

void kfree(void* ptr) {
    if (ptr == NULL) {
        return;
    }
    
    block_header_t* block = (block_header_t*)((u8*)ptr - sizeof(block_header_t));
    
    if (block->magic != BLOCK_MAGIC) {
        printk("[HEAP] ERROR: Invalid free at %p (bad magic)\n", ptr);
        return;
    }
    
    if (block->is_free) {
        printk("[HEAP] WARNING: Double free at %p\n", ptr);
        return;
    }
    
    block->is_free = true;
    used_heap -= block->size + sizeof(block_header_t);
    
    // Merge adjacent free blocks
    merge_free_blocks();
}

void* krealloc(void* ptr, size_t size) {
    if (ptr == NULL) {
        return kmalloc(size);
    }
    
    if (size == 0) {
        kfree(ptr);
        return NULL;
    }
    
    block_header_t* block = (block_header_t*)((u8*)ptr - sizeof(block_header_t));
    
    if (block->magic != BLOCK_MAGIC) {
        printk("[HEAP] ERROR: Invalid realloc at %p\n", ptr);
        return NULL;
    }
    
    if (block->size >= size) {
        // Current block is large enough
        return ptr;
    }
    
    // Allocate new block and copy data
    void* new_ptr = kmalloc(size);
    if (new_ptr == NULL) {
        return NULL;
    }
    
    memcpy(new_ptr, ptr, block->size);
    kfree(ptr);
    
    return new_ptr;
}

void heap_get_stats(size_t* total, size_t* used, size_t* free) {
    if (total) *total = total_heap;
    if (used) *used = used_heap;
    if (free) *free = total_heap - used_heap;
}
