// =============================================================================
// MinimalOS NextGen — IPC Endpoint (Synchronous Rendezvous)
// =============================================================================
//
// An Endpoint is a rendezvous point for synchronous IPC. Two threads
// communicate by one calling send() and the other calling recv() on the
// same Endpoint. The first to arrive blocks until the partner arrives.
//
// OWNERSHIP MODEL (the critical SMP correctness property):
//
//   When a thread blocks on an Endpoint, the Endpoint takes ownership of
//   the thread's Box<Thread>. This is NOT an optimization — it's a
//   correctness requirement:
//
//   1. The scheduler's RunQueue owns threads via Box<Thread>.
//   2. When a thread blocks, it leaves the RunQueue.
//   3. The thread's memory must be owned by EXACTLY ONE entity at all times.
//   4. If we used raw pointers in the Endpoint queues, the Box would be
//      dropped by the scheduler while the thread is asleep → use-after-free.
//   5. Therefore: Endpoint holds Box<Thread> for sleeping threads.
//
//   Ownership flow:
//     RunQueue ─pop→ schedule() ─into_raw→ CpuLocal.current_thread
//     IPC::send/recv ─from_raw→ Box<Thread> ─push→ Endpoint queue
//     Partner wakes → Endpoint ─pop→ Box<Thread> ─push→ RunQueue
//
// SMP SPINLOCK DEADLOCK PREVENTION:
//
//   The "lost wakeup" trap: if Thread A holds the Endpoint lock when it
//   context-switches away, Core 1 spinning on that lock will deadlock.
//
//   The fix (from the architecture review):
//     1. cli (disable interrupts) — protects CPU-local state
//     2. Lock Endpoint
//     3. Move thread into Endpoint queue
//     4. Unlock Endpoint
//     5. schedule() — runs with IF=0, context-switches safely
//     6. sti after schedule returns (thread has been woken)
//
//   Because interrupts are disabled on Core 0, even if Core 1 immediately
//   wakes Thread A and pushes it to Core 0's RunQueue, Core 0 will finish
//   the context switch before processing any timer interrupt.
//
// =============================================================================

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::VecDeque;

use crate::ipc::message::IpcMessage;
use crate::kprintln;
use crate::sched::percpu::CpuLocal;
use crate::sched::thread::{Thread, ThreadState};
use crate::sync::spinlock::SpinLock;

// =============================================================================
// Endpoint
// =============================================================================

/// Internal state protected by the Endpoint's spinlock.
struct EndpointInner {
    /// Threads blocked waiting to send (they have a message in ipc_buffer).
    blocked_senders: VecDeque<Box<Thread>>,

    /// Threads blocked waiting to receive (they need a message).
    blocked_receivers: VecDeque<Box<Thread>>,
}

/// An IPC Endpoint — the rendezvous point for synchronous message passing.
///
/// Threads send and receive messages through Endpoints. The communication
/// is synchronous: the first party to arrive blocks until the second arrives.
///
/// # Thread Safety
/// Protected by an internal SpinLock. The lock ordering (from sync/mod.rs)
/// places IPC endpoint locks at Level 3, below RunQueue locks (Level 6).
pub struct Endpoint {
    /// Unique endpoint identifier.
    id: u64,

    /// Protected internal state (sender/receiver queues).
    inner: SpinLock<EndpointInner>,
}

// SAFETY: Endpoint is designed for cross-core sharing. The SpinLock
// ensures mutual exclusion. Box<Thread> transfers are sound because
// only one entity owns each Thread at a time.
unsafe impl Send for Endpoint {}
unsafe impl Sync for Endpoint {}

impl Endpoint {
    /// Creates a new Endpoint with the given ID.
    ///
    /// This is const-constructable so Endpoints can be global statics.
    pub const fn new(id: u64) -> Self {
        Self {
            id,
            inner: SpinLock::new(EndpointInner {
                blocked_senders: VecDeque::new(),
                blocked_receivers: VecDeque::new(),
            }),
        }
    }

