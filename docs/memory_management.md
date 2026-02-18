---
layout: default
title: Memory Management
---

# Memory Management

## Overview

MinimalOS implements a complete memory management subsystem with three layers:

1. **Physical Memory Manager (PMM)** — bitmap-based frame allocator
2. **Virtual Memory Manager (VMM)** — 4-level x86_64 page tables
3. **Kernel Heap** — linked-list allocator implementing `GlobalAlloc`

All three are fully operational and support the `alloc` crate (`Box`, `Vec`,
`VecDeque`, `String`, etc.) inside the kernel.

## Higher-Half Direct Map (HHDM)

The Limine bootloader creates a Higher-Half Direct Map that identity-maps all
physical memory into the upper half of the virtual address space:

```
virtual = HHDM_BASE + physical
```

The HHDM base is `0xFFFF_8000_0000_0000` (obtained from `HhdmRequest`). This
allows the kernel to access any physical address without setting up additional
page tables, which is essential for bootstrapping the memory subsystem itself.

## Physical Memory Manager

**File:** `kernel/src/memory/pmm.rs`

### Design

The PMM uses a bitmap allocator where each bit represents one 4 KiB physical
frame:

- **Bit = 0**: frame is free
- **Bit = 1**: frame is allocated

### Initialisation

During boot, `pmm::init()`:

1. Receives the HHDM offset and Limine memory map entries.
2. Places the bitmap at a known physical address (`0x53000`), inside a usable
   memory region that does not overlap the kernel image.
3. Marks all frames as **used** by default.
4. Iterates the memory map and marks `Usable` regions as **free**.
5. Marks the bitmap's own pages as **used** so they cannot be allocated.

### Interface

```rust
/// Allocate a single 4 KiB physical frame.
pub fn alloc_frame() -> Option<u64>

/// Free a previously allocated frame.
pub fn free_frame(phys: u64)

/// Number of free frames remaining.
pub fn free_frame_count() -> usize
```

The allocator scans the bitmap linearly for the first free bit. This is O(n) in
the worst case but simple and correct.

### Memory Census

Before PMM initialisation, `memory::census()` walks the Limine memory map to
calculate total and usable RAM. This information is displayed on the framebuffer
console during boot.

## Virtual Memory Manager (Paging)

**File:** `kernel/src/memory/paging.rs`

### Design

MinimalOS uses the standard x86_64 4-level page table hierarchy:

```
CR3 → PML4 → PDPT → PD → PT → Physical Frame
```

Each level contains 512 entries (9 bits of the virtual address each), with the
remaining 12 bits forming the page offset.

### Initialisation

`paging::init()` reads the current CR3 value (set up by Limine) and stores the
HHDM offset for translating physical table addresses to virtual pointers.

### Interface

```rust
/// Map a 4 KiB virtual page to a physical frame with given flags.
pub fn map_page(virt: u64, phys: u64, flags: PageFlags)

/// Translate a virtual address to its physical address.
pub fn translate(virt: u64) -> Option<u64>
```

### Page Flags

```rust
pub struct PageFlags(u64);

impl PageFlags {
    pub const KERNEL_RW: Self;   // Present + Writable
    pub const USER_RW: Self;     // Present + Writable + User
    pub const USER_RX: Self;     // Present + User (no write)
}
```

### Page Table Walker

The `translate()` function walks all four levels of the page table to resolve a
virtual address:

1. Extract the PML4 index (bits 39–47), PDPT index (bits 30–38), PD index
   (bits 21–29), and PT index (bits 12–20).
2. At each level, check the Present bit. Return `None` if not set.
3. Handle 2 MiB huge pages (PSE bit) at the PD level.
4. At the PT level, combine the frame address with the 12-bit offset.

### APIC MMIO Mapping

The APIC's MMIO registers live at a physical address read from the IA32_APIC_BASE
MSR. The kernel maps this page into the HHDM using special flags:

```rust
pub fn map_apic_mmio(hhdm_offset: u64, apic_phys: u64)
```

The mapping uses **Page Cache Disable (PCD)** and **Write-Through (PWT)** flags
to ensure MMIO accesses are not cached by the CPU.

## Kernel Heap

**File:** `kernel/src/memory/heap.rs`

### Design

The kernel heap is a linked-list free-list allocator that implements Rust's
`GlobalAlloc` trait, enabling the `alloc` crate.

### Configuration

| Parameter | Value |
|-----------|-------|
| Base address | `0xFFFF_A000_0000_0000` |
| Initial size | 64 KiB |
| Maximum size | 16 MiB |
| Growth unit | 64 KiB per expansion |

### Initialisation

`heap::init()`:

1. Allocates physical frames via the PMM for the initial 64 KiB region.
2. Maps them into the heap virtual address range.
3. Initialises the free list with a single free block.
4. Registers the global allocator.

### Growth

When an allocation fails due to insufficient free space, the allocator
transparently expands the heap:

1. Allocates additional physical frames from the PMM.
2. Maps them contiguously after the current heap end.
3. Adds the new region to the free list.
4. Retries the allocation.

This continues until the 16 MiB ceiling is reached.

### Usage

Once the heap is initialised, standard Rust collections work:

```rust
extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;

let b = Box::new(42u64);        // Heap-allocated integer
let mut v = Vec::new();          // Dynamically-growing vector
v.push(1);
```

## Memory Map (Example)

A typical QEMU run with 2 GiB RAM shows:

```
Limine Memory Map:
  Usable:          0x00000000 - 0x0009FFFF  (640 KiB)
  Usable:          0x00100000 - 0x7FFDFFFF  (~2 GiB)
  Reserved/ACPI:   (various)
  Framebuffer:     (mapped by bootloader)
  Kernel:          0xFFFFFFFF80000000+

Total RAM:  ~2048 MiB
Usable RAM: ~2047 MiB
```
