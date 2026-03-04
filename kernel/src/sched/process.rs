// =============================================================================
// MinimalOS NextGen — Process Control Block (PCB)
// =============================================================================
//
// A Process is the unit of isolation. It owns:
//   - A PML4 page table (address space)
//   - A CNode (capability table / security context)
//   - A unique PID
//
// Threads no longer carry their own address space or capability table.
// Instead, every Thread holds a raw pointer to its parent Process.
// Multiple threads within the same process share the PML4 and CNode.
//
// WHY SEPARATE PROCESS AND THREAD?
//   seL4 and other formally verified microkernels make this exact split.
//   The Thread is the unit of *scheduling*. The Process (address space +
//   capabilities) is the unit of *isolation*. Mixing them creates the
//   Sprint-7-era bug: every user thread ran on KERNEL_PML4 because the
//   Thread struct didn't model address space ownership correctly.
//
// THE KERNEL PML4 RULE:
//   Kernel threads (Thread.user_rip == 0) belong to the "kernel process"
//   — they use KERNEL_PML4 and have no CNode (all access is implicit).
//   User threads (Thread.user_rip != 0) belong to a real Process with
//   an isolated lower-half PML4 and an explicit CNode for syscall gating.
//
// =============================================================================

extern crate alloc;

use core::sync::atomic::{AtomicU64, Ordering};

use alloc::collections::BTreeMap;

use crate::cap::cnode::CNode;
use crate::kprintln;
use crate::memory::address::PhysAddr;
use crate::memory::pml4;
use crate::sync::spinlock::SpinLock;

// =============================================================================
// Global Process Table
// =============================================================================
//
// Maps PID → *mut Process. Populated by Process::new() and Process::kernel().
// Read by syscall handlers to resolve CapObject::Process { pid } capabilities
// back to the actual Process struct.
//
// SAFETY: The raw pointers are heap-allocated via Box::into_raw and never freed
// during kernel lifetime.  SpinLock ensures atomicity of insert/lookup.
// =============================================================================

/// Wrapper to satisfy Send bound for SpinLock<T: Send>.
pub(crate) struct ProcessTableInner(BTreeMap<u64, *mut Process>);

// SAFETY: The *mut Process pointers represent stable heap allocations that are
// never freed. Access is serialized by the enclosing SpinLock.
unsafe impl Send for ProcessTableInner {}

/// Global table mapping PID → *mut Process.
///
/// Syscalls use this to look up Process pointers from PID values stored in
/// `CapObject::Process { pid }` capabilities.
pub(crate) static PROCESS_TABLE: SpinLock<ProcessTableInner> =
    SpinLock::new(ProcessTableInner(BTreeMap::new()));

/// Inserts a process into the global table.
///
/// Called automatically by `Process::register()` and from main.rs after
/// `Box::into_raw`.
pub fn register_process(pid: u64, ptr: *mut Process) {
    PROCESS_TABLE.lock().0.insert(pid, ptr);
}

/// Looks up a process by PID. Returns None if not found.
pub fn lookup_process(pid: u64) -> Option<*mut Process> {
    PROCESS_TABLE.lock().0.get(&pid).copied()
}

/// Global process ID counter. PID 0 is reserved for the kernel.
static NEXT_PID: AtomicU64 = AtomicU64::new(1);

/// Process Control Block — the kernel's representation of an address space.
///
/// A Process is created by the kernel (during boot for the init process,
/// or via SYS_SPAWN_PROCESS for subsequent processes). It is never directly
/// accessible from Ring 3 — user code manipulates processes through
/// CapObject::Process capabilities and the corresponding syscalls.
pub struct Process {
    /// Unique process identifier. PID 0 = kernel pseudo-process.
    pub pid: u64,

    /// Physical address of this process's PML4 page table root.
    /// For user processes: built by `pml4::build_user_pml4()` — kernel-half
    /// mirrored from KERNEL_PML4, lower-half starts empty.
    /// For the kernel pseudo-process: same as KERNEL_PML4.
    pub pml4_phys: u64,

    /// Capability table — the process's security context.
    /// All threads within this process share the same CNode.
    /// Syscalls validate capabilities against this table.
    pub cnode: CNode,

    /// Human-readable name for debugging.
    pub name: [u8; 32],
    pub name_len: usize,
}

impl Process {
    /// Creates a new user process with an isolated PML4.
    ///
    /// The PML4 has the kernel higher-half mirrored (so SYSCALL/interrupts
    /// work) and an empty lower-half (user pages mapped separately).
    ///
    /// # Parameters
    /// - `name`: Human-readable name for debugging/logging.
    pub fn new(name: &str) -> Self {
        let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
        let user_pml4 = pml4::build_user_pml4();

        let mut name_buf = [0u8; 32];
        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min(32);
        name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

        kprintln!("[process] Created PID {} '{}' (PML4 @ phys {:#010X})",
            pid, name, user_pml4.as_u64());

        Self {
            pid,
            pml4_phys: user_pml4.as_u64(),
            cnode: CNode::new(),
            name: name_buf,
            name_len: copy_len,
        }
    }

    /// Creates the kernel pseudo-process (PID 0).
    ///
    /// Uses KERNEL_PML4 directly — kernel threads share the pristine
    /// kernel address space. The CNode is empty because kernel code
    /// has implicit access to everything.
    pub fn kernel() -> Self {
        let kernel_pml4 = pml4::KERNEL_PML4.load(Ordering::SeqCst);

        Self {
            pid: 0,
            pml4_phys: kernel_pml4,
            cnode: CNode::new(),
            name: {
                let mut buf = [0u8; 32];
                let n = b"kernel";
                buf[..n.len()].copy_from_slice(n);
                buf
            },
            name_len: 6,
        }
    }

    /// Returns the PML4 physical address as a `PhysAddr`.
    pub fn pml4(&self) -> PhysAddr {
        PhysAddr::new(self.pml4_phys)
    }

    /// Returns the process name as a string slice.
    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("???")
    }
}
