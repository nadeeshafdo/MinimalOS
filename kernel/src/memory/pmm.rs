//! Bitmap-based Physical Memory Manager (PMM).
//!
//! Tracks 4 KiB page frames with a simple bitmap: bit **1** = used, bit **0** = free.
//! The bitmap itself is carved from the first usable region large enough to hold it.

use limine::memory_map::{Entry, EntryType};
use spin::Mutex;

/// Size of a single page frame.
const FRAME_SIZE: u64 = 4096;

/// Global PMM instance, initialised once at boot.
static PMM: Mutex<Option<BitmapAllocator>> = Mutex::new(None);

/// A bitmap-based physical frame allocator.
struct BitmapAllocator {
    /// Virtual address of the bitmap (accessed via HHDM).
    bitmap: *mut u8,
    /// Physical address where the bitmap is stored.
    bitmap_phys: u64,
    /// Number of 4 KiB frames consumed by the bitmap itself.
    bitmap_frames: usize,
    /// Total number of page frames tracked by the bitmap.
    total_frames: usize,
    /// Current number of free (allocatable) frames.
    free_frames: usize,
}

// Safety: the bitmap pointer is only accessed under the PMM lock.
unsafe impl Send for BitmapAllocator {}

/// Initialise the physical memory manager from the Limine memory map.
///
/// # Safety
///
/// Must be called exactly once during early kernel init, after HHDM is available.
pub unsafe fn init(hhdm_offset: u64, entries: &[&Entry]) {
    // ── 1. Determine the highest physical address we need to track ──
    //    Only consider USABLE regions; device MMIO above that is irrelevant.
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
    let mut bitmap_phys: u64 = 0;
    for entry in entries.iter() {
        if entry.entry_type == EntryType::USABLE && entry.length >= bitmap_size {
            bitmap_phys = entry.base;
            break;
        }
    }
    assert!(bitmap_phys != 0, "No usable region large enough for the PMM bitmap");

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
        bitmap_phys,
        bitmap_frames,
        total_frames,
        free_frames: free_count,
    });
}

/// Allocate a single 4 KiB physical frame.
///
/// Returns the **physical address** of the frame, or `None` if OOM.
pub fn alloc_frame() -> Option<u64> {
    let mut guard = PMM.lock();
    let alloc = guard.as_mut()?;

    let bitmap_bytes = (alloc.total_frames + 7) / 8;

    for byte_idx in 0..bitmap_bytes {
        let byte = unsafe { *alloc.bitmap.add(byte_idx) };
        if byte == 0xFF {
            continue; // all 8 frames in this byte are used
        }
        // Find the first zero bit
        let bit_idx = byte.trailing_ones() as usize; // index of first 0
        let frame = byte_idx * 8 + bit_idx;
        if frame >= alloc.total_frames {
            return None;
        }
        // Mark used
        unsafe {
            *alloc.bitmap.add(byte_idx) |= 1u8 << bit_idx;
        }
        alloc.free_frames -= 1;
        return Some(frame as u64 * FRAME_SIZE);
    }
    None
}

/// Free a previously allocated 4 KiB physical frame.
///
/// # Panics
///
/// Panics on double-free or out-of-range address.
pub fn free_frame(phys_addr: u64) {
    let mut guard = PMM.lock();
    let alloc = guard.as_mut().expect("PMM not initialised");

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

/// Return the current number of free frames.
pub fn free_frame_count() -> usize {
    PMM.lock().as_ref().map_or(0, |a| a.free_frames)
}
