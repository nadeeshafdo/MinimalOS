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

use crate::cap::{self, CapTable};
use crate::ipc::IpcQueue;

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
	/// Sleeping until a specific tick — [072].
	Sleeping,
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
	/// Pointer to argument string on user stack (0 if none).
	pub args_ptr: u64,
	/// Length of argument string in bytes.
	pub args_len: u64,
	/// [072] Tick at which a Sleeping process should be woken.
	pub wake_tick: u64,
	/// [073] Virtual address this process is blocked on (futex WAIT).
	pub wait_addr: u64,
	/// Kernel stack for this process (heap-allocated).
	pub kernel_stack: Box<KernelStack>,
	/// [091] Per-process capability table (heap-allocated to keep
	/// Process small — VecDeque moves must not bloat the struct).
	pub caps: Box<CapTable>,
	/// [092] Per-process IPC receive queue (heap-allocated).
	pub ipc_queue: Box<IpcQueue>,
	/// [093] Wasm environment (Store + Instance) for Wasm actors.
	/// `None` for legacy ELF processes.
	pub wasm_env: Option<Box<crate::wasm::WasmEnv>>,
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

		let mut caps = Box::new(CapTable::new());
		// Every process gets a default Log capability (handle 0).
		caps.insert(cap::ObjectKind::Log, cap::perms::READ | cap::perms::WRITE);

		Self {
			pid,
			name: String::from(name),
			state: ProcessState::Ready,
			kernel_rsp: 0, // will be set up by prepare_initial_stack()
			cr3,
			entry_point,
			user_rsp,
			args_ptr: 0,
			args_len: 0,
			wake_tick: 0,
			wait_addr: 0,
			kernel_stack,
			caps,
			ipc_queue: Box::new(IpcQueue::new()),
			wasm_env: None,
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
/// All processes are Wasm actors running as kernel threads.
/// The trampoline reads the entry point from the scheduler and
/// calls it directly — no Ring 3 transition needed.
extern "C" fn task_entry_trampoline() {
	let entry = {
		let sched = SCHEDULER.lock();
		let current = sched.current().expect("trampoline: no current task");
		current.entry_point
	};

	// Wasm actors run as kernel threads — call the entry point
	// directly (it's wasm_actor_trampoline).
	klog::info!("[064] Entering wasm actor: RIP={:#x}", entry);
	let func: extern "C" fn() = unsafe { core::mem::transmute(entry) };
	func();
	// Should not return — wasm_actor_trampoline calls do_schedule.
	loop { unsafe { core::arch::asm!("hlt") }; }
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

	/// [073] Mutable iterator over the ready queue (for futex wake).
	pub fn tasks_iter_mut(&mut self) -> impl Iterator<Item = &mut Process> {
		self.tasks.iter_mut()
	}

	/// Immutable iterator over the ready queue (for cap lookup by PID).
	pub fn tasks_iter(&self) -> impl Iterator<Item = &Process> {
		self.tasks.iter()
	}

	/// Look up a process by PID (checks current + ready queue).
	pub fn get_process_mut(&mut self, pid: u64) -> Option<&mut Process> {
		if let Some(ref mut cur) = self.current {
			if cur.pid == pid {
				return Some(cur);
			}
		}
		self.tasks.iter_mut().find(|t| t.pid == pid)
	}

	/// [072] Wake any sleeping tasks whose wake_tick has passed.
	pub fn wake_sleeping(&mut self, now: u64) {
		for task in self.tasks.iter_mut() {
			if task.state == ProcessState::Sleeping && now >= task.wake_tick {
				task.state = ProcessState::Ready;
			}
		}
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

		// [072] Wake sleeping tasks before picking the next one.
		self.wake_sleeping(super::clock::now());

		// Take the current task out.
		let mut old = match self.current.take() {
			Some(t) => t,
			None => {
				// No current — find the next Ready one.
				let queue_len = self.tasks.len();
				for _ in 0..queue_len {
					if let Some(t) = self.tasks.pop_front() {
						if t.state == ProcessState::Ready {
							let mut new = t;
							new.state = ProcessState::Running;
							self.current = Some(new);
							return;
						}
						self.tasks.push_back(t);
					}
				}
				return;
			}
		};

		// Pop the next *ready* task via round-robin.
		let queue_len = self.tasks.len();
		let mut new_task: Option<Process> = None;
		for _ in 0..queue_len {
			if let Some(t) = self.tasks.pop_front() {
				if t.state == ProcessState::Ready {
					new_task = Some(t);
					break;
				}
				self.tasks.push_back(t);
			}
		}

		let new = match new_task {
			Some(t) => t,
			None => {
				// No ready tasks — put old back.
				self.current = Some(old);
				return;
			}
		};

		// Move old to the back of the ready queue (unless it's dead).
		if old.state != ProcessState::Dead {
			if old.state != ProcessState::Sleeping {
				old.state = ProcessState::Ready;
			}
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

// ── Pending wake queue (lock-free, IRQ-safe) ────────────────────

/// Pending wake requests from IRQ context.
/// Each slot holds a PID to wake (0 = empty).
static PENDING_WAKES: [AtomicU64; 8] = [
	AtomicU64::new(0), AtomicU64::new(0),
	AtomicU64::new(0), AtomicU64::new(0),
	AtomicU64::new(0), AtomicU64::new(0),
	AtomicU64::new(0), AtomicU64::new(0),
];

/// Request that a blocked process be woken on the next schedule() call.
///
/// Lock-free — safe to call from IRQ context.
pub fn request_wake(pid: u64) {
	for slot in PENDING_WAKES.iter() {
		if slot.compare_exchange(0, pid, Ordering::AcqRel, Ordering::Relaxed).is_ok() {
			return;
		}
	}
	// All slots full — best-effort; shouldn't happen with few processes.
}

/// Drain pending wake requests.  Called while holding the scheduler lock.
fn process_pending_wakes(sched: &mut Scheduler) {
	for slot in PENDING_WAKES.iter() {
		let pid = slot.swap(0, Ordering::AcqRel);
		if pid != 0 {
			for task in sched.tasks.iter_mut() {
				if task.pid == pid && task.state == ProcessState::Blocked {
					task.state = ProcessState::Ready;
					break;
				}
			}
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
	// Dummy RSP landing pad for dead tasks — AtomicU64 avoids
	// the data race that `static mut` would cause on SMP.
	static DEAD_RSP: AtomicU64 = AtomicU64::new(0);

	let (old_rsp_ptr, new_rsp, new_cr3) = {
		let mut sched = SCHEDULER.lock();

		if sched.tasks.is_empty() {
			return;
		}

		// Process pending wake requests from IRQ handlers.
		process_pending_wakes(&mut sched);

		// [072] Wake any sleeping tasks whose wake_tick has passed.
		sched.wake_sleeping(super::clock::now());

		let old = match sched.current.take() {
			Some(t) => t,
			None => return,
		};

		// Find the next Ready task using round-robin: pop from front,
		// skip non-Ready tasks by pushing them to the back.
		let queue_len = sched.tasks.len();
		let mut new_task: Option<Process> = None;
		for _ in 0..queue_len {
			if let Some(t) = sched.tasks.pop_front() {
				if t.state == ProcessState::Ready {
					new_task = Some(t);
					break;
				}
				// Not ready (Sleeping/Blocked) — push to back
				sched.tasks.push_back(t);
			}
		}

		let new = match new_task {
			Some(t) => t,
			None => {
				// No ready tasks — put old back.
				sched.current = Some(old);
				return;
			}
		};

		let old_is_dead = old.state == ProcessState::Dead;

		if !old_is_dead {
			let mut old = old;
			// Preserve Sleeping and Blocked states; otherwise mark Ready.
			if old.state != ProcessState::Sleeping && old.state != ProcessState::Blocked {
				old.state = ProcessState::Ready;
			}
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
			DEAD_RSP.as_ptr()
		} else {
			&mut sched.tasks.back_mut().unwrap().kernel_rsp as *mut u64
		};
		let new_rsp = sched.current.as_ref().unwrap().kernel_rsp;
		let new_cr3 = sched.current.as_ref().unwrap().cr3;

		(old_rsp_ptr, new_rsp, new_cr3)
		// MutexGuard dropped here — lock is released before switch
	};

	unsafe {
		// Switch page tables if the new task uses a different CR3.
		let current_cr3: u64;
		core::arch::asm!("mov {}, cr3", out(reg) current_cr3, options(nomem, nostack, preserves_flags));
		if (current_cr3 & 0x000F_FFFF_FFFF_F000) != new_cr3 {
			core::arch::asm!("mov cr3, {}", in(reg) new_cr3, options(nostack, preserves_flags));
		}

		context_switch_asm(old_rsp_ptr, new_rsp);
		// Re-enable interrupts so the timer can fire and drive
		// preemptive scheduling.
		core::arch::asm!("sti", options(nomem, nostack));
	}
}
