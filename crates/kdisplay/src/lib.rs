//! Framebuffer graphics subsystem.
#![no_std]

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
    let bpp = fb.bpp() as usize / 8;

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

    for row in 0..height {
        for col in 0..width {
            let byte_index = row * bytes_per_row + col / 8;
            let bit_index = 7 - (col % 8);
            
            if byte_index < glyph_data.len() {
                let byte = glyph_data[byte_index];
                if (byte & (1 << bit_index)) != 0 {
                    draw_pixel(fb, x + col, y + row, color);
                }
            }
        }
    }
}

/// Draw a string at the given coordinates.
/// 
/// # Safety
/// Caller must ensure the framebuffer pointer is valid.
pub unsafe fn draw_string(fb: &Framebuffer, mut x: usize, y: usize, s: &str, color: Color) {
    let char_width = get_font().width() as usize;
    for ch in s.chars() {
        draw_char(fb, x, y, ch, color);
        x += char_width;
    }
}
