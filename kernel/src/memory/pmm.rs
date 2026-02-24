// =============================================================================
// MinimalOS NextGen — Physical Memory Manager (Bitmap Frame Allocator)
// =============================================================================
//
// The PMM tracks which physical page frames (4 KiB each) are free or in use.
// It uses a bitmap: one bit per frame.
//
// BITMAP LAYOUT:
//   bit = 1 → frame is USED (allocated, reserved, or hardware-mapped)
//   bit = 0 → frame is FREE (available for allocation)
//
//   Bit 0 of byte 0 corresponds to frame 0 (physical address 0x0000).
//   Bit 7 of byte 0 corresponds to frame 7 (physical address 0x7000).
//   Bit 0 of byte 1 corresponds to frame 8 (physical address 0x8000).
//   ... and so on.
//
// INITIALIZATION ALGORITHM (3-pass over Limine memory map):
//   Pass 1: Scan entries to find the highest physical address.
//           This determines the bitmap size (highest_addr / PAGE_SIZE / 8).
//   Pass 2: Find a USABLE region large enough to hold the bitmap.
//           Place the bitmap there, accessed via HHDM.
//   Pass 3: Mark USABLE regions as free (clear bits).
//           Then re-mark the bitmap's own pages and frame 0 as used.
//
// ALLOCATION STRATEGY:
//   Single frame: Linear scan using u64-at-a-time for 64× speedup.
//   Contiguous N: Linear scan for N consecutive zero bits.
//   The `search_start` cursor avoids re-scanning already-allocated regions.
//
// SIZING FOR N3710 (8 GB RAM):
//   Max physical address ≈ 8 GB → 2,097,152 frames
//   Bitmap = 2,097,152 / 8 = 256 KiB = 64 pages
//   Negligible overhead.
//
// THREAD SAFETY:
//   The global PMM state is protected by a SpinLock. All public functions
//   acquire the lock before accessing the bitmap.
//
// =============================================================================

use core::ptr;

use crate::kprintln;
use crate::memory::address::{PhysAddr, PAGE_SIZE};
use crate::sync::spinlock::SpinLock;

// =============================================================================
// Public types
// =============================================================================

/// Snapshot of physical memory usage statistics.
///
/// Returned by `stats()` for boot-time reporting and diagnostics.
#[derive(Debug, Clone, Copy)]
pub struct MemoryStats {
    /// Total number of physical frames tracked by the bitmap.
    pub total_frames: usize,

    /// Number of frames currently marked as used.
    pub used_frames: usize,

    /// Number of frames currently marked as free.
    pub free_frames: usize,

    /// Size of the bitmap itself, in bytes.
    pub bitmap_bytes: usize,
}

// =============================================================================
// Global PMM state
// =============================================================================

/// The global physical memory manager, protected by a ticket spinlock.
///
/// `None` before `init()` is called. All public functions panic if the
/// PMM is not yet initialized.
static PMM: SpinLock<Option<BitmapAllocator>> = SpinLock::new(None);

// =============================================================================
// Bitmap Allocator internals
// =============================================================================

/// The bitmap-based physical frame allocator.
///
/// Holds a pointer to the bitmap (via HHDM), its size, and usage counters.
/// Not exposed publicly — all access goes through the module-level functions
/// which hold the spinlock.
struct BitmapAllocator {
    /// Virtual address of the bitmap (accessed through HHDM).
    /// The bitmap lives in physical memory; we access it at phys + HHDM_OFFSET.
    bitmap: *mut u8,

    /// Size of the bitmap in bytes.
    bitmap_bytes: usize,

    /// Physical address where the bitmap starts (needed to mark it as used).
    bitmap_phys: PhysAddr,

    /// Number of physical frames the bitmap occupies.
    bitmap_frame_count: usize,

    /// Total number of physical frames tracked (= highest_addr / PAGE_SIZE).
    total_frames: usize,

    /// Number of frames currently marked as used.
    used_frames: usize,

    /// Optimization: start the next allocation scan from this frame index.
    /// Updated after each alloc/free to avoid rescanning known-used regions.
    search_start: usize,
}

// SAFETY: The bitmap pointer is only dereferenced while holding the PMM spinlock.
// No other code accesses the bitmap concurrently.
unsafe impl Send for BitmapAllocator {}

