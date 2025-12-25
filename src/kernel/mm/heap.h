#ifndef HEAP_H
#define HEAP_H

#include "../include/types.h"

/**
 * Initialize kernel heap allocator
 */
void heap_init(void);

/**
 * Allocate memory from kernel heap
 * Returns NULL on failure
 */
void* kmalloc(size_t size);

/**
 * Allocate and zero-initialize memory
 */
void* kzalloc(size_t size);

/**
 * Reallocate memory
 */
void* krealloc(void* ptr, size_t size);

/**
 * Free allocated memory
 */
void kfree(void* ptr);

/**
 * Get heap statistics
 */
void heap_get_stats(size_t* total, size_t* used, size_t* free);

#endif // HEAP_H
