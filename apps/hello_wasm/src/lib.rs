//! hello_wasm — Minimal WebAssembly module for MinimalOS
//!
//! Exports a single `add(a, b) -> i32` function.
//! This is the first foreign-architecture binary executed inside Ring 3.

#![no_std]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// Pure addition — the simplest possible exported Wasm function.
#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a + b
}
