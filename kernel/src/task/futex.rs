//! [073] Futex — fast user-space mutex support.
//!
//! Provides two operations:
//!   - `FUTEX_WAIT(addr, expected)` — if `*addr == expected`, block the
//!	 calling process until another process calls `FUTEX_WAKE` on `addr`.
//!   - `FUTEX_WAKE(addr, count)` — wake up to `count` processes blocked
//!	 on `addr`.
//!
//! The kernel uses the `Blocked` process state and a `wait_addr` field
//! on each Process to track which address a blocked task is waiting on.
//! Wake scans the scheduler queue for matching blocked tasks.

use crate::task::process::{ProcessState, SCHEDULER};

/// Futex operation: wait if `*addr == expected`.
pub const FUTEX_WAIT: u64 = 0;
/// Futex operation: wake up to `count` waiters on `addr`.
pub const FUTEX_WAKE: u64 = 1;

/// Perform a futex WAIT.
///
/// Reads the u64 at `addr`; if it equals `expected`, marks the
/// current process as Blocked (with `wait_addr = addr`) and yields.
///
/// Returns 0 if the process was woken, or `u64::MAX` if `*addr != expected`
/// (spurious / contention resolved without sleeping).
///
/// # Safety
/// `addr` must be a valid, aligned pointer to a u64 in user memory.
pub unsafe fn futex_wait(addr: u64, expected: u64) -> u64 {
	// Atomic-ish compare: read the user word.
	// Because we're single-CPU and interrupts are masked during
	// syscall entry, this read + state change is effectively atomic.
	let ptr = addr as *const u64;
	let current_val = unsafe { core::ptr::read_volatile(ptr) };

	if current_val != expected {
		// Value already changed — no need to sleep.
		return u64::MAX;
	}

	// Block the current process on this address.
	{
		let mut sched = SCHEDULER.lock();
		if let Some(current) = sched.current_mut() {
			current.state = ProcessState::Blocked;
			current.wait_addr = addr;
		}
	}

	// Yield to the scheduler — we won't run again until woken.
	unsafe { crate::task::process::do_schedule() };

	0 // woken up
}

/// Perform a futex WAKE.
///
/// Scans the scheduler queue for processes blocked on `addr` and
/// marks up to `count` of them as Ready.
///
/// Returns the number of processes actually woken.
pub fn futex_wake(addr: u64, count: u64) -> u64 {
	let mut woken: u64 = 0;
	let mut sched = SCHEDULER.lock();

	for task in sched.tasks_iter_mut() {
		if woken >= count {
			break;
		}
		if task.state == ProcessState::Blocked && task.wait_addr == addr {
			task.state = ProcessState::Ready;
			task.wait_addr = 0;
			woken += 1;
		}
	}

	woken
}
