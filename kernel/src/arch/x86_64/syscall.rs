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
use crate::sched::thread::{Thread, ThreadState};
use crate::sync::spinlock::SpinLock;

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

/// SYS_PORT_OUT — Write a byte to an I/O port (capability-gated).
const SYS_PORT_OUT: u64 = 3;

/// SYS_PORT_IN — Read a byte from an I/O port (capability-gated).
const SYS_PORT_IN: u64 = 4;

/// SYS_WAIT_IRQ — Block until a hardware interrupt fires (capability-gated).
const SYS_WAIT_IRQ: u64 = 5;

/// SYS_SPAWN_PROCESS — Create a new empty process with a fresh PML4 and CNode.
const SYS_SPAWN_PROCESS: u64 = 6;

/// SYS_ALLOC_MEMORY — Allocate a physical frame via PmmAllocator capability.
const SYS_ALLOC_MEMORY: u64 = 7;

/// SYS_MAP_MEMORY — Map a MemoryFrame into a target Process's address space.
const SYS_MAP_MEMORY: u64 = 8;

/// SYS_DELEGATE — Copy a capability from caller's CNode to a target Process's CNode.
const SYS_DELEGATE: u64 = 9;

/// SYS_SPAWN_THREAD — Create a Ring 3 thread inside a target Process.
const SYS_SPAWN_THREAD: u64 = 10;

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
// IRQ Notification Table (SYS_WAIT_IRQ support)
// =============================================================================

/// Maximum hardware IRQ lines supported (ISA IRQs 0-15).
const MAX_IRQ_LINES: usize = 16;

/// Per-IRQ blocked thread. When a thread calls SYS_WAIT_IRQ, its Box<Thread>
/// is stored here (single-waiter per IRQ). When the IRQ fires, the handler
/// reconstructs the Box, marks it Ready, and pushes it to the run queue.
///
/// Protected by a SpinLock. The IRQ handler acquires this briefly to wake
/// the waiting thread.
///
/// We wrap the raw pointer array in a newtype to implement Send.
struct IrqWaitersInner([*mut Thread; MAX_IRQ_LINES]);

// SAFETY: The *mut Thread pointers represent exclusive ownership of Box<Thread>.
// Only one entity (the SpinLock holder) accesses them at a time. Transfer between
// cores is safe because the pointed-to Threads are heap-allocated and not tied
// to any specific core.
unsafe impl Send for IrqWaitersInner {}

static IRQ_WAITERS: SpinLock<IrqWaitersInner> =
    SpinLock::new(IrqWaitersInner([core::ptr::null_mut(); MAX_IRQ_LINES]));

