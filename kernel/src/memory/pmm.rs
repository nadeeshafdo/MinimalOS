//! Bitmap-based Physical Memory Manager (PMM) with per-core frame caches.
//!
//! Tracks 4 KiB page frames with a simple bitmap: bit **1** = used, bit **0** = free.
//! The bitmap itself is carved from the first usable region large enough to hold it.
//!
//! **SMP optimization**: Each core maintains a local cache of pre-allocated
//! frames.  `alloc_frame()` pops from the cache with zero locking; the global
//! bitmap is only touched when the cache is empty or full.

use limine::memory_map::{Entry, EntryType};
use spin::Mutex;

use crate::arch::smp;

/// Size of a single page frame.
const FRAME_SIZE: u64 = 4096;

/// Number of frames each core caches locally.
const CACHE_SIZE: usize = 32;

/// When freeing pushes a cache above this threshold, drain half back to the
/// global bitmap.
const CACHE_DRAIN_THRESHOLD: usize = CACHE_SIZE;

/// Global PMM instance, initialised once at boot.
static PMM: Mutex<Option<BitmapAllocator>> = Mutex::new(None);

/// A bitmap-based physical frame allocator.
struct BitmapAllocator {
	/// Virtual address of the bitmap (accessed via HHDM).
	bitmap: *mut u8,
	/// Physical address where the bitmap is stored.
	_bitmap_phys: u64,
	/// Number of 4 KiB frames consumed by the bitmap itself.
	_bitmap_frames: usize,
	/// Total number of page frames tracked by the bitmap.
	total_frames: usize,
	/// Current number of free (allocatable) frames.
	free_frames: usize,
	/// Hint: byte index where the last successful allocation was found.
	/// Avoids re-scanning the start of the bitmap on every allocation.
	search_hint: usize,
}

// Safety: the bitmap pointer is only accessed under the PMM lock.
unsafe impl Send for BitmapAllocator {}

// ── Per-Core Frame Cache ────────────────────────────────────────

/// A lock-free per-core cache of physical frames.
///
/// Each core only ever accesses its own cache (indexed by core_id),
/// so no atomics or locks are needed.
struct FrameCache {
	frames: [u64; CACHE_SIZE],
	count: usize,
}

impl FrameCache {
	const fn new() -> Self {
		Self {
			frames: [0; CACHE_SIZE],
			count: 0,
		}
	}

	/// Pop a frame from the cache.  Returns `None` if empty.
	#[inline]
	fn pop(&mut self) -> Option<u64> {
		if self.count == 0 {
			return None;
		}
		self.count -= 1;
		Some(self.frames[self.count])
	}

	/// Push a frame into the cache.  Returns `false` if full.
	#[inline]
	fn push(&mut self, frame: u64) -> bool {
		if self.count >= CACHE_SIZE {
			return false;
		}
		self.frames[self.count] = frame;
		self.count += 1;
		true
	}

	/// Is the cache full?
	#[inline]
	fn is_full(&self) -> bool {
		self.count >= CACHE_DRAIN_THRESHOLD
	}
}

/// Per-core frame caches.  Indexed by `smp::core_id()`.
static mut CORE_CACHES: [FrameCache; smp::MAX_CORES] = [
	FrameCache::new(),
	FrameCache::new(),
	FrameCache::new(),
	FrameCache::new(),
];

/// Whether the SMP per-core caches have been activated.
/// Before SMP init, we fall back to the global bitmap directly.
static mut CACHES_ACTIVE: bool = false;

/// Activate per-core frame caches.
///
/// Called after SMP init, once `smp::core_id()` is valid on the BSP.
pub fn activate_caches() {
	unsafe {
		CACHES_ACTIVE = true;
	}
	klog::info!("PMM: per-core frame caches activated ({} frames/core)", CACHE_SIZE);
}

// ── Init ────────────────────────────────────────────────────────

