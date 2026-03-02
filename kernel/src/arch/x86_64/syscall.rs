// =============================================================================
// MinimalOS NextGen — SYSCALL/SYSRET Entry (Sprint 6: Ring 3 Support)
// =============================================================================
//
// This module implements the Ring 0 ↔ Ring 3 boundary:
//
//   1. SYSCALL MSR configuration (IA32_STAR, IA32_LSTAR, IA32_FMASK)
//   2. Naked `syscall_entry` assembly (swapgs → stack swap → save → dispatch)
//   3. `syscall_dispatch` — Rust handler with capability enforcement
//   4. Ring 3 transition via IRETQ (builds a fake interrupt frame)
//   5. User thread creation (spawn_user)
//   6. Global endpoint table for syscall lookups
//
// SYSCALL ABI (matches Linux convention):
//   RAX = syscall number
//   RDI = arg0 (CNode slot index for IPC)
//   RSI = arg1 (label / data)
//   RDX = arg2 (data)
//   R10 = arg3 (NOT RCX — CPU overwrites RCX with user RIP)
//   R8  = arg4
//   R9  = arg5
//
//   Return: RAX = result (0 = success, u64::MAX variants = error)
//   For SYS_RECV return: RDI = label, RSI = data[0], RDX = data[1], R10 = data[2]
//
// SYSRET loads:
//   RCX → user RIP (saved by CPU on SYSCALL)
//   R11 → user RFLAGS (saved by CPU on SYSCALL)
//
// SECURITY:
//   - FMASK clears IF on SYSCALL entry → no interrupt races during stack swap
//   - CLI before user RSP restore → no interrupt on user stack in Ring 0
//   - Capability validation on every syscall → no unauthorized IPC
//
// =============================================================================

extern crate alloc;

use core::arch::naked_asm;

use alloc::boxed::Box;

use crate::arch::cpu;
use crate::arch::x86_64::gdt;
use crate::cap::cnode::{CapObject, CapRights};
use crate::ipc::endpoint::Endpoint;
use crate::ipc::message::IpcMessage;
use crate::kprintln;
use crate::sched::percpu::CpuLocal;
use crate::sched::thread::Thread;

// =============================================================================
// MSR Constants
// =============================================================================

/// IA32_EFER — Extended Feature Enable Register.
/// Bit 0 (SCE) must be set for SYSCALL/SYSRET to work.
const IA32_EFER: u32 = 0xC000_0080;

/// EFER.SCE — System Call Enable. Without this bit, SYSCALL causes #UD.
const EFER_SCE: u64 = 1 << 0;

/// IA32_STAR — SYSCALL/SYSRET segment selector configuration.
const IA32_STAR: u32 = 0xC000_0081;

/// IA32_LSTAR — SYSCALL entry point (RIP loaded on SYSCALL).
const IA32_LSTAR: u32 = 0xC000_0082;

/// IA32_FMASK — RFLAGS bits cleared on SYSCALL entry.
const IA32_FMASK: u32 = 0xC000_0084;

// =============================================================================
// Syscall Numbers
// =============================================================================

/// SYS_SEND — Send an IPC message through a capability.
const SYS_SEND: u64 = 1;

/// SYS_RECV — Receive an IPC message from a capability.
const SYS_RECV: u64 = 2;

// =============================================================================
// CpuLocal Field Offsets (used by naked assembly)
// =============================================================================
// These MUST match the actual repr(C) layout of CpuLocal.
// Compile-time assertions in percpu.rs verify these.

/// CpuLocal.user_rsp_scratch offset — temp save for user RSP.
const CPULOCAL_USER_RSP_SCRATCH: u32 = 48;

/// CpuLocal.kernel_stack_top offset — kernel stack for SYSCALL.
const CPULOCAL_KERNEL_STACK_TOP: u32 = 56;

// =============================================================================
// Endpoint Table
// =============================================================================

/// Maximum endpoints in the global lookup table.
const MAX_ENDPOINTS: usize = 64;

/// Global endpoint table — maps endpoint IDs to Endpoint instances.
/// Written once during boot (single-threaded), read-only during execution.
///
/// SAFETY: Written before threads run (Phase 6 init), read-only after.
static mut ENDPOINT_TABLE: [*const Endpoint; MAX_ENDPOINTS] =
    [core::ptr::null(); MAX_ENDPOINTS];

