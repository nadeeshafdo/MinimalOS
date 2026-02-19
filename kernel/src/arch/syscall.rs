//! Syscall infrastructure for x86_64.
//!
//! Enables the `syscall`/`sysret` instruction pair via MSR configuration,
//! provides the assembly entry stub that saves/restores registers, and
//! dispatches to the Rust handler.

use core::arch::{asm, naked_asm};

// ── MSR addresses ───────────────────────────────────────────────
/// Extended Feature Enable Register — bit 0 enables `syscall`/`sysret`.
const MSR_EFER: u32 = 0xC000_0080;
/// Syscall Target Address Register — kernel/user CS bases.
const MSR_STAR: u32 = 0xC000_0081;
/// Long-mode SYSCALL Target RIP.
const MSR_LSTAR: u32 = 0xC000_0082;
/// Syscall Flag Mask — RFLAGS bits to clear on `syscall`.
const MSR_SFMASK: u32 = 0xC000_0084;

/// EFER bit: System Call Extensions enable.
const EFER_SCE: u64 = 1 << 0;

/// RFLAGS: Interrupt Flag — masked so interrupts are disabled on entry.
const RFLAGS_IF: u64 = 1 << 9;

// ── Scratch space (single-CPU) ──────────────────────────────────
/// Temporary storage for user RSP while running on kernel stack.
#[no_mangle]
static mut SYSCALL_USER_RSP: u64 = 0;

/// Kernel stack pointer loaded by the syscall stub.
/// Set once during `init()` from the TSS RSP0 value.
#[no_mangle]
pub static mut SYSCALL_KERNEL_RSP: u64 = 0;

// ── MSR helpers ─────────────────────────────────────────────────
/// Read a Model-Specific Register.
#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((hi as u64) << 32) | (lo as u64)
}

/// Write a Model-Specific Register.
#[inline]
unsafe fn wrmsr(msr: u32, value: u64) {
    let lo = value as u32;
    let hi = (value >> 32) as u32;
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") lo,
            in("edx") hi,
            options(nomem, nostack, preserves_flags),
        );
    }
}

// ── Initialisation ([046]) ──────────────────────────────────────

/// Enable the `syscall` instruction and configure the STAR / LSTAR / SFMASK MSRs.
///
/// # Arguments
/// * `kernel_rsp` — top of the kernel stack to switch to on `syscall` entry.
///
/// # Safety
/// Must be called exactly once, after the GDT and TSS are loaded.
pub unsafe fn init(kernel_rsp: u64) {
    // Store the kernel RSP so the assembly stub can load it.
    unsafe {
        SYSCALL_KERNEL_RSP = kernel_rsp;
    }

    // 1. Enable SCE (System Call Extensions) in EFER.
    let efer = unsafe { rdmsr(MSR_EFER) };
    unsafe { wrmsr(MSR_EFER, efer | EFER_SCE) };
    klog::debug!("EFER = {:#x} -> {:#x} (SCE enabled)", efer, efer | EFER_SCE);

    // 2. STAR: kernel CS base in bits [47:32], sysret CS base in bits [63:48].
    //
    //  syscall:  CS = STAR[47:32],      SS = STAR[47:32] + 8
    //  sysret64: CS = STAR[63:48] + 16, SS = STAR[63:48] + 8
    //
    // GDT: 0x08=KCode, 0x10=KData, 0x18=UData, 0x20=UCode
    //  → kernel base = 0x08, sysret base = 0x10
    //    sysret64 CS = 0x10+16 = 0x20 (|3 by HW) = 0x23 ✓
    //    sysret64 SS = 0x10+8  = 0x18 (|3 by HW) = 0x1B ✓
    let star: u64 = (0x0010u64 << 48) | (0x0008u64 << 32);
    unsafe { wrmsr(MSR_STAR, star) };
    klog::debug!("STAR = {:#018x}", star);

    // 3. LSTAR: entry point for `syscall`.
    let handler_addr = syscall_entry as usize as u64;
    unsafe { wrmsr(MSR_LSTAR, handler_addr) };
    klog::debug!("LSTAR = {:#x} (syscall_entry)", handler_addr);

    // 4. SFMASK: clear IF on syscall entry (disable interrupts).
    unsafe { wrmsr(MSR_SFMASK, RFLAGS_IF) };
    klog::debug!("SFMASK = {:#x} (mask IF)", RFLAGS_IF);

    klog::info!("[046] syscall enabled (EFER.SCE=1, STAR={:#018x})", star);
}

