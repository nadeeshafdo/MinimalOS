// =============================================================================
// MinimalOS NextGen — Context Switch Assembly
// =============================================================================
//
// switch_context: the most critical function in the kernel.
// It saves the current thread's CPU state and restores another thread's state,
// effectively "teleporting" execution from one stack to another.
//
// MECHANISM:
//   The System V AMD64 ABI says that rbx, rbp, r12-r15 are callee-saved.
//   When switch_context is *called*, the compiler has already saved any
//   caller-saved registers it needs. We only need to save the callee-saved
//   registers and swap RSP.
//
//   For new threads, we construct a synthetic stack frame that looks exactly
//   like a suspended thread, so switch_context can "resume" it.
//
// thread_entry_trampoline:
//   The landing pad for new threads. switch_context's `ret` jumps here.
//   CRITICAL: Re-enables interrupts (sti) because the timer ISR that triggered
//   schedule() disabled them. Without this, the thread runs with IF=0 forever.
//
// =============================================================================

use core::arch::naked_asm;

/// Switches CPU execution from one thread to another.
///
/// # Parameters
/// - `rdi` = `prev_rsp: *mut u64` — where to save the current RSP
/// - `rsi` = `next_rsp: u64` — the RSP to load (resumes next thread)
///
/// # Register Convention (System V AMD64 ABI)
/// Callee-saved: rbx, rbp, r12, r13, r14, r15
/// These are pushed/popped across the context switch.
///
/// # Stack Frame Layout
/// After pushing callee-saved registers:
/// ```text
/// [rsp]      rbx
/// [rsp+8]    rbp
/// [rsp+16]   r12
/// [rsp+24]   r13
/// [rsp+32]   r14
/// [rsp+40]   r15
/// [rsp+48]   return address (from `call switch_context`)
/// ```
#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(_prev_rsp: *mut u64, _next_rsp: u64) {
    naked_asm!(
        // Save callee-saved registers of the CURRENT thread
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Save current RSP into *prev_rsp (rdi)
        "mov [rdi], rsp",

        // Load next thread's RSP (rsi)
        "mov rsp, rsi",

        // Restore callee-saved registers of the NEXT thread
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",

        // Return to the next thread's saved RIP
        // For existing threads: returns into schedule() → finishes timer ISR
        // For new threads: jumps to thread_entry_trampoline
        "ret",
    );
}

/// Entry trampoline for newly spawned threads.
///
/// When `switch_context` executes `ret` on a new thread's synthetic stack,
/// it jumps here. At this point:
///   - r13 = thread payload function pointer
///   - r14 = argument for the payload
///   - Interrupts are DISABLED (IF=0, because we came from a timer ISR)
///
/// We must:
///   1. Enable interrupts (sti) — or this thread will starve the CPU
///   2. Call the payload function with the argument
///   3. Call thread_exit() if the payload returns
#[unsafe(naked)]
pub unsafe extern "C" fn thread_entry_trampoline() {
    naked_asm!(
        // CRITICAL: Re-enable preemption
        // The timer ISR that called schedule() cleared IF.
        // Without this, the thread runs forever without being preempted.
        "sti",

        // Set up System V ABI call: rdi = first argument
        "mov rdi, r14",

        // Call the thread's payload function (r13)
        "call r13",

        // If the payload returns, clean up the thread
        "call {exit}",

        // Should never reach here
        "ud2",
        exit = sym super::thread::thread_exit,
    );
}