/// Registers an endpoint in the global table for syscall lookup.
///
/// # Safety
/// Must be called during single-threaded boot before any userspace execution.
pub unsafe fn register_endpoint(ep: *const Endpoint) {
    let id = unsafe { (*ep).id() as usize };
    assert!(id < MAX_ENDPOINTS, "Endpoint ID {} exceeds table size", id);
    unsafe { ENDPOINT_TABLE[id] = ep; }
}

/// Looks up a registered endpoint by ID.
unsafe fn lookup_endpoint(id: u64) -> Option<&'static Endpoint> {
    let idx = id as usize;
    if idx >= MAX_ENDPOINTS { return None; }
    let ptr = unsafe { ENDPOINT_TABLE[idx] };
    if ptr.is_null() { None } else { Some(unsafe { &*ptr }) }
}

// =============================================================================
// Syscall Frame
// =============================================================================

/// Register save area pushed by syscall_entry assembly.
///
/// Layout matches the push order in the naked assembly exactly.
/// Fields are ordered from lowest address (first popped) to highest.
///
/// ```text
/// [rsp + 0]   r15
/// [rsp + 8]   r14
/// [rsp + 16]  r13
/// [rsp + 24]  r12
/// [rsp + 32]  r11          ← user RFLAGS (saved by CPU on SYSCALL)
/// [rsp + 40]  r10          ← arg3
/// [rsp + 48]  r9
/// [rsp + 56]  r8
/// [rsp + 64]  rbp
/// [rsp + 72]  rdi          ← arg0 (CNode slot index)
/// [rsp + 80]  rsi          ← arg1 (label)
/// [rsp + 88]  rdx          ← arg2 (data0)
/// [rsp + 96]  rcx          ← user RIP (saved by CPU on SYSCALL)
/// [rsp + 104] rbx
/// [rsp + 112] rax          ← syscall number / return value
/// [rsp + 120] user_rsp     ← saved user stack pointer
/// ```
#[repr(C)]
pub struct SyscallFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub user_rsp: u64,
}

// =============================================================================
// MSR Initialization
// =============================================================================

/// Configures SYSCALL/SYSRET MSRs on the calling CPU core.
///
/// Must be called on every core (BSP + APs) after the GDT is loaded.
///
/// # MSR Configuration
///
/// **IA32_STAR** — Segment selectors:
///   Bits [47:32] = KERNEL_CS (0x08). SYSCALL loads CS from here.
///   Bits [63:48] = user base (0x10). SYSRET loads:
///     SS = base + 8  = 0x18 (USER_DS without RPL; CPU adds RPL=3 → 0x1B)
///     CS = base + 16 = 0x20 (USER_CS without RPL; CPU adds RPL=3 → 0x23)
///
/// **IA32_LSTAR** — RIP loaded on SYSCALL (→ `syscall_entry`).
///
/// **IA32_FMASK** — RFLAGS bits cleared on SYSCALL. We clear IF (bit 9)
/// to disable interrupts during the swapgs/stack-swap critical section.
pub fn init() {
    // Enable SYSCALL/SYSRET by setting EFER.SCE (bit 0).
    // Without this, the SYSCALL instruction causes #UD in Ring 3.
    // Limine enables EFER.LME (long mode) and EFER.NXE (no-execute),
    // but does NOT set EFER.SCE — we must do it ourselves.
    unsafe {
        let efer = cpu::read_msr(IA32_EFER);
        cpu::write_msr(IA32_EFER, efer | EFER_SCE);
    }

    // STAR[47:32] = kernel CS for SYSCALL
    // STAR[63:48] = base for SYSRET: user SS = base+8, user CS = base+16
    let star: u64 = ((gdt::KERNEL_DS as u64) << 48) | ((gdt::KERNEL_CS as u64) << 32);

    // LSTAR = address of the naked syscall_entry function
    let lstar: u64 = syscall_entry as *const () as u64;

    // FMASK: clear IF (bit 9) on SYSCALL entry to prevent interrupts
    // during the swapgs → stack swap critical window.
    let fmask: u64 = 0x200;

    unsafe {
        cpu::write_msr(IA32_STAR, star);
        cpu::write_msr(IA32_LSTAR, lstar);
        cpu::write_msr(IA32_FMASK, fmask);
    }

    kprintln!("[syscall] MSRs configured: STAR={:#018X} LSTAR={:#018X} FMASK={:#06X}",
        star, lstar, fmask);
}

// =============================================================================
// Syscall Entry (Naked Assembly)
// =============================================================================

