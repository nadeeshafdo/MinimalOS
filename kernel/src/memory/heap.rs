//! Kernel heap — [034] The Heap.
//!
//! A linked-list allocator with per-core bump arenas for SMP performance.
//!
//! **Fast path**: Small allocations (≤ 4 KiB) are bump-allocated from a
//! per-core arena with zero locking.  Each arena is 256 KiB.
//!
//! **Slow path**: Large allocations or arena exhaustion fall back to the
//! global locked free-list, which grows on demand by requesting 4 KiB
//! frames from the PMM.
//!
//! Deallocations always go to the global free-list (the bump arenas don't
//! support free).  This is acceptable because most kernel allocations are
//! long-lived and deallocation is infrequent.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use spin::Mutex;

use crate::arch::smp;

use super::paging::{self, PageFlags};
use super::pmm;

/// Virtual base address of the kernel heap (in upper-half, away from HHDM).
const HEAP_START: u64 = 0xFFFF_A000_0000_0000;

/// Maximum heap size: 16 MiB (can be raised later).
const HEAP_MAX_SIZE: u64 = 16 * 1024 * 1024;

/// Size of per-core bump arenas: 256 KiB.
const ARENA_SIZE: usize = 256 * 1024;

/// Maximum allocation size served by per-core arenas.
const ARENA_MAX_ALLOC: usize = 4096;

/// The global kernel allocator.
#[global_allocator]
static ALLOCATOR: SmpHeap = SmpHeap {
	global: Mutex::new(Heap {
		head: ptr::null_mut(),
		mapped_end: HEAP_START,
	}),
};

// ── Per-Core Bump Arena ─────────────────────────────────────────

/// A simple bump allocator for a fixed-size arena.
/// Each core gets one.  No locking needed.
struct BumpArena {
	/// Virtual base address of this arena's memory.
	base: u64,
	/// Current allocation pointer (bumps upward).
	cursor: u64,
	/// End of the arena (base + ARENA_SIZE).
	end: u64,
}

impl BumpArena {
	const fn empty() -> Self {
		Self { base: 0, cursor: 0, end: 0 }
	}

	/// Try to bump-allocate `size` bytes with `align` alignment.
	#[inline]
	fn alloc(&mut self, size: usize, align: usize) -> Option<*mut u8> {
		if self.base == 0 {
			return None; // not initialized
		}
		let aligned = align_up(self.cursor, align as u64);
		let new_cursor = aligned + size as u64;
		if new_cursor > self.end {
			return None; // arena exhausted
		}
		self.cursor = new_cursor;
		Some(aligned as *mut u8)
	}

	/// Check if a pointer falls within this arena.
	#[inline]
	fn contains(&self, ptr: *mut u8) -> bool {
		let addr = ptr as u64;
		self.base != 0 && addr >= self.base && addr < self.end
	}
}

/// Per-core arenas.  Indexed by `smp::core_id()`.
static mut CORE_ARENAS: [BumpArena; smp::MAX_CORES] = [
	BumpArena::empty(),
	BumpArena::empty(),
	BumpArena::empty(),
	BumpArena::empty(),
];

/// Whether per-core arenas are active.
static mut ARENAS_ACTIVE: bool = false;

// ── Global Free-List Heap ───────────────────────────────────────

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

struct SmpHeap {
	global: Mutex<Heap>,
}

unsafe impl GlobalAlloc for SmpHeap {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		let size = align_up(
			layout.size().max(core::mem::size_of::<FreeNode>()) as u64,
			core::mem::align_of::<FreeNode>() as u64,
		) as usize;
		let align = layout.align().max(core::mem::align_of::<FreeNode>());

		// ── Fast path: per-core bump arena ──
		if ARENAS_ACTIVE && size <= ARENA_MAX_ALLOC {
			let core = smp::core_id() as usize;
			if core < smp::MAX_CORES {
				let arena = &mut CORE_ARENAS[core];
				if let Some(ptr) = arena.alloc(size, align) {
					return ptr;
				}
				// Arena exhausted — fall through to global.
			}
		}

		// ── Slow path: global locked free-list ──
		let mut heap = self.global.lock();

