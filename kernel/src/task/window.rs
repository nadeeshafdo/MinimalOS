#![allow(dead_code)]
//! Window manager — [080] [082] [084].
//!
//! Kernel-side window tracking for the display compositor.
//! Each window has a pixel buffer mapped at a user-accessible virtual address.
//! The framebuffer info is also stored here for the SYS_FB_INFO syscall.

use crate::memory::{paging, pmm};
use core::sync::atomic::{AtomicU32, Ordering};

/// Maximum number of simultaneous windows.
pub const MAX_WINDOWS: usize = 8;

/// Next window ID counter.
static NEXT_WIN_ID: AtomicU32 = AtomicU32::new(1);

// ── Framebuffer info ([080]) ────────────────────────────────────

/// [080] Framebuffer information returned to user-space.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FbInfo {
	pub phys_addr: u64,
	pub width: u32,
	pub height: u32,
	pub pitch: u32,
	pub bpp: u32,
}

/// Global framebuffer info (set once during kernel init).
static FB_INFO: spin::Mutex<Option<FbInfo>> = spin::Mutex::new(None);

/// Store framebuffer info — called once from `main.rs` during boot.
pub fn set_fb_info(phys_addr: u64, width: u32, height: u32, pitch: u32, bpp: u32) {
	*FB_INFO.lock() = Some(FbInfo { phys_addr, width, height, pitch, bpp });
}

/// Get the stored framebuffer info.
pub fn get_fb_info() -> Option<FbInfo> {
	*FB_INFO.lock()
}

// ── Window struct ([082]) ───────────────────────────────────────

/// [082] A compositor window with a backing pixel buffer.
#[derive(Clone, Copy)]
pub struct Window {
	/// Unique window identifier.
	pub id: u32,
	/// Position on screen.
	pub x: i32,
	pub y: i32,
	/// Dimensions (client area, excluding title bar).
	pub width: u32,
	pub height: u32,
	/// Virtual address of the pixel buffer (USER_RW mapped).
	pub buffer_vaddr: u64,
	/// Number of 4 KiB pages backing the buffer.
	pub buffer_pages: u32,
	/// Dirty flag — set when content or position changed.
	pub dirty: bool,
	/// Z-order (higher = on top).
	pub z_order: u32,
	/// Whether this slot is in use.
	pub active: bool,
	/// Window title (UTF-8, up to 31 bytes + NUL).
	pub title: [u8; 32],
	/// Length of the title in bytes.
	pub title_len: u8,
}

impl Window {
	pub const fn empty() -> Self {
		Self {
			id: 0,
			x: 0,
			y: 0,
			width: 0,
			height: 0,
			buffer_vaddr: 0,
			buffer_pages: 0,
			dirty: false,
			z_order: 0,
			active: false,
			title: [0u8; 32],
			title_len: 0,
		}
	}
}

// ── Window info for user-space ([084]) ──────────────────────────

/// [084] Window information passed to user-space via SYS_WIN_LIST.
/// Matches the C layout so user-space can read it directly.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WindowInfo {
	pub id: u32,
	pub x: i32,
	pub y: i32,
	pub width: u32,
	pub height: u32,
	pub buffer_vaddr: u64,
	pub dirty: u32,
	pub z_order: u32,
	pub title: [u8; 32],
	pub title_len: u32,
}

/// Global window table.
static WINDOWS: spin::Mutex<[Window; MAX_WINDOWS]> =
	spin::Mutex::new([Window::empty(); MAX_WINDOWS]);

/// Base virtual address for window pixel buffers.
/// Each window gets up to 2 MiB at `WIN_BUFFER_BASE + slot * 0x20_0000`.
const WIN_BUFFER_BASE: u64 = 0x400_0000; // 64 MiB

// ── Window management ([084]) ───────────────────────────────────

