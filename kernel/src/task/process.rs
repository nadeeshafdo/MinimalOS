//! Process management — PCB, context switching, and scheduling.
//!
//! [061] ProcessControlBlock — stores per-task register state.
//! [062] context_switch — assembly routine to swap stacks.
//! [063] Round-Robin scheduler with VecDeque of tasks.

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::string::String;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

// ── Process identifiers ─────────────────────────────────────────

/// Monotonically increasing PID counter.
static NEXT_PID: AtomicU64 = AtomicU64::new(1);

/// Allocate a unique PID.
fn alloc_pid() -> u64 {
    NEXT_PID.fetch_add(1, Ordering::Relaxed)
}

// ── Process state ───────────────────────────────────────────────

/// The possible states of a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ProcessState {
    /// Ready to be scheduled.
    Ready,
    /// Currently running on the CPU.
    Running,
    /// Blocked waiting for I/O or an event.
    Blocked,
    /// Terminated, awaiting cleanup.
    Dead,
}

// ── Context (saved registers) ───────────────────────────────────

/// Callee-saved register context for context_switch().
///
/// The context_switch assembly pushes these onto the old task's kernel
/// stack and pops them from the new task's kernel stack.
///
/// Layout must match the push/pop order in `context_switch_asm`.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
#[allow(dead_code)]
pub struct Context {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub rip: u64,   // return address (pushed by `call`)
}

#[allow(dead_code)]
impl Context {
    /// Create a zero-initialised context.
    pub const fn empty() -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbx: 0,
            rbp: 0,
            rip: 0,
        }
    }
}

// ── Kernel stack ────────────────────────────────────────────────

/// Size of each task's kernel-mode stack (32 KiB).
///
/// Needs to be large enough for nested interrupt frames, the syscall
/// stub, and any kernel functions called from syscall context (e.g.
/// `spawn_from_ramdisk` which does ELF parsing + page mapping).
pub const KERNEL_STACK_SIZE: usize = 4096 * 8;

/// An aligned kernel stack.
#[repr(C, align(16))]
pub struct KernelStack {
    pub data: [u8; KERNEL_STACK_SIZE],
}

impl KernelStack {
    /// Top of the stack (stacks grow downward).
    pub fn top(&self) -> u64 {
        self.data.as_ptr() as u64 + KERNEL_STACK_SIZE as u64
    }
}

// ── Process Control Block ───────────────────────────────────────

/// [061] The Process Control Block — stores everything the kernel
/// needs to manage and schedule a single task.
#[allow(dead_code)]
pub struct Process {
    /// Unique process identifier.
    pub pid: u64,
    /// Human-readable name (e.g. "init", "shell").
    pub name: String,
    /// Current scheduling state.
    pub state: ProcessState,
    /// Saved kernel RSP (points into `kernel_stack`).
    /// Updated by context_switch when suspending.
    pub kernel_rsp: u64,
    /// CR3 — physical address of this task's PML4.
    pub cr3: u64,
    /// The user-mode entry point (RIP for iretq).
    pub entry_point: u64,
    /// User-mode stack pointer.
    pub user_rsp: u64,
    /// Kernel stack for this process (heap-allocated).
    pub kernel_stack: Box<KernelStack>,
}

impl Process {
    /// Create a new process with the given name and user-space parameters.
    ///
    /// `cr3` is the page table root, `entry_point` is the user RIP,
    /// `user_rsp` is the user stack top.
    pub fn new(name: &str, cr3: u64, entry_point: u64, user_rsp: u64) -> Self {
        let pid = alloc_pid();
        // Allocate the kernel stack directly on the heap without placing
        // the full array on the current stack first (which would blow a
        // 32 KiB kernel stack when spawning from syscall context).
        let kernel_stack = unsafe {
            let layout = core::alloc::Layout::new::<KernelStack>();
            let ptr = alloc::alloc::alloc_zeroed(layout) as *mut KernelStack;
            if ptr.is_null() {
                panic!("failed to allocate kernel stack for PID {}", pid);
            }
            Box::from_raw(ptr)
        };

        Self {
            pid,
            name: String::from(name),
            state: ProcessState::Ready,
            kernel_rsp: 0, // will be set up by prepare_initial_stack()
            cr3,
            entry_point,
            user_rsp,
            kernel_stack,
        }
    }

