//! Interrupt and trap handling.

mod handlers;
mod idt;

pub use idt::init_idt;

/// Trigger a breakpoint exception for testing.
#[inline]
pub fn trigger_breakpoint() {
    unsafe {
        core::arch::asm!("int3");
    }
}
