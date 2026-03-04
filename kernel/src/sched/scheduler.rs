// =============================================================================
// MinimalOS NextGen — Preemptive Round-Robin Scheduler
// =============================================================================
//
// FLOW:
//   LAPIC timer fires → IDT stub (IF=0) → irq_dispatch:
//     1. Send EOI immediately (before schedule!)
//     2. Call schedule()
//     3. Return early (skip second EOI)
//
//   schedule():
//     1. Read CpuLocal via gs:0
//     2. Pop next thread from run queue
//     3. Requeue current thread (if Running)
//     4. Update CpuLocal.current_thread
//     5. Call switch_context(prev_rsp, next_rsp)
//     6. switch_context returns when this thread is resumed later
//
// =============================================================================

extern crate alloc;
use alloc::collections::VecDeque;
use alloc::boxed::Box;



use crate::kprintln;
use crate::sched::thread::{Thread, ThreadState};
use crate::sched::context;
use crate::sched::percpu::CpuLocal;
use crate::sched::process::Process;
use crate::ipc::message::IpcMessage;

/// Per-core run queue. One per core, stored via raw pointer in CpuLocal.
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

    /// Removes the next ready thread from the front.
    pub fn pop(&mut self) -> Option<Box<Thread>> {
        self.ready.pop_front()
    }

    /// Number of threads in the ready queue.
    pub fn len(&self) -> usize {
        self.ready.len()
    }

    /// True if no threads are ready.
    pub fn is_empty(&self) -> bool {
        self.ready.is_empty()
    }
}

/// Temporary global queue for threads spawned before CpuLocal is ready.
static BOOT_QUEUE: crate::sync::spinlock::SpinLock<RunQueue> =
    crate::sync::spinlock::SpinLock::new(RunQueue::new());

/// Spawns a new kernel thread and adds it to the boot queue.
///
/// # Parameters
/// - `name`: Human-readable name for debugging.
/// - `entry`: The function the thread will execute.
/// - `arg`: Argument passed to `entry` (via r14 in dummy frame).
/// - `process`: Raw pointer to the parent Process that owns this thread.
pub fn spawn(name: &str, entry: extern "C" fn(u64), arg: u64, process: *mut Process) {
    let thread = Thread::new(name, entry, arg, process);
    kprintln!("[sched] Spawned thread {} '{}'", thread.id, name);
    BOOT_QUEUE.lock().push(thread);
}

/// Enqueues a pre-built thread (e.g., from syscall::spawn_user) into the boot queue.
///
/// The thread must be fully configured (CNode capabilities, user_rip, user_rsp)
/// before calling this. Used for user threads that need custom setup before spawning.
pub fn spawn_thread(thread: Box<Thread>) {
    kprintln!("[sched] Spawned thread {} '{}' (pre-built)",
        thread.id, thread.name_str());
    BOOT_QUEUE.lock().push(thread);
}

/// Initializes the scheduler on the BSP.
///
/// 1. Allocates a RunQueue for the BSP's CpuLocal
/// 2. Creates a "BSP main" thread to represent the currently executing context
/// 3. Drains BOOT_QUEUE into the BSP's local run queue
/// 4. Arms the LAPIC timer for preemptive scheduling
/// Initializes the scheduler on the BSP.
///
/// # Parameters
/// - `kernel_process`: Raw pointer to the kernel pseudo-process (PID 0).
///   The BSP main thread will be assigned to this process.
pub fn init(kernel_process: *mut Process) {
    kprintln!("[sched] Initializing preemptive scheduler on BSP");

    // 1. Allocate a per-core RunQueue on the heap, leak it into CpuLocal
    let rq = Box::new(RunQueue::new());
    let rq_ptr = Box::into_raw(rq);

    // 2. Create the "BSP main" thread — represents the current execution context.
    //    This thread doesn't need a synthetic stack frame because it IS the running
    //    context. Its RSP will be saved by switch_context when it gets preempted.
    let bsp_thread = Box::new(Thread {
        id: 0,
        state: ThreadState::Running,
        rsp: 0, // Will be filled by switch_context on first preemption
        kernel_stack_base: 0, // Using Limine's original stack
        kernel_stack_size: 0,
        name: {
            let mut buf = [0u8; 32];
            let name = b"bsp-main";
            buf[..name.len()].copy_from_slice(name);
            buf
        },
        name_len: 8,
        process: kernel_process,
        ipc_buffer: IpcMessage::EMPTY,
        user_rip: 0,
        user_rsp: 0,
    });
    // Convert to raw pointer via the canonical API — Box::into_raw.
    // schedule() will later reconstruct via Box::from_raw to requeue.
    let bsp_thread_ptr = Box::into_raw(bsp_thread);

    // 3. Install RunQueue and current thread into CpuLocal
    unsafe {
        let cpu_local = CpuLocal::get_mut();
        cpu_local.run_queue = rq_ptr;
        cpu_local.current_thread = bsp_thread_ptr;
        cpu_local.online = true;
    }

    // 4. Drain BOOT_QUEUE into the BSP's local run queue
    {
        let mut boot_q = BOOT_QUEUE.lock();
        let mut drained = 0u32;
        while let Some(thread) = boot_q.pop() {
            unsafe { (*rq_ptr).push(thread); }
            drained += 1;
        }
        kprintln!("[sched] Drained {} threads from boot queue to BSP run queue", drained);
    }

    // 5. Arm the LAPIC timer for periodic preemption (10ms quantum)
    crate::arch::lapic::set_timer_oneshot(10_000); // 10ms
    kprintln!("[sched] LAPIC timer armed (10ms quantum)");
    kprintln!("[sched] Preemptive scheduler active on BSP");
}

