#![no_std]
#![no_main]

use actor_sdk as sdk;
use sdk::{log, Message};

// ── Capability Slots ────────────────────────────────────────────
/// Endpoint to VFS actor (slot 1, seeded by kernel post-spawn).
const EP_VFS: i64 = 1;
/// Framebuffer Memory capability (slot 2, seeded by kernel, WRITE|GRANT).
const FB_CAP: i64 = 2;
/// Endpoint to Shell actor (slot 3, seeded by kernel post-spawn).
const _EP_SHELL: i64 = 3;

// ── Framebuffer layout (1280×800×32bpp BGRA) ────────────────────
const FB_WIDTH: usize = 1280;
const FB_HEIGHT: usize = 800;
const FB_BPP: usize = 4;
const FB_PITCH: usize = FB_WIDTH * FB_BPP; // 5120 bytes per row

// ── PSF v2 header magic ─────────────────────────────────────────
const PSF2_MAGIC: u32 = 0x864a_b572;

/// Parsed PSF v2 font header.
struct PsfFont {
    header_size: u32,
    num_glyph: u32,
    bytes_per_glyph: u32,
    height: u32,
    width: u32,
    /// Capability slot holding the font data (ramdisk mem cap).
    mem_cap: i64,
    /// Byte offset within the ramdisk where font.psf starts.
    file_offset: i32,
}

/// Pack a filename into the 24-byte `data` field of a Message.
fn pack_filename(name: &[u8]) -> [u64; 3] {
    let mut buf = [0u8; 24];
    let len = name.len().min(23);
    buf[..len].copy_from_slice(&name[..len]);
    let d0 = u64::from_le_bytes([buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]]);
    let d1 = u64::from_le_bytes([buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]]);
    let d2 = u64::from_le_bytes([buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23]]);
    [d0, d1, d2]
}

/// Load font.psf from VFS via IPC.  Returns a `PsfFont` on success.
fn load_font() -> Option<PsfFont> {
    log!("UI: requesting font.psf from VFS...");

    // Send VFS_READ_REQ for "font.psf".
    // data[2] = 3 tells VFS to reply on its EP→UI slot (slot 3).
    let mut data = pack_filename(b"font.psf");
    data[2] = 3; // reply hint: VFS slot 3 = EP→UI
    let req = Message {
        label: sdk::VFS_READ_REQ,
        data,
        cap_grant: 0,
        cap_perms: 0,
        _pad: 0,
    };
    let res = unsafe { sdk::sys_cap_send(EP_VFS, &req as *const Message as i32) };
    if res != 0 {
        log!("UI: ERROR — sys_cap_send to VFS failed ({})", res);
        return None;
    }

    // Block waiting for VFS reply.
    let mut reply = Message::empty();
    let res = unsafe { sdk::sys_cap_recv(&mut reply as *mut Message as i32) };
    if res != 0 {
        log!("UI: ERROR — sys_cap_recv failed ({})", res);
        return None;
    }
    if reply.label != sdk::VFS_READ_REPLY {
        log!("UI: ERROR — unexpected reply label {}", reply.label);
        return None;
    }

    let file_offset = reply.data[0] as i32;
    let file_size = reply.data[1] as usize;
    let mem_cap = reply.cap_grant as i64;

    log!("UI: VFS replied — offset={}, size={}, cap={}", file_offset, file_size, mem_cap);

    if file_size < 32 {
        log!("UI: ERROR — font.psf too small ({} bytes)", file_size);
        return None;
    }

    // Read PSF v2 header (32 bytes).
    let mut hdr = [0u8; 32];
    let res = unsafe { sdk::sys_cap_mem_read(mem_cap, file_offset, hdr.as_mut_ptr() as i32, 32) };
    if res != 0 {
        log!("UI: ERROR — failed to read PSF header ({})", res);
        return None;
    }

    let magic = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
    if magic != PSF2_MAGIC {
        log!("UI: ERROR — bad PSF magic: {:#x}", magic);
        return None;
    }

    let header_size = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]);
    let num_glyph = u32::from_le_bytes([hdr[16], hdr[17], hdr[18], hdr[19]]);
    let bytes_per_glyph = u32::from_le_bytes([hdr[20], hdr[21], hdr[22], hdr[23]]);
    let height = u32::from_le_bytes([hdr[24], hdr[25], hdr[26], hdr[27]]);
    let width = u32::from_le_bytes([hdr[28], hdr[29], hdr[30], hdr[31]]);

    log!("UI: PSF font loaded — {}x{}, {} glyphs, {} bytes/glyph",
         width, height, num_glyph, bytes_per_glyph);

    Some(PsfFont {
        header_size,
        num_glyph,
        bytes_per_glyph,
        height,
        width,
        mem_cap,
        file_offset,
    })
}

