---
title: Memory Management
layout: default
parent: Subsystems
nav_order: 2
---

# Sprint 2 — Memory Management
{: .no_toc }

Teach the kernel to manage physical and virtual memory.
{: .fs-6 .fw-300 }

**Status**: ✅ Complete
{: .label .label-green }

<details open markdown="block">
  <summary>Table of contents</summary>
  {: .text-delta }
- TOC
{:toc}
</details>

---

## Overview

Sprint 2 builds the memory management subsystem in three layers:

1. **PMM (Physical Memory Manager)** — tracks which 4 KiB frames are free or used
2. **VMM (Virtual Memory Manager)** — manipulates x86_64 4-level page tables
3. **Kernel Heap** — enables `alloc` crate (`Vec`, `Box`, `String` in kernel code)

After Sprint 2, the kernel can dynamically allocate and free both physical frames and kernel heap memory.

---

## Physical Memory Manager (`memory/pmm.rs`)

### Design

A **bitmap allocator** where each bit represents one 4 KiB page frame:

```
Bit 0  → Frame at physical address 0x0000
Bit 1  → Frame at physical address 0x1000
Bit 2  → Frame at physical address 0x2000
...
Bit N  → Frame at physical address N × 0x1000
```

- **0** = free, **1** = used
- Bitmap is stored in the first usable physical memory region large enough to hold it
- Only tracks up to the highest usable address (avoids wasting memory on PCI MMIO holes at `0xFD00000000+`)

### Data Structure

```rust
pub struct BitmapAllocator {
    bitmap: *mut u64,          // Pointer to bitmap (via HHDM)
    bitmap_frames: usize,      // Frames used by bitmap itself
    total_frames: usize,       // Total tracked frames
    used_frames: usize,        // Currently allocated frames
    highest_frame: usize,      // Highest tracked frame index
    search_start: usize,       // Optimization: resume search from last alloc
}
```

Protected by a global `SpinLock<BitmapAllocator>` — `PMM`.

### Initialization

`pmm::init(memory_map)`:

1. **Scan phases** — walk the Limine memory map twice:
   - Phase 1: Find the highest usable address to determine bitmap size
   - Phase 2: Find a usable region large enough to store the bitmap
2. **Place bitmap** — store it in physical memory, accessed via HHDM
3. **Mark regions** — iterate the memory map, marking each frame as free or used based on region type
4. **Reserve kernel** — mark kernel binary, framebuffer, and bitmap frames as used

### API

| Function | Description |
|:---------|:------------|
| `pmm::init(memory_map)` | Initialize from Limine memory map |
| `pmm::alloc_frame() → Option<PhysAddr>` | Allocate one 4 KiB frame |
| `pmm::free_frame(addr)` | Return a frame (double-free detection) |
| `pmm::alloc_frame_zeroed() → Option<PhysAddr>` | Allocate and zero a frame (for page tables) |
| `pmm::alloc_contiguous(n) → Option<PhysAddr>` | Allocate N physically contiguous frames |
| `pmm::stats() → MemStats` | Get total/used/free frame counts |

### Optimizations

- **u64-at-a-time scanning** — checks 64 frames per comparison instead of one. Only falls back to bit-level scan when a u64 word has a free bit.
- **Search cursor** — `search_start` remembers where the last allocation came from, avoiding repeated scans of fully-allocated regions.
- **Optimized range operations** — `clear_range()` uses full-word zeroing for bulk free operations.

### Statistics (QEMU, 512 MB)

```
[pmm] 525288 total frames, 127737 free (498 MiB free)
[pmm] Bitmap: 65 KiB at physical address 0x100000
```

---

## Virtual Memory Manager (`memory/vmm.rs`)

### Design

Implements x86_64 **4-level page table** infrastructure for creating and managing virtual-to-physical address mappings.

```
Virtual Address (48-bit):
┌───────┬───────┬───────┬───────┬────────────┐
│ PML4  │ PDP   │  PD   │  PT   │   Offset   │
│ [47:39]│[38:30]│[29:21]│[20:12]│  [11:0]    │
│ 9 bits│ 9 bits│ 9 bits│ 9 bits│  12 bits   │
└───────┴───────┴───────┴───────┴────────────┘
   │        │        │        │
   ▼        ▼        ▼        ▼
PML4 ──→ PDP ──→ PD ──→ PT ──→ Physical Frame
Table    Table   Table   Table
```

Each table has 512 entries, each entry is 8 bytes. Tables are 4 KiB (one page frame).

### Page Table Flags

```rust
bitflags! {
    pub struct PageTableFlags: u64 {
        const PRESENT    = 1 << 0;   // Page is mapped
        const WRITABLE   = 1 << 1;   // Read+Write (vs Read-only)
        const USER       = 1 << 2;   // Accessible from Ring 3
        const PWT        = 1 << 3;   // Page-level write-through
        const PCD        = 1 << 4;   // Page-level cache disable
        const ACCESSED   = 1 << 5;   // Has been read
        const DIRTY      = 1 << 6;   // Has been written
        const HUGE       = 1 << 7;   // 2 MiB page (in PD entries)
        const GLOBAL     = 1 << 8;   // Don't flush on CR3 switch
        const NO_EXECUTE = 1 << 63;  // XD: cannot execute code
    }
}
```

### Preset Flag Combinations

