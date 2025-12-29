/**
 * MinimalOS - Physical Memory Manager (Frame Allocator)
 * Bitmap-based tracking of physical page frames
 */

#include "pmm.h"
#include <minimalos/multiboot2.h>

extern void printk(const char *fmt, ...);

/* External symbols from linker script */
extern uint8_t _kernel_start_phys[];
extern uint8_t _kernel_end_phys[];

/* Bitmap for tracking page frames (1 bit per 4KB page) */
/* Support up to 4GB of RAM initially (1M pages = 128KB bitmap) */
#define MAX_MEMORY (4UL * 1024 * 1024 * 1024)
#define MAX_FRAMES (MAX_MEMORY / PAGE_SIZE)
#define BITMAP_SIZE (MAX_FRAMES / 8)

static uint8_t frame_bitmap[BITMAP_SIZE] __aligned(PAGE_SIZE);

/* Statistics */
static size_t total_frames = 0;
static size_t used_frames = 0;

/**
 * Set a bit in the bitmap (mark frame as used)
 */
static inline void bitmap_set(size_t frame) {
  if (frame < MAX_FRAMES) {
    frame_bitmap[frame / 8] |= (1 << (frame % 8));
  }
}

/**
 * Clear a bit in the bitmap (mark frame as free)
 */
static inline void bitmap_clear(size_t frame) {
  if (frame < MAX_FRAMES) {
    frame_bitmap[frame / 8] &= ~(1 << (frame % 8));
  }
}

/**
 * Test a bit in the bitmap
 */
static inline bool bitmap_test(size_t frame) {
  if (frame >= MAX_FRAMES)
    return true; /* Out of range = used */
  return (frame_bitmap[frame / 8] & (1 << (frame % 8))) != 0;
}

/**
 * Find first free frame
 */
static size_t find_free_frame(void) {
  for (size_t i = 0; i < BITMAP_SIZE; i++) {
    if (frame_bitmap[i] != 0xFF) {
      /* At least one free bit in this byte */
      for (int bit = 0; bit < 8; bit++) {
        if (!(frame_bitmap[i] & (1 << bit))) {
          return i * 8 + bit;
        }
      }
    }
  }
  return (size_t)-1; /* No free frames */
}

/**
 * Find contiguous free frames
 */
static size_t find_free_frames(size_t count) {
  size_t consecutive = 0;
  size_t start_frame = 0;

  for (size_t frame = 0; frame < MAX_FRAMES; frame++) {
    if (bitmap_test(frame)) {
      consecutive = 0;
      start_frame = frame + 1;
    } else {
      consecutive++;
      if (consecutive >= count) {
        return start_frame;
      }
    }
  }
  return (size_t)-1;
}

/**
 * Mark a single frame as used
 */
void pmm_mark_used(uint64_t addr) {
  size_t frame = ADDR_TO_PFN(addr);
  if (!bitmap_test(frame)) {
    bitmap_set(frame);
    used_frames++;
  }
}

/**
 * Mark a range of memory as used
 */
void pmm_mark_range_used(uint64_t start, uint64_t end) {
  start = PAGE_ALIGN_DOWN(start);
  end = PAGE_ALIGN_UP(end);

  for (uint64_t addr = start; addr < end; addr += PAGE_SIZE) {
    pmm_mark_used(addr);
  }
}

/**
 * Mark a range of memory as free
 */
void pmm_mark_range_free(uint64_t start, uint64_t end) {
  start = PAGE_ALIGN_UP(start);
  end = PAGE_ALIGN_DOWN(end);

  for (uint64_t addr = start; addr < end; addr += PAGE_SIZE) {
    size_t frame = ADDR_TO_PFN(addr);
    if (bitmap_test(frame)) {
      bitmap_clear(frame);
      if (used_frames > 0)
        used_frames--;
    }
  }
}

/**
 * Allocate a single physical page frame
 * @return Physical address of allocated frame, or NULL if no memory
 */
void *pmm_alloc_frame(void) {
  size_t frame = find_free_frame();
  if (frame == (size_t)-1) {
    printk("PMM: Out of memory!\n");
    return NULL;
  }

  bitmap_set(frame);
  used_frames++;

  return (void *)PFN_TO_ADDR(frame);
}

