# Memory Management

## Overview
MinimalOS employs a straightforward memory management strategy, leveraging the groundwork laid by the Limine bootloader. It features a physical memory manager (bitmap-based), a kernel heap (linked-list based), and relies on Limine for initial paging setup.

## Physical Memory Manager (PMM)
The PMM is responsible for tracking free and used physical memory frames (4KB blocks). It is implemented in `kernel/mm/pmm.c`.

### Implementation Details
- **Algorithm**: Bitmap Allocator.
- **Bitmap Location**: Placed at the beginning of the largest usable memory region found in the memory map.
- **HHDM Usage**: The bitmap is accessed via the Higher Half Direct Map (HHDM) to avoid identity mapping issues.
- **Allocation**: `pmm_alloc_frame()` scans the bitmap for the first free bit (0), marks it as used (1), and returns the physical address.
- **Deallocation**: `pmm_free_frame()` calculates the bit index from the physical address and clears it.

## Virtual Memory (Paging)
The kernel currently relies on the 4-level paging structure set up by the Limine bootloader.
- **Kernel Space**: Mapped in the higher half (e.g., `0xFFFFFFFF80000000`).
- **Physical Memory**: Identity mapped starting at the HHDM offset (provided by Limine).
- **Page Faults**: A handler is registered to catch page faults (ISR 14), dumping the faulting address and register state before halting the system.

## Kernel Heap
The kernel heap provides dynamic memory allocation (`kmalloc`, `kfree`) for the kernel. It is implemented in `kernel/mm/kheap.c`.

### Implementation Details
- **Placement**: The heap is initialized at `HHDM_OFFSET + 16MB`. This ensures it resides in a known free region of physical memory, accessed comfortably via the higher half.
- **Structure**: A doubly linked list of block headers.
  ```c
  typedef struct block_header {
    size_t size;               /* Size of block (including header) */
    uint8_t is_free;           /* Is this block free? */
    struct block_header *next; /* Next block in list */
    struct block_header *prev; /* Previous block in list */
  } block_header_t;
  ```
- **Allocation Strategy**: First-fit. It iterates through the list to find the first free block that is large enough. If the block is significantly larger than requested, it is split into two.
- **Deallocation**: Marks the block as free and attempts to merge it with adjacent free blocks (coalescing) to prevent fragmentation.
