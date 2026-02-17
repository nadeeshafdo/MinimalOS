//! Interrupt and trap handling.

mod handlers;
mod idt;

pub use idt::init_idt;