/**
 * Allocate contiguous physical page frames
 */
void *pmm_alloc_frames(size_t count) {
  if (count == 0)
    return NULL;
  if (count == 1)
    return pmm_alloc_frame();

  size_t start = find_free_frames(count);
  if (start == (size_t)-1) {
    printk("PMM: Cannot allocate %lu contiguous frames\n", count);
    return NULL;
  }

  for (size_t i = 0; i < count; i++) {
    bitmap_set(start + i);
  }
  used_frames += count;

  return (void *)PFN_TO_ADDR(start);
}

/**
 * Free a single physical page frame
 */
void pmm_free_frame(void *addr) {
  size_t frame = ADDR_TO_PFN((uint64_t)addr);

  if (frame >= MAX_FRAMES) {
    printk("PMM: Invalid free address 0x%p\n", addr);
    return;
  }

  if (!bitmap_test(frame)) {
    printk("PMM: Double free at 0x%p\n", addr);
    return;
  }

  bitmap_clear(frame);
  if (used_frames > 0)
    used_frames--;
}

/**
 * Free contiguous physical page frames
 */
void pmm_free_frames(void *addr, size_t count) {
  uint64_t base = (uint64_t)addr;
  for (size_t i = 0; i < count; i++) {
    pmm_free_frame((void *)(base + i * PAGE_SIZE));
  }
}

/**
 * Get number of free frames
 */
size_t pmm_get_free_frames(void) { return total_frames - used_frames; }

/**
 * Get total number of frames
 */
size_t pmm_get_total_frames(void) { return total_frames; }

/**
 * Initialize physical memory manager
 */
void pmm_init(void) {
  struct multiboot2_tag_mmap *mmap = multiboot2_get_mmap();

  if (!mmap) {
    printk("  ERROR: No memory map from bootloader!\n");
    return;
  }

  /* Start with all memory marked as used */
  for (size_t i = 0; i < BITMAP_SIZE; i++) {
    frame_bitmap[i] = 0xFF;
  }
  used_frames = MAX_FRAMES;
  total_frames = 0;

  /* Parse memory map and mark available regions as free */
  struct multiboot2_mmap_entry *entry = mmap->entries;
  uintptr_t entries_end = (uintptr_t)mmap + mmap->size;

  while ((uintptr_t)entry < entries_end) {
    if (entry->type == MULTIBOOT2_MEMORY_AVAILABLE) {
      uint64_t start = entry->addr;
      uint64_t end = start + entry->len;

      /* Limit to our supported range */
      if (end > MAX_MEMORY) {
        end = MAX_MEMORY;
      }

      if (start < MAX_MEMORY) {
        /* Skip first 1MB (BIOS, real mode, etc.) */
        if (start < 0x100000) {
          start = 0x100000;
        }

        if (start < end) {
          size_t region_frames = (end - start) / PAGE_SIZE;
          total_frames += region_frames;

          /* Mark as free */
          pmm_mark_range_free(start, end);
        }
      }
    }

    entry =
        (struct multiboot2_mmap_entry *)((uintptr_t)entry + mmap->entry_size);
  }

  /* Mark kernel as used */
  uint64_t kernel_start = (uint64_t)_kernel_start_phys;
  uint64_t kernel_end = (uint64_t)_kernel_end_phys;
  pmm_mark_range_used(kernel_start, kernel_end);

  /* Mark bitmap itself as used (it's in BSS which is in kernel range, but be
   * safe) */
  pmm_mark_range_used((uint64_t)frame_bitmap - KERNEL_VMA,
                      (uint64_t)frame_bitmap - KERNEL_VMA + BITMAP_SIZE);

  size_t free_frames = pmm_get_free_frames();
  size_t free_mb = (free_frames * PAGE_SIZE) / MB;

  printk("  Total memory: %lu frames (%lu MB)\n", total_frames,
         (total_frames * PAGE_SIZE) / MB);
  printk("  Free memory: %lu frames (%lu MB)\n", free_frames, free_mb);
  printk("  Kernel: 0x%lx - 0x%lx\n", kernel_start, kernel_end);
}