/// Initialise the physical memory manager from the Limine memory map.
///
/// # Safety
///
/// Must be called exactly once during early kernel init, after HHDM is available.
pub unsafe fn init(hhdm_offset: u64, entries: &[&Entry]) {
	// ── 1. Determine the highest physical address we need to track ──
	//	Only consider USABLE regions; device MMIO above that is irrelevant.
	let mut max_usable_addr: u64 = 0;
	for entry in entries.iter() {
		if entry.entry_type == EntryType::USABLE {
			let end = entry.base + entry.length;
			if end > max_usable_addr {
				max_usable_addr = end;
			}
		}
	}

	let total_frames = (max_usable_addr / FRAME_SIZE) as usize;
	let bitmap_bytes = (total_frames + 7) / 8;
	let bitmap_size = ((bitmap_bytes as u64) + FRAME_SIZE - 1) & !(FRAME_SIZE - 1); // round up
	let bitmap_frames = (bitmap_size / FRAME_SIZE) as usize;

	klog::debug!(
		"PMM: tracking {} frames up to {:#x} — bitmap needs {} bytes ({} frames)",
		total_frames,
		max_usable_addr,
		bitmap_bytes,
		bitmap_frames,
	);

	// ── 2. Find a usable region large enough to hold the bitmap ──
	let mut bitmap_phys: Option<u64> = None;
	for entry in entries.iter() {
		if entry.entry_type != EntryType::USABLE {
			continue;
		}
		// Candidate start: skip the null page if the region starts at 0.
		let candidate = if entry.base == 0 {
			FRAME_SIZE // start at 4 KiB instead of 0
		} else {
			entry.base
		};
		let region_end = entry.base + entry.length;
		if candidate + bitmap_size <= region_end {
			bitmap_phys = Some(candidate);
			break;
		}
	}
	let bitmap_phys = bitmap_phys
		.expect("No usable region large enough for the PMM bitmap");

	let bitmap_ptr = (hhdm_offset + bitmap_phys) as *mut u8;

	// ── 3. Mark every frame as USED (0xFF) ──
	core::ptr::write_bytes(bitmap_ptr, 0xFF, bitmap_bytes);

	// ── 4. Clear bits for frames inside USABLE regions ──
	let mut free_count: usize = 0;
	for entry in entries.iter() {
		if entry.entry_type == EntryType::USABLE {
			let start_frame = entry.base / FRAME_SIZE;
			let frame_count = entry.length / FRAME_SIZE;
			for i in 0..frame_count {
				let frame = (start_frame + i) as usize;
				if frame < total_frames {
					let byte_idx = frame / 8;
					let bit_idx = frame % 8;
					*bitmap_ptr.add(byte_idx) &= !(1u8 << bit_idx);
					free_count += 1;
				}
			}
		}
	}

	// ── 5. Re-mark the bitmap's own frames as used ──
	let bitmap_start_frame = (bitmap_phys / FRAME_SIZE) as usize;
	for i in 0..bitmap_frames {
		let frame = bitmap_start_frame + i;
		let byte_idx = frame / 8;
		let bit_idx = frame % 8;
		*bitmap_ptr.add(byte_idx) |= 1u8 << bit_idx;
		free_count -= 1;
	}

	// ── 6. Guard: keep frame 0 permanently used (null-page protection) ──
	if total_frames > 0 && (*bitmap_ptr & 1) == 0 {
		*bitmap_ptr |= 1;
		free_count -= 1;
	}

	klog::info!(
		"[028] PMM initialised: {} frames tracked, {} free ({} MiB), bitmap at {:#x} ({} frames)",
		total_frames,
		free_count,
		(free_count * FRAME_SIZE as usize) / (1024 * 1024),
		bitmap_phys,
		bitmap_frames,
	);

	*PMM.lock() = Some(BitmapAllocator {
		bitmap: bitmap_ptr,
		_bitmap_phys: bitmap_phys,
		_bitmap_frames: bitmap_frames,
		total_frames,
		free_frames: free_count,
		search_hint: 0,
	});
}

// ── Allocation / Free ───────────────────────────────────────────

/// Allocate a single 4 KiB physical frame.
///
/// Returns the **physical address** of the frame, or `None` if OOM.
///
/// **Fast path**: pops from the per-core cache (zero locking).
/// **Slow path**: locks the global bitmap and refills the cache.
pub fn alloc_frame() -> Option<u64> {
	// Fast path: per-core cache (after SMP is initialized).
	unsafe {
		if CACHES_ACTIVE {
			let core = smp::core_id() as usize;
			let cache = &mut CORE_CACHES[core];
			if let Some(frame) = cache.pop() {
				return Some(frame);
			}
			// Cache empty — refill from global bitmap.
			return refill_and_alloc(cache);
		}
	}

	// Pre-SMP path: global bitmap directly.
	alloc_from_global()
}

/// Free a previously allocated 4 KiB physical frame.
///
/// **Fast path**: pushes to the per-core cache (zero locking).
/// **Slow path**: if cache is full, drains half back to the global bitmap.
///
/// # Panics
///
/// Panics on double-free or out-of-range address.
pub fn free_frame(phys_addr: u64) {
	// Fast path: per-core cache.
	unsafe {
		if CACHES_ACTIVE {
			let core = smp::core_id() as usize;
			let cache = &mut CORE_CACHES[core];
			if cache.push(phys_addr) {
				// If cache is getting full, drain half back to bitmap.
				if cache.is_full() {
					drain_cache(cache);
				}
				return;
			}
			// Cache was full even before push — drain and retry.
			drain_cache(cache);
			cache.push(phys_addr);
			return;
		}
	}

	// Pre-SMP path: global bitmap directly.
	free_to_global(phys_addr);
}

