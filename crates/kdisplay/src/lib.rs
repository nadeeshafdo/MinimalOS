//! Framebuffer graphics subsystem.
#![no_std]

use core::fmt;
use limine::framebuffer::Framebuffer;

/// PSF2 font header (32 bytes).
#[repr(C, packed)]
struct Psf2Header {
    magic: [u8; 4],      // 0x72, 0xb5, 0x4a, 0x86
    version: u32,        // 0
    headersize: u32,     // 32
    flags: u32,          // 0 or 1 (has unicode table)
    length: u32,         // number of glyphs
    charsize: u32,       // bytes per glyph
    height: u32,         // pixels
    width: u32,          // pixels
}

/// PSF2 font with embedded glyph data.
pub struct Psf2Font {
    header: &'static Psf2Header,
    glyphs: &'static [u8],
}

impl Psf2Font {
    /// Parse a PSF2 font from embedded bytes.
    /// 
    /// # Safety
    /// Font data must be valid PSF2 format and live for 'static.
    pub unsafe fn from_bytes(data: &'static [u8]) -> Result<Self, &'static str> {
        if data.len() < 32 {
            return Err("PSF2: data too small");
        }

        let header = &*(data.as_ptr() as *const Psf2Header);
        
        // Verify magic
        if header.magic[0] != 0x72 || header.magic[1] != 0xb5 
            || header.magic[2] != 0x4a || header.magic[3] != 0x86 {
            return Err("PSF2: invalid magic");
        }

        let glyphs_start = header.headersize as usize;
        let glyphs_size = (header.length * header.charsize) as usize;
        
        if data.len() < glyphs_start + glyphs_size {
            return Err("PSF2: truncated glyph data");
        }

        let glyphs = &data[glyphs_start..glyphs_start + glyphs_size];

        Ok(Self { header, glyphs })
    }

    /// Get glyph bitmap for a character.
    pub fn get_glyph(&self, ch: char) -> Option<&[u8]> {
        let index = ch as u32;
        if index >= self.header.length {
            return None;
        }

        let start = (index * self.header.charsize) as usize;
        let end = start + self.header.charsize as usize;
        Some(&self.glyphs[start..end])
    }

    pub fn width(&self) -> u32 {
        self.header.width
    }

    pub fn height(&self) -> u32 {
        self.header.height
    }

    pub fn bytes_per_glyph(&self) -> u32 {
        self.header.charsize
    }
}

/// Embedded PSF2 font data (16x30 Lat15-DejaVu).
static FONT_DATA: &[u8] = include_bytes!("../../../assets/font.psf");

/// Global font instance (lazily initialized).
static FONT: spin::Once<Psf2Font> = spin::Once::new();

/// Get the global font, initializing it if needed.
fn get_font() -> &'static Psf2Font {
    FONT.call_once(|| unsafe {
        Psf2Font::from_bytes(FONT_DATA).expect("Failed to load embedded font")
    })
}

/// Color represented as 32-bit RGBA.
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const WHITE: Color = Color::new(255, 255, 255);
    pub const BLACK: Color = Color::new(0, 0, 0);
    pub const RED: Color = Color::new(255, 0, 0);
    pub const GREEN: Color = Color::new(0, 255, 0);
    pub const BLUE: Color = Color::new(0, 0, 255);
}

/// Draw a single pixel at the given coordinates.
/// 
/// # Safety
/// Caller must ensure the framebuffer pointer is valid and the coordinates are within bounds.
pub unsafe fn draw_pixel(fb: &Framebuffer, x: usize, y: usize, color: Color) {
    if x >= fb.width() as usize || y >= fb.height() as usize {
        return; // Out of bounds
    }

    let pitch = fb.pitch() as usize;
    let bpp = fb.bpp() as usize / 8; // bytes per pixel
    let offset = y * pitch + x * bpp;

    let fb_ptr = fb.addr() as *mut u8;
    let pixel = fb_ptr.add(offset) as *mut u32;

    // Pack color as 0xAARRGGBB (assuming 32-bit framebuffer)
    let packed = ((color.a as u32) << 24)
        | ((color.r as u32) << 16)
        | ((color.g as u32) << 8)
        | (color.b as u32);

    pixel.write_volatile(packed);
}