/// Called from `irq_dispatch` (idt.rs) when a hardware IRQ fires.
/// Checks if any thread is blocked waiting for this IRQ, and if so,
/// wakes it by pushing it back to the current core's run queue.
///
/// # Parameters
/// - `irq`: IRQ line number (0-15, NOT the IDT vector).
///
/// # Safety
/// Must be called from an interrupt handler context (IF=0).
pub fn notify_irq_waiters(irq: usize) {
    if irq >= MAX_IRQ_LINES { return; }

    let mut waiters = IRQ_WAITERS.lock();
    let ptr = waiters.0[irq];
    if ptr.is_null() { return; }

    // Take ownership back from the waiters table.
    waiters.0[irq] = core::ptr::null_mut();
    drop(waiters);

    // Reconstruct Box, mark Ready, push to run queue.
    let mut thread = unsafe { Box::from_raw(ptr) };
    let tid = thread.id;
    thread.state = ThreadState::Ready;

    let cpu_local = unsafe { CpuLocal::get_mut() };
    let rq = unsafe { &mut *cpu_local.run_queue };
    rq.push(thread);

    kprintln!("[syscall] IRQ {} woke thread {}", irq, tid);
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
        SYS_PORT_OUT => {
            let slot = frame.rdi;
            let port = frame.rsi;
            let value = frame.rdx;
            sys_port_out(slot, port, value)
        }
        SYS_PORT_IN => {
            let slot = frame.rdi;
            let port = frame.rsi;
            sys_port_in(frame, slot, port)
        }
        SYS_WAIT_IRQ => {
            let slot = frame.rdi;
            sys_wait_irq(slot)
        }
        SYS_SPAWN_PROCESS => {
            sys_spawn_process()
        }
        SYS_ALLOC_MEMORY => {
            let alloc_slot = frame.rdi;
            let target_slot = frame.rsi;
            sys_alloc_memory(alloc_slot, target_slot)
        }
        SYS_MAP_MEMORY => {
            let proc_slot = frame.rdi;
            let frame_slot = frame.rsi;
            let vaddr = frame.rdx;
            let flags = frame.r10;
            sys_map_memory(proc_slot, frame_slot, vaddr, flags)
        }
        SYS_DELEGATE => {
            let proc_slot = frame.rdi;
            let src_slot = frame.rsi;
            let dst_slot = frame.rdx;
            sys_delegate(proc_slot, src_slot, dst_slot)
        }
        SYS_SPAWN_THREAD => {
            let proc_slot = frame.rdi;
            let user_rip = frame.rsi;
            let user_rsp = frame.rdx;
            sys_spawn_thread(proc_slot, user_rip, user_rsp)
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
    let process = unsafe { &*thread.process };

    // 1. Validate capability
    let cap = match process.cnode.lookup(slot as usize) {
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
    let process = unsafe { &*thread.process };

    // 1. Validate capability
    let cap = match process.cnode.lookup(slot as usize) {
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
// SYS_PORT_OUT — Write a byte to an I/O port
// =============================================================================

/// Writes a byte to a hardware I/O port, gated by an IoPort capability.
///
/// The kernel mediates ALL port I/O from userspace. Ring 3 code cannot
/// execute IN/OUT instructions directly (#GP). Instead, the user calls
/// SYS_PORT_OUT and the kernel validates the IoPort capability before
/// performing the privileged OUT instruction.
///
/// # Arguments (from syscall registers)
///   - slot:  CNode slot index containing an IoPort capability with WRITE
///   - port:  16-bit I/O port address
///   - value: byte value to write (low 8 bits of u64)
///
/// # Returns
///   0 on success. Error codes like the other syscalls.
fn sys_port_out(slot: u64, port: u64, value: u64) -> u64 {
    let cpu_local = unsafe { CpuLocal::get() };
    let thread = unsafe { &*cpu_local.current_thread };
    let process = unsafe { &*thread.process };

    // 1. Validate capability
    let cap = match process.cnode.lookup(slot as usize) {
        Some(c) => c,
        None => {
            kprintln!("[syscall] SYS_PORT_OUT: thread {} bad slot {}", thread.id, slot);
            return u64::MAX;
        }
    };

    // 2. Check it's an IoPort with WRITE rights
    let (base, size) = match cap.object {
        CapObject::IoPort { base, size } => {
            if !cap.rights.contains(CapRights::WRITE) {
                kprintln!("[syscall] SYS_PORT_OUT: thread {} no WRITE right on slot {}",
                    thread.id, slot);
                return u64::MAX - 1;
            }
            (base, size)
        }
        _ => {
            kprintln!("[syscall] SYS_PORT_OUT: thread {} slot {} is not an IoPort",
                thread.id, slot);
            return u64::MAX - 2;
        }
    };

    // 3. Validate port is within the capability's range
    let port16 = port as u16;
    if port16 < base || port16 >= base + size {
        kprintln!("[syscall] SYS_PORT_OUT: thread {} port {:#06X} outside cap range [{:#06X}..{:#06X})",
            thread.id, port16, base, base + size);
        return u64::MAX - 4;
    }

    // 4. Perform the privileged OUT instruction
    let byte = value as u8;
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port16,
            in("al") byte,
            options(nomem, nostack, preserves_flags)
        );
    }

    0 // Success
}

// =============================================================================
// SYS_PORT_IN — Read a byte from an I/O port
// =============================================================================

/// Reads a byte from a hardware I/O port, gated by an IoPort capability.
///
/// # Arguments (from syscall registers)
///   - slot: CNode slot index containing an IoPort capability with READ
///   - port: 16-bit I/O port address
///
/// # Returns
///   RAX = 0 on success. The read byte is placed in frame.rdi so the
///   user sees it in RDI after SYSRET.
fn sys_port_in(frame: &mut SyscallFrame, slot: u64, port: u64) -> u64 {
    let cpu_local = unsafe { CpuLocal::get() };
    let thread = unsafe { &*cpu_local.current_thread };
    let process = unsafe { &*thread.process };

    // 1. Validate capability
    let cap = match process.cnode.lookup(slot as usize) {
        Some(c) => c,
        None => {
            kprintln!("[syscall] SYS_PORT_IN: thread {} bad slot {}", thread.id, slot);
            return u64::MAX;
        }
    };

    // 2. Check it's an IoPort with READ rights
    let (base, size) = match cap.object {
        CapObject::IoPort { base, size } => {
            if !cap.rights.contains(CapRights::READ) {
                kprintln!("[syscall] SYS_PORT_IN: thread {} no READ right on slot {}",
                    thread.id, slot);
                return u64::MAX - 1;
            }
            (base, size)
        }
        _ => {
            kprintln!("[syscall] SYS_PORT_IN: thread {} slot {} is not an IoPort",
                thread.id, slot);
            return u64::MAX - 2;
        }
    };

    // 3. Validate port is within the capability's range
    let port16 = port as u16;
    if port16 < base || port16 >= base + size {
        kprintln!("[syscall] SYS_PORT_IN: thread {} port {:#06X} outside cap range [{:#06X}..{:#06X})",
            thread.id, port16, base, base + size);
        return u64::MAX - 4;
    }

    // 4. Perform the privileged IN instruction
    let byte: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            out("al") byte,
            in("dx") port16,
            options(nomem, nostack, preserves_flags)
        );
    }

    // 5. Return value in RDI (user sees it in RDI register after SYSRET)
    frame.rdi = byte as u64;

    0 // Success
}

