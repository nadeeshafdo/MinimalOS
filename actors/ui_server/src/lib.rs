#![no_std]
#![no_main]

use actor_sdk as sdk;

const FB_CAP: u64 = 2; // Slot 2 will be the Framebuffer

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    sdk::log!("UI Server Actor started. Blitting white square...");

    // A small 100x100 white square (40,000 bytes = 10,000 pixels × 4 bytes BGRA)
    let square = [0xFFu8; 40_000];

    // Assuming 1280x800x32bpp, offset 0 is top-left
    unsafe {
        sdk::sys_cap_mem_write(
            FB_CAP as i64,
            0,                        // offset in framebuffer
            square.as_ptr() as i32,
            square.len() as i32,
        );
    }

    sdk::log!("UI Server: Blit complete.");
    unsafe { sdk::sys_exit(0); }
}
