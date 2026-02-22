//! Kernel heap — reclaiming linked-list allocator with per-core arenas.
//!
//! **Foundation:** Uses `linked_list_allocator::Heap` which properly recycles
//! freed memory.  When `tinywasm` drops a parsed AST, the memory is returned
//! to the free list and reused by the next actor spawn.
//!
//! **Architecture:**
//! - Each of the 4 SMP cores gets a dedicated 2 MiB arena (8 MiB total).
//! - Small allocations (≤ 4 KiB) go to the per-core arena (no locking).
//! - Large allocations and arena exhaustion fall back to the global heap,
//!   which grows on demand from the PMM.
//! - **Deallocations are always routed back** to the correct arena or the
//!   global heap — memory is genuinely recycled.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::RefCell;
use core::ptr;

use linked_list_allocator::Heap;
use spin::Mutex;

use crate::arch::smp;

use super::paging::{self, PageFlags};
use super::pmm;

/// Virtual base address of the kernel heap (in upper-half, away from HHDM).
const HEAP_START: u64 = 0xFFFF_A000_0000_0000;

/// Maximum global heap size: 16 MiB (can be raised later).
const HEAP_MAX_SIZE: u64 = 16 * 1024 * 1024;

/// Virtual base for per-core arenas — 4 GiB past HEAP_START to avoid overlap.
const ARENA_BASE: u64 = 0xFFFF_A000_1000_0000;

/// Size of per-core arenas: 2 MiB each.
const ARENA_SIZE: usize = 2 * 1024 * 1024;

/// Maximum allocation size served by per-core arenas.
const ARENA_MAX_ALLOC: usize = 4096;

// ── Per-Core Arena ──────────────────────────────────────────────

/// A per-core heap backed by `linked_list_allocator::Heap`.
/// Uses `RefCell` — safe because each core only accesses its own arena
/// with interrupts disabled during allocation.
struct LocalHeap {
	inner: RefCell<Heap>,
	/// Virtual base of this arena (for containment checks).
	base: u64,
	/// Virtual end of this arena.
	end: u64,
}

// Safety: each core only accesses its own LocalHeap.  The global
// allocator routes by `smp::core_id()`.  Context switches never
// happen inside the allocator because interrupt handlers don't
// allocate, and the scheduler is not invoked mid-alloc.
unsafe impl Sync for LocalHeap {}

impl LocalHeap {
	const fn empty() -> Self {
		Self {
			inner: RefCell::new(Heap::empty()),
			base: 0,
			end: 0,
		}
	}

	fn alloc(&self, layout: Layout) -> *mut u8 {
		self.inner
			.borrow_mut()
			.allocate_first_fit(layout)
			.ok()
			.map_or(ptr::null_mut(), |ptr| ptr.as_ptr())
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		unsafe {
			self.inner
				.borrow_mut()
				.deallocate(core::ptr::NonNull::new_unchecked(ptr), layout);
		}
	}

	/// Check if a pointer falls within this arena's address range.
	fn contains(&self, ptr: *mut u8) -> bool {
		let addr = ptr as u64;
		self.base != 0 && addr >= self.base && addr < self.end
	}
}

/// Per-core arenas indexed by `smp::core_id()`.
static CORE_ARENAS: [LocalHeap; smp::MAX_CORES] = [
	LocalHeap::empty(),
	LocalHeap::empty(),
	LocalHeap::empty(),
	LocalHeap::empty(),
];

/// Whether per-core arenas are active.
static mut ARENAS_ACTIVE: bool = false;

// ── Global Heap ─────────────────────────────────────────────────

/// The global fallback heap — grows on demand from the PMM.
struct GlobalHeap {
	inner: Heap,
	/// Current end of mapped heap pages (grows upward from HEAP_START).
	mapped_end: u64,
}

// Safety: protected by spin::Mutex.
unsafe impl Send for GlobalHeap {}

/// The global kernel allocator.
#[global_allocator]
static ALLOCATOR: SmpAllocator = SmpAllocator {
	global: Mutex::new(GlobalHeap {
		inner: Heap::empty(),
		mapped_end: HEAP_START,
	}),
};

struct SmpAllocator {
	global: Mutex<GlobalHeap>,
}

unsafe impl GlobalAlloc for SmpAllocator {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		// Disable interrupts to prevent re-entrant allocation from
		// timer ISR (which would deadlock the global Mutex or panic
		// the per-core RefCell).
		let flags = save_and_disable_interrupts();

		// ── Fast path: per-core arena ──
		if unsafe { ARENAS_ACTIVE } && layout.size() <= ARENA_MAX_ALLOC {
			let core = smp::core_id() as usize;
			if core < smp::MAX_CORES {
				let ptr = CORE_ARENAS[core].alloc(layout);
				if !ptr.is_null() {
					restore_interrupts(flags);
					return ptr;
				}
				// Arena exhausted — fall through to global.
			}
		}

		// ── Slow path: global locked heap ──
		let mut heap = self.global.lock();

		// Try allocation first.
		if let Ok(ptr) = heap.inner.allocate_first_fit(layout) {
			restore_interrupts(flags);
			return ptr.as_ptr();
		}

