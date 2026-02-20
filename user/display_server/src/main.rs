//! MinimalOS Display Server — a pure compositor.
//!
//! Responsibilities:
//!   - Map the hardware framebuffer via `SYS_MMAP` ([080])
//!   - Maintain a list of windows (kernel-managed pixel buffers) ([082])
//!   - Composite window buffers to the framebuffer back-to-front
//!     using the Painter's Algorithm ([085])
//!   - Track dirty state to skip unnecessary redraws ([086])
//!   - Provide the `blit_rect` fast-copy primitive ([081])
//!
//! What this is NOT:
//!   - No window decorations (title bars, borders) — that's the WM's job.
//!   - No input handling (dragging, focus) — that's the WM's job.
//!   - No application content rendering — that's each client's job.
//!
//! The display server simply blits every registered window buffer onto
//! the framebuffer every frame, in z-order, and fills uncovered areas
//! with a solid background colour.

#![no_std]
#![no_main]
#![allow(dead_code)]

use core::arch::asm;

// ─────────────────────────────────────────────────────────────
// Syscall numbers (must match kernel/src/arch/syscall.rs)
// ─────────────────────────────────────────────────────────────

const SYS_LOG: u64 = 0;
const SYS_EXIT: u64 = 1;
const SYS_YIELD: u64 = 2;
const SYS_TIME: u64 = 9;
const SYS_SLEEP: u64 = 10;
const SYS_FB_INFO: u64 = 15;
const SYS_MMAP: u64 = 16;
const SYS_WIN_LIST: u64 = 19;

// ─────────────────────────────────────────────────────────────
// Syscall wrappers
// ─────────────────────────────────────────────────────────────

#[inline(always)]
unsafe fn syscall0(nr: u64) -> u64 {
let ret: u64;
asm!("syscall",
inlateout("rax") nr => ret,
lateout("rcx") _, lateout("r11") _,
options(nostack));
ret
}

#[inline(always)]
unsafe fn syscall1(nr: u64, a0: u64) -> u64 {
let ret: u64;
asm!("syscall",
inlateout("rax") nr => ret,
in("rdi") a0,
lateout("rcx") _, lateout("r11") _,
options(nostack));
ret
}

#[inline(always)]
unsafe fn syscall2(nr: u64, a0: u64, a1: u64) -> u64 {
let ret: u64;
asm!("syscall",
inlateout("rax") nr => ret,
in("rdi") a0, in("rsi") a1,
lateout("rcx") _, lateout("r11") _,
options(nostack));
ret
}

#[inline(always)]
unsafe fn syscall3(nr: u64, a0: u64, a1: u64, a2: u64) -> u64 {
let ret: u64;
asm!("syscall",
inlateout("rax") nr => ret,
in("rdi") a0, in("rsi") a1, in("rdx") a2,
lateout("rcx") _, lateout("r11") _,
options(nostack));
ret
}

// ─────────────────────────────────────────────────────────────
// High-level syscall helpers
// ─────────────────────────────────────────────────────────────

fn log(msg: &str) {
unsafe { syscall2(SYS_LOG, msg.as_ptr() as u64, msg.len() as u64); }
}

fn exit(code: u64) -> ! {
unsafe { syscall1(SYS_EXIT, code); }
loop { core::hint::spin_loop(); }
}

fn time() -> u64 {
unsafe { syscall0(SYS_TIME) }
}

fn sleep(ticks: u64) {
unsafe { syscall1(SYS_SLEEP, ticks); }
}

// ─────────────────────────────────────────────────────────────
// Framebuffer info struct (matches kernel's FbInfo)
// ─────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
struct FbInfo {
phys_addr: u64,
width: u32,
height: u32,
pitch: u32,
bpp: u32,
}

fn get_fb_info() -> Option<FbInfo> {
let mut info = FbInfo { phys_addr: 0, width: 0, height: 0, pitch: 0, bpp: 0 };
let ret = unsafe { syscall1(SYS_FB_INFO, &mut info as *mut FbInfo as u64) };
if ret == 0 { Some(info) } else { None }
}

fn mmap(vaddr: u64, num_pages: u64, phys: u64) -> u64 {
unsafe { syscall3(SYS_MMAP, vaddr, num_pages, phys) }
}

// ─────────────────────────────────────────────────────────────
// Window info struct (matches kernel's WindowInfo)
// ─────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
struct WindowInfo {
id: u32,
x: i32,
y: i32,
width: u32,
height: u32,
buffer_vaddr: u64,
dirty: u32,
z_order: u32,
title: [u8; 32],
title_len: u32,
}

fn win_list(buf: &mut [WindowInfo]) -> usize {
unsafe {
syscall2(
SYS_WIN_LIST,
buf.as_mut_ptr() as u64,
buf.len() as u64,
) as usize
}
}

// ─────────────────────────────────────────────────────────────
// Pixel / blit helpers ([081])
// ─────────────────────────────────────────────────────────────

