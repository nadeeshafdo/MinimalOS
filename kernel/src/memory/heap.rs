// =============================================================================
// MinimalOS NextGen — Kernel Heap Allocator
// =============================================================================
//
// This module provides dynamic memory allocation for the kernel, enabling
// use of Rust's `alloc` crate (Box, Vec, String, Arc, etc.).
//
// DESIGN: Linked-list free-list allocator
// ========================================
//
// The heap is a contiguous region of virtual memory (HHDM-mapped physical
// pages allocated from the PMM). Within this region, a linked list of
// free blocks tracks available memory:
//
//   ┌──────────┐     ┌──────────────┐     ┌───────────┐
//   │ FreeBlock│ ──→ │  FreeBlock   │ ──→ │ FreeBlock  │ ──→ null
//   │ size: 64 │     │ size: 4096   │     │ size: 128  │
//   └──────────┘     └──────────────┘     └───────────┘
//
// The list is kept sorted by address so that adjacent free blocks can be
// coalesced (merged) when memory is freed, reducing fragmentation.
//
// ALLOCATION ALGORITHM (first-fit):
//   1. Walk the free list looking for a block large enough.
//   2. Handle alignment: compute padding so the returned pointer is aligned.
//   3. Split: if the block is larger than needed, create a new free block
//      from the excess and insert it back into the list.
//   4. Return the aligned pointer.
//
// DEALLOCATION ALGORITHM:
//   1. Insert the freed region back into the free list (sorted by address).
//   2. Coalesce with predecessor and successor if they are adjacent.
//
// HEAP SIZING:
//   Initial heap: 256 KiB (64 contiguous physical pages via PMM).
//   This is enough for kernel data structures during early boot.
//   Heap growth (allocating more pages from PMM) can be added later
//   if needed, but 256 KiB handles thousands of small allocations.
//
// THREAD SAFETY:
//   The allocator is wrapped in a SpinLock. `GlobalAlloc::alloc/dealloc`
//   acquire the lock before accessing the free list.
//
// WHY NOT A SLAB/BUDDY ALLOCATOR?
//   Simplicity. A linked-list allocator is easy to audit and debug.
//   For a microkernel where most work happens in userspace, the kernel
//   heap sees modest use (capability tables, IPC buffers, page table
//   caches). Advanced allocators are overkill.
//
// =============================================================================

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

use crate::kprintln;
use crate::memory::address::PAGE_SIZE;
use crate::memory::pmm;
use crate::sync::spinlock::SpinLock;

// =============================================================================
// Configuration
// =============================================================================

/// Number of physical pages to allocate for the initial kernel heap.
/// 64 pages × 4 KiB = 256 KiB.
const INITIAL_HEAP_PAGES: usize = 64;

/// Minimum block size: must be at least `size_of::<FreeBlock>()` so that
/// every free region can hold the linked-list node header.
const MIN_BLOCK_SIZE: usize = core::mem::size_of::<FreeBlock>();

// =============================================================================
// Free block node
// =============================================================================

/// Header stored at the beginning of each free block in the heap.
///
/// When a region is freed, we write this header at its start address.
/// The region must be at least `size_of::<FreeBlock>()` bytes (16 bytes
/// on 64-bit) to hold this header.
///
/// # Memory layout
/// ```text
/// ┌──────────────────┐
/// │ size: usize (8B) │ ← total size of this free block INCLUDING header
/// │ next: *mut (8B)  │ ← pointer to next free block (or null)
/// ├──────────────────┤
/// │ ... free space ..│ ← remaining bytes available for allocation
/// └──────────────────┘
/// ```
#[repr(C)]
struct FreeBlock {
    /// Total size of this free block in bytes (including the header).
    size: usize,
    /// Pointer to the next free block, or null if this is the last one.
    next: *mut FreeBlock,
}

// =============================================================================
// Heap internals
// =============================================================================

/// The internal heap state: a sorted linked list of free blocks.
struct Heap {
    /// Head of the free list (sorted by address, lowest first).
    free_list: *mut FreeBlock,

    /// Start of the heap region (for bounds checking in debug mode).
    heap_start: usize,

    /// End of the heap region (exclusive).
    heap_end: usize,

    /// Total bytes currently allocated (for statistics).
    allocated_bytes: usize,

    /// Total heap size in bytes.
    total_bytes: usize,
}

