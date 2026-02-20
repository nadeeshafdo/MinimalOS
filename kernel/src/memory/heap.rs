//! Kernel heap — [034] The Heap.
//!
//! A simple linked-list allocator that grows on demand by requesting
//! 4 KiB frames from the PMM and mapping them into a dedicated
//! virtual region via the paging subsystem.
//!
//! The heap virtual range starts at `HEAP_START` and grows upward.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use spin::Mutex;

use super::paging::{self, PageFlags};
use super::pmm;

/// Virtual base address of the kernel heap (in upper-half, away from HHDM).
const HEAP_START: u64 = 0xFFFF_A000_0000_0000;

/// Maximum heap size: 16 MiB (can be raised later).
const HEAP_MAX_SIZE: u64 = 16 * 1024 * 1024;

/// The global kernel allocator.
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap(Mutex::new(Heap {
	head: ptr::null_mut(),
	mapped_end: HEAP_START,
}));

/// A free-list node stored in-place inside freed memory blocks.
#[repr(C)]
struct FreeNode {
	size: usize,
	next: *mut FreeNode,
}

struct Heap {
	/// Head of the free list.
	head: *mut FreeNode,
	/// Current end of mapped heap pages (grows upward from `HEAP_START`).
	mapped_end: u64,
}

// Safety: we protect all access with a spin::Mutex.
unsafe impl Send for Heap {}

struct LockedHeap(Mutex<Heap>);

unsafe impl GlobalAlloc for LockedHeap {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		let mut heap = self.0.lock();
		// Round up to FreeNode alignment (8) so block splits always land on
		// 8-byte boundaries, avoiding misaligned FreeNode pointers.
		let size = align_up(
			layout.size().max(core::mem::size_of::<FreeNode>()) as u64,
			core::mem::align_of::<FreeNode>() as u64,
		) as usize;
		let align = layout.align().max(core::mem::align_of::<FreeNode>());

		// ── 1. Try to find a suitable block in the free list ──
		if let Some(ptr) = find_free_block(&mut heap.head, size, align) {
			return ptr;
		}

		// ── 2. No suitable block — grow the heap ──
		let needed = align_up(size as u64, 4096);
		if heap.mapped_end + needed > HEAP_START + HEAP_MAX_SIZE {
			// OOM
			return ptr::null_mut();
		}

		let grow_base = heap.mapped_end;
		let mut offset: u64 = 0;
		while offset < needed {
			let phys = match pmm::alloc_frame() {
				Some(f) => f,
				None => return ptr::null_mut(),
			};
			paging::map_page(grow_base + offset, phys, PageFlags::KERNEL_RW);
			offset += 4096;
		}
		heap.mapped_end = grow_base + needed;

		// Insert the new region as a free block and re-try
		let node = grow_base as *mut FreeNode;
		(*node).size = needed as usize;
		(*node).next = heap.head;
		heap.head = node;

		find_free_block(&mut heap.head, size, align).unwrap_or(ptr::null_mut())
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		let mut heap = self.0.lock();
		let size = align_up(
			layout.size().max(core::mem::size_of::<FreeNode>()) as u64,
			core::mem::align_of::<FreeNode>() as u64,
		) as usize;

		// Push the freed block onto the free list
		let node = ptr as *mut FreeNode;
		(*node).size = size;
		(*node).next = heap.head;
		heap.head = node;
	}
}

/// Search the free list for a block that satisfies `size` and `align`.
/// If found, unlink it and return the aligned pointer.
unsafe fn find_free_block(
	head: &mut *mut FreeNode,
	size: usize,
	align: usize,
) -> Option<*mut u8> {
	let mut current = head as *mut *mut FreeNode;

	while !(*current).is_null() {
		let node = *current;
		let addr = node as usize;
		let aligned_addr = align_up(addr as u64, align as u64) as usize;
		let padding = aligned_addr - addr;
		let total_needed = size + padding;

		if (*node).size >= total_needed {
			let remaining = (*node).size - total_needed;
			if remaining >= core::mem::size_of::<FreeNode>() {
				// Split: create a new node for the remainder
				let rest = (aligned_addr + size) as *mut FreeNode;
				(*rest).size = remaining;
				(*rest).next = (*node).next;
				*current = rest;
			} else {
				// Use the whole block
				*current = (*node).next;
			}
			return Some(aligned_addr as *mut u8);
		}

		current = &mut (*node).next;
	}

	None
}

/// Initialise the heap. Must be called after PMM + paging init.
///
/// Pre-maps an initial set of pages so small early allocations
/// don't each trigger a page-fault growth path.
pub unsafe fn init() {
	const INITIAL_PAGES: u64 = 16; // 64 KiB initial heap

	let mut heap = ALLOCATOR.0.lock();

	let mut offset: u64 = 0;
	while offset < INITIAL_PAGES * 4096 {
		let phys = pmm::alloc_frame().expect("heap init: out of physical memory");
		paging::map_page(HEAP_START + offset, phys, PageFlags::KERNEL_RW);
		offset += 4096;
	}
	heap.mapped_end = HEAP_START + offset;

	// Insert the entire initial region as one free block
	let node = HEAP_START as *mut FreeNode;
	(*node).size = offset as usize;
	(*node).next = ptr::null_mut();
	heap.head = node;

	klog::info!(
		"[034] Kernel heap initialised: {} KiB at {:#x}..{:#x}",
		offset / 1024,
		HEAP_START,
		heap.mapped_end,
	);
}

#[inline]
const fn align_up(addr: u64, align: u64) -> u64 {
	(addr + align - 1) & !(align - 1)
}
