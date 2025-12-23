#include "memory.h"

// Memory block header
typedef struct block_header {
    size_t size;
    int is_free;
    struct block_header* next;
} block_header_t;

// Heap metadata
static void* heap_start = 0;
static size_t heap_size = 0;
static block_header_t* free_list = 0;

// Align size to 16 bytes
static size_t align_size(size_t size) {
    return (size + 15) & ~15;
}

void heap_init(void* start, size_t size) {
    heap_start = start;
    heap_size = size;
    
    // Initialize first free block
    free_list = (block_header_t*)start;
    free_list->size = size - sizeof(block_header_t);
    free_list->is_free = 1;
    free_list->next = 0;
}

void* malloc(size_t size) {
    if (size == 0) return 0;
    
    size = align_size(size);
    
    // Find first fit
    block_header_t* current = free_list;
    
    while (current) {
        if (current->is_free && current->size >= size) {
            // Found suitable block
            
            // Split block if large enough
            if (current->size > size + sizeof(block_header_t) + 16) {
                block_header_t* new_block = (block_header_t*)((char*)current + sizeof(block_header_t) + size);
                new_block->size = current->size - size - sizeof(block_header_t);
                new_block->is_free = 1;
                new_block->next = current->next;
                
                current->size = size;
                current->next = new_block;
            }
            
            current->is_free = 0;
            return (void*)((char*)current + sizeof(block_header_t));
        }
        
        current = current->next;
    }
    
    // No suitable block found
    return 0;
}

void free(void* ptr) {
    if (!ptr) return;
    
    // Get block header
    block_header_t* block = (block_header_t*)((char*)ptr - sizeof(block_header_t));
    block->is_free = 1;
    
    // Coalesce with next block if free
    if (block->next && block->next->is_free) {
        block->size += sizeof(block_header_t) + block->next->size;
        block->next = block->next->next;
    }
    
    // Coalesce with previous block if free
    block_header_t* current = free_list;
    while (current && current->next != block) {
        current = current->next;
    }
    
    if (current && current->is_free) {
        current->size += sizeof(block_header_t) + block->size;
        current->next = block->next;
    }
}

size_t mem_available(void) {
    size_t available = 0;
    block_header_t* current = free_list;
    
    while (current) {
        if (current->is_free) {
            available += current->size;
        }
        current = current->next;
    }
    
    return available;
}

size_t mem_used(void) {
    return heap_size - mem_available() - sizeof(block_header_t);
}

size_t mem_total(void) {
    return heap_size;
}
