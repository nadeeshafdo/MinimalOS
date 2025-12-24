#ifndef _KHEAP_H
#define _KHEAP_H

#include <stdint.h>
#include <stddef.h>

/* Initialize kernel heap */
void kheap_init(void);

/* Allocate memory */
void *kmalloc(size_t size);

/* Allocate zeroed memory */
void *kzalloc(size_t size);

/* Allocate aligned memory */
void *kmalloc_aligned(size_t size, size_t alignment);

/* Free memory */
void kfree(void *ptr);

/* Get heap statistics */
size_t kheap_get_used(void);
size_t kheap_get_free(void);

#endif /* _KHEAP_H */