// =============================================================================
// SYS_WAIT_IRQ — Block until a hardware interrupt fires
// =============================================================================

/// Blocks the calling thread until the specified hardware IRQ fires.
///
/// The thread's Box<Thread> ownership is transferred to the IRQ_WAITERS
/// table. When the IRQ fires, `notify_irq_waiters()` (called from
/// irq_dispatch in idt.rs) reconstructs the Box and pushes the thread
/// back to the run queue.
///
/// Single-waiter per IRQ: if another thread is already waiting on the
/// same IRQ, this returns an error.
///
/// # Arguments (from syscall registers)
///   - slot: CNode slot index containing an Interrupt capability
///
/// # Returns
///   0 on success (IRQ fired). Error codes as usual.
fn sys_wait_irq(slot: u64) -> u64 {
    let cpu_local = unsafe { CpuLocal::get_mut() };
    let thread = unsafe { &*cpu_local.current_thread };
    let process = unsafe { &*thread.process };

    // 1. Validate capability
    let cap = match process.cnode.lookup(slot as usize) {
        Some(c) => c,
        None => {
            kprintln!("[syscall] SYS_WAIT_IRQ: thread {} bad slot {}", thread.id, slot);
            return u64::MAX;
        }
    };

    // 2. Check it's an Interrupt capability
    let irq = match cap.object {
        CapObject::Interrupt { irq } => irq as usize,
        _ => {
            kprintln!("[syscall] SYS_WAIT_IRQ: thread {} slot {} is not an Interrupt",
                thread.id, slot);
            return u64::MAX - 2;
        }
    };

    if irq >= MAX_IRQ_LINES {
        kprintln!("[syscall] SYS_WAIT_IRQ: IRQ {} out of range", irq);
        return u64::MAX - 4;
    }

    kprintln!("[syscall] SYS_WAIT_IRQ: thread {} blocking on IRQ {}", thread.id, irq);

    // 3. Take Box ownership of current thread (IF already 0 from FMASK)
    let current_ptr = cpu_local.current_thread;
    let mut current_box = unsafe { Box::from_raw(current_ptr) };
    current_box.state = ThreadState::BlockedRecv;

    // 4. Try to register in the IRQ waiters table
    {
        let mut waiters = IRQ_WAITERS.lock();
        if !waiters.0[irq].is_null() {
            // Another thread already waiting — restore and error
            kprintln!("[syscall] SYS_WAIT_IRQ: IRQ {} already has a waiter", irq);
            current_box.state = ThreadState::Running;
            let _ = Box::into_raw(current_box); // leak back to CpuLocal ownership
            return u64::MAX - 5;
        }
        waiters.0[irq] = Box::into_raw(current_box);
    }

    // 5. Yield the CPU. We'll resume when notify_irq_waiters() wakes us.
    unsafe { crate::sched::scheduler::schedule(); }

    // 6. We're back — IRQ fired and we were woken
    unsafe { core::arch::asm!("sti", options(nomem, nostack)); }

    0 // Success
}

