// =============================================================================
// MinimalOS NextGen — Ticket Spinlock
// =============================================================================
//
// A ticket spinlock provides mutual exclusion in a multi-core kernel.
// It's the simplest fair lock: threads acquire the lock in FIFO order,
// preventing starvation.
//
// HOW IT WORKS:
//   - Two counters: `next_ticket` and `now_serving`
//   - To lock: atomically increment `next_ticket`, get your ticket number.
//     Spin until `now_serving` equals your ticket.
//   - To unlock: increment `now_serving`, which lets the next waiter proceed.
//
// WHY TICKET SPINLOCK (not test-and-set)?
//   - Fair: threads are served in arrival order (FIFO)
//   - No starvation: every thread eventually gets the lock
//   - Predictable: bounded wait time proportional to number of waiters
//   - Test-and-set has thundering herd problems on N3710's shared L2 cache
//
// IRQ SAFETY:
//   When we acquire a spinlock, we MUST disable interrupts on the current
//   core first. Otherwise:
//     1. Thread A holds lock L with interrupts enabled
//     2. Interrupt fires on same core
//     3. Interrupt handler tries to acquire lock L
//     4. DEADLOCK — the handler spins forever because Thread A can't release
//        the lock until the handler returns
//
//   We save the previous interrupt state (RFLAGS.IF) so we can restore it
//   exactly on unlock — nested lock/unlock pairs work correctly.
//
// PERFORMANCE ON N3710:
//   - 4 cores sharing 2MB L2 cache
//   - Spinlock state fits in one cache line (2 × u32 = 8 bytes)
//   - PAUSE instruction in spin loop reduces bus contention and power
//   - PAUSE on Airmont (N3710): ~5 cycle delay, hints CPU to save power
//
// =============================================================================

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

/// A ticket-based spinlock that disables interrupts while held.
///
/// This lock is suitable for protecting shared kernel data structures
/// in a multi-core environment. It guarantees FIFO ordering of waiters.
///
/// # Type Parameter
/// - `T`: The data protected by the lock. Must be `Send` because ownership
///   effectively transfers between cores when the lock is acquired.
///
/// # Examples
/// ```
/// static COUNTER: SpinLock<u64> = SpinLock::new(0);
///
/// // In some kernel function:
/// {
///     let mut guard = COUNTER.lock();
///     *guard += 1;
/// } // Lock automatically released when guard goes out of scope
/// ```
pub struct SpinLock<T> {
    /// The next ticket to be dispensed (atomically incremented by each locker).
    next_ticket: AtomicU32,

    /// The ticket number currently being served (incremented on unlock).
    now_serving: AtomicU32,

    /// The protected data. UnsafeCell is required because we mutate through
    /// a shared reference (the lock ensures exclusive access at runtime).
    data: UnsafeCell<T>,
}

// SAFETY: SpinLock<T> can be shared between threads (sent across core boundaries)
// as long as T itself can be sent between threads. The lock ensures that only
// one core accesses T at a time.
unsafe impl<T: Send> Send for SpinLock<T> {}
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Creates a new spinlock wrapping the given value.
    ///
    /// The lock is initially unlocked (next_ticket == now_serving == 0).
    /// This is a const fn so spinlocks can be used in statics:
    /// ```
    /// static MY_LOCK: SpinLock<Vec<u8>> = SpinLock::new(Vec::new());
    /// ```
    pub const fn new(value: T) -> Self {
        Self {
            next_ticket: AtomicU32::new(0),
            now_serving: AtomicU32::new(0),
            data: UnsafeCell::new(value),
        }
    }

    /// Acquires the lock, disabling interrupts on the current core.
    ///
    /// Returns a `SpinLockGuard` that provides `Deref`/`DerefMut` access
    /// to the protected data. The lock is automatically released (and
    /// interrupts restored) when the guard is dropped.
    ///
    /// This function will spin (busy-wait) if the lock is held by another
    /// core. On the N3710, the PAUSE instruction is used to reduce power
    /// consumption and bus contention during spinning.
    ///
    /// # Interrupt Safety
    /// Interrupts are disabled BEFORE attempting to acquire the lock.
    /// The previous interrupt state is saved and restored on unlock.
    /// This means:
    ///   - If interrupts were enabled → they're disabled during lock, re-enabled on unlock
    ///   - If interrupts were already disabled → they stay disabled after unlock
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        // Step 1: Save current interrupt state and disable interrupts.
        // We read RFLAGS to check if IF (Interrupt Flag, bit 9) is set.
        let irq_was_enabled = interrupts_enabled();
        disable_interrupts();

        // Step 2: Take a ticket number atomically.
        // Relaxed ordering is fine here — the spin loop below provides
        // the necessary synchronization barrier.
        let my_ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);

        // Step 3: Spin until our ticket is being served.
        // Acquire ordering ensures we see all writes made by the previous
        // lock holder before we access the protected data.
        while self.now_serving.load(Ordering::Acquire) != my_ticket {
            // PAUSE instruction: tells the CPU we're in a spin loop.
            // On N3710 (Airmont):
            //   - Reduces power consumption during spinning
            //   - Prevents memory order violation pipeline flushes
            //   - ~5 cycle delay, which is ideal for short critical sections
            core::hint::spin_loop();
        }

        // Step 4: Lock acquired! Return the guard.
        SpinLockGuard {
            lock: self,
            irq_was_enabled,
        }
    }

    /// Attempts to acquire the lock without spinning.
    ///
    /// Returns `Some(guard)` if the lock was immediately available,
    /// or `None` if the lock is currently held by another core.
    ///
    /// Useful in interrupt handlers where spinning is dangerous:
    /// if the interrupted code holds the lock, try_lock fails immediately
    /// instead of deadlocking.
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        let irq_was_enabled = interrupts_enabled();
        disable_interrupts();

        let current = self.now_serving.load(Ordering::Relaxed);
        // Try to atomically take the next ticket, but only if it equals
        // the currently-served ticket (meaning the lock is free).
        let result = self.next_ticket.compare_exchange(
            current,
            current + 1,
            Ordering::Acquire,
            Ordering::Relaxed,
        );

        match result {
            Ok(_) => Some(SpinLockGuard {
                lock: self,
                irq_was_enabled,
            }),
            Err(_) => {
                // Lock is held — restore interrupt state and fail.
                if irq_was_enabled {
                    enable_interrupts();
                }
                None
            }
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// This is safe because `&mut self` guarantees exclusive access
    /// at compile time — no lock needed. Useful during initialization
    /// before the lock is shared between cores.
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
}

