// =============================================================================
// MinimalOS NextGen — Thread Control Block (TCB)
// =============================================================================
//
// A Thread is the unit of CPU scheduling. Each thread has:
//   - A saved kernel stack pointer (RSP) for context switching
//   - A CR3 value for address space isolation (future: per-process)
//   - A kernel stack (allocated from PMM + HHDM)
//   - A state machine (Ready, Running, Blocked, Dead)
//
// DESIGN:
//   Kernel threads share the kernel's address space (same CR3).
//   User threads will have distinct CR3 values (Sprint 6).
//   The scheduler manipulates TCBs to decide which thread runs next.
//
// =============================================================================

use alloc::boxed::Box;

use crate::ipc::message::IpcMessage;
use crate::kprintln;
use crate::memory::address::PAGE_SIZE;
use crate::memory::pmm;

use core::sync::atomic::{AtomicU64, Ordering};

use super::process::Process;

/// Global thread ID counter.
static NEXT_TID: AtomicU64 = AtomicU64::new(1);

/// Number of pages per kernel thread stack (16 KiB = 4 pages).
const KERNEL_STACK_PAGES: usize = 4;

/// Thread states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// In a run queue, waiting for CPU time.
    Ready,
    /// Currently executing on a core.
    Running,
    /// Blocked on an IPC endpoint, waiting for a receiver.
    /// Ownership of the Box<Thread> is held by the Endpoint.
    BlockedSend,
    /// Blocked on an IPC endpoint, waiting for a sender.
    /// Ownership of the Box<Thread> is held by the Endpoint.
    BlockedRecv,
    /// Terminated, waiting for cleanup.
    Dead,
}

/// Thread Control Block — the kernel's representation of a thread.
///
/// Threads are the unit of scheduling. They do NOT own an address space
/// or capability table — those belong to the parent Process.
///
/// Each thread holds a raw pointer to its parent Process. Multiple threads
/// within the same process share the PML4 and CNode.
#[repr(C)]
pub struct Thread {
    /// Unique thread ID.
    pub id: u64,
    /// Current state.
    pub state: ThreadState,
    /// Saved kernel RSP (set by switch_context when suspended).
    pub rsp: u64,
    /// Base virtual address of the kernel stack (HHDM mapped).
    pub kernel_stack_base: u64,
    /// Size of the kernel stack in bytes.
    pub kernel_stack_size: usize,
    /// Name for debugging.
    pub name: [u8; 32],
    pub name_len: usize,

    /// Pointer to the parent Process.
    ///
    /// The Process owns the PML4 (address space) and CNode (capabilities).
    /// Kernel threads (user_rip == 0) point to the kernel pseudo-process.
    /// User threads point to a heap-allocated Process.
    ///
    /// SAFETY: This pointer must remain valid for the lifetime of the thread.
    /// The Process is heap-allocated and leaked (or reference-counted in the
    /// future). It must NOT be freed while any thread references it.
    pub process: *mut Process,

    /// IPC message buffer for send/recv.
    /// Senders write their message here before blocking (slowpath),
    /// or the kernel copies directly between buffers (fastpath).
    pub ipc_buffer: IpcMessage,

    // ─── Sprint 6: Userspace fields ─────────────────────────────────────────

    /// User-space entry point (RIP for iretq transition to Ring 3).
    /// Zero means this is a kernel-only thread.
    pub user_rip: u64,

    /// User-space stack pointer (top of allocated user stack).
    /// Used by the ring3_entry trampoline to build the iretq frame.
    pub user_rsp: u64,
}

// SAFETY: Thread contains a `*mut Process` raw pointer which is not inherently
// `Send`. We guarantee safety because:
//   1. Process objects are heap-allocated and leaked (`Box::into_raw`) — they
//      remain valid for the entire kernel lifetime.
//   2. The Process pointer is only read (never written) after Thread creation,
//      except by the owning core during a context switch.
//   3. CNode access through the process pointer is always done with interrupts
//      disabled (from within the SYSCALL handler or scheduler).
unsafe impl Send for Thread {}

