// =============================================================================
// MinimalOS NextGen — Memory Subsystem
// =============================================================================
//
// The memory subsystem manages all physical and virtual memory in the kernel.
// It's organized into layers:
//
//   address.rs  — PhysAddr/VirtAddr newtypes (type safety for addresses)
//   frame.rs    — PhysFrame abstraction (a 4KB-aligned physical page frame)
//   pmm.rs      — Physical Memory Manager (bitmap allocator for frames)
//   vmm.rs      — Virtual Memory Manager (page table operations)
//   heap.rs     — Kernel heap allocator (Box, Vec, etc.)
//
// This module only exposes what's needed. Internal details stay private.
// =============================================================================

pub mod address;