// SAFETY: The heap pointers are only accessed while holding the SpinLock.
unsafe impl Send for Heap {}

impl Heap {
    /// Creates an uninitialized heap. Must call `init()` before use.
    const fn new() -> Self {
        Self {
            free_list: ptr::null_mut(),
            heap_start: 0,
            heap_end: 0,
            allocated_bytes: 0,
            total_bytes: 0,
        }
    }

    /// Initializes the heap with the given memory region.
    ///
    /// Creates a single free block spanning the entire region.
    ///
    /// # Parameters
    /// - `start`: Virtual address of the heap region (must be aligned to at
    ///   least `align_of::<FreeBlock>()`).
    /// - `size`: Size of the heap region in bytes.
    fn init(&mut self, start: usize, size: usize) {
        assert!(size >= MIN_BLOCK_SIZE, "Heap region too small");
        assert!(
            start % core::mem::align_of::<FreeBlock>() == 0,
            "Heap start must be aligned to FreeBlock alignment"
        );

        self.heap_start = start;
        self.heap_end = start + size;
        self.total_bytes = size;
        self.allocated_bytes = 0;

        // Create a single free block spanning the entire heap.
        let block = start as *mut FreeBlock;
        unsafe {
            (*block).size = size;
            (*block).next = ptr::null_mut();
        }
        self.free_list = block;
    }

    /// Allocates memory with the given layout.
    ///
    /// Uses first-fit: walks the free list and picks the first block that
    /// can satisfy the request (including alignment).
    ///
    /// # Returns
    /// A pointer to the allocated memory, or null if out of memory.
    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(MIN_BLOCK_SIZE);
        let align = layout.align().max(core::mem::align_of::<FreeBlock>());

        let mut prev: *mut FreeBlock = ptr::null_mut();
        let mut current = self.free_list;

        while !current.is_null() {
            let block_start = current as usize;
            let block_size = unsafe { (*current).size };
            let block_end = block_start + block_size;

            // Calculate the aligned start address within this block.
            let alloc_start = align_up(block_start, align);
            let alloc_end = alloc_start + size;

            if alloc_end <= block_end {
                // This block can satisfy the request.

                // Unlink this block from the free list.
                let next = unsafe { (*current).next };
                if prev.is_null() {
                    self.free_list = next;
                } else {
                    unsafe {
                        (*prev).next = next;
                    }
                }

                // Front gap: space between block_start and alloc_start.
                // If big enough, return it to the free list.
                let front_gap = alloc_start - block_start;
                if front_gap >= MIN_BLOCK_SIZE {
                    self.insert_free_block(block_start, front_gap);
                }

                // Back gap: space between alloc_end and block_end.
                // If big enough, return it to the free list.
                let back_gap = block_end - alloc_end;
                if back_gap >= MIN_BLOCK_SIZE {
                    self.insert_free_block(alloc_end, back_gap);
                }

                self.allocated_bytes += size;
                return alloc_start as *mut u8;
            }

            prev = current;
            current = unsafe { (*current).next };
        }

        // No suitable block found.
        ptr::null_mut()
    }

    /// Frees previously allocated memory.
    ///
    /// Inserts the freed region back into the free list (sorted by address)
    /// and coalesces with adjacent free blocks to reduce fragmentation.
    fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let addr = ptr as usize;
        let size = layout.size().max(MIN_BLOCK_SIZE);

        debug_assert!(
            addr >= self.heap_start && addr + size <= self.heap_end,
            "Heap: dealloc address outside heap bounds"
        );

        self.allocated_bytes -= size;
        self.insert_free_block(addr, size);
    }

    /// Inserts a free region into the free list, maintaining address order,
    /// and coalesces with adjacent blocks.
    fn insert_free_block(&mut self, addr: usize, size: usize) {
        debug_assert!(size >= MIN_BLOCK_SIZE);

        let new_block = addr as *mut FreeBlock;

        // Find the correct insertion point: walk the list until we find
        // a block with a higher address (or reach the end).
        let mut prev: *mut FreeBlock = ptr::null_mut();
        let mut current = self.free_list;

        while !current.is_null() && (current as usize) < addr {
            prev = current;
            current = unsafe { (*current).next };
        }

        // Initialize the new block.
        unsafe {
            (*new_block).size = size;
            (*new_block).next = current;
        }

        // Link from predecessor (or update head).
        if prev.is_null() {
            self.free_list = new_block;
        } else {
            unsafe {
                (*prev).next = new_block;
            }
        }

        // --- Coalesce with successor ---
        // If the new block ends exactly where the next block starts,
        // merge them into one larger block.
        if !current.is_null() {
            let new_end = addr + unsafe { (*new_block).size };
            if new_end == current as usize {
                unsafe {
                    (*new_block).size += (*current).size;
                    (*new_block).next = (*current).next;
                }
            }
        }

        // --- Coalesce with predecessor ---
        // If the predecessor block ends exactly where the new block starts,
        // merge them.
        if !prev.is_null() {
            let prev_end = prev as usize + unsafe { (*prev).size };
            if prev_end == addr {
                unsafe {
                    (*prev).size += (*new_block).size;
                    (*prev).next = (*new_block).next;
                }
            }
        }
    }
}