    /// Prepare the kernel stack so that when `context_switch` pops
    /// from it for the first time, execution arrives at `task_entry_trampoline`.
    ///
    /// The stack is laid out as if `context_switch` had been called:
    ///   [top - 8]  rip (return address → trampoline)
    ///   [top - 16] rbp
    ///   [top - 24] rbx
    ///   [top - 32] r12
    ///   [top - 40] r13
    ///   [top - 48] r14
    ///   [top - 56] r15
    pub fn prepare_initial_stack(&mut self) {
        let top = self.kernel_stack.top();
        let sp = top - 7 * 8; // 7 u64s

        // SAFETY: we own this stack and it's big enough.
        unsafe {
            let ptr = sp as *mut u64;
            // Must match the pop order in context_switch_asm:
            //   pop r15, pop r14, pop r13, pop r12, pop rbx, pop rbp, ret
            ptr.add(0).write(0); // r15
            ptr.add(1).write(0); // r14
            ptr.add(2).write(0); // r13
            ptr.add(3).write(0); // r12
            ptr.add(4).write(0); // rbx
            ptr.add(5).write(0); // rbp
            ptr.add(6).write(task_entry_trampoline as u64); // rip (ret target)
        }

        self.kernel_rsp = sp;
    }
}

// ── Context switch ([062]) ──────────────────────────────────────

core::arch::global_asm!(
    ".global context_switch_asm",
    "context_switch_asm:",
    // rdi = &mut old_task.kernel_rsp
    // rsi = new_task.kernel_rsp
    //
    // Save callee-saved registers on old stack
    "push rbp",
    "push rbx",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    // Save old RSP
    "mov [rdi], rsp",
    // Load new RSP
    "mov rsp, rsi",
    // Restore callee-saved registers from new stack
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbx",
    "pop rbp",
    // Return to wherever the new task left off (pops RIP from stack)
    "ret",
);

extern "C" {
    /// Raw assembly context switch.
    ///
    /// Saves callee-saved registers on old stack, writes RSP to `*old_rsp_ptr`,
    /// loads new RSP, restores registers, and `ret`s to the new task's saved RIP.
    fn context_switch_asm(old_rsp_ptr: *mut u64, new_rsp: u64);
}

/// [062] Perform a context switch between two tasks.
///
/// # Safety
/// Both RSP values must point to valid, correctly laid-out kernel stacks.
#[allow(dead_code)]
pub unsafe fn context_switch(old: &mut Process, new: &Process) {
    unsafe {
        context_switch_asm(
            &mut old.kernel_rsp as *mut u64,
            new.kernel_rsp,
        );
    }
}

// ── Trampoline for first entry ──────────────────────────────────

/// When a newly-created task is switched to for the first time,
/// `context_switch_asm` will `ret` into this function.
///
/// It reads the current process's entry point and user RSP from
/// the global scheduler, then drops to Ring 3 via `iretq`.
extern "C" fn task_entry_trampoline() {
    // Read the current task's user-mode parameters from the scheduler.
    let (entry, user_rsp) = {
        let sched = SCHEDULER.lock();
        let current = sched.current().expect("trampoline: no current task");
        (current.entry_point, current.user_rsp)
    };

    klog::info!("[064] Entering user mode: RIP={:#x} RSP={:#x}", entry, user_rsp);

    // User CS = 0x20 | 3 = 0x23, User SS = 0x18 | 3 = 0x1b
    let frame = super::usermode::IretqFrame::new(entry, 0x23, 0x1b, user_rsp);
    unsafe {
        super::usermode::jump_to_ring3(&frame);
    }
}

// ── Scheduler ([063]) ───────────────────────────────────────────

/// The global scheduler instance, protected by a spinlock.
pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// [063] Round-Robin scheduler.
///
/// Maintains a queue of ready tasks and tracks which task is currently
/// running.  `schedule()` picks the next task and performs a context switch.
pub struct Scheduler {
    /// Ready queue (round-robin order).
    tasks: VecDeque<Process>,
    /// The currently running process (removed from the queue).
    current: Option<Process>,
}

#[allow(dead_code)]
impl Scheduler {
    pub const fn new() -> Self {
        Self {
            tasks: VecDeque::new(),
            current: None,
        }
    }

    /// Add a process to the ready queue.
    pub fn push(&mut self, task: Process) {
        self.tasks.push_back(task);
    }

    /// Number of tasks in the ready queue (excluding current).
    pub fn ready_count(&self) -> usize {
        self.tasks.len()
    }

    /// Total tasks (ready + current).
    pub fn task_count(&self) -> usize {
        self.tasks.len() + if self.current.is_some() { 1 } else { 0 }
    }

    /// Reference to the currently running process.
    pub fn current(&self) -> Option<&Process> {
        self.current.as_ref()
    }

    /// Mutable reference to the currently running process.
    pub fn current_mut(&mut self) -> Option<&mut Process> {
        self.current.as_mut()
    }

