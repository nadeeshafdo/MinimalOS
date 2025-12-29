/**
 * MinimalOS - Kernel Heap Allocator Implementation
 * First-fit allocator with block coalescing
 * Uses memory already mapped by boot page tables (within first 1GB)
 */

#include "heap.h"
#include "pmm.h"

extern void printk(const char *fmt, ...);

/* Block header structure */
struct heap_block {
  size_t size;             /* Size of data area (not including header) */
  bool used;               /* Is this block allocated? */
  struct heap_block *next; /* Next block in list */
  struct heap_block *prev; /* Previous block in list */
  uint32_t magic;          /* Magic number for validation */
};

#define HEAP_MAGIC 0xDEADBEEF
#define BLOCK_OVERHEAD sizeof(struct heap_block)

/* Heap state */
static struct heap_block *heap_start = NULL;
static struct heap_block *heap_end = NULL;
static size_t heap_total = 0;
static size_t heap_used = 0;

/*
 * Heap virtual address - use a fixed region within the already-mapped
 * higher-half kernel space. The boot page tables map the first 1GB at
 * KERNEL_VMA + 0 to KERNEL_VMA + 1GB.
 *
 * We'll use memory starting at KERNEL_VMA + 16MB (0xFFFFFFFF81000000)
 * which should be well past the kernel and safe to use.
 */
#define HEAP_VIRT_START (KERNEL_VMA + (16 * MB))
#define HEAP_PHYS_START (16 * MB)

/**
 * Initialize the kernel heap
 * Uses a fixed region of already-mapped memory
 */
void heap_init(void) {
  /* Mark the heap physical frames as used in PMM */
  size_t heap_pages = HEAP_INITIAL_SIZE / PAGE_SIZE;

  for (size_t i = 0; i < heap_pages; i++) {
    uint64_t phys = HEAP_PHYS_START + (i * PAGE_SIZE);
    pmm_mark_used(phys);
  }

  heap_total = HEAP_INITIAL_SIZE;

  /* Initialize first free block */
  heap_start = (struct heap_block *)HEAP_VIRT_START;
  heap_start->size = heap_total - BLOCK_OVERHEAD;
  heap_start->used = false;
  heap_start->next = NULL;
  heap_start->prev = NULL;
  heap_start->magic = HEAP_MAGIC;
  heap_end = heap_start;

  printk("  Heap at 0x%lx (phys 0x%lx), size %lu KB\n", HEAP_VIRT_START,
         HEAP_PHYS_START, heap_total / KB);
}

/**
 * Find a free block that fits the requested size
 */
static struct heap_block *find_free_block(size_t size) {
  struct heap_block *block = heap_start;

  while (block) {
    if (!block->used && block->size >= size) {
      return block;
    }
    block = block->next;
  }

  return NULL;
}

/**
 * Split a block if it's larger than needed
 */
static void split_block(struct heap_block *block, size_t size) {
  /* Only split if remaining space is large enough for a new block */
  if (block->size >= size + BLOCK_OVERHEAD + HEAP_MIN_BLOCK) {
    struct heap_block *new_block =
        (struct heap_block *)((uint8_t *)block + BLOCK_OVERHEAD + size);

    new_block->size = block->size - size - BLOCK_OVERHEAD;
    new_block->used = false;
    new_block->next = block->next;
    new_block->prev = block;
    new_block->magic = HEAP_MAGIC;

    if (block->next) {
      block->next->prev = new_block;
    }

    block->next = new_block;
    block->size = size;

    if (block == heap_end) {
      heap_end = new_block;
    }
  }
}

/**
 * Coalesce free blocks
 */
static void coalesce_blocks(struct heap_block *block) {
  /* Merge with next block if free */
  if (block->next && !block->next->used) {
    block->size += BLOCK_OVERHEAD + block->next->size;
    block->next = block->next->next;
    if (block->next) {
      block->next->prev = block;
    } else {
      heap_end = block;
    }
  }

  /* Merge with previous block if free */
  if (block->prev && !block->prev->used) {
    block->prev->size += BLOCK_OVERHEAD + block->size;
    block->prev->next = block->next;
    if (block->next) {
      block->next->prev = block->prev;
    } else {
      heap_end = block->prev;
    }
  }
}

/**
 * Allocate memory from the kernel heap
 */
void *kmalloc(size_t size) {
  if (size == 0 || !heap_start)
    return NULL;

  /* Align size */
  size = ALIGN_UP(size, HEAP_BLOCK_ALIGN);
  if (size < HEAP_MIN_BLOCK)
    size = HEAP_MIN_BLOCK;

  /* Find a free block */
  struct heap_block *block = find_free_block(size);

  if (!block) {
    printk("HEAP: Out of memory (requested %lu bytes)\n", size);
    return NULL;
  }

  /* Split block if too large */
  split_block(block, size);

  block->used = true;
  heap_used += block->size;

  return (void *)((uint8_t *)block + BLOCK_OVERHEAD);
}

/**
 * Allocate zeroed memory
 */
void *kzalloc(size_t size) {
  void *ptr = kmalloc(size);
  if (ptr) {
    uint8_t *p = (uint8_t *)ptr;
    for (size_t i = 0; i < size; i++) {
      p[i] = 0;
    }
  }
  return ptr;
}

/**
 * Allocate aligned memory
 */
void *kmalloc_aligned(size_t size, size_t align) {
  /* For simplicity, allocate extra space and align within */
  void *ptr = kmalloc(size + align);
  if (!ptr)
    return NULL;

  uintptr_t addr = (uintptr_t)ptr;
  uintptr_t aligned = ALIGN_UP(addr, align);

  return (void *)aligned;
}

/**
 * Free memory
 */
void kfree(void *ptr) {
  if (!ptr)
    return;

  struct heap_block *block =
      (struct heap_block *)((uint8_t *)ptr - BLOCK_OVERHEAD);

  /* Validate block */
  if (block->magic != HEAP_MAGIC) {
    printk("HEAP: Invalid free at %p (bad magic)\n", ptr);
    return;
  }

  if (!block->used) {
    printk("HEAP: Double free at %p\n", ptr);
    return;
  }

  heap_used -= block->size;
  block->used = false;

  /* Coalesce with neighbors */
  coalesce_blocks(block);
}

/**
 * Get heap statistics
 */
void heap_stats(size_t *total_out, size_t *used_out, size_t *free_out) {
  if (total_out)
    *total_out = heap_total;
  if (used_out)
    *used_out = heap_used;
  if (free_out)
    *free_out = heap_total - heap_used - BLOCK_OVERHEAD;
}