/// Blit a single character glyph to the framebuffer at pixel (x, y).
fn blit_char(font: &PsfFont, ch: u8, x: usize, y: usize) {
    if ch as u32 >= font.num_glyph {
        return; // unprintable
    }

    // Calculate glyph offset within the font file.
    let glyph_offset = font.file_offset
        + font.header_size as i32
        + (ch as i32) * (font.bytes_per_glyph as i32);

    // Read the glyph bitmap (up to 128 bytes — enough for 32×32 font).
    let bpg = font.bytes_per_glyph as usize;
    if bpg > 128 {
        return;
    }
    let mut bitmap = [0u8; 128];
    let res = unsafe {
        sdk::sys_cap_mem_read(font.mem_cap, glyph_offset, bitmap.as_mut_ptr() as i32, bpg as i32)
    };
    if res != 0 {
        return; // silently skip on error
    }

    // Bytes per row in the PSF bitmap (ceil(width / 8)).
    let row_bytes = ((font.width as usize) + 7) / 8;

    // White pixel in BGRA format.
    let white: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

    // Blit each row of the glyph.
    for row in 0..(font.height as usize) {
        let py = y + row;
        if py >= FB_HEIGHT {
            break;
        }

        // Build a row of pixels: 4 bytes per pixel × glyph width.
        // Max glyph width 32 → 128 bytes.
        let mut row_buf = [0u8; 128];
        let mut pixels_written = 0usize;

        for col in 0..(font.width as usize) {
            let px = x + col;
            if px >= FB_WIDTH {
                break;
            }
            let byte_idx = row * row_bytes + col / 8;
            let bit = 7 - (col % 8);
            if bitmap[byte_idx] & (1 << bit) != 0 {
                // Set pixel (white).
                let off = col * FB_BPP;
                row_buf[off] = white[0];
                row_buf[off + 1] = white[1];
                row_buf[off + 2] = white[2];
                row_buf[off + 3] = white[3];
            }
            // else: leave transparent (black / 0x00000000)
            pixels_written = col + 1;
        }

        // Write this row to the framebuffer.
        let fb_offset = (py * FB_PITCH + x * FB_BPP) as i32;
        let write_len = (pixels_written * FB_BPP) as i32;
        if write_len > 0 {
            unsafe {
                sdk::sys_cap_mem_write(FB_CAP, fb_offset, row_buf.as_ptr() as i32, write_len);
            }
        }
    }
}

/// Render a string at pixel position (x, y), advancing the cursor.
fn draw_text(font: &PsfFont, text: &[u8], start_x: usize, start_y: usize) {
    let glyph_w = font.width as usize;
    let glyph_h = font.height as usize;
    let mut cx = start_x;
    let mut cy = start_y;

    for &ch in text {
        if ch == 0 {
            break;
        }
        if ch == b'\n' {
            cx = start_x;
            cy += glyph_h;
            continue;
        }
        if cx + glyph_w > FB_WIDTH {
            cx = start_x;
            cy += glyph_h;
        }
        if cy + glyph_h > FB_HEIGHT {
            break; // off-screen
        }
        blit_char(font, ch, cx, cy);
        cx += glyph_w;
    }
}

/// Unpack text from Message.data (24 bytes, null-terminated).
fn unpack_text(data: &[u64; 3]) -> [u8; 24] {
    let mut buf = [0u8; 24];
    let b0 = data[0].to_le_bytes();
    let b1 = data[1].to_le_bytes();
    let b2 = data[2].to_le_bytes();
    buf[0..8].copy_from_slice(&b0);
    buf[8..16].copy_from_slice(&b1);
    buf[16..24].copy_from_slice(&b2);
    buf
}

// ── Cursor state (persistent across draw requests) ──────────────
static mut CURSOR_X: usize = 8;
static mut CURSOR_Y: usize = 8;

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    log!("UI Server Actor started.");

    // ── Phase 1: Load font from VFS ────────────────────────────
    let font = match load_font() {
        Some(f) => f,
        None => {
            log!("UI: FATAL — could not load font.psf, exiting.");
            unsafe { sdk::sys_exit(1); }
            loop {}
        }
    };

    log!("UI: Font ready. Entering draw service loop...");

    // ── Phase 2: Service loop — wait for UI_DRAW_REQ ───────────
    loop {
        let mut msg = Message::empty();
        let res = unsafe { sdk::sys_cap_recv(&mut msg as *mut Message as i32) };
        if res != 0 {
            log!("UI: recv error ({})", res);
            continue;
        }

        match msg.label {
            sdk::UI_DRAW_REQ => {
                let text = unpack_text(&msg.data);
                // Find text length (null-terminated).
                let len = text.iter().position(|&b| b == 0).unwrap_or(24);
                if len > 0 {
                    if let Ok(s) = core::str::from_utf8(&text[..len]) {
                        log!("UI: Rendering text -> '{}'", s);
                    }
                    let cx = unsafe { CURSOR_X };
                    let cy = unsafe { CURSOR_Y };
                    draw_text(&font, &text[..len], cx, cy);

                    // Advance cursor Y by one line.
                    unsafe {
                        CURSOR_Y += font.height as usize;
                        if CURSOR_Y + font.height as usize > FB_HEIGHT {
                            CURSOR_Y = 8; // wrap
                        }
                    }
                }
            }
            other => {
                log!("UI: unknown message label {}", other);
            }
        }
    }
}
