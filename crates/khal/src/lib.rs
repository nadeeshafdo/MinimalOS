//! Hardware Abstraction Layer.
#![no_std]

pub mod pic;
pub mod port;
pub mod serial;

pub use serial::Serial;