    /// Returns this endpoint's ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Send a message through this endpoint.
    ///
    /// If a receiver is already blocked and waiting, this is the **fastpath**:
    /// the message is copied directly into the receiver's IPC buffer, the
    /// receiver is woken (pushed to a RunQueue), and the sender returns
    /// immediately without blocking.
    ///
    /// If no receiver is waiting, this is the **slowpath**: the sender's
    /// message is stored in its ipc_buffer, the sender blocks (ownership
    /// transferred to the Endpoint), and schedule() is called to yield
    /// the CPU.
    ///
    /// # SMP Safety
    /// Interrupts are disabled before locking to prevent the lost-wakeup
    /// race. See module-level documentation for the full analysis.
    pub fn send(&self, msg: &IpcMessage) {
        // Step 1: Disable interrupts BEFORE locking.
        // This protects CPU-local state (RunQueue, current_thread) from
        // being corrupted by a timer ISR calling schedule() concurrently.
        unsafe { core::arch::asm!("cli", options(nomem, nostack)); }

        // Step 2: Lock the Endpoint.
        // SpinLock sees IF=0, saves irq_was_enabled=false.
        // When the guard drops, it restores IF to 0 (not re-enabled).
        let mut inner = self.inner.lock();

        if let Some(mut receiver) = inner.blocked_receivers.pop_front() {
            // ── FASTPATH: Receiver is already waiting ──
            //
            // Copy message directly into the receiver's IPC buffer.
            // This is safe because the receiver is asleep (blocked) and
            // we hold the Endpoint lock.
            receiver.ipc_buffer = *msg;
            receiver.state = ThreadState::Ready;

            let receiver_id = receiver.id;

            // Push the woken receiver to the current core's RunQueue.
            // (Future optimization: send IPI to the receiver's home core)
            let cpu_local = unsafe { CpuLocal::get_mut() };
            let rq = unsafe { &mut *cpu_local.run_queue };
            rq.push(receiver);

            // Unlock endpoint
            drop(inner);

            kprintln!("[ipc] EP{}: send fastpath — woke receiver thread {}",
                self.id, receiver_id);

            // Re-enable interrupts and return (sender continues running)
            unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
        } else {
            // ── SLOWPATH: No receiver — sender must block ──
            let cpu_local = unsafe { CpuLocal::get_mut() };
            let current_ptr = cpu_local.current_thread;

            // Take ownership of the current thread.
            // Reconstruct the Box from the raw pointer stored in CpuLocal.
            // This is valid because schedule() did Box::into_raw when it
            // started running this thread.
            let mut current_box = unsafe { Box::from_raw(current_ptr) };
            current_box.ipc_buffer = *msg; // Store message for receiver to read later
            current_box.state = ThreadState::BlockedSend;

            let sender_id = current_box.id;

            // Transfer ownership to the Endpoint.
            // The Endpoint now keeps this thread alive while it sleeps.
            inner.blocked_senders.push_back(current_box);

            // Unlock endpoint (IF stays 0 because SpinLock saved IF=0)
            drop(inner);

            kprintln!("[ipc] EP{}: send slowpath — thread {} blocking (no receiver)",
                self.id, sender_id);

            // Yield the CPU. schedule() sees BlockedSend state and will
            // NOT try to requeue this thread (ownership is in the Endpoint).
            // Interrupts are still disabled — schedule runs safely.
            unsafe { crate::sched::scheduler::schedule(); }

            // ── We return here when a receiver wakes us ──
            // Re-enable interrupts.
            unsafe { core::arch::asm!("sti", options(nomem, nostack)); }

            kprintln!("[ipc] EP{}: sender thread {} resumed after block", self.id, sender_id);
        }
    }

    /// Receive a message from this endpoint.
    ///
    /// If a sender is already blocked and waiting, this is the **fastpath**:
    /// the sender's message is copied to the receiver, the sender is woken,
    /// and the receiver returns immediately with the message.
    ///
    /// If no sender is waiting, this is the **slowpath**: the receiver blocks
    /// (ownership transferred to the Endpoint), and schedule() is called.
    /// When a sender eventually arrives, it copies its message into the
    /// receiver's ipc_buffer and wakes it.
    ///
    /// # Returns
    /// The received IPC message.
    pub fn recv(&self) -> IpcMessage {
        // Step 1: Disable interrupts
        unsafe { core::arch::asm!("cli", options(nomem, nostack)); }

        // Step 2: Lock the Endpoint
        let mut inner = self.inner.lock();

        if let Some(mut sender) = inner.blocked_senders.pop_front() {
            // ── FASTPATH: Sender is already waiting ──
            //
            // Copy the sender's message directly.
            let msg = sender.ipc_buffer;
            sender.ipc_buffer = IpcMessage::EMPTY; // Clear sender's buffer
            sender.state = ThreadState::Ready;

            let sender_id = sender.id;

            // Push the woken sender to the current core's RunQueue
            let cpu_local = unsafe { CpuLocal::get_mut() };
            let rq = unsafe { &mut *cpu_local.run_queue };
            rq.push(sender);

            // Unlock endpoint
            drop(inner);

            kprintln!("[ipc] EP{}: recv fastpath — woke sender thread {}, label={}",
                self.id, sender_id, msg.label);

            // Re-enable interrupts and return the message
            unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
            msg
        } else {
            // ── SLOWPATH: No sender — receiver must block ──
            let cpu_local = unsafe { CpuLocal::get_mut() };
            let current_ptr = cpu_local.current_thread;

            // Take ownership of the current thread
            let mut current_box = unsafe { Box::from_raw(current_ptr) };
            current_box.state = ThreadState::BlockedRecv;

            let receiver_id = current_box.id;

            // Transfer ownership to the Endpoint
            inner.blocked_receivers.push_back(current_box);

            // Unlock endpoint
            drop(inner);

            kprintln!("[ipc] EP{}: recv slowpath — thread {} blocking (no sender)",
                self.id, receiver_id);

            // Yield the CPU
            unsafe { crate::sched::scheduler::schedule(); }

            // ── We return here when a sender wakes us ──
            unsafe { core::arch::asm!("sti", options(nomem, nostack)); }

            // Re-read our ipc_buffer. The sender wrote to it while we slept.
            // We need to re-acquire CpuLocal because we may have been
            // migrated to a different core (Phase 1: single-core, but correct).
            let cpu_local = unsafe { CpuLocal::get() };
            let msg = unsafe { (*cpu_local.current_thread).ipc_buffer };

            kprintln!("[ipc] EP{}: receiver thread {} resumed, label={}",
                self.id, receiver_id, msg.label);

            msg
        }
    }
}