// =============================================================================
// SYS_SPAWN_PROCESS — Create a new empty process (Syscall 6)
// =============================================================================

/// Creates a new process with a fresh isolated PML4 and empty CNode.
///
/// The kernel allocates the Process, registers it in the global PROCESS_TABLE,
/// then finds the first empty slot in the **caller's** CNode and inserts a
/// `CapObject::Process { pid }` capability with full rights.
///
/// # Returns
///   RAX = slot index where the Process capability was inserted (on success).
///   Error codes:
///   - `u64::MAX`     — caller's CNode is full (no empty slots)
///   - `u64::MAX - 1` — internal error (process allocation failed)
fn sys_spawn_process() -> u64 {
    use crate::cap::cnode::{Capability};
    use crate::sched::process::{self, Process};

    let cpu_local = unsafe { CpuLocal::get_mut() };
    let thread = unsafe { &mut *cpu_local.current_thread };
    let process = unsafe { &mut *thread.process };

    // 1. Create the new child process
    let child = Box::new(Process::new("user-proc"));
    let child_pid = child.pid;
    let child_ptr = Box::into_raw(child);

    // 2. Register it in the global process table
    process::register_process(child_pid, child_ptr);

    // 3. Mint a Process capability into the first empty slot of caller's CNode
    let cap = Capability::new(
        CapObject::Process { pid: child_pid },
        CapRights::ALL,
    );
    match process.cnode.insert(cap) {
        Some(slot) => {
            kprintln!("[syscall] SYS_SPAWN_PROCESS: PID {} created child PID {} → slot {}",
                process.pid, child_pid, slot);
            slot as u64
        }
        None => {
            kprintln!("[syscall] SYS_SPAWN_PROCESS: PID {} CNode full, cannot mint Process cap",
                process.pid);
            // TODO: Destroy the process and reclaim memory
            u64::MAX
        }
    }
}

// =============================================================================
// SYS_ALLOC_MEMORY — Allocate a physical frame (Syscall 7)
// =============================================================================

/// Allocates a zeroed physical frame from the PMM, gated by a PmmAllocator
/// capability.
///
/// # Arguments
///   - alloc_slot: CNode slot containing a PmmAllocator capability (WRITE)
///   - target_slot: CNode slot where the new MemoryFrame cap will be placed
///
/// # Returns
///   0 on success. Error codes:
///   - `u64::MAX`     — invalid alloc_slot (empty or out of bounds)
///   - `u64::MAX - 1` — alloc_slot is not a PmmAllocator capability
///   - `u64::MAX - 2` — insufficient rights (no WRITE)
///   - `u64::MAX - 3` — target_slot out of bounds or already occupied
///   - `u64::MAX - 4` — PMM out of memory
fn sys_alloc_memory(alloc_slot: u64, target_slot: u64) -> u64 {
    use crate::cap::cnode::Capability;
    use crate::memory::pmm;

    let cpu_local = unsafe { CpuLocal::get_mut() };
    let thread = unsafe { &*cpu_local.current_thread };
    let process = unsafe { &mut *thread.process };

    // 1. Validate PmmAllocator capability
    let cap = match process.cnode.lookup(alloc_slot as usize) {
        Some(c) => c,
        None => {
            kprintln!("[syscall] SYS_ALLOC_MEMORY: PID {} bad alloc slot {}",
                process.pid, alloc_slot);
            return u64::MAX;
        }
    };

    match cap.object {
        CapObject::PmmAllocator => {}
        _ => {
            kprintln!("[syscall] SYS_ALLOC_MEMORY: PID {} slot {} is not PmmAllocator",
                process.pid, alloc_slot);
            return u64::MAX - 1;
        }
    }

    if !cap.rights.contains(CapRights::WRITE) {
        kprintln!("[syscall] SYS_ALLOC_MEMORY: PID {} no WRITE right on PmmAllocator",
            process.pid);
        return u64::MAX - 2;
    }

    // 2. Allocate a zeroed physical frame
    let phys = match pmm::alloc_frame_zeroed() {
        Some(p) => p,
        None => {
            kprintln!("[syscall] SYS_ALLOC_MEMORY: PMM out of memory");
            return u64::MAX - 4;
        }
    };

    // 3. Mint MemoryFrame capability into target slot
    let mem_cap = Capability::new(
        CapObject::MemoryFrame { phys: phys.as_u64(), order: 0 },
        CapRights::ALL,
    );
    match process.cnode.insert_at(target_slot as usize, mem_cap) {
        Ok(()) => {
            kprintln!("[syscall] SYS_ALLOC_MEMORY: PID {} allocated frame P:{:#010X} → slot {}",
                process.pid, phys.as_u64(), target_slot);
            0
        }
        Err(()) => {
            kprintln!("[syscall] SYS_ALLOC_MEMORY: PID {} target slot {} invalid/occupied",
                process.pid, target_slot);
            // TODO: Free the frame back to PMM
            u64::MAX - 3
        }
    }
}

