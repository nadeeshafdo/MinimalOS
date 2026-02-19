//! Kernel tick counter — [072].
//!
//! Monotonically increasing counter incremented on every APIC Timer
//! tick.  Used by `sys_time()` and `sys_sleep()`.

use core::sync::atomic::{AtomicU64, Ordering};

/// Global tick counter.
static TICKS: AtomicU64 = AtomicU64::new(0);

/// Called by the timer interrupt handler on every tick.
#[inline]
pub fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// Return the current tick count.
#[inline]
pub fn now() -> u64 {
    TICKS.load(Ordering::Relaxed)
}