// ── Assembly entry stub ([047]) ─────────────────────────────────
//
// On `syscall`:
//   RCX = user RIP (return address)
//   R11 = user RFLAGS
//   RSP = still the user stack (NOT switched by hardware)
//
// We must:
//   1. Save the user RSP and switch to the kernel stack.
//   2. Push a register frame that the Rust dispatcher can read.
//   3. Call `syscall_dispatch(number, arg0..arg4)`.
//   4. Restore registers and `sysretq` back to user mode.
//
// Register convention (Linux-style):
//   RAX = syscall number
//   RDI, RSI, RDX, R10, R8, R9 = arguments 0-5
//   RAX = return value

/// The raw `syscall` entry point written in inline assembly.
///
/// # Safety
/// This is a naked function invoked directly by the CPU on `syscall`.
#[naked]
unsafe extern "C" fn syscall_entry() {
    naked_asm!(
        // ── swap to kernel stack ──
        // Save user RSP into scratch variable
        "mov [rip + SYSCALL_USER_RSP], rsp",
        // Load kernel RSP
        "mov rsp, [rip + SYSCALL_KERNEL_RSP]",

        // ── save callee-context (we need to restore on sysretq) ──
        "push rcx",       // user RIP
        "push r11",       // user RFLAGS
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        // Save user RSP on kernel stack too
        "push qword ptr [rip + SYSCALL_USER_RSP]",

        // ── call Rust dispatcher ──
        // fn syscall_dispatch(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> u64
        //
        // Incoming register state from user-mode `syscall`:
        //   RAX = syscall number
        //   RDI = arg0, RSI = arg1, RDX = arg2, R10 = arg3, R8 = arg4, R9 = arg5
        //
        // SysV calling convention for the Rust callee:
        //   RDI = nr, RSI = a0, RDX = a1, RCX = a2, R8 = a3, R9 = a4

        // Shuffle arguments into SysV slots:
        "mov r15, rdi",   // save original arg0 (RDI will be overwritten)
        "mov rdi, rax",   // arg0 → nr (syscall number)
        "mov rcx, rdx",   // arg2 → RCX (SysV slot 3)
        "mov rdx, rsi",   // arg1 → RDX (SysV slot 2)
        "mov rsi, r15",   // original arg0 → RSI (SysV slot 1)
        // R10 → R8 for a3, but R8 already has a4 — need to shift:
        "mov r9, r8",     // a4 → R9 (SysV slot 5)
        "mov r8, r10",    // a3 → R8 (SysV slot 4)

        "call syscall_dispatch",

        // RAX now holds the return value — leave it there.

        // ── restore registers (reverse push order) ──
        "pop r15",        // saved user RSP → r15 (temp)
        "mov [rip + SYSCALL_USER_RSP], r15",

        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "pop r11",        // user RFLAGS
        "pop rcx",        // user RIP

        // Restore user RSP
        "mov rsp, [rip + SYSCALL_USER_RSP]",

        "sysretq",
    );
}

// ── Rust dispatcher ([048]) ─────────────────────────────────────