// =============================================================================
// SYS_MAP_MEMORY — Map a frame into a process's address space (Syscall 8)
// =============================================================================

/// Maps a physical frame (held by a MemoryFrame capability) into the address
/// space of a target process (held by a Process capability).
///
/// The caller must hold BOTH capabilities in their CNode.
///
/// # Arguments
///   - proc_slot:  CNode slot containing Process capability
///   - frame_slot: CNode slot containing MemoryFrame capability
///   - vaddr:      Requested virtual address (must be page-aligned, lower half)
///   - flags:      Page table flags encoded as a bitmask:
///                   bit 0 = WRITABLE
///                   bit 1 = EXECUTABLE (inversely: if clear, set NO_EXECUTE)
///                 PRESENT and USER are always set.
///
/// # Returns
///   0 on success. Error codes:
///   - `u64::MAX`     — invalid proc_slot
///   - `u64::MAX - 1` — proc_slot is not a Process capability
///   - `u64::MAX - 2` — invalid frame_slot
///   - `u64::MAX - 3` — frame_slot is not a MemoryFrame capability
///   - `u64::MAX - 4` — vaddr not page-aligned or not in user space
///   - `u64::MAX - 5` — process PID not found in global table
///   - `u64::MAX - 6` — vmm::map_page failed
fn sys_map_memory(proc_slot: u64, frame_slot: u64, vaddr: u64, flags_raw: u64) -> u64 {
    use crate::memory::address::{PhysAddr, VirtAddr, PAGE_SIZE};
    use crate::memory::vmm::{self, PageTableFlags};
    use crate::sched::process;

    let cpu_local = unsafe { CpuLocal::get_mut() };
    let thread = unsafe { &*cpu_local.current_thread };
    let caller = unsafe { &mut *thread.process };

    // 1. Validate Process capability
    let proc_cap = match caller.cnode.lookup(proc_slot as usize) {
        Some(c) => c,
        None => {
            kprintln!("[syscall] SYS_MAP_MEMORY: PID {} bad proc slot {}",
                caller.pid, proc_slot);
            return u64::MAX;
        }
    };

    let target_pid = match proc_cap.object {
        CapObject::Process { pid } => pid,
        _ => {
            kprintln!("[syscall] SYS_MAP_MEMORY: PID {} slot {} is not a Process cap",
                caller.pid, proc_slot);
            return u64::MAX - 1;
        }
    };

    // 2. Validate MemoryFrame capability
    let frame_cap = match caller.cnode.lookup(frame_slot as usize) {
        Some(c) => c,
        None => {
            kprintln!("[syscall] SYS_MAP_MEMORY: PID {} bad frame slot {}",
                caller.pid, frame_slot);
            return u64::MAX - 2;
        }
    };

    let frame_phys = match frame_cap.object {
        CapObject::MemoryFrame { phys, .. } => phys,
        _ => {
            kprintln!("[syscall] SYS_MAP_MEMORY: PID {} slot {} is not a MemoryFrame cap",
                caller.pid, frame_slot);
            return u64::MAX - 3;
        }
    };

    // 3. Validate vaddr: must be page-aligned, in the lower canonical half
    if vaddr % PAGE_SIZE as u64 != 0 || vaddr >= 0x0000_8000_0000_0000 {
        kprintln!("[syscall] SYS_MAP_MEMORY: bad vaddr {:#018X}", vaddr);
        return u64::MAX - 4;
    }

    // 4. Look up the target process
    let target_ptr = match process::lookup_process(target_pid) {
        Some(p) => p,
        None => {
            kprintln!("[syscall] SYS_MAP_MEMORY: PID {} not found in process table",
                target_pid);
            return u64::MAX - 5;
        }
    };

    let target = unsafe { &*target_ptr };
    let pml4_phys = target.pml4();

    // 5. Translate flags
    //    User always gets PRESENT | USER.
    //    bit 0 of flags_raw = WRITABLE
    //    bit 1 of flags_raw = EXECUTABLE (if clear → NO_EXECUTE)
    let mut pt_flags = PageTableFlags::PRESENT | PageTableFlags::USER;
    if flags_raw & 0x01 != 0 {
        pt_flags |= PageTableFlags::WRITABLE;
    }
    if flags_raw & 0x02 == 0 {
        pt_flags |= PageTableFlags::NO_EXECUTE;
    }

    // 6. Map the page in the target process's PML4
    let result = unsafe {
        vmm::map_page(
            pml4_phys,
            VirtAddr::new(vaddr),
            PhysAddr::new(frame_phys),
            pt_flags,
        )
    };

    match result {
        Ok(()) => {
            kprintln!("[syscall] SYS_MAP_MEMORY: mapped P:{:#010X} → V:{:#010X} in PID {} (flags={:#X})",
                frame_phys, vaddr, target_pid, pt_flags.bits());
            0
        }
        Err(e) => {
            kprintln!("[syscall] SYS_MAP_MEMORY: map_page failed for PID {}: {:?}",
                target_pid, e);
            u64::MAX - 6
        }
    }
}