/// The main scheduling function. Called from:
///   1. LAPIC timer ISR (vector 32) — preemptive context switch
///   2. IPC endpoint send/recv — voluntary yield when blocking
///
/// Picks the next Ready thread from the run queue and context-switches to it.
/// Handles the current thread based on its state:
///   - Running → mark Ready, requeue (normal preemption)
///   - BlockedSend/BlockedRecv → don't touch (ownership transferred to Endpoint)
///   - Dead → don't requeue (leak for now, proper cleanup later)
///
/// # Safety
/// - Must be called with interrupts disabled (IF=0).
///   For timer ISR: interrupt gate clears IF automatically.
///   For IPC: caller must `cli` before calling.
/// - EOI must have been sent BEFORE calling (for timer path).
/// - CpuLocal must be initialized on this core.
///
/// # Ownership Model
/// `cpu_local.current_thread` is a raw `*mut Thread` — the memory is
/// heap-allocated but NOT wrapped in a Box. When the thread state is:
///   - Running: schedule() reconstructs Box via `Box::from_raw` to requeue.
///   - Blocked*: an Endpoint already holds the Box. schedule() must NOT
///     reconstruct another Box (that would be a double-free).
///   - Dead: schedule() reconstructs Box to allow eventual deallocation.
pub unsafe fn schedule() {
    let cpu_local = unsafe { CpuLocal::get_mut() };
    let rq = unsafe { &mut *cpu_local.run_queue };
    let current_ptr = cpu_local.current_thread;

    // Determine current thread state (needed before we check RunQueue)
    let current_state = if !current_ptr.is_null() {
        unsafe { (*current_ptr).state }
    } else {
        ThreadState::Dead
    };

    // --- Handle empty RunQueue ---
    if rq.is_empty() {
        if current_state == ThreadState::Running {
            // Normal preemption with nothing to switch to — let current keep running.
            crate::arch::lapic::set_timer_oneshot(10_000);
            return;
        }
        // Current thread is blocked/dead — no runnable threads exist.
        // Spin-wait: on multi-core, another core's IPC wakeup may push here.
        // On single-core with all threads blocked, this is a legitimate deadlock.
        while rq.is_empty() {
            core::hint::spin_loop();
        }
    }

    // Pop the next Ready thread
    let mut next_box = rq.pop().unwrap();
    next_box.state = ThreadState::Running;
    let next_ptr = Box::into_raw(next_box);

    // Null check — shouldn't happen after init, but be defensive
    if current_ptr.is_null() {
        cpu_local.current_thread = next_ptr;
        crate::arch::lapic::set_timer_oneshot(10_000);
        return;
    }

    // Grab prev RSP save location BEFORE any ownership transfer.
    // This raw pointer remains valid because:
    //   - Running: we'll Box::from_raw → push to RunQueue (memory stays alive)
    //   - Blocked*: Endpoint owns the Box (memory stays alive)
    //   - Dead: memory stays alive until we drop (after switch_context returns)
    let prev_rsp_ptr = unsafe { &raw mut (*current_ptr).rsp };
    let next_rsp_val = unsafe { (*next_ptr).rsp };

    // --- Handle current thread based on state ---
    match current_state {
        ThreadState::Running => {
            // Normal preemption: requeue the current thread.
            // Reconstruct Box (valid: into_raw was the last ownership op).
            unsafe { (*current_ptr).state = ThreadState::Ready; }
            let current_box = unsafe { Box::from_raw(current_ptr) };
            rq.push(current_box);
        }
        ThreadState::BlockedSend | ThreadState::BlockedRecv => {
            // Thread's Box<Thread> ownership was transferred to an Endpoint
            // by the IPC code BEFORE calling schedule(). Do NOT reconstruct
            // a Box here — that would create a second Box for the same
            // allocation, causing a double-free when both are dropped.
            //
            // The raw pointer (current_ptr) is still valid for the
            // switch_context RSP save because the Endpoint keeps it alive.
        }
        ThreadState::Dead => {
            // Thread has terminated. For now, leak the TCB memory.
            // TODO: Free kernel stack pages and TCB in a future sprint.
            // We can't free the stack RIGHT NOW because switch_context
            // is about to save RSP into it — we're still on this stack.
        }
        ThreadState::Ready => {
            // Shouldn't happen — Ready means it should be in the RunQueue.
            // Defensive: just requeue it.
            let current_box = unsafe { Box::from_raw(current_ptr) };
            rq.push(current_box);
        }
    }

    // Install next thread as current and re-arm the timer
    cpu_local.current_thread = next_ptr;

    // Update kernel stack pointers for Ring 3 support.
    // TSS.rsp[0]: loaded by CPU on Ring 3 → Ring 0 interrupt/exception.
    // CpuLocal.kernel_stack_top: loaded by syscall_entry on SYSCALL.
    // Both must point to the TOP of the next thread's kernel stack.
    let kstack_base = unsafe { (*next_ptr).kernel_stack_base };
    let kstack_size = unsafe { (*next_ptr).kernel_stack_size };
    if kstack_base != 0 {
        let stack_top = kstack_base + kstack_size as u64;
        crate::arch::gdt::set_rsp0(cpu_local.core_index as usize, stack_top);
        cpu_local.kernel_stack_top = stack_top;
    }

    // ─── CR3 swap: per-process address space isolation ─────────────────────
    // Compare the current and next thread's parent process PML4.
    // If they belong to different processes (different address spaces),
    // swap CR3 to activate the next thread's page tables.
    //
    // We MUST do this BEFORE switch_context because after the switch we are
    // executing on the next thread's kernel stack. The kernel half of every
    // PML4 is identical (shallow copy of entries 256-511), so this is safe.
    //
    // Skip the write if same PML4 — same-process thread switch or two
    // kernel threads both pointing at KERNEL_PML4.
    let current_pml4 = if !current_ptr.is_null() {
        let proc = unsafe { (*current_ptr).process };
        if !proc.is_null() { unsafe { (*proc).pml4().as_u64() } } else { 0 }
    } else {
        0
    };
    let next_pml4 = {
        let proc = unsafe { (*next_ptr).process };
        if !proc.is_null() { unsafe { (*proc).pml4().as_u64() } } else { 0 }
    };
    if next_pml4 != 0 && next_pml4 != current_pml4 {
        unsafe { crate::arch::cpu::write_cr3(next_pml4); }
    }

    crate::arch::lapic::set_timer_oneshot(10_000);

    // Execute the hardware context switch.
    // Saves current callee-saved regs + RSP into *prev_rsp_ptr,
    // loads next thread's RSP and callee-saved regs, then `ret`.
    // We reach the line below when THIS thread gets scheduled back.
    unsafe { context::switch_context(prev_rsp_ptr, next_rsp_val); }
}

/// Test thread A — prints iterations to verify preemption.
pub extern "C" fn test_thread_a(_arg: u64) {
    for i in 0..5 {
        kprintln!("[thread-A] iteration {}", i);
        busy_wait();
    }
    kprintln!("[thread-A] DONE");
}

/// Test thread B — prints iterations to verify preemption.
pub extern "C" fn test_thread_b(_arg: u64) {
    for i in 0..5 {
        kprintln!("[thread-B] iteration {}", i);
        busy_wait();
    }
    kprintln!("[thread-B] DONE");
}

/// Busy-wait that actually burns CPU time (~50ms at typical clock speeds).
/// Uses volatile reads so the compiler can't optimize away the loop.
#[inline(never)]
fn busy_wait() {
    let mut x: u64 = 0;
    for _ in 0..5_000_000u64 {
        unsafe { core::ptr::read_volatile(&x); }
        x = x.wrapping_add(1);
    }
    // Prevent the compiler from removing x entirely
    unsafe { core::ptr::write_volatile(&mut x, x); }
}