/// Syscall numbers.
pub mod nr {
    /// `sys_log(msg_ptr: *const u8, msg_len: usize)` — write a message to the kernel log.
    pub const SYS_LOG: u64 = 0;
    /// `sys_exit(code: u64)` — terminate the current process.
    pub const SYS_EXIT: u64 = 1;
    /// `sys_yield()` — voluntarily give up the CPU.
    pub const SYS_YIELD: u64 = 2;
    /// `sys_spawn(path_ptr: *const u8, path_len: usize)` — launch a new process.
    pub const SYS_SPAWN: u64 = 3;
    /// `sys_read(fd: u64, buf_ptr: *mut u8, buf_len: usize)` — read from fd.
    pub const SYS_READ: u64 = 4;
    /// `sys_pipe_create()` — create an IPC pipe, returns pipe_id.
    pub const SYS_PIPE_CREATE: u64 = 5;
    /// `sys_pipe_write(pipe_id, buf_ptr, buf_len)` — write to a pipe.
    pub const SYS_PIPE_WRITE: u64 = 6;
    /// `sys_pipe_read(pipe_id, buf_ptr, buf_len)` — read from a pipe.
    pub const SYS_PIPE_READ: u64 = 7;
    /// `sys_pipe_close(pipe_id)` — close/destroy a pipe.
    pub const SYS_PIPE_CLOSE: u64 = 8;
    /// `sys_time()` — return the current kernel tick count.
    pub const SYS_TIME: u64 = 9;
    /// `sys_sleep(ticks: u64)` — sleep for at least `ticks` timer ticks.
    pub const SYS_SLEEP: u64 = 10;
    /// `sys_futex(addr, op, val)` — futex wait/wake.
    pub const SYS_FUTEX: u64 = 11;
    /// `sys_read_event(buf_ptr)` — read next input event (12 bytes).
    pub const SYS_READ_EVENT: u64 = 12;
}