// =============================================================================
// SYS_DELEGATE — Copy a capability to another process (Syscall 9)
// =============================================================================

/// Copies a capability from the caller's CNode to a target process's CNode.
///
/// The caller must hold a Process capability for the destination process.
/// An exact copy of the source capability is inserted at the specified
/// destination slot.
///
/// # Arguments
///   - proc_slot: CNode slot containing Process capability (destination)
///   - src_slot:  Slot index in the caller's CNode to copy FROM
///   - dst_slot:  Slot index in the target process's CNode to copy TO
///
/// # Returns
///   0 on success. Error codes:
///   - `u64::MAX`     — invalid proc_slot
///   - `u64::MAX - 1` — proc_slot is not a Process capability
///   - `u64::MAX - 2` — invalid src_slot (empty or out of bounds)
///   - `u64::MAX - 3` — target PID not found in process table
///   - `u64::MAX - 4` — destination slot out of bounds or occupied
fn sys_delegate(proc_slot: u64, src_slot: u64, dst_slot: u64) -> u64 {
    use crate::sched::process;

    let cpu_local = unsafe { CpuLocal::get_mut() };
    let thread = unsafe { &*cpu_local.current_thread };
    let caller = unsafe { &mut *thread.process };

    // 1. Validate Process capability
    let proc_cap = match caller.cnode.lookup(proc_slot as usize) {
        Some(c) => c,
        None => {
            kprintln!("[syscall] SYS_DELEGATE: PID {} bad proc slot {}",
                caller.pid, proc_slot);
            return u64::MAX;
        }
    };

    let target_pid = match proc_cap.object {
        CapObject::Process { pid } => pid,
        _ => {
            kprintln!("[syscall] SYS_DELEGATE: PID {} slot {} is not a Process cap",
                caller.pid, proc_slot);
            return u64::MAX - 1;
        }
    };

    // 2. Validate and read source capability (copy out of borrow)
    let src_cap = match caller.cnode.lookup(src_slot as usize) {
        Some(c) => *c, // Copy the value so we release the borrow
        None => {
            kprintln!("[syscall] SYS_DELEGATE: PID {} bad source slot {}",
                caller.pid, src_slot);
            return u64::MAX - 2;
        }
    };

    // 3. Look up target process
    let target_ptr = match process::lookup_process(target_pid) {
        Some(p) => p,
        None => {
            kprintln!("[syscall] SYS_DELEGATE: PID {} not found in process table",
                target_pid);
            return u64::MAX - 3;
        }
    };

    let target = unsafe { &mut *target_ptr };

    // 4. Insert into target's CNode at the specified slot
    match target.cnode.insert_at(dst_slot as usize, src_cap) {
        Ok(()) => {
            kprintln!("[syscall] SYS_DELEGATE: PID {} [{} → PID {} [{}]: {:?}",
                caller.pid, src_slot, target_pid, dst_slot, src_cap.object);
            0
        }
        Err(()) => {
            kprintln!("[syscall] SYS_DELEGATE: PID {} target slot {} invalid/occupied",
                target_pid, dst_slot);
            u64::MAX - 4
        }
    }
}

