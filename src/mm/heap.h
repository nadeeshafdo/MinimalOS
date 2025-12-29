/**
 * MinimalOS - Kernel Heap Allocator
 * Simple first-fit allocator for kernel dynamic memory
 */

#ifndef MM_HEAP_H
#define MM_HEAP_H

#include <minimalos/types.h>

/* Heap configuration */
#define HEAP_INITIAL_SIZE (1 * MB) /* Initial heap size */
#define HEAP_MAX_SIZE (16 * MB)    /* Maximum heap size */
#define HEAP_BLOCK_ALIGN 16        /* Alignment for allocations */
#define HEAP_MIN_BLOCK 32          /* Minimum block size */

/**
 * Initialize the kernel heap
 * Must be called after PMM is initialized
 */
void heap_init(void);

/**
 * Allocate memory from the kernel heap
 * @param size Number of bytes to allocate
 * @return Pointer to allocated memory, or NULL on failure
 */
void *kmalloc(size_t size);

/**
 * Allocate zeroed memory from the kernel heap
 * @param size Number of bytes to allocate
 * @return Pointer to zero-initialized memory, or NULL on failure
 */
void *kzalloc(size_t size);

/**
 * Allocate aligned memory from the kernel heap
 * @param size Number of bytes to allocate
 * @param align Alignment requirement (must be power of 2)
 * @return Pointer to aligned memory, or NULL on failure
 */
void *kmalloc_aligned(size_t size, size_t align);

/**
 * Free memory allocated with kmalloc
 * @param ptr Pointer to memory to free (NULL is safe)
 */
void kfree(void *ptr);

/**
 * Get heap statistics
 * @param total_out Pointer to store total heap size
 * @param used_out Pointer to store used bytes
 * @param free_out Pointer to store free bytes
 */
void heap_stats(size_t *total_out, size_t *used_out, size_t *free_out);

#endif /* MM_HEAP_H */