/// Fill the entire screen with a single color.
/// 
/// # Safety
/// Caller must ensure the framebuffer pointer is valid.
pub unsafe fn fill_screen(fb: &Framebuffer, color: Color) {
    let width = fb.width() as usize;
    let height = fb.height() as usize;
    let pitch = fb.pitch() as usize;
    let _bpp = fb.bpp() as usize / 8;

    let fb_ptr = fb.addr() as *mut u8;
    
    // Pack color once
    let packed = ((color.a as u32) << 24)
        | ((color.r as u32) << 16)
        | ((color.g as u32) << 8)
        | (color.b as u32);

    // Fill each row
    for y in 0..height {
        let row_start = fb_ptr.add(y * pitch) as *mut u32;
        for x in 0..width {
            row_start.add(x).write_volatile(packed);
        }
    }
}

/// Draw a character glyph at the given coordinates using PSF2 font.
/// 
/// # Safety
/// Caller must ensure the framebuffer pointer is valid and coordinates are within bounds.
pub unsafe fn draw_char(fb: &Framebuffer, x: usize, y: usize, ch: char, color: Color) {
    let font = get_font();
    let glyph_data = match font.get_glyph(ch) {
        Some(g) => g,
        None => return, // Character not in font
    };

    let width = font.width() as usize;
    let height = font.height() as usize;
    let bytes_per_row = (width + 7) / 8; // Round up to nearest byte

    // Bounds check
    let fb_width = fb.width() as usize;
    let fb_height = fb.height() as usize;
    if x >= fb_width || y >= fb_height {
        return;
    }

    let pitch = fb.pitch() as usize;
    let bpp = fb.bpp() as usize / 8;
    let fb_ptr = fb.addr() as *mut u8;

    // Pack color once
    let packed = ((color.a as u32) << 24)
        | ((color.r as u32) << 16)
        | ((color.g as u32) << 8)
        | (color.b as u32);

    // Draw glyph using direct pointer arithmetic (avoid draw_pixel overhead)
    for row in 0..height {
        let py = y + row;
        if py >= fb_height {
            break;
        }

        let row_start = fb_ptr.add(py * pitch + x * bpp) as *mut u32;

        for col in 0..width {
            let px = x + col;
            if px >= fb_width {
                break;
            }

            let byte_index = row * bytes_per_row + col / 8;
            let bit_index = 7 - (col % 8);
            
            if byte_index < glyph_data.len() {
                let byte = glyph_data[byte_index];
                if (byte & (1 << bit_index)) != 0 {
                    row_start.add(col).write_volatile(packed);
                }
            }
        }
    }
}

/// Draw a string at the given coordinates.
/// Wraps to the next line if text exceeds screen width.
/// 
/// # Safety
/// Caller must ensure the framebuffer pointer is valid.
pub unsafe fn draw_string(fb: &Framebuffer, mut x: usize, mut y: usize, s: &str, color: Color) {
    let font = get_font();
    let char_width = font.width() as usize;
    let char_height = font.height() as usize;
    let fb_width = fb.width() as usize;
    let fb_height = fb.height() as usize;

    for ch in s.chars() {
        // Check if character would exceed screen width
        if x + char_width > fb_width {
            x = 0;
            y += char_height;
        }

        // Stop if we've run out of vertical space
        if y + char_height > fb_height {
            break;
        }

        draw_char(fb, x, y, ch, color);
        x += char_width;
    }
}

// ========================
// Console / kprint! system
// ========================

/// Framebuffer text console with cursor tracking and scrolling.
pub struct Console {
    fb_addr: usize,
    fb_width: usize,
    fb_height: usize,
    fb_pitch: usize,
    fb_bpp: usize,
    cursor_x: usize,  // pixel position
    cursor_y: usize,
    char_width: usize,
    char_height: usize,
    fg: Color,
    bg: Color,
}

