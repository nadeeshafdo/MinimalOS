//! Framebuffer graphics subsystem.
#![no_std]

use limine::framebuffer::Framebuffer;

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

/// 8x8 bitmap font data.
/// Each byte represents one row of the glyph (bit 7 = leftmost pixel).
pub mod font {
    /// Bitmap for letter 'A' (8x8).
    pub const LETTER_A: [u8; 8] = [
        0b00111100,  // __####__
        0b01100110,  // _##__##_
        0b01100110,  // _##__##_
        0b01111110,  // _######_
        0b01100110,  // _##__##_
        0b01100110,  // _##__##_
        0b01100110,  // _##__##_
        0b00000000,  // ________
    ];
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