/// RAII guard for a held spinlock.
///
/// While this guard exists:
///   - The holder has exclusive access to the protected data
///   - Interrupts are disabled on the holder's core
///   - Other cores trying to lock() will spin
///
/// When the guard is dropped (goes out of scope, or explicitly via `drop()`):
///   - The lock is released (now_serving incremented)
///   - Interrupts are restored to their previous state
///
/// This follows the RAII pattern — you can never forget to unlock because
/// the compiler ensures `drop()` is called.
pub struct SpinLockGuard<'a, T> {
    /// Reference to the lock we're guarding.
    lock: &'a SpinLock<T>,

    /// Whether interrupts were enabled before we acquired the lock.
    /// Used to restore the correct state on unlock.
    irq_was_enabled: bool,
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    /// Provides read access to the protected data.
    fn deref(&self) -> &T {
        // SAFETY: We hold the lock, so we have exclusive access.
        // No other core can access the data while we hold the guard.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    /// Provides write access to the protected data.
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: We hold the lock, so we have exclusive access.
        // No other core can access the data while we hold the guard.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    /// Releases the lock and restores the previous interrupt state.
    ///
    /// This increments `now_serving`, which allows the next waiter
    /// (with the next ticket number) to proceed.
    fn drop(&mut self) {
        // Release ordering ensures all our writes to the protected data
        // are visible to the next lock holder before they see the
        // incremented `now_serving` value.
        self.lock.now_serving.fetch_add(1, Ordering::Release);

        // Restore interrupt state. If interrupts were enabled before
        // we took the lock, re-enable them now.
        if self.irq_was_enabled {
            enable_interrupts();
        }
    }
}

// =============================================================================
// Interrupt state management
// =============================================================================
//
// These functions directly manipulate the x86_64 RFLAGS register to
// control interrupts. They compile down to single instructions (STI/CLI)
// with no function call overhead in release builds.
// =============================================================================

/// Checks whether interrupts are currently enabled on this core.
///
/// Reads the RFLAGS register and checks bit 9 (IF — Interrupt Flag).
/// When IF is set, the CPU will respond to maskable external interrupts.
#[inline(always)]
fn interrupts_enabled() -> bool {
    let rflags: u64;
    // SAFETY: Reading RFLAGS is always safe — it's a read-only observation
    // of the current CPU state. The `pushfq` instruction pushes RFLAGS
    // onto the stack, and we pop it into our variable.
    unsafe {
        core::arch::asm!(
            "pushfq",      // Push RFLAGS onto stack
            "pop {}",      // Pop into our variable
            out(reg) rflags,
            options(nomem, preserves_flags)
        );
    }
    // Bit 9 is the Interrupt Flag (IF)
    rflags & (1 << 9) != 0
}

/// Disables maskable interrupts on the current core.
///
/// Executes the CLI (Clear Interrupt Flag) instruction.
/// After this, the current core will not respond to maskable interrupts
/// (NMI and machine checks can still fire).
#[inline(always)]
fn disable_interrupts() {
    // SAFETY: Disabling interrupts is safe in kernel code.
    // We always re-enable them when dropping the SpinLockGuard.
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
    }
}

/// Enables maskable interrupts on the current core.
///
/// Executes the STI (Set Interrupt Flag) instruction.
/// After this, the CPU will respond to maskable interrupts again.
/// Note: the CPU guarantees that the instruction AFTER STI executes
/// before any pending interrupt is delivered.
#[inline(always)]
fn enable_interrupts() {
    // SAFETY: Re-enabling interrupts is safe — we only do this when
    // restoring the previous state after releasing a lock.
    unsafe {
        core::arch::asm!("sti", options(nomem, nostack));
    }
}