impl Console {
    /// Create a new console from a Limine Framebuffer.
    ///
    /// # Safety
    /// The framebuffer address must remain valid for the console's lifetime.
    pub unsafe fn new(fb: &Framebuffer, fg: Color, bg: Color) -> Self {
        let font = get_font();
        Self {
            fb_addr: fb.addr() as usize,
            fb_width: fb.width() as usize,
            fb_height: fb.height() as usize,
            fb_pitch: fb.pitch() as usize,
            fb_bpp: fb.bpp() as usize / 8,
            cursor_x: 0,
            cursor_y: 0,
            char_width: font.width() as usize,
            char_height: font.height() as usize,
            fg,
            bg,
        }
    }

    /// Number of character columns on screen.
    pub fn cols(&self) -> usize {
        self.fb_width / self.char_width
    }

    /// Number of character rows on screen.
    pub fn rows(&self) -> usize {
        self.fb_height / self.char_height
    }

    /// Clear the entire screen with the background color.
    pub fn clear(&mut self) {
        let packed = self.pack_color(self.bg);
        let fb_ptr = self.fb_addr as *mut u8;

        for y in 0..self.fb_height {
            let row_start = unsafe { fb_ptr.add(y * self.fb_pitch) as *mut u32 };
            for x in 0..self.fb_width {
                unsafe { row_start.add(x).write_volatile(packed) };
            }
        }

        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    /// Write a single character at the current cursor position.
    pub fn put_char(&mut self, ch: char) {
        match ch {
            '\x08' => {
                // [042] Backspace: move cursor back and erase the character
                if self.cursor_x >= self.char_width {
                    self.cursor_x -= self.char_width;
                    self.draw_glyph(self.cursor_x, self.cursor_y, ' ');
                }
            }
            '\n' => {
                self.cursor_x = 0;
                self.cursor_y += self.char_height;
            }
            '\r' => {
                self.cursor_x = 0;
            }
            '\t' => {
                // Tab to next 4-column boundary
                let tab_stop = ((self.cursor_x / self.char_width / 4) + 1) * 4 * self.char_width;
                self.cursor_x = tab_stop;
                if self.cursor_x + self.char_width > self.fb_width {
                    self.cursor_x = 0;
                    self.cursor_y += self.char_height;
                }
            }
            ch => {
                // Wrap if we'd go off the right edge
                if self.cursor_x + self.char_width > self.fb_width {
                    self.cursor_x = 0;
                    self.cursor_y += self.char_height;
                }

                // Scroll if we'd go off the bottom
                if self.cursor_y + self.char_height > self.fb_height {
                    self.scroll_up();
                }

                self.draw_glyph(self.cursor_x, self.cursor_y, ch);
                self.cursor_x += self.char_width;
            }
        }

        // Scroll check after newlines too
        if self.cursor_y + self.char_height > self.fb_height {
            self.scroll_up();
        }
    }

    /// Scroll the entire screen up by one line of text.
    fn scroll_up(&mut self) {
        let fb_ptr = self.fb_addr as *mut u8;
        let row_bytes = self.char_height * self.fb_pitch;
        let total_bytes = self.fb_height * self.fb_pitch;

        unsafe {
            // Copy everything up by one text row
            core::ptr::copy(
                fb_ptr.add(row_bytes),
                fb_ptr,
                total_bytes - row_bytes,
            );

            // Clear the last text row with background color
            let packed = self.pack_color(self.bg);
            let clear_start_y = self.fb_height - self.char_height;
            for y in clear_start_y..self.fb_height {
                let row_start = fb_ptr.add(y * self.fb_pitch) as *mut u32;
                for x in 0..self.fb_width {
                    row_start.add(x).write_volatile(packed);
                }
            }
        }

        self.cursor_y -= self.char_height;
    }

    /// Draw a glyph at pixel position (x, y) with foreground on background.
    fn draw_glyph(&self, x: usize, y: usize, ch: char) {
        let font = get_font();
        let glyph_data = match font.get_glyph(ch) {
            Some(g) => g,
            None => return,
        };

        let width = self.char_width;
        let height = self.char_height;
        let bytes_per_row = (width + 7) / 8;
        let fb_ptr = self.fb_addr as *mut u8;
        let packed_fg = self.pack_color(self.fg);
        let packed_bg = self.pack_color(self.bg);

        for row in 0..height {
            let py = y + row;
            if py >= self.fb_height {
                break;
            }

            let row_start = unsafe { fb_ptr.add(py * self.fb_pitch + x * self.fb_bpp) as *mut u32 };

            for col in 0..width {
                if x + col >= self.fb_width {
                    break;
                }

                let byte_index = row * bytes_per_row + col / 8;
                let bit_index = 7 - (col % 8);

                let is_set = byte_index < glyph_data.len()
                    && (glyph_data[byte_index] & (1 << bit_index)) != 0;

                // Draw both foreground AND background pixels for proper text rendering
                let packed = if is_set { packed_fg } else { packed_bg };
                unsafe { row_start.add(col).write_volatile(packed) };
            }
        }
    }

    #[inline]
    fn pack_color(&self, c: Color) -> u32 {
        ((c.a as u32) << 24) | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for ch in s.chars() {
            self.put_char(ch);
        }
        Ok(())
    }
}

/// Global console instance, protected by a spinlock.
static CONSOLE: spin::Mutex<Option<Console>> = spin::Mutex::new(None);

/// Initialize the global framebuffer console.
///
/// # Safety
/// Must be called once with a valid framebuffer reference.
pub unsafe fn init_console(fb: &Framebuffer, fg: Color, bg: Color) {
    let mut console = Console::new(fb, fg, bg);
    console.clear();
    *CONSOLE.lock() = Some(console);
}

/// Write a formatted string to the framebuffer console.
pub fn console_write_fmt(args: fmt::Arguments) {
    use fmt::Write;
    if let Some(ref mut console) = *CONSOLE.lock() {
        let _ = console.write_fmt(args);
    }
}

/// Write a formatted string to the framebuffer console (interrupt-safe).
///
/// Uses `try_lock` instead of `lock` to avoid deadlocking when called
/// from an interrupt handler while the main thread holds the console lock.
/// Silently drops the output if the lock is not available.
pub fn console_try_write_fmt(args: fmt::Arguments) {
    use fmt::Write;
    if let Some(ref mut guard) = CONSOLE.try_lock() {
        if let Some(ref mut console) = **guard {
            let _ = console.write_fmt(args);
        }
    }
}

/// Handle a single keyboard character (interrupt-safe).
///
/// Prints printable characters and handles backspace.
/// Uses `try_lock` to avoid deadlock from interrupt context.
pub fn console_try_put_char(ch: char) {
    if let Some(ref mut guard) = CONSOLE.try_lock() {
        if let Some(ref mut console) = **guard {
            console.put_char(ch);
        }
    }
}

/// Print to the framebuffer console.
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ($crate::console_write_fmt(format_args!($($arg)*)));
}