		// Not enough space — grow the heap by requesting frames from PMM.
		let needed = align_up(layout.size() as u64, 4096).max(4096);
		if heap.mapped_end + needed > HEAP_START + HEAP_MAX_SIZE {
			restore_interrupts(flags);
			return ptr::null_mut(); // OOM
		}

		let grow_base = heap.mapped_end;
		let mut offset: u64 = 0;
		while offset < needed {
			let phys = match pmm::alloc_frame() {
				Some(f) => f,
				None => {
					restore_interrupts(flags);
					return ptr::null_mut();
				}
			};
			paging::map_page(grow_base + offset, phys, PageFlags::KERNEL_RW);
			offset += 4096;
		}
		heap.mapped_end = grow_base + offset;

		// Extend the heap with the new region.
		unsafe {
			heap.inner.extend(offset as usize);
		}

		// Retry the allocation.
		let result = heap.inner
			.allocate_first_fit(layout)
			.ok()
			.map_or(ptr::null_mut(), |ptr| ptr.as_ptr());
		restore_interrupts(flags);
		result
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		let flags = save_and_disable_interrupts();

		// Route deallocation to the correct arena if the pointer belongs
		// to one — this is what makes memory actually recyclable.
		if unsafe { ARENAS_ACTIVE } {
			for i in 0..smp::MAX_CORES {
				if CORE_ARENAS[i].contains(ptr) {
					unsafe { CORE_ARENAS[i].dealloc(ptr, layout) };
					restore_interrupts(flags);
					return;
				}
			}
		}

		// Global heap path.
		let mut heap = self.global.lock();
		unsafe {
			heap.inner.deallocate(
				core::ptr::NonNull::new_unchecked(ptr),
				layout,
			);
		}
		restore_interrupts(flags);
	}
}

// ── Interrupt Guard ──────────────────────────────────────────────

/// Save RFLAGS and disable interrupts.  Returns the original RFLAGS.
#[inline(always)]
fn save_and_disable_interrupts() -> u64 {
	let flags: u64;
	unsafe {
		core::arch::asm!(
			"pushfq; pop {}; cli",
			out(reg) flags,
			options(nomem, preserves_flags)
		);
	}
	flags
}

/// Restore RFLAGS (re-enables interrupts if they were enabled before).
#[inline(always)]
fn restore_interrupts(flags: u64) {
	unsafe {
		core::arch::asm!(
			"push {}; popfq",
			in(reg) flags,
			options(nomem)
		);
	}
}

// ── Init ────────────────────────────────────────────────────────

/// Initialise the global heap.  Must be called after PMM + paging init.
///
/// Pre-maps an initial set of pages so small early allocations
/// don't each trigger a growth path.
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

	// Initialise the linked-list allocator with the mapped region.
	unsafe {
		heap.inner.init(HEAP_START as *mut u8, offset as usize);
	}

	klog::info!(
		"[034] Kernel heap initialised: {} KiB at {:#x}..{:#x}",
		offset / 1024,
		HEAP_START,
		heap.mapped_end,
	);
}

/// Initialise per-core arenas (2 MiB each).
///
/// Called after SMP init when the global heap is available.
/// Each core gets a full recycling allocator — not a bump arena.
/// Arenas live at `ARENA_BASE`, completely separate from the global
/// heap's virtual range so `extend()` can never collide.
///
/// Gracefully handles low-memory situations by skipping arenas that
/// cannot be fully backed by physical frames.
pub unsafe fn init_arenas() {
	let mut cores_done = 0usize;

	for core_id in 0..smp::core_count() as usize {
		let arena_virt = ARENA_BASE + (core_id as u64) * (ARENA_SIZE as u64);
		let arena_pages = (ARENA_SIZE as u64) / 4096;

		// Map arena pages — break gracefully if PMM is exhausted.
		let mut offset: u64 = 0;
		let mut ok = true;
		while offset < arena_pages * 4096 {
			let phys = match pmm::alloc_frame() {
				Some(f) => f,
				None => {
					klog::warn!(
						"Heap: PMM exhausted after {} pages for core {} arena — skipping remaining cores",
						offset / 4096, core_id
					);
					ok = false;
					break;
				}
			};
			paging::map_page(arena_virt + offset, phys, PageFlags::KERNEL_RW);
			offset += 4096;
		}

		if !ok {
			break; // Don't partially init an arena.
		}

		// Initialise the per-core linked-list heap.
		unsafe {
			let arena = &CORE_ARENAS[core_id] as *const LocalHeap as *mut LocalHeap;
			(*arena).inner.borrow_mut().init(arena_virt as *mut u8, ARENA_SIZE);
			(*arena).base = arena_virt;
			(*arena).end = arena_virt + ARENA_SIZE as u64;
		}
		cores_done += 1;
	}

	if cores_done > 0 {
		unsafe { ARENAS_ACTIVE = true; }
		klog::info!(
			"Heap: per-core arenas activated ({} MiB/core, {}/{} cores)",
			ARENA_SIZE / (1024 * 1024),
			cores_done,
			smp::core_count(),
		);
	} else {
		klog::warn!("Heap: no per-core arenas — all allocations use global heap");
	}
}

#[inline]
const fn align_up(addr: u64, align: u64) -> u64 {
	(addr + align - 1) & !(align - 1)
}