// =============================================================================
// SYS_SPAWN_THREAD — Create a Ring 3 thread in a process (Syscall 10)
// =============================================================================

/// Creates a new Ring 3 thread inside a target process and pushes it to
/// the run queue.
///
/// The caller must hold a Process capability for the target process.
/// The thread starts executing at `user_rip` with stack pointer `user_rsp`.
///
/// # Arguments
///   - proc_slot: CNode slot containing Process capability
///   - user_rip:  Virtual address for the thread's entry point (in target's VA)
///   - user_rsp:  Top of user stack (in target's VA)
///
/// # Returns
///   The new thread's TID on success. Error codes:
///   - `u64::MAX`     — invalid proc_slot
///   - `u64::MAX - 1` — proc_slot is not a Process capability
///   - `u64::MAX - 2` — target PID not found in process table
fn sys_spawn_thread(proc_slot: u64, user_rip: u64, user_rsp: u64) -> u64 {
    use crate::sched::process;

    let cpu_local = unsafe { CpuLocal::get_mut() };
    let thread = unsafe { &*cpu_local.current_thread };
    let caller = unsafe { &*thread.process };

    // 1. Validate Process capability
    let proc_cap = match caller.cnode.lookup(proc_slot as usize) {
        Some(c) => c,
        None => {
            kprintln!("[syscall] SYS_SPAWN_THREAD: PID {} bad proc slot {}",
                caller.pid, proc_slot);
            return u64::MAX;
        }
    };

    let target_pid = match proc_cap.object {
        CapObject::Process { pid } => pid,
        _ => {
            kprintln!("[syscall] SYS_SPAWN_THREAD: PID {} slot {} is not a Process cap",
                caller.pid, proc_slot);
            return u64::MAX - 1;
        }
    };

    // 2. Look up target process
    let target_ptr = match process::lookup_process(target_pid) {
        Some(p) => p,
        None => {
            kprintln!("[syscall] SYS_SPAWN_THREAD: PID {} not found in process table",
                target_pid);
            return u64::MAX - 2;
        }
    };

    // 3. Create user thread inside the target process
    let mut new_thread = Thread::new("user-thread", ring3_entry, 0, target_ptr);
    new_thread.user_rip = user_rip;
    new_thread.user_rsp = user_rsp;
    let tid = new_thread.id;

    kprintln!("[syscall] SYS_SPAWN_THREAD: PID {} spawning thread {} → RIP={:#018X} RSP={:#018X}",
        target_pid, tid, user_rip, user_rsp);

    // 4. Push to run queue
    crate::sched::scheduler::spawn_thread(new_thread);

    tid
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
/// - `process`: Raw pointer to the Process that owns this thread.
///
/// # Returns
/// A `Box<Thread>` ready to be enqueued. The caller should:
/// 1. Install capabilities on the Process's CNode before spawning.
/// 2. Call `spawn_thread()` to enqueue it.
pub fn spawn_user(
    name: &str,
    user_rip: u64,
    user_rsp: u64,
    process: *mut crate::sched::process::Process,
) -> Box<Thread> {
    let mut thread = Thread::new(name, ring3_entry, 0, process);
    thread.user_rip = user_rip;
    thread.user_rsp = user_rsp;
    kprintln!("[syscall] Created user thread {} '{}' → Ring 3 @ RIP={:#018X} RSP={:#018X}",
        thread.id, name, user_rip, user_rsp);
    thread
}