/// Print to the framebuffer console with a newline.
#[macro_export]
macro_rules! kprintln {
    () => ($crate::kprint!("\n"));
    ($($arg:tt)*) => ($crate::kprint!("{}\n", format_args!($($arg)*)));
}

// ========================================
// [077] Software cursor (XOR sprite)
// ========================================

/// Framebuffer info stored for cursor drawing (set once during init).
static CURSOR_FB: spin::Mutex<Option<CursorFb>> = spin::Mutex::new(None);

/// Mouse position and visibility state.
static CURSOR_STATE: spin::Mutex<CursorState> = spin::Mutex::new(CursorState::new());

/// Minimal framebuffer info needed for cursor drawing.
struct CursorFb {
    addr: usize,
    width: usize,
    height: usize,
    pitch: usize,
}

/// Mouse cursor state.
struct CursorState {
    x: i32,
    y: i32,
    visible: bool,
}

impl CursorState {
    const fn new() -> Self {
        Self { x: 0, y: 0, visible: false }
    }
}

/// 12×19 arrow cursor bitmap (1 = white, 0 = transparent).
/// Classic Windows-style arrow pointer.
const CURSOR_W: usize = 12;
const CURSOR_H: usize = 19;
static CURSOR_BITMAP: [u16; CURSOR_H] = [
    0b1000_0000_0000_0000,
    0b1100_0000_0000_0000,
    0b1110_0000_0000_0000,
    0b1111_0000_0000_0000,
    0b1111_1000_0000_0000,
    0b1111_1100_0000_0000,
    0b1111_1110_0000_0000,
    0b1111_1111_0000_0000,
    0b1111_1111_1000_0000,
    0b1111_1111_1100_0000,
    0b1111_1111_1110_0000,
    0b1111_1111_1111_0000,
    0b1111_1111_0000_0000,
    0b1111_1100_0000_0000,
    0b1111_1100_0000_0000,
    0b1100_0110_0000_0000,
    0b0000_0110_0000_0000,
    0b0000_0011_0000_0000,
    0b0000_0011_0000_0000,
];

