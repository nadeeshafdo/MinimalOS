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

use crate::kprintln;
use crate::memory::address::PAGE_SIZE;
use crate::memory::pmm;
use crate::memory::pml4;

use core::sync::atomic::{AtomicU64, Ordering};

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
    /// Waiting for an event (IPC, timer, I/O).
    Blocked,
    /// Terminated, waiting for cleanup.
    Dead,
}

/// Thread Control Block — the kernel's representation of a thread.
#[repr(C)]
pub struct Thread {
    /// Unique thread ID.
    pub id: u64,
    /// Current state.
    pub state: ThreadState,
    /// Saved kernel RSP (set by switch_context when suspended).
    pub rsp: u64,
    /// Page table root (CR3 value). For kernel threads, this is KERNEL_PML4.
    pub cr3: u64,
    /// Base virtual address of the kernel stack (HHDM mapped).
    pub kernel_stack_base: u64,
    /// Size of the kernel stack in bytes.
    pub kernel_stack_size: usize,
    /// Name for debugging.
    pub name: [u8; 32],
    pub name_len: usize,
}

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
    pub fn new(
        name: &str,
        entry_fn: extern "C" fn(u64),
        arg: u64,
    ) -> Box<Thread> {
        let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed);

        // Allocate a kernel stack
        let stack_phys = pmm::alloc_contiguous(KERNEL_STACK_PAGES)
            .expect("Thread: failed to allocate kernel stack");
        let stack_base = stack_phys.to_virt().as_u64();
        let stack_size = KERNEL_STACK_PAGES * PAGE_SIZE as usize;
        let stack_top = stack_base + stack_size as u64;

        // Build the synthetic stack frame that switch_context expects
        // Layout (growing downward):
        //   [stack_top - 8]   thread_entry_trampoline  (return address for ret)
        //   [stack_top - 16]  r15 = 0
        //   [stack_top - 24]  r14 = arg
        //   [stack_top - 32]  r13 = entry_fn
        //   [stack_top - 40]  r12 = 0
        //   [stack_top - 48]  rbp = 0
        //   [stack_top - 56]  rbx = 0
        //   <- initial RSP saved in TCB
        let frame_ptr = stack_top as *mut u64;
        unsafe {
            // Return address — where `ret` in switch_context will jump
            *frame_ptr.offset(-1) = super::context::thread_entry_trampoline as *const () as u64;
            // Callee-saved registers (popped by switch_context in reverse)
            *frame_ptr.offset(-2) = 0;                     // r15
            *frame_ptr.offset(-3) = arg;                    // r14 = argument
            *frame_ptr.offset(-4) = entry_fn as u64;        // r13 = payload fn
            *frame_ptr.offset(-5) = 0;                      // r12
            *frame_ptr.offset(-6) = 0;                      // rbp
            *frame_ptr.offset(-7) = 0;                      // rbx
        }
        let initial_rsp = stack_top - 7 * 8; // 7 pushes of 8 bytes each

        let mut name_buf = [0u8; 32];
        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min(32);
        name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

        let thread = Box::new(Thread {
            id: tid,
            state: ThreadState::Ready,
            rsp: initial_rsp,
            cr3: pml4::KERNEL_PML4.load(Ordering::SeqCst),
            kernel_stack_base: stack_base,
            kernel_stack_size: stack_size,
            name: name_buf,
            name_len: copy_len,
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
