# Memory Management

## Current Status

The memory management subsystem (`kernel/src/memory/mod.rs`) is **stubbed out** and
awaiting implementation. This document describes the planned architecture and the
infrastructure already in place.

## Higher-Half Direct Map (HHDM)

The Limine bootloader establishes a Higher-Half Direct Map, which identity-maps all
physical memory into the upper half of the virtual address space. The kernel is linked
at `0xFFFFFFFF80000000` (see `build/linker.ld`).

This means any physical address `phys` can be accessed by the kernel at
`HHDM_BASE + phys` without setting up additional page tables.

## Planned Components

### Physical Memory Manager (PMM)

A bitmap-based physical frame allocator is planned:

- Parse the Limine memory map to discover usable RAM regions.
- Maintain a bitmap where each bit represents a 4 KiB physical frame.
- Provide `alloc_frame()` and `free_frame()` interfaces.
- Track total, used, and free frame counts.

### Virtual Memory Manager (VMM)

Once the PMM is in place, a virtual memory manager will:

- Create and manipulate 4-level x86_64 page tables.
- Map/unmap virtual pages to physical frames.
- Support kernel-space and (future) user-space address spaces.

### Kernel Heap

A kernel heap allocator will be built on top of PMM + VMM:

- Implement the `GlobalAlloc` trait so `alloc::` collections work in the kernel.
- Likely a simple linked-list or bump allocator initially.

## Available Crates

The `x86_64` crate (v0.15) already provides page table structures, frame types, and
address types that the implementation will leverage.

## Quest Tracking

Related quests from `QUESTS.md`:

- **[009]** Physical Memory Manager (PMM)
- **[010]** Virtual Memory Manager (VMM)