/// SYSCALL entry point — the CPU jumps here when a Ring 3 thread executes
/// the `syscall` instruction.
///
/// CPU state on entry:
///   RCX = user RIP (return address for SYSRET)
///   R11 = user RFLAGS
///   CS  = KERNEL_CS (from STAR[47:32])
///   SS  = KERNEL_DS (from STAR[47:32] + 8)
///   RIP = this function (from LSTAR)
///   RFLAGS &= ~FMASK (IF cleared — interrupts disabled)
///
/// CRITICAL: RSP is still the USER stack pointer. We must swap to the
/// kernel stack before touching any kernel data or calling Rust code.
///
/// ## Stack swap protocol:
/// 1. `swapgs` — GS now points to kernel CpuLocal
/// 2. Save user RSP to `gs:[48]` (CpuLocal.user_rsp_scratch)
/// 3. Load kernel RSP from `gs:[56]` (CpuLocal.kernel_stack_top)
/// 4. Push user state onto kernel stack → SyscallFrame
/// 5. Call Rust dispatcher
/// 6. Restore user state from SyscallFrame
/// 7. `cli` + restore user RSP + `swapgs` + `sysretq`
#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() {
    naked_asm!(
        // ─── Phase 1: Stack swap ────────────────────────────────────────
        "swapgs",                              // GS → kernel CpuLocal
        "mov gs:[{scratch}], rsp",             // Save user RSP to scratch
        "mov rsp, gs:[{kstack}]",              // Load kernel RSP

        // ─── Phase 2: Build SyscallFrame ────────────────────────────────
        // Reserve 8-byte slot for user_rsp (filled after GPR saves)
        "sub rsp, 8",

        // Push all 15 GPRs (rax first → highest offset, r15 last → offset 0)
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rbp",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Fill user_rsp slot at [rsp + 120] using now-free rax
        "mov rax, gs:[{scratch}]",
        "mov [rsp + 120], rax",

        // ─── Phase 3: Call Rust dispatcher ──────────────────────────────
        // System V ABI: rdi = first argument = pointer to SyscallFrame
        "mov rdi, rsp",
        "call syscall_dispatch",

        // ─── Phase 4: Store return value in frame ───────────────────────
        // RAX from dispatch overwrites the saved rax slot [rsp + 112]
        "mov [rsp + 112], rax",

        // ─── Phase 5: Restore GPRs from SyscallFrame ───────────────────
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",                             // Restored user RFLAGS for SYSRET
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rbp",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",                             // Restored user RIP for SYSRET
        "pop rbx",
        "pop rax",                             // Return value (from modified slot)

        // ─── Phase 6: Return to Ring 3 ─────────────────────────────────
        // CRITICAL: Disable interrupts before restoring user RSP.
        // Between `pop rsp` and `sysretq`, we're in Ring 0 with a user stack.
        // An interrupt here would push a Ring 0 frame onto the user stack → SECURITY BUG.
        "cli",
        "pop rsp",                             // User RSP (from frame.user_rsp)
        "swapgs",                              // Restore user GS
        "sysretq",                             // Ring 0 → Ring 3

        scratch = const CPULOCAL_USER_RSP_SCRATCH,
        kstack = const CPULOCAL_KERNEL_STACK_TOP,
    );
}

// =============================================================================
// Syscall Dispatcher (Rust)
// =============================================================================

/// Central syscall dispatcher — called from the naked assembly entry point.
///
/// Reads the syscall number from `frame.rax` and dispatches to the
/// appropriate handler. Each handler validates capabilities before
/// performing any privileged operation.
///
/// # Returns
/// Result code in RAX: 0 = success, nonzero = error.
#[unsafe(no_mangle)]
pub extern "C" fn syscall_dispatch(frame: &mut SyscallFrame) -> u64 {
    let number = frame.rax;

    match number {
        SYS_SEND => {
            let slot = frame.rdi;
            let label = frame.rsi;
            let data0 = frame.rdx;
            let data1 = frame.r10;
            sys_send(slot, label, data0, data1)
        }
        SYS_RECV => {
            let slot = frame.rdi;
            sys_recv(frame, slot)
        }
        _ => {
            kprintln!("[syscall] UNKNOWN syscall number {} from RIP={:#018X}",
                number, frame.rcx);
            u64::MAX
        }
    }
}

// =============================================================================
// SYS_SEND — Send an IPC message
// =============================================================================

