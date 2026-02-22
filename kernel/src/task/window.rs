#![allow(dead_code)]
//! Minimal framebuffer info storage.
//!
//! Stores the physical address and geometry of the boot framebuffer
//! so that the capability engine can hand it to Wasm UI actors.
//! This is the *only* display knowledge the kernel retains.

/// Framebuffer information.
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