impl Thread {
    /// Creates a new thread with its own kernel stack.
    ///
    /// The thread is initially in `Ready` state with a synthetic stack frame
    /// so that `switch_context` can switch to it and it will start executing
    /// at `entry_fn(arg)`.
    ///
    /// # Parameters
    /// - `name`: Human-readable name for debugging.
    /// - `entry_fn`: The function the thread will execute.
    /// - `arg`: Argument passed to `entry_fn` (via r14 in dummy frame).
    /// - `process`: Raw pointer to the parent Process that owns this thread.
    ///              The Process must live at least as long as the thread.
    pub fn new(
        name: &str,
        entry_fn: extern "C" fn(u64),
        arg: u64,
        process: *mut Process,
    ) -> Box<Thread> {
        let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed);

        // Allocate a kernel stack
        let stack_phys = pmm::alloc_contiguous(KERNEL_STACK_PAGES)
            .expect("Thread: failed to allocate kernel stack");
        let stack_base = stack_phys.to_virt().as_u64();
        let stack_size = KERNEL_STACK_PAGES * PAGE_SIZE as usize;
        let stack_top = stack_base + stack_size as u64;

        // Build the synthetic stack frame that switch_context expects.
        //
        // switch_context PUSHES in this order: rbx, rbp, r12, r13, r14, r15
        // switch_context POPS in reverse:      r15, r14, r13, r12, rbp, rbx, then ret
        //
        // Pops read from lowest address (RSP) upward:
        //   [rsp + 0]  = offset(-7) → pop r15
        //   [rsp + 8]  = offset(-6) → pop r14  (= arg for trampoline)
        //   [rsp + 16] = offset(-5) → pop r13  (= entry_fn for trampoline)
        //   [rsp + 24] = offset(-4) → pop r12
        //   [rsp + 32] = offset(-3) → pop rbp
        //   [rsp + 40] = offset(-2) → pop rbx
        //   [rsp + 48] = offset(-1) → ret      (= thread_entry_trampoline)
        //
        let frame_ptr = stack_top as *mut u64;
        unsafe {
            *frame_ptr.offset(-7) = 0;                                                           // r15
            *frame_ptr.offset(-6) = arg;                                                         // r14 = argument
            *frame_ptr.offset(-5) = entry_fn as *const () as u64;                                // r13 = payload fn
            *frame_ptr.offset(-4) = 0;                                                           // r12
            *frame_ptr.offset(-3) = 0;                                                           // rbp
            *frame_ptr.offset(-2) = 0;                                                           // rbx
            *frame_ptr.offset(-1) = super::context::thread_entry_trampoline as *const () as u64; // ret target
        }
        let initial_rsp = stack_top - 7 * 8; // 7 slots × 8 bytes

        let mut name_buf = [0u8; 32];
        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min(32);
        name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

        let thread = Box::new(Thread {
            id: tid,
            state: ThreadState::Ready,
            rsp: initial_rsp,
            kernel_stack_base: stack_base,
            kernel_stack_size: stack_size,
            name: name_buf,
            name_len: copy_len,
            process,
            ipc_buffer: IpcMessage::EMPTY,
            user_rip: 0,
            user_rsp: 0,
        });

        kprintln!("[thread] Created thread {} '{}' (stack={:#018X}—{:#018X}, rsp={:#018X})",
            tid, name, stack_base, stack_top, initial_rsp);

        thread
    }

    /// Returns the thread name as a string slice.
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("???")
    }
}

/// Called when a thread's entry function returns.
/// Marks the thread as Dead and yields to the scheduler.
///
/// This is referenced by `thread_entry_trampoline` — if the payload returns,
/// we reach here.
#[unsafe(no_mangle)]
pub extern "C" fn thread_exit() {
    kprintln!("[thread] Thread exited — marking Dead");
    // TODO: Mark current thread as Dead, yield to scheduler
    // For now, just halt — we'll implement proper cleanup with the scheduler
    loop {
        crate::arch::cpu::halt();
    }
}
