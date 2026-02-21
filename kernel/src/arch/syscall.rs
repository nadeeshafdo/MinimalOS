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
	//  syscall:  CS = STAR[47:32],	  SS = STAR[47:32] + 8
	//  sysret64: CS = STAR[63:48] + 16, SS = STAR[63:48] + 8
	//
	// GDT: 0x08=KCode, 0x10=KData, 0x18=UData, 0x20=UCode
	//  → kernel base = 0x08, sysret base = 0x10
	//	sysret64 CS = 0x10+16 = 0x20 (|3 by HW) = 0x23 ✓
	//	sysret64 SS = 0x10+8  = 0x18 (|3 by HW) = 0x1B ✓
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
		"push rcx",	   // user RIP
		"push r11",	   // user RFLAGS
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
		"mov r9, r8",	 // a4 → R9 (SysV slot 5)
		"mov r8, r10",	// a3 → R8 (SysV slot 4)

		"call syscall_dispatch",

		// RAX now holds the return value — leave it there.

		// ── restore registers (reverse push order) ──
		"pop r15",		// saved user RSP → r15 (temp)
		"mov [rip + SYSCALL_USER_RSP], r15",

		"pop r15",
		"pop r14",
		"pop r13",
		"pop r12",
		"pop rbx",
		"pop rbp",
		"pop r11",		// user RFLAGS
		"pop rcx",		// user RIP

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
	/// `sys_list(buf_ptr, buf_len)` — list ramdisk filenames into buffer.
	pub const SYS_LIST: u64 = 13;
	/// `sys_print(msg_ptr, msg_len)` — write raw text to serial + framebuffer.
	pub const SYS_PRINT: u64 = 14;
	/// `sys_fb_info(buf_ptr)` — fill buffer with framebuffer info.
	pub const SYS_FB_INFO: u64 = 15;
	/// `sys_mmap(vaddr, num_pages, phys_addr)` — map pages into user space.
	pub const SYS_MMAP: u64 = 16;
	/// `sys_win_create(result_ptr, xy_packed, wh_packed, title_ptr)` — create window.
	pub const SYS_WIN_CREATE: u64 = 17;
	/// `sys_win_update(win_id)` — mark a window as dirty.
	pub const SYS_WIN_UPDATE: u64 = 18;
	/// `sys_win_list(buf_ptr, max_count)` — list windows into buffer.
	pub const SYS_WIN_LIST: u64 = 19;
	/// `sys_win_move(win_id, xy_packed)` — move a window.
	pub const SYS_WIN_MOVE: u64 = 20;
	/// `sys_cap_grant(source_handle, endpoint_handle, requested_perms)` — delegate a capability.
	pub const SYS_CAP_GRANT: u64 = 21;
	/// `sys_cap_send(endpoint_handle, msg_ptr)` — send a message via IPC.
	pub const SYS_CAP_SEND: u64 = 22;
	/// `sys_cap_recv(msg_buf_ptr)` — receive a message (blocks if queue empty).
	pub const SYS_CAP_RECV: u64 = 23;
	/// `sys_cap_mem_read(cap, offset, dst_ptr, len)` — blit from Memory cap to user buffer.
	#[allow(dead_code)]
	pub const SYS_CAP_MEM_READ: u64 = 24;
	/// `sys_cap_mem_write(cap, offset, src_ptr, len)` — blit from user buffer to Memory cap.
	#[allow(dead_code)]
	pub const SYS_CAP_MEM_WRITE: u64 = 25;
}

// ── User-pointer validation ────────────────────────────────────

/// The upper bound of user-space canonical addresses.
/// Anything at or above this address is kernel memory.
const USER_SPACE_END: u64 = 0x0000_8000_0000_0000;

