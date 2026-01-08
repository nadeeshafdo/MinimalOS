/* Kernel heap header for x86_64 */
#ifndef KERNEL_KHEAP_H
#define KERNEL_KHEAP_H

#include <stddef.h>
#include <stdint.h>

/* Initialize kernel heap */
void kheap_init(void);

/* Memory allocation */
void *kmalloc(size_t size);
void *kmalloc_aligned(size_t size, size_t alignment);
void kfree(void *ptr);

/* Memory info */
size_t kheap_get_used(void);
size_t kheap_get_free(void);

#endif
