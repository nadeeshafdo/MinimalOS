/* Physical Memory Manager for x86_64 with Limine */
#include <kernel/pmm.h>
#include <limine.h>
#include <stddef.h>
#include <stdint.h>

/* Memory constants */
#define BLOCKS_PER_BYTE 8
#define BLOCK_SIZE PAGE_SIZE

/* Bitmap for tracking free/used pages */
static uint64_t *memory_bitmap = 0;
static uint64_t total_blocks = 0;
static uint64_t used_blocks = 0;
static uint64_t bitmap_size = 0;
static uint64_t total_memory = 0;

/* HHDM offset for physical-to-virtual conversion */
extern uint64_t get_hhdm_offset(void);

/* Set a bit in the bitmap (mark block as used) */
static inline void bitmap_set(uint64_t bit) {
  memory_bitmap[bit / 64] |= (1ULL << (bit % 64));
}

/* Clear a bit in the bitmap (mark block as free) */
static inline void bitmap_clear(uint64_t bit) {
  memory_bitmap[bit / 64] &= ~(1ULL << (bit % 64));
}

/* Test a bit in the bitmap */
static inline uint64_t bitmap_test(uint64_t bit) {
  return memory_bitmap[bit / 64] & (1ULL << (bit % 64));
}

/* Find first free block */
static int64_t bitmap_first_free(void) {
  for (uint64_t i = 0; i < total_blocks / 64; i++) {
    if (memory_bitmap[i] != 0xFFFFFFFFFFFFFFFFULL) {
      for (uint64_t j = 0; j < 64; j++) {
        uint64_t bit = 1ULL << j;
        if (!(memory_bitmap[i] & bit)) {
          return i * 64 + j;
        }
      }
    }
  }
  return -1; /* No free blocks */
}

void pmm_init_limine(struct limine_memmap_response *memmap) {
  if (memmap == NULL) {
    return;
  }

  uint64_t hhdm = get_hhdm_offset();

  /* First pass: find total memory and largest usable region for bitmap */
  uint64_t highest_addr = 0;
  uint64_t best_region_base = 0;
  uint64_t best_region_size = 0;

  for (uint64_t i = 0; i < memmap->entry_count; i++) {
    struct limine_memmap_entry *entry = memmap->entries[i];
    uint64_t end = entry->base + entry->length;

    if (end > highest_addr) {
      highest_addr = end;
    }

    /* Find largest usable region for our bitmap */
    if (entry->type == LIMINE_MEMMAP_USABLE &&
        entry->length > best_region_size) {
      best_region_base = entry->base;
      best_region_size = entry->length;
    }
  }

  total_memory = highest_addr;
  total_blocks = total_memory / BLOCK_SIZE;
  bitmap_size = (total_blocks + 7) / 8; /* Bytes needed for bitmap */

  /* Round up to 64-bit alignment */
  if (bitmap_size % 8) {
    bitmap_size = (bitmap_size / 8 + 1) * 8;
  }

  /* Place bitmap at start of largest usable region */
  memory_bitmap = (uint64_t *)(best_region_base + hhdm);

  /* Mark all blocks as used initially */
  for (uint64_t i = 0; i < bitmap_size / 8; i++) {
    memory_bitmap[i] = 0xFFFFFFFFFFFFFFFFULL;
  }
  used_blocks = total_blocks;

  /* Mark usable regions as free */
  for (uint64_t i = 0; i < memmap->entry_count; i++) {
    struct limine_memmap_entry *entry = memmap->entries[i];

    if (entry->type == LIMINE_MEMMAP_USABLE) {
      pmm_mark_region_free(entry->base, entry->length);
    }
  }

  /* Mark the bitmap itself as used */
  pmm_mark_region_used(best_region_base, bitmap_size);
}

void *pmm_alloc_frame(void) {
  if (used_blocks >= total_blocks) {
    return 0;
  }

  int64_t frame = bitmap_first_free();
  if (frame == -1) {
    return 0;
  }

  bitmap_set(frame);
  used_blocks++;

  return (void *)(frame * BLOCK_SIZE);
}

void pmm_free_frame(void *frame) {
  uint64_t addr = (uint64_t)frame;
  uint64_t block = addr / BLOCK_SIZE;

  if (bitmap_test(block)) {
    bitmap_clear(block);
    used_blocks--;
  }
}

void pmm_mark_region_used(uint64_t base, size_t size) {
  uint64_t align = base / BLOCK_SIZE;
  uint64_t blocks = size / BLOCK_SIZE;

  if (size % BLOCK_SIZE) {
    blocks++;
  }

  for (uint64_t i = 0; i < blocks; i++) {
    if (!bitmap_test(align + i)) {
      bitmap_set(align + i);
      used_blocks++;
    }
  }
}

void pmm_mark_region_free(uint64_t base, size_t size) {
  uint64_t align = base / BLOCK_SIZE;
  uint64_t blocks = size / BLOCK_SIZE;

  if (size % BLOCK_SIZE) {
    blocks++;
  }

  for (uint64_t i = 0; i < blocks; i++) {
    if (bitmap_test(align + i)) {
      bitmap_clear(align + i);
      used_blocks--;
    }
  }
}

uint64_t pmm_get_total_memory(void) { return total_memory; }

uint64_t pmm_get_free_memory(void) {
  return (total_blocks - used_blocks) * BLOCK_SIZE;
}