/// [084] Create a new window, allocate its pixel buffer, and map it.
///
/// Returns `(window_id, buffer_vaddr)` on success.
pub fn create_window(x: i32, y: i32, w: u32, h: u32, title: &str) -> Option<(u32, u64)> {
	let mut windows = WINDOWS.lock();

	// Find a free slot.
	let slot = windows.iter().position(|win| !win.active)?;

	let id = NEXT_WIN_ID.fetch_add(1, Ordering::Relaxed);
	let buffer_size = (w as usize) * (h as usize) * 4; // ARGB, 4 bytes/pixel
	let num_pages = (buffer_size + 4095) / 4096;

	// Virtual address for this window's buffer.
	let buffer_vaddr = WIN_BUFFER_BASE + (slot as u64) * 0x20_0000;

	// Allocate and map pages.
	for i in 0..num_pages {
		let phys = pmm::alloc_frame()?;
		let vaddr = buffer_vaddr + (i as u64) * 4096;
		unsafe {
			paging::map_page(vaddr, phys, paging::PageFlags::USER_RW);
			core::ptr::write_bytes(vaddr as *mut u8, 0, 4096);
		}
	}

	// Copy title.
	let mut title_buf = [0u8; 32];
	let title_len = title.len().min(31);
	title_buf[..title_len].copy_from_slice(&title.as_bytes()[..title_len]);

	// Find the highest z_order among active windows.
	let max_z = windows
		.iter()
		.filter(|w| w.active)
		.map(|w| w.z_order)
		.max()
		.unwrap_or(0);

	windows[slot] = Window {
		id,
		x,
		y,
		width: w,
		height: h,
		buffer_vaddr,
		buffer_pages: num_pages as u32,
		dirty: true,
		z_order: max_z + 1,
		active: true,
		title: title_buf,
		title_len: title_len as u8,
	};

	klog::info!(
		"[082] Window {} created: {}x{} at ({},{}) buf={:#x}",
		id, w, h, x, y, buffer_vaddr
	);

	Some((id, buffer_vaddr))
}

/// [084] Mark a window as dirty (content or position changed).
pub fn mark_dirty(win_id: u32) {
	let mut windows = WINDOWS.lock();
	for w in windows.iter_mut() {
		if w.active && w.id == win_id {
			w.dirty = true;
			return;
		}
	}
}

/// Clear all dirty flags (called by compositor after a full redraw).
pub fn clear_all_dirty() {
	let mut windows = WINDOWS.lock();
	for w in windows.iter_mut() {
		if w.active {
			w.dirty = false;
		}
	}
}

/// Check if any window is dirty.
pub fn any_dirty() -> bool {
	let windows = WINDOWS.lock();
	windows.iter().any(|w| w.active && w.dirty)
}

/// Move a window to a new position.
pub fn move_window(win_id: u32, new_x: i32, new_y: i32) {
	let mut windows = WINDOWS.lock();
	for w in windows.iter_mut() {
		if w.active && w.id == win_id {
			w.x = new_x;
			w.y = new_y;
			w.dirty = true;
			return;
		}
	}
}

/// Write window info list to a user-space buffer.
///
/// Returns the number of windows written, sorted by z_order (low → high).
///
/// # Safety
/// `buf_ptr` must point to a valid array of at least `max_count` WindowInfo entries.
pub unsafe fn list_windows(buf_ptr: *mut WindowInfo, max_count: usize) -> usize {
	let windows = WINDOWS.lock();

	// Collect active windows sorted by z_order (back → front).
	let mut sorted: [Option<&Window>; MAX_WINDOWS] = [None; MAX_WINDOWS];
	let mut count = 0;
	for w in windows.iter() {
		if w.active && count < MAX_WINDOWS {
			sorted[count] = Some(w);
			count += 1;
		}
	}

	// Simple insertion sort by z_order.
	for i in 1..count {
		let mut j = i;
		while j > 0 {
			let a = sorted[j - 1].unwrap().z_order;
			let b = sorted[j].unwrap().z_order;
			if a > b {
				sorted.swap(j - 1, j);
			}
			j -= 1;
		}
	}

	let write_count = count.min(max_count);
	for i in 0..write_count {
		let w = sorted[i].unwrap();
		let info = WindowInfo {
			id: w.id,
			x: w.x,
			y: w.y,
			width: w.width,
			height: w.height,
			buffer_vaddr: w.buffer_vaddr,
			dirty: if w.dirty { 1 } else { 0 },
			z_order: w.z_order,
			title: w.title,
			title_len: w.title_len as u32,
		};
		core::ptr::write(buf_ptr.add(i), info);
	}
	write_count
}
