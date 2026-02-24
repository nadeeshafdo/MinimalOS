// =============================================================================
// MinimalOS NextGen — Kernel Logger (kprint! / kprintln!)
// =============================================================================
//
// Provides formatted text output for the kernel, similar to Linux's printk().
// Output goes to:
//   1. Serial port (COM1) — always, from the earliest boot
//   2. Framebuffer console — after framebuffer is initialized
//
// WHY NOT USE THE `log` CRATE DIRECTLY?
//   The `log` crate requires a global logger to be set at runtime, which
//   needs heap allocation. We need output BEFORE the heap is initialized.
//   Our macros work from the very first instruction of kmain().
//
// DESIGN:
//   - kprint!() / kprintln!() always output to serial
//   - When the framebuffer console is initialized, they also output there
//   - This is controlled by a static flag + function pointer pattern
//   - The macros use Rust's format_args!() for zero-allocation formatting
//
// USAGE:
//   kprintln!("Hello, {}!", "world");
//   kprintln!("Memory: {} MB free", free_pages * 4096 / 1024 / 1024);
//   kprint!("Loading..."); // No newline
//   kprintln!(" done!");
//
// THREAD SAFETY:
//   The serial port is protected by a SpinLock. Multiple cores calling
//   kprintln!() simultaneously will serialize their output (no interleaving).
//   Each kprintln!() call is atomic — you won't see mixed characters from
//   different cores, but the ORDER of messages from different cores is
//   non-deterministic.
//
// =============================================================================

use crate::arch::serial::SERIAL;
use core::fmt;
use core::fmt::Write;

/// The internal print function that sends formatted text to serial output.
///
/// This is not meant to be called directly — use the `kprint!()` and
/// `kprintln!()` macros instead.
///
/// # Arguments
/// - `args`: Format arguments created by `format_args!()` macro.
///
/// # How it works
/// 1. Acquires the serial port spinlock (disabling interrupts)
/// 2. Writes the formatted text to serial
/// 3. Releases the lock (restoring interrupts)
///
/// The lock ensures that a complete message is written atomically — no
/// interleaving from other cores or interrupt handlers.
#[doc(hidden)]
pub fn _kprint(args: fmt::Arguments) {
    // Acquire the serial port lock. This disables interrupts on the
    // current core to prevent deadlock if an interrupt handler also
    // tries to print.
    let mut serial = SERIAL.lock();
    let _ = serial.write_fmt(args);

    // TODO: When framebuffer console is initialized, also write there.
    // This will be added after the framebuffer driver is implemented.
}

/// Prints formatted text to the kernel console (serial + framebuffer).
///
/// Works exactly like `print!()` in standard Rust, but outputs to serial
/// and framebuffer instead of stdout.
///
/// # Examples
/// ```
/// kprint!("Loading");
/// kprint!(".");
/// kprint!(".");
/// kprintln!(" done!"); // "Loading... done!\n"
/// ```
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {
        $crate::util::logger::_kprint(format_args!($($arg)*))
    };
}

/// Prints formatted text followed by a newline to the kernel console.
///
/// Works exactly like `println!()` in standard Rust.
///
/// # Examples
/// ```
/// kprintln!();                          // Just a newline
/// kprintln!("Hello!");                  // Simple string
/// kprintln!("x = {}", 42);             // Formatted
/// kprintln!("addr = {:#018X}", 0xDEAD); // Hex formatted
/// ```
#[macro_export]
macro_rules! kprintln {
    () => {
        $crate::kprint!("\n")
    };
    ($($arg:tt)*) => {
        $crate::kprint!("{}\n", format_args!($($arg)*))
    };
}