/// Sends an IPC message through a capability-referenced endpoint.
///
/// # Arguments (from syscall registers)
///   - slot:  CNode slot index containing an Endpoint capability with WRITE
///   - label: IPC message label
///   - data0: IPC message data register 0
///   - data1: IPC message data register 1
///
/// # Returns
///   0 on success. Error codes:
///   - `u64::MAX`     — invalid CNode slot (empty or out of bounds)
///   - `u64::MAX - 1` — insufficient rights (no WRITE permission)
///   - `u64::MAX - 2` — capability is not an Endpoint
///   - `u64::MAX - 3` — endpoint not found in global table
fn sys_send(slot: u64, label: u64, data0: u64, data1: u64) -> u64 {
    let cpu_local = unsafe { CpuLocal::get() };
    let thread = unsafe { &*cpu_local.current_thread };

    // 1. Validate capability
    let cap = match thread.cnode.lookup(slot as usize) {
        Some(c) => c,
        None => {
            kprintln!("[syscall] SYS_SEND: thread {} bad slot {}", thread.id, slot);
            return u64::MAX;
        }
    };

    // 2. Check it's an Endpoint with WRITE rights
    let ep_id = match cap.object {
        CapObject::Endpoint { id } => {
            if !cap.rights.contains(CapRights::WRITE) {
                kprintln!("[syscall] SYS_SEND: thread {} no WRITE right on slot {}",
                    thread.id, slot);
                return u64::MAX - 1;
            }
            id
        }
        _ => {
            kprintln!("[syscall] SYS_SEND: thread {} slot {} is not an Endpoint",
                thread.id, slot);
            return u64::MAX - 2;
        }
    };

    // 3. Look up the endpoint
    let ep = match unsafe { lookup_endpoint(ep_id) } {
        Some(ep) => ep,
        None => {
            kprintln!("[syscall] SYS_SEND: endpoint ID {} not registered", ep_id);
            return u64::MAX - 3;
        }
    };

    // 4. Build message from register arguments
    let msg = IpcMessage::with_data(label, [data0, data1, 0, 0]);

    kprintln!("[syscall] SYS_SEND: thread {} → EP{} label={:#X} data=[{:#X}, {:#X}]",
        thread.id, ep_id, label, data0, data1);

    // 5. Perform the IPC send (may block → schedule → resume)
    ep.send(&msg);

    0 // Success
}

// =============================================================================
// SYS_RECV — Receive an IPC message
// =============================================================================

/// Receives an IPC message from a capability-referenced endpoint.
///
/// On success, the received message fields are written into the
/// SyscallFrame so the user sees them in registers on return:
///   RDI = message label
///   RSI = data[0]
///   RDX = data[1]
///   R10 = data[2]
///
/// # Arguments
///   - slot: CNode slot index containing an Endpoint capability with READ
///
/// # Returns
///   0 on success (message data in frame registers). Error codes as above.
fn sys_recv(frame: &mut SyscallFrame, slot: u64) -> u64 {
    let cpu_local = unsafe { CpuLocal::get() };
    let thread = unsafe { &*cpu_local.current_thread };

    // 1. Validate capability
    let cap = match thread.cnode.lookup(slot as usize) {
        Some(c) => c,
        None => {
            kprintln!("[syscall] SYS_RECV: thread {} bad slot {}", thread.id, slot);
            return u64::MAX;
        }
    };

    // 2. Check it's an Endpoint with READ rights
    let ep_id = match cap.object {
        CapObject::Endpoint { id } => {
            if !cap.rights.contains(CapRights::READ) {
                kprintln!("[syscall] SYS_RECV: thread {} no READ right on slot {}",
                    thread.id, slot);
                return u64::MAX - 1;
            }
            id
        }
        _ => {
            kprintln!("[syscall] SYS_RECV: thread {} slot {} is not an Endpoint",
                thread.id, slot);
            return u64::MAX - 2;
        }
    };

    // 3. Look up the endpoint
    let ep = match unsafe { lookup_endpoint(ep_id) } {
        Some(ep) => ep,
        None => {
            kprintln!("[syscall] SYS_RECV: endpoint ID {} not registered", ep_id);
            return u64::MAX - 3;
        }
    };

    kprintln!("[syscall] SYS_RECV: thread {} blocking on EP{}", thread.id, ep_id);

    // 4. Perform the IPC recv (may block → schedule → resume)
    let msg = ep.recv();

    kprintln!("[syscall] SYS_RECV: thread {} got label={:#X} data=[{:#X}, {:#X}, {:#X}]",
        thread.id, msg.label, msg.regs[0], msg.regs[1], msg.regs[2]);

    // 5. Write message data into frame registers so user sees them on return
    frame.rdi = msg.label;
    frame.rsi = msg.regs[0];
    frame.rdx = msg.regs[1];
    frame.r10 = msg.regs[2];

    0 // Success
}

