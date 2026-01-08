/* Kernel heap for x86_64 */
#include <kernel/kheap.h>
#include <kernel/pmm.h>
#include <stddef.h>
#include <stdint.h>

/* Heap configuration - placed in higher-half memory */
#define HEAP_START 0xFFFF800000000000ULL /* Start of heap in higher half */
#define HEAP_INITIAL 0x00100000          /* 1MB initial heap */
#define HEAP_MAX 0x10000000              /* 256MB max heap */

/* Block header structure */
typedef struct block_header {
  size_t size;               /* Size of block (including header) */
  uint8_t is_free;           /* Is this block free? */
  struct block_header *next; /* Next block in list */
  struct block_header *prev; /* Previous block in list */
} block_header_t;

#define HEADER_SIZE sizeof(block_header_t)
#define MIN_BLOCK_SIZE (HEADER_SIZE + 16)

/* Heap state */
static uint64_t heap_start = 0;
static uint64_t heap_end = 0;
static block_header_t *first_block = NULL;
static size_t total_allocated = 0;

/* Align size to 16 bytes (for 64-bit) */
static inline size_t align_size(size_t size) { return (size + 15) & ~15; }

/* Get HHDM offset */
extern uint64_t get_hhdm_offset(void);

void kheap_init(void) {
  /* For now, use a simple approach - allocate from HHDM region */
  /* We use physical memory starting at 16MB, mapped via HHDM */
  uint64_t hhdm = get_hhdm_offset();

  /* Start heap at 16MB physical, accessed via HHDM */
  heap_start = hhdm + 0x1000000; /* 16MB */
  heap_end = heap_start + HEAP_INITIAL;

  /* Create initial free block */
  first_block = (block_header_t *)heap_start;
  first_block->size = HEAP_INITIAL;
  first_block->is_free = 1;
  first_block->next = NULL;
  first_block->prev = NULL;

  total_allocated = 0;
}

/* Find a free block that fits the requested size */
static block_header_t *find_free_block(size_t size) {
  block_header_t *current = first_block;

  while (current) {
    if (current->is_free && current->size >= size) {
      return current;
    }
    current = current->next;
  }

  return NULL;
}

/* Split a block if it's too large */
static void split_block(block_header_t *block, size_t size) {
  if (block->size >= size + MIN_BLOCK_SIZE) {
    block_header_t *new_block = (block_header_t *)((uint8_t *)block + size);
    new_block->size = block->size - size;
    new_block->is_free = 1;
    new_block->next = block->next;
    new_block->prev = block;

    if (block->next) {
      block->next->prev = new_block;
    }

    block->size = size;
    block->next = new_block;
  }
}

/* Merge adjacent free blocks */
static void merge_blocks(block_header_t *block) {
  /* Merge with next block if free */
  if (block->next && block->next->is_free) {
    block->size += block->next->size;
    block->next = block->next->next;
    if (block->next) {
      block->next->prev = block;
    }
  }

  /* Merge with previous block if free */
  if (block->prev && block->prev->is_free) {
    block->prev->size += block->size;
    block->prev->next = block->next;
    if (block->next) {
      block->next->prev = block->prev;
    }
  }
}

void *kmalloc(size_t size) {
  if (size == 0) {
    return NULL;
  }

  size = align_size(size + HEADER_SIZE);

  block_header_t *block = find_free_block(size);

  if (!block) {
    return NULL;
  }

  split_block(block, size);
  block->is_free = 0;
  total_allocated += block->size;

  /* Return pointer after header */
  return (void *)((uint8_t *)block + HEADER_SIZE);
}

void *kmalloc_aligned(size_t size, size_t alignment) {
  /* Simple implementation - allocate extra space for alignment */
  size_t total = size + alignment;
  void *ptr = kmalloc(total);

  if (!ptr) {
    return NULL;
  }

  /* Align the pointer */
  uint64_t addr = (uint64_t)ptr;
  uint64_t aligned = (addr + alignment - 1) & ~(alignment - 1);

  return (void *)aligned;
}

void kfree(void *ptr) {
  if (!ptr) {
    return;
  }

  /* Get block header */
  block_header_t *block = (block_header_t *)((uint8_t *)ptr - HEADER_SIZE);

  if (block->is_free) {
    return; /* Already free */
  }

  block->is_free = 1;
  total_allocated -= block->size;

  /* Merge with adjacent free blocks */
  merge_blocks(block);
}

size_t kheap_get_used(void) { return total_allocated; }

size_t kheap_get_free(void) {
  size_t free_size = 0;
  block_header_t *current = first_block;

  while (current) {
    if (current->is_free) {
      free_size += current->size;
    }
    current = current->next;
  }

  return free_size;
}