    /// Set the initial "current" process (used during kernel init).
    pub fn set_current(&mut self, task: Process) {
        self.current = Some(task);
    }

    /// Remove dead tasks from the ready queue.
    pub fn reap_dead(&mut self) {
        self.tasks.retain(|t| t.state != ProcessState::Dead);
    }

    /// Remove a dead current task (called by sys_exit).
    pub fn reap_current(&mut self) {
        if let Some(ref cur) = self.current {
            if cur.state == ProcessState::Dead {
                self.current = None;
            }
        }
    }

    /// [064] Pick the next task and context-switch to it.
    ///
    /// The current task is moved to the back of the ready queue,
    /// and the front task becomes the new current.
    ///
    /// # Safety
    /// Must be called with interrupts disabled (or from an interrupt context).
    pub unsafe fn schedule(&mut self) {
        // Nothing to switch to?
        if self.tasks.is_empty() {
            return;
        }

        // Take the current task out.
        let mut old = match self.current.take() {
            Some(t) => t,
            None => {
                // No current — just pop the next one and enter it.
                let mut new = self.tasks.pop_front().unwrap();
                new.state = ProcessState::Running;
                self.current = Some(new);
                return;
            }
        };

        // Pop the next ready task.
        let new = match self.tasks.pop_front() {
            Some(t) => t,
            None => {
                // Put old back — it's the only task.
                self.current = Some(old);
                return;
            }
        };

        // Move old to the back of the ready queue (unless it's dead).
        if old.state != ProcessState::Dead {
            old.state = ProcessState::Ready;
            self.tasks.push_back(old);
        }

        let mut new = new;
        new.state = ProcessState::Running;

        // Install `new` as current before switching, so the trampoline
        // can find it.
        self.current = Some(new);

        // We need raw pointers to do the switch while the scheduler
        // is borrowed.  This is safe because:
        //   - `old` is now in self.tasks (back)
        //   - `new` is now in self.current
        let old_ref = self.tasks.back_mut().unwrap();
        let new_ref = self.current.as_ref().unwrap();

        // Update SYSCALL_KERNEL_RSP so syscall entry uses the new task's
        // kernel stack.
        unsafe {
            let new_kstack_top = new_ref.kernel_stack.top();
            core::ptr::write_volatile(
                &raw mut crate::arch::syscall::SYSCALL_KERNEL_RSP,
                new_kstack_top,
            );
        }

        // Save old_rsp_ptr and new_rsp to local variables so we can
        // release the spinlock before the actual context switch.
        let old_rsp_ptr = &mut old_ref.kernel_rsp as *mut u64;
        let new_rsp = new_ref.kernel_rsp;

        // CRITICAL: We must NOT hold the SCHEDULER lock across the
        // context switch, because the target task's trampoline may
        // need to lock SCHEDULER to read its own entry point.
        //
        // We use raw asm directly here instead of the wrapper so we
        // can drop the lock right before switching.
        //
        // The caller must drop the MutexGuard before this point.
        // Since `self` is `&mut Scheduler` borrowed from the guard,
        // we can't drop the guard here.  Instead, we use a separate
        // free function `do_schedule()` that manages this.
        unsafe {
            context_switch_asm(old_rsp_ptr, new_rsp);
        }
    }
}

