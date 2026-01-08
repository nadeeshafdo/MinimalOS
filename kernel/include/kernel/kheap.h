#ifndef _KERNEL_KHEAP_H
#define _KERNEL_KHEAP_H

#include <stddef.h>

/* Initialize kernel heap */
void kheap_init(void);

/* Allocate memory from kernel heap */
void *kmalloc(size_t size);

/* Allocate aligned memory from kernel heap */
void *kmalloc_aligned(size_t size, size_t alignment);

/* Free memory back to kernel heap */
void kfree(void *ptr);

/* Get heap statistics */
size_t kheap_get_used(void);
size_t kheap_get_free(void);

#endif /* _KERNEL_KHEAP_H */
