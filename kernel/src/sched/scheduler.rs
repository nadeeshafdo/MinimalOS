// =============================================================================
// MinimalOS NextGen — Tickless Scheduler
// =============================================================================
//
// A simple round-robin scheduler for Sprint 4. Per-core run queues are
// stored in CpuLocal (accessed via IA32_GS_BASE — no locking needed for
// the local queue).
//
// FLOW:
//   LAPIC timer fires → IDT stub → irq_dispatch → schedule()
//   schedule() picks the next Ready thread, calls switch_context()
//   switch_context() swaps RSP, execution continues on the new thread
//
// =============================================================================

extern crate alloc;
use alloc::collections::VecDeque;
use alloc::boxed::Box;

use crate::kprintln;

use super::thread::Thread;

/// Per-core run queue. Stored inside CpuLocal, one per core.
pub struct RunQueue {
    /// Ready threads waiting for CPU time (FIFO round-robin).
    pub ready: VecDeque<Box<Thread>>,
}

impl RunQueue {
    /// Creates an empty run queue.
    pub const fn new() -> Self {
        Self {
            ready: VecDeque::new(),
        }
    }

    /// Adds a thread to the back of the ready queue.
    pub fn push(&mut self, thread: Box<Thread>) {
        self.ready.push_back(thread);
    }

    /// Removes the next ready thread from the front of the queue.
    pub fn pop(&mut self) -> Option<Box<Thread>> {
        self.ready.pop_front()
    }

    /// Returns the number of threads in the ready queue.
    pub fn len(&self) -> usize {
        self.ready.len()
    }

    /// Returns true if the ready queue is empty.
    pub fn is_empty(&self) -> bool {
        self.ready.is_empty()
    }
}

/// The main scheduling function. Called from the LAPIC timer ISR.
///
/// Picks the next Ready thread from the run queue and context-switches to it.
/// The current thread is placed back in the Ready queue (round-robin).
///
/// # Safety
/// - Must be called with interrupts disabled (inside ISR context).
/// - CpuLocal must be initialized on this core.
pub unsafe fn schedule() {
    // For now, this is a simplified scheduler that will be expanded.
    // The full implementation requires CpuLocal access via GS base.
    // This stub exists to allow compilation and basic testing.
}

/// Spawns a new kernel thread and adds it to the BSP's run queue.
///
/// This is the user-facing API for creating threads during boot.
pub fn spawn(name: &str, entry: extern "C" fn(u64), arg: u64) {
    let thread = Thread::new(name, entry, arg);
    kprintln!("[sched] Spawned thread {} '{}'", thread.id, name);
    BOOT_QUEUE.lock().push(thread);
}

/// Temporary global queue used during boot before CpuLocal is initialized.
/// Will be drained into per-core run queues during scheduler startup.
static BOOT_QUEUE: crate::sync::spinlock::SpinLock<RunQueue> =
    crate::sync::spinlock::SpinLock::new(RunQueue::new());

/// Initializes the scheduler on the BSP.
///
/// Sets up the BSP's CpuLocal, creates the idle thread, and starts
/// scheduling any threads that were spawned during boot.
pub fn init() {
    kprintln!("[sched] Initializing scheduler on BSP");

    let ready_count = BOOT_QUEUE.lock().len();
    kprintln!("[sched] {} threads in boot queue", ready_count);
}

/// A test entry point for verifying context switching works.
pub extern "C" fn test_thread_a(_arg: u64) {
    for i in 0..5 {
        kprintln!("[thread-A] iteration {}", i);
        // Yield to let other threads run
        for _ in 0..1_000_000 { core::hint::spin_loop(); }
    }
}

/// A test entry point for verifying context switching works.
pub extern "C" fn test_thread_b(_arg: u64) {
    for i in 0..5 {
        kprintln!("[thread-B] iteration {}", i);
        for _ in 0..1_000_000 { core::hint::spin_loop(); }
    }
}