// =============================================================================
// Global allocator
// =============================================================================

/// The kernel's global heap allocator.
///
/// Wraps the `Heap` in a `SpinLock` to satisfy `GlobalAlloc`'s `Sync`
/// requirement. All allocation/deallocation calls acquire the lock.
pub struct KernelAllocator {
    inner: SpinLock<Heap>,
}

impl KernelAllocator {
    /// Creates a new, uninitialized kernel allocator.
    ///
    /// Must call `init()` before any allocations occur.
    const fn new() -> Self {
        Self {
            inner: SpinLock::new(Heap::new()),
        }
    }
}

/// SAFETY: The SpinLock ensures exclusive access to the Heap internals.
/// `GlobalAlloc` requires `Sync`, which we provide through the lock.
unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner.lock().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.lock().dealloc(ptr, layout)
    }
}

/// The global kernel heap allocator instance.
///
/// Rust's `alloc` crate (Box, Vec, String, etc.) uses this allocator
/// for all dynamic memory allocation in the kernel.
#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator::new();

// =============================================================================
// Initialization
// =============================================================================

/// Initializes the kernel heap by allocating contiguous physical pages
/// from the PMM and creating a free-list allocator over them.
///
/// After this call, `alloc::vec::Vec`, `alloc::boxed::Box`, and other
/// heap-allocated types are available for use in the kernel.
///
/// # Panics
/// - If the PMM cannot allocate enough contiguous frames.
/// - If the PMM is not initialized (must call `pmm::init()` first).
///
/// # Prerequisites
/// - PMM must be initialized (`pmm::init()`)
/// - HHDM offset must be set (`address::init_hhdm()`)
pub fn init() {
    let heap_size = INITIAL_HEAP_PAGES * PAGE_SIZE as usize;

    // Allocate contiguous physical pages from the PMM.
    let heap_phys = pmm::alloc_contiguous(INITIAL_HEAP_PAGES)
        .expect("Heap: failed to allocate contiguous physical pages for kernel heap");

    // Convert to virtual address via HHDM.
    let heap_virt = heap_phys.to_virt().as_u64() as usize;

    kprintln!(
        "[heap] Allocated {} KiB at phys {} / virt {:#018X}",
        heap_size / 1024,
        heap_phys,
        heap_virt,
    );

    // Initialize the linked-list allocator over this region.
    ALLOCATOR.inner.lock().init(heap_virt, heap_size);

    kprintln!("[heap] Kernel heap initialized ({} KiB)", heap_size / 1024);
}

/// Returns the number of bytes currently allocated from the kernel heap.
pub fn allocated_bytes() -> usize {
    ALLOCATOR.inner.lock().allocated_bytes
}

/// Returns the total size of the kernel heap in bytes.
pub fn total_bytes() -> usize {
    ALLOCATOR.inner.lock().total_bytes
}

// =============================================================================
// Alignment helper
// =============================================================================

/// Aligns `value` up to the nearest multiple of `align`.
///
/// `align` must be a power of two.
#[inline]
const fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

// =============================================================================
// OOM handler
// =============================================================================

/// Called by the `alloc` crate when an allocation fails (returns null).
///
/// In a kernel, OOM is fatal — we can't swap to disk or ask the user
/// to close applications. Panic with a diagnostic message.
#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    panic!(
        "Kernel heap allocation failed: size={}, align={}",
        layout.size(),
        layout.align()
    );
}