// =============================================================================
// Ring 3 Transition
// =============================================================================

/// Entry function for user threads — called from the scheduler via
/// switch_context → thread_entry_trampoline → this function.
///
/// Reads user_rip and user_rsp from the current thread's TCB, then
/// builds a fake interrupt frame and executes IRETQ to jump to Ring 3.
///
/// After this function, the thread executes in Ring 3. Any privileged
/// operation (I/O, HLT, MOV CR3, etc.) will cause a #GP, forcing the
/// thread to use SYSCALL for kernel services.
pub extern "C" fn ring3_entry(_arg: u64) {
    let cpu_local = unsafe { CpuLocal::get() };
    let thread = unsafe { &*cpu_local.current_thread };

    let user_rip = thread.user_rip;
    let user_rsp = thread.user_rsp;

    kprintln!("[syscall] Thread {} '{}' entering Ring 3: RIP={:#018X} RSP={:#018X}",
        thread.id, thread.name_str(), user_rip, user_rsp);

    // Jump to Ring 3 — this never returns.
    unsafe { jump_to_ring3(user_rip, user_rsp); }
}

/// Performs the Ring 0 → Ring 3 transition via IRETQ.
///
/// Builds a fake interrupt frame on the kernel stack:
///   SS    = USER_DS (0x1B)
///   RSP   = user_rsp
///   RFLAGS = 0x202 (IF=1, reserved bit 1 = 1)
///   CS    = USER_CS (0x23)
///   RIP   = user_rip
///
/// Then `swapgs` (save kernel GS, prepare for user GS) + `iretq`.
///
/// # Safety
/// - `user_rip` must be a valid, USER-accessible, mapped code address.
/// - `user_rsp` must be a valid, USER-accessible, mapped stack address.
/// - GDT, TSS, and SYSCALL MSRs must be properly configured.
#[inline(never)]
pub unsafe fn jump_to_ring3(user_rip: u64, user_rsp: u64) -> ! {
    let ss_val = gdt::USER_DS as u64;
    let cs_val = gdt::USER_CS as u64;
    unsafe {
        core::arch::asm!(
            "swapgs",          // Save kernel GS → KERNEL_GS_BASE; load user GS
            "push {ss}",      // SS = USER_DS
            "push {ursp}",    // RSP = user stack top
            "push 0x202",     // RFLAGS: IF=1, reserved bit 1 always set
            "push {cs}",      // CS = USER_CS
            "push {urip}",    // RIP = user entry point
            "iretq",          // "Return" to Ring 3
            ss = in(reg) ss_val,
            ursp = in(reg) user_rsp,
            cs = in(reg) cs_val,
            urip = in(reg) user_rip,
            options(noreturn),
        );
    }
}

// =============================================================================
// User Thread Creation
// =============================================================================

/// Creates a new user-mode thread that will enter Ring 3 at `user_rip`.
///
/// The thread starts as a kernel thread with `ring3_entry` as its entry
/// function. When the scheduler first switches to it, `ring3_entry` reads
/// `user_rip` and `user_rsp` from the TCB and executes IRETQ to Ring 3.
///
/// # Parameters
/// - `name`: Human-readable name for debugging.
/// - `user_rip`: Virtual address of user code (must be USER-mapped).
/// - `user_rsp`: Top of user stack (must be USER-mapped).
///
/// # Returns
/// A `Box<Thread>` ready to be enqueued. The caller should:
/// 1. Install capabilities (CNode.insert_at) before spawning.
/// 2. Call `spawn_thread()` to enqueue it.
pub fn spawn_user(name: &str, user_rip: u64, user_rsp: u64) -> Box<Thread> {
    let mut thread = Thread::new(name, ring3_entry, 0);
    thread.user_rip = user_rip;
    thread.user_rsp = user_rsp;
    kprintln!("[syscall] Created user thread {} '{}' → Ring 3 @ RIP={:#018X} RSP={:#018X}",
        thread.id, name, user_rip, user_rsp);
    thread
}
