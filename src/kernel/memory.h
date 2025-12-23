#ifndef MEMORY_H
#define MEMORY_H

#include "stdint.h"
#include "stddef.h"

// Initialize heap memory allocator
void heap_init(void* start, size_t size);

// Allocate memory block
void* malloc(size_t size);

// Free memory block
void free(void* ptr);

// Get available heap memory
size_t mem_available(void);

// Get used heap memory
size_t mem_used(void);

// Get total heap size
size_t mem_total(void);

#endif
