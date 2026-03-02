// =============================================================================
// MinimalOS NextGen — CPU-Local Storage (IA32_GS_BASE)
// =============================================================================
//
// Each CPU core needs its own private variables (run queue, current thread,
// LAPIC ID, etc.) without taking a lock. x86_64 provides IA32_GS_BASE
// (MSR 0xC0000101) for this purpose — the GS segment base points to a
// per-core data structure, and any core can read its own data via gs:offset.
//
// USAGE:
//   During boot, the BSP allocates a CpuLocal on the heap and writes its
//   virtual address to IA32_GS_BASE. Each AP does the same in ap_rust_entry.
//   Then `CpuLocal::get()` returns the local core's data without locking.
//
// =============================================================================

use core::ptr;

use crate::arch::cpu;
use crate::kprintln;

/// IA32_GS_BASE MSR index.
const IA32_GS_BASE: u32 = 0xC000_0101;

/// Per-core local data. Accessed via the GS segment register (no locks).
///
/// The `self_ptr` field at offset 0 allows fast `gs:0` access to verify
/// the GS base is correctly set.
#[repr(C)]
pub struct CpuLocal {
    /// Pointer to self — allows `mov rax, gs:[0]` to get the CpuLocal address.
    pub self_ptr: *const CpuLocal,
    /// LAPIC ID of this core.
    pub lapic_id: u32,
    /// Core index (0 = BSP, 1..N-1 = APs).
    pub core_index: u32,
    /// Pointer to the currently running thread's TCB (or null if idle).
    pub current_thread: *mut super::thread::Thread,
    /// Pointer to this core's idle thread TCB.
    pub idle_thread: *mut super::thread::Thread,
    /// Pointer to this core's run queue (heap-allocated).
    pub run_queue: *mut super::scheduler::RunQueue,
    /// Whether this core is fully initialized and running the scheduler.
    pub online: bool,
}

// SAFETY: CpuLocal is only accessed from the core it belongs to (via gs:).
// Inter-core access requires explicit synchronization via atomics.
unsafe impl Send for CpuLocal {}
unsafe impl Sync for CpuLocal {}

impl CpuLocal {
    /// Creates a new, zeroed CpuLocal.
    pub fn new(lapic_id: u32, core_index: u32) -> Self {
        Self {
            self_ptr: ptr::null(),
            lapic_id,
            core_index,
            current_thread: ptr::null_mut(),
            idle_thread: ptr::null_mut(),
            run_queue: ptr::null_mut(),
            online: false,
        }
    }

    /// Installs this CpuLocal as the current core's GS-based local storage.
    ///
    /// Writes the address of this struct to IA32_GS_BASE. After this call,
    /// `CpuLocal::get()` returns a reference to this struct on the calling core.
    ///
    /// # Safety
    /// - Must be called exactly once per core during boot.
    /// - The CpuLocal must live for the lifetime of the kernel (leaked Box).
    pub unsafe fn install(&mut self) {
        self.self_ptr = self as *const CpuLocal;
        let addr = self.self_ptr as u64;
        unsafe { cpu::write_msr(IA32_GS_BASE, addr); }
        kprintln!("[percpu] Core {} (LAPIC {}) GS base set to {:#018X}",
            self.core_index, self.lapic_id, addr);
    }

    /// Returns a reference to the current core's CpuLocal data.
    ///
    /// Reads `gs:0` to get the self_ptr, which is the CpuLocal address.
    ///
    /// # Safety
    /// - `install()` must have been called on this core first.
    /// - Must be called from kernel mode (GS base is set to kernel CpuLocal).
    #[inline]
    pub unsafe fn get() -> &'static CpuLocal {
        let ptr: *const CpuLocal;
        unsafe {
            core::arch::asm!(
                "mov {}, gs:[0]",
                out(reg) ptr,
                options(nostack, preserves_flags),
            );
            &*ptr
        }
    }

    /// Returns a mutable reference to the current core's CpuLocal data.
    ///
    /// # Safety
    /// Same as `get()`, plus caller must ensure no concurrent access.
    #[inline]
    pub unsafe fn get_mut() -> &'static mut CpuLocal {
        let ptr: *mut CpuLocal;
        unsafe {
            core::arch::asm!(
                "mov {}, gs:[0]",
                out(reg) ptr,
                options(nostack, preserves_flags),
            );
            &mut *ptr
        }
    }
}
