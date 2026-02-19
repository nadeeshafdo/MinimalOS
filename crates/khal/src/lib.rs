//! Hardware Abstraction Layer.
#![no_std]

pub mod apic;
pub mod ioapic;
pub mod keyboard;
pub mod mouse;
pub mod pic;
pub mod port;
pub mod ramdisk;
pub mod serial;

pub use serial::Serial;