		// 1. Try to find a suitable block in the free list.
		if let Some(ptr) = find_free_block(&mut heap.head, size, align) {
			return ptr;
		}

		// 2. No suitable block — grow the heap.
		let needed = align_up(size as u64, 4096);
		if heap.mapped_end + needed > HEAP_START + HEAP_MAX_SIZE {
			return ptr::null_mut(); // OOM
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

		// Insert the new region as a free block and re-try.
		let node = grow_base as *mut FreeNode;
		(*node).size = needed as usize;
		(*node).next = heap.head;
		heap.head = node;

		find_free_block(&mut heap.head, size, align).unwrap_or(ptr::null_mut())
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		// If the pointer is in a per-core arena, we can't free it
		// (bump arenas don't support individual free).  Just leak it.
		// This is acceptable: kernel allocations are mostly long-lived.
		if ARENAS_ACTIVE {
			for i in 0..smp::MAX_CORES {
				if CORE_ARENAS[i].contains(ptr) {
					return; // Leak — bump arenas don't free.
				}
			}
		}

		// Global free-list path.
		let mut heap = self.global.lock();
		let size = align_up(
			layout.size().max(core::mem::size_of::<FreeNode>()) as u64,
			core::mem::align_of::<FreeNode>() as u64,
		) as usize;

		let node = ptr as *mut FreeNode;
		(*node).size = size;
		(*node).next = heap.head;
		heap.head = node;
	}
}

// ── Free-list search ────────────────────────────────────────────

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
				// Split: create a new node for the remainder.
				let rest = (aligned_addr + size) as *mut FreeNode;
				(*rest).size = remaining;
				(*rest).next = (*node).next;
				*current = rest;
			} else {
				// Use the whole block.
				*current = (*node).next;
			}
			return Some(aligned_addr as *mut u8);
		}

		current = &mut (*node).next;
	}

	None
}

// ── Init ────────────────────────────────────────────────────────

/// Initialise the heap. Must be called after PMM + paging init.
///
/// Pre-maps an initial set of pages so small early allocations
/// don't each trigger a page-fault growth path.
pub unsafe fn init() {
	const INITIAL_PAGES: u64 = 16; // 64 KiB initial heap

	let mut heap = ALLOCATOR.global.lock();

	let mut offset: u64 = 0;
	while offset < INITIAL_PAGES * 4096 {
		let phys = pmm::alloc_frame().expect("heap init: out of physical memory");
		paging::map_page(HEAP_START + offset, phys, PageFlags::KERNEL_RW);
		offset += 4096;
	}
	heap.mapped_end = HEAP_START + offset;

	// Insert the entire initial region as one free block.
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

/// Initialize per-core bump arenas.
///
/// Called after SMP init and after the global heap is available.
/// Each core gets a 256 KiB bump arena mapped into the heap region.
pub unsafe fn init_arenas() {
	let mut heap = ALLOCATOR.global.lock();

	for core_id in 0..smp::core_count() as usize {
		let arena_virt = heap.mapped_end;
		let arena_pages = (ARENA_SIZE as u64) / 4096;

		// Check we don't exceed heap maximum.
		if arena_virt + ARENA_SIZE as u64 > HEAP_START + HEAP_MAX_SIZE {
			klog::warn!("Heap: not enough space for core {} arena", core_id);
			break;
		}

		// Map arena pages.
		let mut offset: u64 = 0;
		while offset < arena_pages * 4096 {
			let phys = pmm::alloc_frame().expect("arena init: out of physical memory");
			paging::map_page(arena_virt + offset, phys, PageFlags::KERNEL_RW);
			offset += 4096;
		}
		heap.mapped_end = arena_virt + ARENA_SIZE as u64;

		CORE_ARENAS[core_id] = BumpArena {
			base: arena_virt,
			cursor: arena_virt,
			end: arena_virt + ARENA_SIZE as u64,
		};
	}

	drop(heap);

	ARENAS_ACTIVE = true;
	klog::info!(
		"Heap: per-core arenas activated ({} KiB/core, {} cores)",
		ARENA_SIZE / 1024,
		smp::core_count(),
	);
}

#[inline]
const fn align_up(addr: u64, align: u64) -> u64 {
	(addr + align - 1) & !(align - 1)
}