/// [064] Free-standing schedule function that safely manages the lock.
///
/// Acquires the scheduler lock, rearranges tasks, extracts the
/// raw RSP pointers needed for the switch, drops the lock, then
/// performs the context switch. This avoids deadlock when the
/// target task's trampoline needs to re-acquire the lock.
///
/// # Safety
/// Must be called with interrupts disabled or from interrupt context.
pub unsafe fn do_schedule() {
    // We use a static "dummy" to receive the old RSP when the old
    // task is dead (so we don't need a valid pointer into VecDeque).
    static mut DEAD_RSP: u64 = 0;

    let (old_rsp_ptr, new_rsp) = {
        let mut sched = SCHEDULER.lock();

        if sched.tasks.is_empty() {
            return;
        }

        let old = match sched.current.take() {
            Some(t) => t,
            None => return,
        };

        let new = match sched.tasks.pop_front() {
            Some(t) => t,
            None => {
                sched.current = Some(old);
                return;
            }
        };

        let old_is_dead = old.state == ProcessState::Dead;

        if !old_is_dead {
            let mut old = old;
            old.state = ProcessState::Ready;
            sched.tasks.push_back(old);
        }
        // else: drop old — it's dead

        let mut new = new;
        new.state = ProcessState::Running;
        sched.current = Some(new);

        // Update SYSCALL_KERNEL_RSP for the new task.
        let new_kstack_top = sched.current.as_ref().unwrap().kernel_stack.top();
        unsafe {
            core::ptr::write_volatile(
                &raw mut crate::arch::syscall::SYSCALL_KERNEL_RSP,
                new_kstack_top,
            );
            // Update TSS RSP0 so Ring 3→0 transitions (interrupts)
            // land on the new task's kernel stack.
            let tss = crate::traps::tss_ptr();
            if !tss.is_null() {
                crate::arch::tss::Tss::set_rsp0(tss, new_kstack_top);
            }
        }

        let old_rsp_ptr = if old_is_dead {
            // Task is dead, use dummy so context_switch_asm has
            // somewhere to write the RSP (which we'll never use).
            &raw mut DEAD_RSP
        } else {
            &mut sched.tasks.back_mut().unwrap().kernel_rsp as *mut u64
        };
        let new_rsp = sched.current.as_ref().unwrap().kernel_rsp;

        (old_rsp_ptr, new_rsp)
        // MutexGuard dropped here — lock is released before switch
    };

    unsafe {
        context_switch_asm(old_rsp_ptr, new_rsp);
        // The context switch may return from an interrupt context where
        // IF=0.  Re-enable interrupts so the timer can fire and drive
        // preemptive scheduling.
        core::arch::asm!("sti", options(nomem, nostack));
    }
}

// ── Spawn helper ([066]) ────────────────────────────────────────

/// Spawn a new process from an ELF file in the ramdisk.
///
/// Loads the ELF, maps its segments, allocates a user stack,
/// creates a `Process`, prepares its kernel stack, and pushes it
/// into the scheduler.
///
/// Returns the new PID on success, or an error message on failure.
pub fn spawn_from_ramdisk(path: &str) -> Result<u64, &'static str> {
    let ramdisk = crate::fs::ramdisk::get()
        .ok_or("ramdisk not initialised")?;

    let entry = crate::fs::tar::find_file(ramdisk, path)
        .ok_or("file not found in ramdisk")?;

    let elf = crate::fs::elf::parse(entry.data)
        .map_err(|_| "invalid ELF")?;

    klog::info!("[066] Spawning '{}': entry={:#x}", path, elf.entry);

    // We share the kernel page tables (same CR3) for simplicity.
    // All user mappings go into the same address space.
    // TODO: per-process page tables for isolation.
    let cr3: u64;
    unsafe {
        core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
    }

    // Map PT_LOAD segments.
    for phdr in elf.phdrs {
        if !phdr.is_load() {
            continue;
        }
        let vaddr = phdr.p_vaddr;
        let memsz = phdr.p_memsz as usize;
        let filesz = phdr.p_filesz as usize;
        let offset = phdr.p_offset as usize;

        let page_start = vaddr & !0xFFF;
        let page_end = (vaddr + memsz as u64 + 0xFFF) & !0xFFF;
        let num_pages = ((page_end - page_start) / 4096) as usize;

        for i in 0..num_pages {
            let page_virt = page_start + (i as u64) * 4096;
            if unsafe { crate::memory::paging::translate(page_virt) }.is_none() {
                let phys = crate::memory::pmm::alloc_frame()
                    .ok_or("out of physical memory")?;
                unsafe {
                    crate::memory::paging::map_page(
                        page_virt,
                        phys,
                        crate::memory::paging::PageFlags::USER_RW,
                    );
                    core::ptr::write_bytes(page_virt as *mut u8, 0, 4096);
                }
            }
        }

        if filesz > 0 {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    elf.data.as_ptr().add(offset),
                    vaddr as *mut u8,
                    filesz,
                );
            }
        }
    }

    // Allocate a unique user stack for this process.
    // Use PID-based offset to avoid collisions: 0x80_0000 + pid * 0x10000
    let pid_for_stack = NEXT_PID.load(Ordering::Relaxed);
    let user_stack_base = 0x80_0000u64 + pid_for_stack * 0x1_0000;
    let user_stack_phys = crate::memory::pmm::alloc_frame()
        .ok_or("out of physical memory for user stack")?;
    unsafe {
        crate::memory::paging::map_page(
            user_stack_base,
            user_stack_phys,
            crate::memory::paging::PageFlags::USER_RW,
        );
    }
    let user_rsp = user_stack_base + 4096;

    let mut proc = Process::new(path, cr3, elf.entry, user_rsp);
    proc.prepare_initial_stack();
    let pid = proc.pid;

    SCHEDULER.lock().push(proc);
    klog::info!("[066] Process '{}' (PID {}) ready", path, pid);
    Ok(pid)
}
