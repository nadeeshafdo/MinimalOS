//! x86 port I/O primitives.
//!
//! Provides `inb` and `outb` wrappers for x86 port-mapped I/O
//! using inline assembly.

/// Write a byte to an x86 I/O port.
///
/// # Safety
///
/// Writing to an arbitrary I/O port can have side effects on hardware.
/// The caller must ensure the port and value are valid.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// Read a byte from an x86 I/O port.
///
/// # Safety
///
/// Reading from an arbitrary I/O port can have side effects on hardware.
/// The caller must ensure the port is valid.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!(
        "in al, dx",
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}