/// Rust syscall dispatcher — called from the assembly stub.
///
/// Returns the syscall result in RAX.
#[no_mangle]
unsafe extern "C" fn syscall_dispatch(
    nr: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    _a4: u64,
) -> u64 {
    match nr {
        nr::SYS_LOG => {
            // a0 = pointer to UTF-8 string, a1 = length
            let ptr = a0 as *const u8;
            let len = a1 as usize;
            if !ptr.is_null() && len > 0 && len <= 1024 {
                let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
                if let Ok(msg) = core::str::from_utf8(slice) {
                    klog::info!("[syscall] SYS_LOG: {}", msg);
                    return 0; // success
                }
            }
            u64::MAX // error: bad pointer or encoding
        }
        nr::SYS_EXIT => {
            klog::info!("[syscall] SYS_EXIT(code={})", a0);
            // [067] Mark the current process dead and schedule the next one.
            {
                let mut sched = crate::task::process::SCHEDULER.lock();
                if let Some(current) = sched.current_mut() {
                    current.state = crate::task::process::ProcessState::Dead;
                }
            }
            // Schedule away from the dead task.
            unsafe { crate::task::process::do_schedule() };
            // If we return here, no tasks are left — halt.
            klog::info!("[067] All tasks exited, halting");
            loop {
                unsafe { core::arch::asm!("hlt") };
            }
        }
        nr::SYS_YIELD => {
            // [065] Voluntarily yield the CPU to the next ready task.
            unsafe {
                crate::task::process::do_schedule();
            }
            0
        }
        nr::SYS_SPAWN => {
            // [066] a0 = path pointer, a1 = path length
            // [071] a2 = args pointer (0 = none), a3 = args length
            let ptr = a0 as *const u8;
            let len = a1 as usize;
            if ptr.is_null() || len == 0 || len > 256 {
                return u64::MAX;
            }
            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            let path = match core::str::from_utf8(slice) {
                Ok(s) => s,
                Err(_) => return u64::MAX,
            };
            // Extract optional args
            let args_ptr = a2 as *const u8;
            let args_len = a3 as usize;
            let args = if !args_ptr.is_null() && args_len > 0 && args_len <= 256 {
                let args_slice = unsafe { core::slice::from_raw_parts(args_ptr, args_len) };
                core::str::from_utf8(args_slice).unwrap_or("")
            } else {
                ""
            };
            klog::info!("[syscall] SYS_SPAWN(\"{}\", args=\"{}\")", path, args);
            match crate::task::process::spawn_from_ramdisk(path, args) {
                Ok(pid) => pid,
                Err(e) => {
                    klog::warn!("[syscall] SYS_SPAWN failed: {}", e);
                    u64::MAX
                }
            }
        }
        nr::SYS_READ => {
            // [068] a0 = fd (0 = STDIN), a1 = buf_ptr, a2 = buf_len
            let fd = a0;
            if fd != 0 {
                return u64::MAX; // only STDIN supported
            }
            // Non-blocking: return one byte if available, 0 if not.
            let ch = crate::task::input::pop_char();
            ch as u64
        }
        nr::SYS_PIPE_CREATE => {
            // [070] Create a new IPC pipe.
            match crate::task::pipe::create() {
                Some(id) => {
                    klog::info!("[syscall] SYS_PIPE_CREATE -> pipe_id={}", id);
                    id as u64
                }
                None => {
                    klog::warn!("[syscall] SYS_PIPE_CREATE: pipe table full");
                    u64::MAX
                }
            }
        }
        nr::SYS_PIPE_WRITE => {
            // [070] a0 = pipe_id, a1 = buf_ptr, a2 = buf_len
            let pipe_id = a0 as usize;
            let ptr = a1 as *const u8;
            let len = a2 as usize;
            if ptr.is_null() || len == 0 || len > 4096 {
                return u64::MAX;
            }
            let data = unsafe { core::slice::from_raw_parts(ptr, len) };
            crate::task::pipe::write(pipe_id, data) as u64
        }
        nr::SYS_PIPE_READ => {
            // [070] a0 = pipe_id, a1 = buf_ptr, a2 = buf_len
            let pipe_id = a0 as usize;
            let ptr = a1 as *mut u8;
            let len = a2 as usize;
            if ptr.is_null() || len == 0 || len > 4096 {
                return u64::MAX;
            }
            let buf = unsafe { core::slice::from_raw_parts_mut(ptr, len) };
            crate::task::pipe::read(pipe_id, buf) as u64
        }
        nr::SYS_PIPE_CLOSE => {
            // [070] a0 = pipe_id
            crate::task::pipe::close(a0 as usize);
            klog::info!("[syscall] SYS_PIPE_CLOSE({})", a0);
            0
        }
        nr::SYS_TIME => {
            // [072] Return the current tick count.
            crate::task::clock::now()
        }
        nr::SYS_SLEEP => {
            // [072] a0 = number of ticks to sleep.
            let ticks = a0;
            if ticks == 0 {
                return 0;
            }
            let wake_at = crate::task::clock::now() + ticks;
            {
                let mut sched = crate::task::process::SCHEDULER.lock();
                if let Some(current) = sched.current_mut() {
                    current.state = crate::task::process::ProcessState::Sleeping;
                    current.wake_tick = wake_at;
                }
            }
            // Yield to the scheduler so we stop running immediately.
            unsafe { crate::task::process::do_schedule() };
            0
        }
        nr::SYS_FUTEX => {
            // [073] a0 = address, a1 = operation, a2 = value
            let addr = a0;
            let op = a1;
            let val = a2;
            match op {
                crate::task::futex::FUTEX_WAIT => {
                    unsafe { crate::task::futex::futex_wait(addr, val) }
                }
                crate::task::futex::FUTEX_WAKE => {
                    crate::task::futex::futex_wake(addr, val)
                }
                _ => {
                    klog::warn!("[syscall] SYS_FUTEX: unknown op={}", op);
                    u64::MAX
                }
            }
        }
        nr::SYS_READ_EVENT => {
            // [079] a0 = pointer to 12-byte buffer in user space.
            let buf_ptr = a0 as *mut u8;
            if buf_ptr.is_null() {
                return u64::MAX;
            }
            unsafe { crate::task::events::read_event_to_user(buf_ptr) as u64 }
        }
        _ => {
            klog::warn!("[syscall] unknown syscall nr={}", nr);
            u64::MAX // ENOSYS
        }
    }
}