/// XOR mask colour — white XOR gives good contrast on most backgrounds.
const XOR_MASK: u32 = 0x00FF_FFFF;

/// Initialise the cursor subsystem with framebuffer info.
///
/// # Safety
/// Must be called after framebuffer is available.
pub unsafe fn init_cursor(fb: &Framebuffer) {
    *CURSOR_FB.lock() = Some(CursorFb {
        addr: fb.addr() as usize,
        width: fb.width() as usize,
        height: fb.height() as usize,
        pitch: fb.pitch() as usize,
    });

    let mut state = CURSOR_STATE.lock();
    state.x = fb.width() as i32 / 2;
    state.y = fb.height() as i32 / 2;
}

/// Draw or erase the cursor at its current position using XOR.
///
/// Because XOR is its own inverse, calling this twice at the same
/// position erases the cursor.
fn xor_cursor(fb: &CursorFb, x: i32, y: i32) {
    for row in 0..CURSOR_H {
        let py = y + row as i32;
        if py < 0 || py >= fb.height as i32 {
            continue;
        }
        let bits = CURSOR_BITMAP[row];
        for col in 0..CURSOR_W {
            let px = x + col as i32;
            if px < 0 || px >= fb.width as i32 {
                continue;
            }
            if bits & (1 << (15 - col)) != 0 {
                let offset = py as usize * fb.pitch + px as usize * 4;
                let ptr = (fb.addr + offset) as *mut u32;
                unsafe {
                    let old = ptr.read_volatile();
                    ptr.write_volatile(old ^ XOR_MASK);
                }
            }
        }
    }
}

/// Move the cursor by a relative delta and redraw.
///
/// Erases the cursor at the old position, clamps the new position to
/// the screen bounds, and redraws.  Safe to call from interrupt context
/// (uses `try_lock`).
pub fn cursor_move(dx: i16, dy: i16) {
    let fb_guard = CURSOR_FB.try_lock();
    let fb = match fb_guard.as_ref().and_then(|g| g.as_ref()) {
        Some(f) => f,
        None => return,
    };

    let mut state = match CURSOR_STATE.try_lock() {
        Some(s) => s,
        None => return,
    };

    // Erase old cursor.
    if state.visible {
        xor_cursor(fb, state.x, state.y);
    }

    // Update position with clamping.
    state.x = (state.x + dx as i32).clamp(0, fb.width as i32 - 1);
    // PS/2 dy is positive=up, screen y is positive=down, so subtract.
    state.y = (state.y - dy as i32).clamp(0, fb.height as i32 - 1);

    // Redraw at new position.
    state.visible = true;
    xor_cursor(fb, state.x, state.y);
}

/// Get the current cursor position.
pub fn cursor_position() -> (i32, i32) {
    let state = CURSOR_STATE.lock();
    (state.x, state.y)
}