/// [081] Fast rectangular blit using u64 (8-byte) word copies.
///
/// Copies `w × h` pixels from `src` buffer (pitch `src_pitch` bytes)
/// to `dst` buffer (pitch `dst_pitch` bytes) at offsets given by pointers.
#[inline(never)]
unsafe fn blit_rect(
src: *const u32, src_pitch: usize,
dst: *mut u32,   dst_pitch: usize,
w: usize, h: usize,
) {
let src_stride = src_pitch / 4;
let dst_stride = dst_pitch / 4;

for row in 0..h {
let s = src.add(row * src_stride);
let d = dst.add(row * dst_stride);

let pairs = w / 2;
let s8 = s as *const u64;
let d8 = d as *mut u64;
for i in 0..pairs {
d8.add(i).write(s8.add(i).read());
}
if w & 1 != 0 {
d.add(w - 1).write(s.add(w - 1).read());
}
}
}

/// Fill a rectangle in the framebuffer with a solid colour.
unsafe fn fill_rect(dst: *mut u32, pitch: usize, x: usize, y: usize, w: usize, h: usize, color: u32) {
let stride = pitch / 4;
for row in 0..h {
let base = dst.add((y + row) * stride + x);
for col in 0..w {
base.add(col).write(color);
}
}
}

// ─────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────

/// Where we map the framebuffer in user virtual address space.
const FB_VADDR: u64 = 0x200_0000; // 32 MiB

/// Maximum number of windows we query per frame.
const MAX_WINDOWS: usize = 16;

/// Desktop background colour (dark teal).
const BG_COLOR: u32 = 0xFF1A3A3A;

// ─────────────────────────────────────────────────────────────
// Main compositor loop ([083])
// ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn _start() -> ! {
log("[083] Display server starting...");

// ── [080] Get framebuffer info and map it ────────────────
let fb = match get_fb_info() {
Some(f) => f,
None => {
log("[083] ERROR: no framebuffer info");
exit(1);
}
};

let fb_w = fb.width as usize;
let fb_h = fb.height as usize;
let fb_pitch = fb.pitch as usize;
let fb_pages = (fb_h * fb_pitch + 4095) / 4096;

log("[080] Mapping framebuffer into user space...");
let ret = mmap(FB_VADDR, fb_pages as u64, fb.phys_addr);
if ret != 0 {
log("[080] ERROR: mmap failed for framebuffer");
exit(1);
}
log("[080] Framebuffer mapped OK");

let fb_ptr = FB_VADDR as *mut u32;

// ── Initial full-screen clear ────────────────────────────
unsafe { fill_rect(fb_ptr, fb_pitch, 0, 0, fb_w, fb_h, BG_COLOR); }

// ── Compositor state ─────────────────────────────────────
let mut needs_full_redraw = true;
let mut win_buf = [WindowInfo {
id: 0, x: 0, y: 0, width: 0, height: 0,
buffer_vaddr: 0, dirty: 0, z_order: 0,
title: [0u8; 32], title_len: 0,
}; MAX_WINDOWS];
let mut prev_count: usize = 0;

log("[083] Entering compositor loop...");

loop {
// ── Query window list from kernel ────────────────────
// The kernel returns windows sorted by z_order (back -> front).
let wcount = win_list(&mut win_buf);

// If the window count changed, we need a full redraw
// (a window was created or destroyed).
if wcount != prev_count {
needs_full_redraw = true;
prev_count = wcount;
}

// [086] Check if any window is dirty.
let any_dirty = needs_full_redraw
|| (0..wcount).any(|i| win_buf[i].dirty != 0);

if any_dirty {
unsafe {
// [085] Painter's Algorithm: draw back -> front.

// Step 1: Clear the entire framebuffer to the desktop
// background.  Only on full redraws; when only window
// content changed, the blit overwrites the relevant
// pixels.
if needs_full_redraw {
fill_rect(fb_ptr, fb_pitch, 0, 0, fb_w, fb_h, BG_COLOR);
}

// Step 2: Blit each window's pixel buffer onto the
// framebuffer, back to front.
for i in 0..wcount {
let w = &win_buf[i];
let wx = w.x;
let wy = w.y;
let ww = w.width as usize;
let wh = w.height as usize;

// Skip windows that are fully off-screen.
if wx >= fb_w as i32 || wy >= fb_h as i32 {
continue;
}
if wx + ww as i32 <= 0 || wy + wh as i32 <= 0 {
continue;
}

// Compute clipped source and destination regions.
let src_x = if wx < 0 { (-wx) as usize } else { 0 };
let src_y = if wy < 0 { (-wy) as usize } else { 0 };
let dst_x = if wx < 0 { 0 } else { wx as usize };
let dst_y = if wy < 0 { 0 } else { wy as usize };
let draw_w = (ww - src_x).min(fb_w - dst_x);
let draw_h = (wh - src_y).min(fb_h - dst_y);

if draw_w == 0 || draw_h == 0 { continue; }

// [081] Blit the visible portion.
let src = (w.buffer_vaddr as *const u32)
.add(src_y * ww + src_x);
let dst = fb_ptr.add(dst_y * (fb_pitch / 4) + dst_x);
blit_rect(
src, ww * 4,
dst, fb_pitch,
draw_w, draw_h,
);
}
}

needs_full_redraw = false;
}

// Yield CPU — sleep a few ticks to avoid burning cycles.
sleep(2);
}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
log("PANIC in display_server!");
exit(1);
}