| Name | Flags | Use |
|:-----|:------|:----|
| `KERNEL_CODE` | Present + Global + (no NX) | Kernel `.text` |
| `KERNEL_RODATA` | Present + Global + NX | Kernel `.rodata` |
| `KERNEL_DATA` | Present + Writable + Global + NX | Kernel `.data`, `.bss` |
| `USER_CODE` | Present + User + (no NX) | Userspace `.text` |
| `USER_DATA` | Present + Writable + User + NX | Userspace data |
| `INTERMEDIATE` | Present + Writable + User | Intermediate table entries |

### API

| Function | Description |
|:---------|:------------|
| `map_page(pml4, virt, phys, flags) → Result<(), MapError>` | Map a 4 KiB page |
| `unmap_page(pml4, virt) → Result<PhysAddr, UnmapError>` | Unmap a page, return its physical address |
| `translate(pml4, virt) → Option<PhysAddr>` | Walk page tables, resolve virtual → physical |
| `new_table() → *mut PageTable` | Allocate a zeroed page table from PMM |
| `flush(virt)` | Invalidate TLB for a single page (`invlpg`) |
| `flush_all()` | Flush entire TLB (reload CR3) |

### Error Types

```rust
pub enum MapError {
    AlreadyMapped,      // Target virtual address already has a mapping
    AllocationFailed,   // PMM couldn't allocate a page table frame
}

pub enum UnmapError {
    NotMapped,          // Target virtual address is not mapped
    HugePage,           // 2 MiB page — cannot unmap as 4 KiB
}
```

### Current Limitations

{: .warning }
> **CR3 switch deferred to Sprint 3.** The VMM infrastructure is complete, but the kernel still uses Limine's page tables. Switching to our own tables requires IDT/exception handlers for debugging page faults. This is the first task in Sprint 3.

---

## Kernel Heap (`memory/heap.rs`)

### Design

A **linked-list free-list allocator** that enables the Rust `alloc` crate in the kernel. After initialization, `Vec`, `Box`, `String`, `Arc`, and other heap types work normally.

### Free Block Structure

```
┌──────────────────────┐
│  FreeBlock Header    │
│  ┌────────────────┐  │
│  │ size: usize    │  │  ← Total block size (header + payload)
│  │ next: *mut     │  │  ← Pointer to next free block (or null)
│  └────────────────┘  │
│  ... free space ...  │
└──────────────────────┘
```

Free blocks are maintained in a **sorted linked list** (sorted by address), enabling efficient coalescing.

### Allocation (First-Fit)

1. Walk the free list, find the first block large enough for the requested size + alignment
2. If the block is much larger than needed, split it — allocate the front, return the rest to the free list
3. If the block is a close fit, allocate the entire block (avoids fragmentation from tiny remnants)
4. Return a pointer past the header to the caller

### Deallocation (with Coalescing)

1. Find the correct position in the sorted free list (by address)
2. Insert the freed block
3. **Coalesce with successor**: if the freed block is adjacent to the next free block, merge them
4. **Coalesce with predecessor**: if the previous free block is adjacent to the freed block, merge them

Coalescing prevents fragmentation — when allocations are freed in any order, adjacent free blocks merge back into large contiguous regions.

### Initialization

`heap::init()`:

1. Allocates 64 contiguous physical frames from the PMM (256 KiB)
2. Maps them into virtual memory via the HHDM
3. Creates a single large free block covering the entire heap
4. Registers as `#[global_allocator]`

### API

| Function | Description |
|:---------|:------------|
| `heap::init()` | Initialize the kernel heap (256 KiB) |
| `heap::allocated_bytes() → usize` | Current heap usage |
| `heap::total_bytes() → usize` | Total heap capacity |

The allocator implements `GlobalAlloc` (`alloc` and `dealloc`), so standard Rust allocation works transparently.

### Verification

Tested during boot — Vec allocation/deallocation with coalescing verification:

```
[heap] Test allocation OK: [42, 1337, 3735928559] (heap used: 32 bytes)
[heap] After drop: 0 bytes used / 256 KiB total
```

Zero bytes after drop confirms coalescing works correctly.

---

## Linker Script Changes

Sprint 2 required a linker script fix: the compiler generated a `.got` (Global Offset Table) section that created a non-page-aligned LOAD segment, breaking Limine's ELF loader.

**Fix**: Merge `.got` and `.got.*` into the `.rodata` section:

```ld
.rodata ALIGN(4K) : {
    _rodata_start = .;
    *(.rodata .rodata.*)
    *(.got .got.*)          /* GOT merged here */
    _rodata_end = .;
}
```

---

## Memory Subsystem Interaction

```
                ┌──────────────┐
                │  alloc crate │  Vec, Box, String, etc.
                │  (Rust std)  │
                └──────┬───────┘
                       │ GlobalAlloc::alloc / dealloc
                ┌──────▼───────┐
                │  Kernel Heap │  Linked-list free-list
                │  (heap.rs)   │  with coalescing
                └──────┬───────┘
                       │ pmm::alloc_contiguous(64)
                ┌──────▼───────┐
                │     PMM      │  Bitmap frame allocator
                │  (pmm.rs)    │  Tracks 4 KiB frames
                └──────┬───────┘
                       │ HHDM translation
                ┌──────▼───────┐
                │   Physical   │  RAM on the machine
                │   Memory     │
                └──────────────┘
```

The VMM (`vmm.rs`) sits alongside this stack, providing page table manipulation that will be used when the kernel creates its own page tables in Sprint 3.