/// Allocate `count` physically contiguous 4 KiB frames.
///
/// Scans the global bitmap (bypassing per-core caches) for a run of
/// `count` consecutive free frames.  Returns the physical address of the
/// first frame, or `None` if insufficient contiguous space exists.
pub fn alloc_contiguous(count: usize) -> Option<u64> {
	if count == 0 { return None; }
	let mut guard = PMM.lock();
	let alloc = guard.as_mut()?;

	if count == 1 {
		return alloc_from_bitmap(alloc);
	}

	let mut run_start: usize = 0;
	let mut run_len: usize = 0;

	for frame in 0..alloc.total_frames {
		let byte_idx = frame / 8;
		let bit_idx = frame % 8;
		let used = unsafe { *alloc.bitmap.add(byte_idx) } & (1u8 << bit_idx) != 0;

		if used {
			run_start = frame + 1;
			run_len = 0;
		} else {
			run_len += 1;
			if run_len == count {
				// Mark all frames in the run as used.
				for f in run_start..run_start + count {
					let bi = f / 8;
					let bt = f % 8;
					unsafe { *alloc.bitmap.add(bi) |= 1u8 << bt; }
				}
				alloc.free_frames -= count;
				alloc.search_hint = (run_start + count) / 8;
				return Some(run_start as u64 * FRAME_SIZE);
			}
		}
	}
	None
}

/// Return the current number of free frames (approximate — excludes
/// frames held in per-core caches).
pub fn free_frame_count() -> usize {
	PMM.lock().as_ref().map_or(0, |a| a.free_frames)
}

// ── Internal: global bitmap operations ──────────────────────────

/// Allocate a frame directly from the global bitmap (under lock).
fn alloc_from_global() -> Option<u64> {
	let mut guard = PMM.lock();
	let alloc = guard.as_mut()?;
	alloc_from_bitmap(alloc)
}

/// Search the bitmap for a free frame, mark it used, return its address.
fn alloc_from_bitmap(alloc: &mut BitmapAllocator) -> Option<u64> {
	let bitmap_bytes = (alloc.total_frames + 7) / 8;
	let start = alloc.search_hint;

	// Scan from hint to end, then wrap around.
	for offset in 0..bitmap_bytes {
		let byte_idx = (start + offset) % bitmap_bytes;
		let byte = unsafe { *alloc.bitmap.add(byte_idx) };
		if byte == 0xFF {
			continue;
		}
		let bit_idx = byte.trailing_ones() as usize;
		let frame = byte_idx * 8 + bit_idx;
		if frame >= alloc.total_frames {
			continue;
		}
		unsafe {
			*alloc.bitmap.add(byte_idx) |= 1u8 << bit_idx;
		}
		alloc.free_frames -= 1;
		alloc.search_hint = byte_idx; // remember for next time
		return Some(frame as u64 * FRAME_SIZE);
	}
	None
}

/// Free a frame directly to the global bitmap (under lock).
fn free_to_global(phys_addr: u64) {
	let mut guard = PMM.lock();
	let alloc = guard.as_mut().expect("PMM not initialised");
	free_to_bitmap(alloc, phys_addr);
}

/// Mark a frame as free in the bitmap.
fn free_to_bitmap(alloc: &mut BitmapAllocator, phys_addr: u64) {
	assert!(phys_addr % FRAME_SIZE == 0, "free_frame: address not frame-aligned");

	let frame = (phys_addr / FRAME_SIZE) as usize;
	assert!(frame < alloc.total_frames, "free_frame: frame {} out of range", frame);

	let byte_idx = frame / 8;
	let bit_idx = frame % 8;

	unsafe {
		let byte = *alloc.bitmap.add(byte_idx);
		assert!(byte & (1u8 << bit_idx) != 0, "free_frame: double free of frame {}", frame);
		*alloc.bitmap.add(byte_idx) = byte & !(1u8 << bit_idx);
	}
	alloc.free_frames += 1;
}

/// Refill the per-core cache from the global bitmap, then return one frame.
fn refill_and_alloc(cache: &mut FrameCache) -> Option<u64> {
	let mut guard = PMM.lock();
	let alloc = guard.as_mut()?;

	// Refill: pull up to CACHE_SIZE/2 frames into the cache.
	let refill_count = CACHE_SIZE / 2;
	for _ in 0..refill_count {
		if let Some(frame) = alloc_from_bitmap(alloc) {
			cache.push(frame);
		} else {
			break;
		}
	}

	// Return one frame from the (now hopefully non-empty) cache.
	if let Some(frame) = cache.pop() {
		Some(frame)
	} else {
		// Truly OOM.
		None
	}
}

/// Drain half the per-core cache back to the global bitmap.
fn drain_cache(cache: &mut FrameCache) {
	let drain_count = cache.count / 2;
	if drain_count == 0 {
		return;
	}

	let mut guard = PMM.lock();
	let alloc = guard.as_mut().expect("PMM not initialised");

	for _ in 0..drain_count {
		if let Some(frame) = cache.pop() {
			free_to_bitmap(alloc, frame);
		}
	}
}