/// Validate that a user-space pointer range `[ptr, ptr+len)` is
/// safe for the kernel to access on behalf of a user process.
///
/// Returns `false` if:
/// - `ptr` is null
/// - `ptr + len` overflows
/// - any byte in the range falls in kernel address space
#[inline]
fn validate_user_ptr(ptr: u64, len: usize) -> bool {
	if ptr == 0 {
		return false;
	}
	if len == 0 {
		return true;
	}
	match ptr.checked_add(len as u64) {
		Some(end) => end <= USER_SPACE_END,
		None => false,
	}
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
			if !validate_user_ptr(a0, len) || len > 1024 {
				return u64::MAX;
			}
			let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
			if let Ok(msg) = core::str::from_utf8(slice) {
				klog::info!("[syscall] SYS_LOG: {}", msg);
				return 0; // success
			}
			u64::MAX // error: bad encoding
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
			let len = a1 as usize;
			if !validate_user_ptr(a0, len) || len == 0 || len > 256 {
				return u64::MAX;
			}
			let slice = unsafe { core::slice::from_raw_parts(a0 as *const u8, len) };
			let path = match core::str::from_utf8(slice) {
				Ok(s) => s,
				Err(_) => return u64::MAX,
			};
			// Extract optional args
			let args_len = a3 as usize;
			let args = if a2 != 0 && args_len > 0 && args_len <= 256
				&& validate_user_ptr(a2, args_len)
			{
				let args_slice = unsafe { core::slice::from_raw_parts(a2 as *const u8, args_len) };
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
			// Blocking read: if no input is available, suspend the
			// calling process until the keyboard IRQ delivers a byte.
			loop {
				let ch = crate::task::input::pop_char();
				if ch != 0 {
					return ch as u64;
				}
				// Buffer empty — block.
				let pid = {
					let sched = crate::task::process::SCHEDULER.lock();
					sched.current().map(|p| p.pid).unwrap_or(0)
				};
				crate::task::input::set_waiter(pid);
				{
					let mut sched = crate::task::process::SCHEDULER.lock();
					if let Some(current) = sched.current_mut() {
						current.state = crate::task::process::ProcessState::Blocked;
					}
				}
				unsafe { crate::task::process::do_schedule() };
				// Woken up — loop back and try again.
			}
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
			let len = a2 as usize;
			if !validate_user_ptr(a1, len) || len == 0 || len > 4096 {
				return u64::MAX;
			}
			let data = unsafe { core::slice::from_raw_parts(a1 as *const u8, len) };
			crate::task::pipe::write(pipe_id, data) as u64
		}
		nr::SYS_PIPE_READ => {
			// [070] a0 = pipe_id, a1 = buf_ptr, a2 = buf_len
			let pipe_id = a0 as usize;
			let len = a2 as usize;
			if !validate_user_ptr(a1, len) || len == 0 || len > 4096 {
				return u64::MAX;
			}
			let buf = unsafe { core::slice::from_raw_parts_mut(a1 as *mut u8, len) };
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
			if !validate_user_ptr(a0, 12) {
				return u64::MAX;
			}
			unsafe { crate::task::events::read_event_to_user(a0 as *mut u8) as u64 }
		}
		nr::SYS_LIST => {
			// a0 = buf_ptr, a1 = buf_len
			// Write newline-separated ramdisk filenames to user buffer.
			let buf_len = a1 as usize;
			if !validate_user_ptr(a0, buf_len) || buf_len == 0 || buf_len > 4096 {
				return u64::MAX;
			}
			let buf = unsafe { core::slice::from_raw_parts_mut(a0 as *mut u8, buf_len) };
			let ramdisk = match crate::fs::ramdisk::get() {
				Some(rd) => rd,
				None => return u64::MAX,
			};
			let iter = unsafe { crate::fs::tar::TarIter::new(ramdisk) };
			let mut pos = 0;
			for entry in iter {
				let name = entry.name.strip_prefix("./").unwrap_or(entry.name);
				if name.is_empty() || name == "." {
					continue;
				}
				let name_bytes = name.as_bytes();
				let needed = name_bytes.len() + 1;
				if pos + needed > buf_len {
					break;
				}
				buf[pos..pos + name_bytes.len()].copy_from_slice(name_bytes);
				pos += name_bytes.len();
				buf[pos] = b'\n';
				pos += 1;
			}
			pos as u64
		}
		nr::SYS_PRINT => {
			// a0 = pointer to UTF-8 string, a1 = length
			// Write raw text to serial AND framebuffer (no prefix/formatting).
			let len = a1 as usize;
			if validate_user_ptr(a0, len) && len > 0 && len <= 4096 {
				let slice = unsafe { core::slice::from_raw_parts(a0 as *const u8, len) };
				if let Ok(msg) = core::str::from_utf8(slice) {
					khal::serial::write_str(msg);
					kdisplay::console_write_fmt(format_args!("{}", msg));
					return 0;
				}
			}
			u64::MAX
		}
		nr::SYS_FB_INFO => {
			// [080] a0 = pointer to FbInfo struct (24 bytes) in user space.
			if !validate_user_ptr(a0, core::mem::size_of::<crate::task::window::FbInfo>()) {
				return u64::MAX;
			}
			match crate::task::window::get_fb_info() {
				Some(info) => {
					unsafe { core::ptr::write(a0 as *mut crate::task::window::FbInfo, info); }
					0
				}
				None => u64::MAX,
			}
		}
		nr::SYS_MMAP => {
			// [080] a0 = user vaddr, a1 = number of 4KiB pages, a2 = phys addr (0 = alloc fresh)
			let vaddr = a0;
			let num_pages = a1 as usize;
			let phys_start = a2;

			if num_pages == 0 || num_pages > 4096 {
				return u64::MAX; // Sanity limit: max 16 MiB per mmap
			}
			if vaddr & 0xFFF != 0 {
				return u64::MAX; // Must be page-aligned
			}

			for i in 0..num_pages {
				let page_vaddr = vaddr + (i as u64) * 4096;
				let page_phys = if phys_start != 0 {
					phys_start + (i as u64) * 4096
				} else {
					match crate::memory::pmm::alloc_frame() {
						Some(f) => f,
						None => return u64::MAX,
					}
				};
				unsafe {
					crate::memory::paging::map_page(
						page_vaddr,
						page_phys,
						crate::memory::paging::PageFlags::USER_RW,
					);
					// Zero fresh pages (not framebuffer mappings).
					if phys_start == 0 {
						core::ptr::write_bytes(page_vaddr as *mut u8, 0, 4096);
					}
				}
			}
			klog::info!("[080] mmap: {:#x} → {} pages (phys={:#x})", vaddr, num_pages, phys_start);
			0
		}
		nr::SYS_WIN_CREATE => {
			// [084] a0 = result_ptr (&mut [u64; 2]), a1 = xy_packed, a2 = wh_packed, a3 = title_ptr
			let x = a1 as i32;
			let y = (a1 >> 32) as i32;
			let w = a2 as u32;
			let h = (a2 >> 32) as u32;

			if !validate_user_ptr(a0, 16) || w == 0 || h == 0 || w > 2048 || h > 2048 {
				return u64::MAX;
			}

			// Read title (up to 31 bytes).
			let title = if a3 != 0 && validate_user_ptr(a3, 1) {
				// Find NUL or limit to 31 chars.
				let title_ptr = a3 as *const u8;
				let mut len = 0usize;
				while len < 31 {
					let b = unsafe { core::ptr::read(title_ptr.add(len)) };
					if b == 0 { break; }
					len += 1;
				}
				if len > 0 {
					let slice = unsafe { core::slice::from_raw_parts(title_ptr, len) };
					core::str::from_utf8(slice).unwrap_or("?")
				} else {
					"Window"
				}
			} else {
				"Window"
			};

			match crate::task::window::create_window(x, y, w, h, title) {
				Some((id, buf_vaddr)) => {
					unsafe {
						core::ptr::write((a0) as *mut u64, id as u64);
						core::ptr::write((a0 as *mut u64).add(1), buf_vaddr);
					}
					0
				}
				None => u64::MAX,
			}
		}
		nr::SYS_WIN_UPDATE => {
			// [086] a0 = window id
			crate::task::window::mark_dirty(a0 as u32);
			0
		}
		nr::SYS_WIN_LIST => {
			// [084] a0 = buf_ptr (array of WindowInfo), a1 = max_count
			let max_count = a1 as usize;
			let buf_size = max_count * core::mem::size_of::<crate::task::window::WindowInfo>();
			if !validate_user_ptr(a0, buf_size) || max_count == 0 {
				return u64::MAX;
			}
			unsafe { crate::task::window::list_windows(a0 as *mut crate::task::window::WindowInfo, max_count) as u64 }
		}
		nr::SYS_WIN_MOVE => {
			// [087] a0 = window id, a1 = xy_packed (x in low 32, y in high 32)
			let win_id = a0 as u32;
			let new_x = a1 as i32;
			let new_y = (a1 >> 32) as i32;
			crate::task::window::move_window(win_id, new_x, new_y);
			0
		}
		nr::SYS_CAP_GRANT => {
			// [091] a0 = source_handle, a1 = endpoint_handle, a2 = requested_perms
			//
			// Capability delegation: copy a capability from the caller's
			// table to a target actor identified by an Endpoint capability.
			// Permissions can only be narrowed (never widened).
			let source_handle = a0;
			let endpoint_handle = a1;
			let requested_perms = a2 as u32;

			let mut sched = crate::task::process::SCHEDULER.lock();

			// 1. Read source and endpoint from caller's cap table.
			let (src_object, src_perms, target_actor_id) = {
				let caller = match sched.current() {
					Some(p) => p,
					None => return u64::MAX,
				};

				// Validate source capability exists and has GRANT.
				let src = match caller.caps.get(source_handle) {
					Some(c) => c,
					None => {
						klog::warn!("[syscall] SYS_CAP_GRANT: invalid source_handle={:#x}", source_handle);
						return u64::MAX;
					}
				};
				if !src.has_perms(crate::cap::perms::GRANT) {
					klog::warn!("[syscall] SYS_CAP_GRANT: source lacks GRANT permission");
					return u64::MAX;
				}

				// Validate endpoint capability.
				let ep = match caller.caps.get(endpoint_handle) {
					Some(c) => c,
					None => {
						klog::warn!("[syscall] SYS_CAP_GRANT: invalid endpoint_handle={:#x}", endpoint_handle);
						return u64::MAX;
					}
				};
				let target_id = match &ep.object {
					crate::cap::ObjectKind::Endpoint { target_actor_id } => *target_actor_id,
					_ => {
						klog::warn!("[syscall] SYS_CAP_GRANT: endpoint_handle is not an Endpoint");
						return u64::MAX;
					}
				};

				// Clone the source object and compute narrowed perms.
				(src.object.clone(), src.perms & requested_perms, target_id)
			};

			// 2. Find target process by actor_id (== pid for now).
			let target = sched.tasks_iter_mut()
				.find(|p| p.pid == target_actor_id);

			match target {
				Some(target_proc) => {
					match target_proc.caps.insert(src_object, src_perms) {
						Some(new_handle) => {
							klog::info!("[syscall] SYS_CAP_GRANT: granted cap to actor {} (handle={:#x})",
								target_actor_id, new_handle);
							0 // success
						}
						None => {
							klog::warn!("[syscall] SYS_CAP_GRANT: target cap table full");
							u64::MAX
						}
					}
				}
				None => {
					klog::warn!("[syscall] SYS_CAP_GRANT: target actor {} not found", target_actor_id);
					u64::MAX
				}
			}
		}
		nr::SYS_CAP_SEND => {
			// [092] a0 = endpoint_handle, a1 = msg_ptr (48-byte Message)
			//
			// Send a message to a target actor via IPC.  If the message
			// carries a capability grant, the kernel performs an inline
			// cap transfer (with GRANT check and permission narrowing)
			// before pushing the message.  If the target's CapTable is
			// full, the syscall aborts — no message enters the queue.
			let endpoint_handle = a0;
			let msg_ptr = a1;

			// Validate user pointer for 48-byte Message.
			if !validate_user_ptr(msg_ptr, core::mem::size_of::<crate::ipc::Message>()) {
				return u64::MAX;
			}

			// Copy message from user space.
			let mut msg = unsafe {
				core::ptr::read(msg_ptr as *const crate::ipc::Message)
			};

			let mut sched = crate::task::process::SCHEDULER.lock();

			// 1. Resolve endpoint → target_actor_id, and extract cap
			//    transfer data from the caller's table.
			let (target_actor_id, cap_transfer) = {
				let caller = match sched.current() {
					Some(p) => p,
					None => return u64::MAX,
				};

				// Validate endpoint.
				let ep = match caller.caps.get(endpoint_handle) {
					Some(c) => c,
					None => {
						klog::warn!("[syscall] SYS_CAP_SEND: invalid endpoint_handle={:#x}", endpoint_handle);
						return u64::MAX;
					}
				};
				let target_id = match &ep.object {
					crate::cap::ObjectKind::Endpoint { target_actor_id } => *target_actor_id,
					_ => {
						klog::warn!("[syscall] SYS_CAP_SEND: not an Endpoint");
						return u64::MAX;
					}
				};

				// If cap_grant is set, validate the source cap.
				let transfer = if msg.cap_grant != 0 {
					let src = match caller.caps.get(msg.cap_grant) {
						Some(c) => c,
						None => {
							klog::warn!("[syscall] SYS_CAP_SEND: invalid cap_grant={:#x}", msg.cap_grant);
							return u64::MAX;
						}
					};
					if !src.has_perms(crate::cap::perms::GRANT) {
						klog::warn!("[syscall] SYS_CAP_SEND: cap lacks GRANT permission");
						return u64::MAX;
					}
					let granted_perms = src.perms & msg.cap_perms;
					Some((src.object.clone(), granted_perms))
				} else {
					None
				};

				(target_id, transfer)
			};

			// 2. Find the target process.
			let mut target_woken = false;
			let mut found = false;
			for target in sched.tasks_iter_mut() {
				if target.pid != target_actor_id { continue; }
				found = true;

				// Check IpcQueue capacity first.
				if target.ipc_queue.is_full() {
					klog::warn!("[syscall] SYS_CAP_SEND: target IPC queue full");
					return u64::MAX;
				}

				// Atomic: insert cap BEFORE pushing message.
				if let Some((obj, perms)) = cap_transfer {
					match target.caps.insert(obj, perms) {
						Some(new_handle) => {
							// Overwrite cap_grant with the new handle
							// in the target's table.
							msg.cap_grant = new_handle;
						}
						None => {
							klog::warn!("[syscall] SYS_CAP_SEND: target CapTable full, aborting");
							return u64::MAX;
						}
					}
				}

				// Push message into target's IPC queue.
				let _ = target.ipc_queue.push(msg);

				// Wake target if blocked.
				if target.state == crate::task::process::ProcessState::Blocked {
					target.state = crate::task::process::ProcessState::Ready;
					target_woken = true;
				}
				break;
			}

			if !found {
				klog::warn!("[syscall] SYS_CAP_SEND: target actor {} not found", target_actor_id);
				return u64::MAX;
			}

			// Release scheduler lock before IPI.
			drop(sched);

			// Broadcast IPI to wake halted cores so they pick up
			// the newly readied target.
			if target_woken {
				khal::apic::send_ipi_all_excluding_self();
			}

			0
		}
		nr::SYS_CAP_RECV => {
			// [092] a0 = msg_buf_ptr (48-byte buffer in user space)
			//
			// Blocking receive: pop the next message from the caller's
			// IPC queue.  If empty, block until a message arrives.
			let msg_buf_ptr = a0;

			// Validate user pointer BEFORE entering the blocking loop.
			if !validate_user_ptr(msg_buf_ptr, core::mem::size_of::<crate::ipc::Message>()) {
				return u64::MAX;
			}

			loop {
				// Try to pop a message.
				{
					let mut sched = crate::task::process::SCHEDULER.lock();
					if let Some(current) = sched.current_mut() {
						if let Some(msg) = current.ipc_queue.pop() {
							// Copy message to user buffer.
							unsafe {
								core::ptr::write(msg_buf_ptr as *mut crate::ipc::Message, msg);
							}
							return 0;
						}
						// Queue empty — block.
						current.state = crate::task::process::ProcessState::Blocked;
					}
				}
				// Yield CPU until woken by SYS_CAP_SEND.
				unsafe { crate::task::process::do_schedule(); }
				// Woken — loop back and try again.
			}
		}
		_ => {
			klog::warn!("[syscall] unknown syscall nr={}", nr);
			u64::MAX // ENOSYS
		}
	}
}
