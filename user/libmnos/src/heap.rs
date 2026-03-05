// =============================================================================
// libmnos — Ring 3 Heap Bootstrap
// =============================================================================
//
// Bootstraps a dynamic heap for Ring 3 processes by allocating physical frames
// from the kernel (via the PmmAllocator capability) and mapping them into the
// caller's own address space (via the Process(self) capability).
//
// After mapping, the pages are fed to the `linked_list_allocator` LockedHeap
// which provides the standard Rust `#[global_allocator]` interface.
//
// CAPABILITY REQUIREMENTS:
//   - `alloc_slot` must hold a PmmAllocator capability (typically Slot 1)
//   - `proc_slot`  must hold a Process(self) capability (typically Slot 3)
//   - `scratch_slot` is a temporary CNode slot for each allocated frame
//
// =============================================================================

use crate::process::{sys_alloc_memory, sys_drop_cap, sys_map_memory};
use crate::HEAP;

const PAGE_SIZE: u64 = 4096;

/// Bootstraps the Ring 3 heap by allocating `pages` physical frames and
/// mapping them contiguously starting at `heap_base`.
///
/// # Arguments
/// - `heap_base`:    Virtual address where the heap starts (must be page-aligned).
/// - `pages`:        Number of 4 KiB pages to allocate.
/// - `alloc_slot`:   CNode slot holding the PmmAllocator capability.
/// - `proc_slot`:    CNode slot holding the Process(self) capability.
/// - `scratch_slot`: CNode slot used temporarily for each MemoryFrame.
///
/// # Panics
/// Panics if any syscall fails (allocation or mapping).
pub fn init_heap(
    heap_base: u64,
    pages: u64,
    alloc_slot: u64,
    proc_slot: u64,
    scratch_slot: u64,
) {
    for i in 0..pages {
        // 1. Allocate a zeroed physical frame into scratch_slot
        match sys_alloc_memory(alloc_slot, scratch_slot) {
            Ok(()) => {}
            Err(e) => panic!("heap: alloc_memory failed on page {}: err={}", i, e.0),
        }

        // 2. Map it at heap_base + i * PAGE_SIZE (WRITABLE, no-exec)
        let vaddr = heap_base + i * PAGE_SIZE;
        // flags: bit 0 = WRITABLE
        match sys_map_memory(proc_slot, scratch_slot, vaddr, 0x01) {
            Ok(()) => {}
            Err(e) => panic!("heap: map_memory failed on page {} @ {:#x}: err={}", i, vaddr, e.0),
        }

        // 3. Drop the MemoryFrame cap so the scratch slot is free for reuse
        let _ = sys_drop_cap(scratch_slot);

        // Next iteration re-uses the same slot number for the next frame.
    }

    // 4. Hand the entire region to the linked-list allocator
    let heap_size = (pages * PAGE_SIZE) as usize;
    unsafe {
        HEAP.lock().init(heap_base as *mut u8, heap_size);
    }
}