impl BitmapAllocator {
    /// Creates and initializes a new bitmap allocator from the Limine memory map.
    ///
    /// # Algorithm
    /// 1. Find the highest physical address to size the bitmap.
    /// 2. Find a USABLE region to store the bitmap.
    /// 3. memset bitmap to 0xFF (all frames = used).
    /// 4. Clear bits for USABLE regions (mark them free).
    /// 5. Re-mark the bitmap's own frames and frame 0 as used.
    ///
    /// # Panics
    /// - If no usable region is large enough for the bitmap.
    fn new(memory_map: &[&limine::memory_map::Entry]) -> Self {
        // =====================================================================
        // Pass 1: Determine bitmap size from highest physical address
        // =====================================================================
        //
        // We only need to track frames up to the highest USABLE, BOOTLOADER_
        // RECLAIMABLE, or ACPI_RECLAIMABLE address. Reserved regions above
        // that (e.g., PCI MMIO windows at 0xFD00000000+) don't need tracking
        // because they'll never be freed.
        let mut highest_addr: u64 = 0;
        for entry in memory_map {
            let dominated = matches!(
                entry.entry_type,
                limine::memory_map::EntryType::USABLE
                    | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
                    | limine::memory_map::EntryType::ACPI_RECLAIMABLE
                    | limine::memory_map::EntryType::EXECUTABLE_AND_MODULES
                    | limine::memory_map::EntryType::FRAMEBUFFER
            );
            if dominated {
                let end = entry.base + entry.length;
                if end > highest_addr {
                    highest_addr = end;
                }
            }
        }

        let total_frames = (highest_addr / PAGE_SIZE) as usize;
        let bitmap_bytes = (total_frames + 7) / 8; // round up to whole bytes
        let bitmap_frame_count =
            (bitmap_bytes + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;

        kprintln!(
            "[pmm] Highest physical address: {:#012X} ({} MiB)",
            highest_addr,
            highest_addr / 1024 / 1024
        );
        kprintln!(
            "[pmm] Tracking {} frames, bitmap = {} bytes ({} pages)",
            total_frames,
            bitmap_bytes,
            bitmap_frame_count
        );

        // =====================================================================
        // Pass 2: Find a USABLE region for the bitmap
        // =====================================================================
        //
        // We need `bitmap_frame_count` contiguous pages of usable memory.
        // Pick the first usable region that's large enough. The bitmap is
        // tiny (65 KiB for ~512 MiB), so any region > 256 KiB works.
        //
        // We skip regions starting at address 0 because frame 0 is reserved
        // as a null-safety guard. Placing the bitmap there would overlap
        // with the reserved frame.
        let mut bitmap_phys: Option<PhysAddr> = None;
        for entry in memory_map {
            if entry.entry_type == limine::memory_map::EntryType::USABLE
                && entry.length >= (bitmap_frame_count as u64 * PAGE_SIZE)
                && entry.base > 0 // Don't use the region starting at physical 0
            {
                bitmap_phys = Some(PhysAddr::new(entry.base));
                break;
            }
        }

        let bitmap_phys = bitmap_phys.expect(
            "PMM: no usable region large enough for bitmap",
        );

        kprintln!("[pmm] Bitmap placed at physical {}", bitmap_phys);

        // Access the bitmap through the HHDM (Higher Half Direct Map).
        let bitmap = bitmap_phys.to_virt().as_mut_ptr::<u8>();

        // =====================================================================
        // Step 3: Initialize all bits to 1 (every frame = USED)
        // =====================================================================
        //
        // We start pessimistic: everything is used. Then we selectively
        // free the regions that are actually available.
        //
        // SAFETY: `bitmap` points to `bitmap_bytes` bytes of valid physical
        // memory mapped through HHDM. We hold exclusive access (single-core
        // boot, PMM lock not released yet).
        unsafe {
            ptr::write_bytes(bitmap, 0xFF, bitmap_bytes);
        }
        let mut used_frames = total_frames;

        // =====================================================================
        // Pass 3: Mark USABLE regions as free (clear their bits)
        // =====================================================================
        for entry in memory_map {
            if entry.entry_type == limine::memory_map::EntryType::USABLE {
                let start_frame = (entry.base / PAGE_SIZE) as usize;
                let end_frame = ((entry.base + entry.length) / PAGE_SIZE) as usize;
                let freed = clear_range(bitmap, start_frame, end_frame.min(total_frames));
                used_frames -= freed;
            }
        }

        // =====================================================================
        // Step 5a: Re-mark the bitmap's own pages as used
        // =====================================================================
        //
        // The bitmap lives inside a USABLE region, so Pass 3 freed its bits.
        // We need to mark them used again so the bitmap memory is not
        // given out as a free frame.
        let bitmap_start_frame = (bitmap_phys.as_u64() / PAGE_SIZE) as usize;
        for frame in bitmap_start_frame..bitmap_start_frame + bitmap_frame_count {
            used_frames += set_bit(bitmap, frame);
        }

        // =====================================================================
        // Step 5b: Ensure frame 0 is always marked used (null safety)
        // =====================================================================
        //
        // Physical address 0 is conventionally treated as "null".
        // Allocating frame 0 and handing it to a caller would look like
        // a null pointer, causing subtle bugs. Mark it unconditionally used.
        used_frames += set_bit(bitmap, 0);

        kprintln!(
            "[pmm] Free frames: {} ({} MiB), used: {} ({} MiB)",
            total_frames - used_frames,
            (total_frames - used_frames) as u64 * PAGE_SIZE / 1024 / 1024,
            used_frames,
            used_frames as u64 * PAGE_SIZE / 1024 / 1024,
        );

        Self {
            bitmap,
            bitmap_bytes,
            bitmap_phys,
            bitmap_frame_count,
            total_frames,
            used_frames,
            search_start: 0,
        }
    }

    // =========================================================================
    // Allocation
    // =========================================================================

    /// Allocates a single physical frame.
    ///
    /// Scans the bitmap using u64-at-a-time reads for performance:
    /// if all 64 bits in a u64 are 1, the entire chunk is used and we skip
    /// ahead by 64 frames. On the N3710, this makes the common case
    /// (scanning past fully-allocated regions) very fast.
    ///
    /// # Returns
    /// `Some(PhysAddr)` — the page-aligned physical address of the allocated frame.
    /// `None` — if all frames are used (out of memory).
    fn alloc_frame(&mut self) -> Option<PhysAddr> {
        let total_chunks = (self.total_frames + 63) / 64;
        let start_chunk = self.search_start / 64;
        let bitmap_u64 = self.bitmap as *const u64;

        for i in 0..total_chunks {
            let chunk_idx = (start_chunk + i) % total_chunks;
            // SAFETY: The bitmap is page-aligned (≥ 8-byte aligned), so
            // reading u64-at-a-time from the start is always aligned.
            // The bitmap allocation is rounded up to whole pages, so
            // reading beyond `bitmap_bytes` by up to 7 bytes is within
            // the allocated page(s).
            let chunk = unsafe { *bitmap_u64.add(chunk_idx) };

            if chunk == u64::MAX {
                // All 64 frames in this chunk are used. Skip.
                continue;
            }

            // At least one bit is 0 (free). Find it.
            // `trailing_zeros` on the inverse gives the index of the
            // first 0 bit (first free frame in this chunk).
            let bit_in_chunk = (!chunk).trailing_zeros() as usize;
            let frame_idx = chunk_idx * 64 + bit_in_chunk;

            if frame_idx >= self.total_frames {
                continue; // Past the end of tracked memory
            }

            // Mark the frame as used (set its bit to 1).
            unsafe {
                let byte = &mut *self.bitmap.add(frame_idx / 8);
                *byte |= 1 << (frame_idx % 8);
            }

            self.used_frames += 1;
            self.search_start = frame_idx + 1;

            return Some(PhysAddr::new(frame_idx as u64 * PAGE_SIZE));
        }

        None // Out of memory — all frames used
    }

    /// Frees a previously allocated physical frame.
    ///
    /// # Panics
    /// - If `addr` is not page-aligned.
    /// - If the frame index is out of range.
    /// - If the frame is not currently allocated (double-free detection).
    fn free_frame(&mut self, addr: PhysAddr) {
        assert!(addr.is_page_aligned(), "PMM: cannot free unaligned address {}", addr);

        let frame_idx = (addr.as_u64() / PAGE_SIZE) as usize;
        assert!(
            frame_idx < self.total_frames,
            "PMM: frame index {} out of range (max {})",
            frame_idx,
            self.total_frames
        );

        let byte_idx = frame_idx / 8;
        let bit_mask = 1u8 << (frame_idx % 8);

        unsafe {
            let byte = &mut *self.bitmap.add(byte_idx);
            assert!(
                *byte & bit_mask != 0,
                "PMM: double free detected at frame {} ({})",
                frame_idx,
                addr
            );
            *byte &= !bit_mask; // Clear bit → frame is now free
        }

        self.used_frames -= 1;

        // Move the search cursor back so this freed frame can be reused
        // quickly by the next allocation.
        if frame_idx < self.search_start {
            self.search_start = frame_idx;
        }
    }

    /// Allocates `count` physically contiguous frames.
    ///
    /// Used by the kernel heap to get a contiguous virtual mapping through
    /// HHDM (contiguous physical → contiguous virtual under HHDM).
    ///
    /// # Algorithm
    /// Linear scan for `count` consecutive zero bits. Not the fastest
    /// approach, but contiguous allocation is rare (heap init, DMA buffers).
    ///
    /// # Returns
    /// `Some(PhysAddr)` — base address of the first frame in the run.
    /// `None` — not enough contiguous free frames.
    fn alloc_contiguous(&mut self, count: usize) -> Option<PhysAddr> {
        if count == 0 {
            return None;
        }
        if count == 1 {
            return self.alloc_frame();
        }

        let mut run_start: usize = 0;
        let mut run_length: usize = 0;

        for frame in 0..self.total_frames {
            if is_frame_free(self.bitmap, frame) {
                if run_length == 0 {
                    run_start = frame;
                }
                run_length += 1;

                if run_length >= count {
                    // Found enough consecutive free frames. Mark them all used.
                    for f in run_start..run_start + count {
                        unsafe {
                            let byte = &mut *self.bitmap.add(f / 8);
                            *byte |= 1 << (f % 8);
                        }
                    }
                    self.used_frames += count;
                    return Some(PhysAddr::new(run_start as u64 * PAGE_SIZE));
                }
            } else {
                run_length = 0;
            }
        }

        None
    }

    /// Returns a snapshot of current physical memory statistics.
    fn stats(&self) -> MemoryStats {
        MemoryStats {
            total_frames: self.total_frames,
            used_frames: self.used_frames,
            free_frames: self.total_frames - self.used_frames,
            bitmap_bytes: self.bitmap_bytes,
        }
    }

    /// Allocates a single frame and zeros its contents.
    ///
    /// Useful for page table allocation — page tables must be zeroed
    /// (all entries marked non-present) before use.
    fn alloc_frame_zeroed(&mut self) -> Option<PhysAddr> {
        let frame = self.alloc_frame()?;
        // SAFETY: The frame is valid physical memory accessible via HHDM.
        unsafe {
            ptr::write_bytes(
                frame.to_virt().as_mut_ptr::<u8>(),
                0,
                PAGE_SIZE as usize,
            );
        }
        Some(frame)
    }
}

// =============================================================================
// Bitmap manipulation helpers
// =============================================================================

/// Sets bit `frame` in the bitmap (marks frame as used).
///
/// Returns 1 if the bit was previously clear (frame was free), 0 if it
/// was already set. This allows callers to correctly adjust `used_frames`.
#[inline]
fn set_bit(bitmap: *mut u8, frame: usize) -> usize {
    let byte_idx = frame / 8;
    let bit_mask = 1u8 << (frame % 8);
    unsafe {
        let byte = &mut *bitmap.add(byte_idx);
        if *byte & bit_mask == 0 {
            *byte |= bit_mask;
            1 // was free, now used
        } else {
            0 // already used
        }
    }
}

/// Returns `true` if the given frame is free (bit is 0).
#[inline]
fn is_frame_free(bitmap: *const u8, frame: usize) -> bool {
    let byte_idx = frame / 8;
    let bit_mask = 1u8 << (frame % 8);
    unsafe { *bitmap.add(byte_idx) & bit_mask == 0 }
}

/// Clears all bits in the range `[start_frame, end_frame)`.
///
/// Optimized for large ranges: handles unaligned head/tail bit-by-bit,
/// and clears aligned middle bytes whole-byte-at-a-time using popcount
/// to track how many bits were actually changed.
///
/// # Returns
/// The number of bits that were changed from 1 → 0.
fn clear_range(bitmap: *mut u8, start_frame: usize, end_frame: usize) -> usize {
    if start_frame >= end_frame {
        return 0;
    }

    let mut cleared = 0usize;
    let mut frame = start_frame;

    // --- Unaligned head: clear bits until we reach a byte boundary ---
    while frame < end_frame && (frame % 8) != 0 {
        unsafe {
            let byte = &mut *bitmap.add(frame / 8);
            let mask = 1u8 << (frame % 8);
            if *byte & mask != 0 {
                *byte &= !mask;
                cleared += 1;
            }
        }
        frame += 1;
    }

    // --- Aligned middle: clear whole bytes at a time ---
    // Each byte covers 8 frames. Count how many were set (popcount)
    // before zeroing the byte.
    while frame + 8 <= end_frame {
        let byte_idx = frame / 8;
        unsafe {
            let byte = &mut *bitmap.add(byte_idx);
            cleared += (*byte).count_ones() as usize;
            *byte = 0;
        }
        frame += 8;
    }

    // --- Unaligned tail: clear remaining bits ---
    while frame < end_frame {
        unsafe {
            let byte = &mut *bitmap.add(frame / 8);
            let mask = 1u8 << (frame % 8);
            if *byte & mask != 0 {
                *byte &= !mask;
                cleared += 1;
            }
        }
        frame += 1;
    }

    cleared
}

// =============================================================================
// Public API — module-level functions that acquire the spinlock
// =============================================================================

/// Initializes the physical memory manager from the Limine memory map.
///
/// Must be called exactly once during early boot (single-core, before
/// any allocations).
///
/// # Panics
/// - If no usable region is large enough for the bitmap.
/// - If called more than once.
pub fn init(memory_map: &[&limine::memory_map::Entry]) {
    let mut pmm = PMM.lock();
    assert!(pmm.is_none(), "PMM: init called more than once");
    *pmm = Some(BitmapAllocator::new(memory_map));
}

/// Allocates a single 4 KiB physical frame.
///
/// The returned address is page-aligned. The frame contents are
/// **uninitialized** — use `alloc_frame_zeroed()` if you need zeroed memory.
///
/// # Returns
/// `Some(PhysAddr)` on success, `None` if out of memory.
///
/// # Panics
/// If the PMM is not initialized.
pub fn alloc_frame() -> Option<PhysAddr> {
    PMM.lock()
        .as_mut()
        .expect("PMM: not initialized — call pmm::init() first")
        .alloc_frame()
}

/// Allocates a single 4 KiB physical frame, zeroed.
///
/// The returned frame is filled with zeros. Use this for page table
/// allocation (all entries must start as non-present = 0).
///
/// # Returns
/// `Some(PhysAddr)` on success, `None` if out of memory.
///
/// # Panics
/// If the PMM is not initialized.
pub fn alloc_frame_zeroed() -> Option<PhysAddr> {
    PMM.lock()
        .as_mut()
        .expect("PMM: not initialized — call pmm::init() first")
        .alloc_frame_zeroed()
}

/// Frees a previously allocated physical frame.
///
/// # Panics
/// - If the PMM is not initialized.
/// - If `addr` is not page-aligned.
/// - If the frame was not previously allocated (double-free).
pub fn free_frame(addr: PhysAddr) {
    PMM.lock()
        .as_mut()
        .expect("PMM: not initialized — call pmm::init() first")
        .free_frame(addr);
}

/// Allocates `count` physically contiguous frames.
///
/// Used for regions that require physical contiguity (DMA buffers,
/// kernel heap via HHDM).
///
/// # Returns
/// `Some(PhysAddr)` — base address of the first frame.
/// `None` — insufficient contiguous free frames.
///
/// # Panics
/// If the PMM is not initialized.
pub fn alloc_contiguous(count: usize) -> Option<PhysAddr> {
    PMM.lock()
        .as_mut()
        .expect("PMM: not initialized — call pmm::init() first")
        .alloc_contiguous(count)
}

/// Returns a snapshot of current physical memory statistics.
///
/// # Panics
/// If the PMM is not initialized.
pub fn stats() -> MemoryStats {
    PMM.lock()
        .as_ref()
        .expect("PMM: not initialized — call pmm::init() first")
        .stats()
}
