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
	/// `sys_cap_send(endpoint_handle, msg_ptr)` — send a message via IPC.
	pub const SYS_CAP_SEND: u64 = 22;
	/// `sys_cap_recv(msg_buf_ptr)` — receive a message (blocks if queue empty).
	pub const SYS_CAP_RECV: u64 = 23;
	/// `sys_cap_mem_read(cap, offset, dst_ptr, len)` — blit from Memory cap to user buffer.
	pub const SYS_CAP_MEM_READ: u64 = 24;
	/// `sys_cap_mem_write(cap, offset, src_ptr, len)` — blit from user buffer to Memory cap.
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
/// Only the pure capability syscalls remain.  All legacy POSIX-style
/// syscalls (spawn, read, pipe, futex, mmap, window, etc.) have been
/// eradicated.  Wasm actors call host functions directly; this
/// dispatcher exists for potential future Ring 3 actors.
///
/// Returns the syscall result in RAX.
#[no_mangle]
unsafe extern "C" fn syscall_dispatch(
	nr: u64,
	a0: u64,
	a1: u64,
	_a2: u64,
	_a3: u64,
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
				return 0;
			}
			u64::MAX
		}
		nr::SYS_EXIT => {
			klog::info!("[syscall] SYS_EXIT(code={})", a0);
			{
				let mut sched = crate::task::process::SCHEDULER.lock();
				if let Some(current) = sched.current_mut() {
					current.state = crate::task::process::ProcessState::Dead;
				}
			}
			unsafe { crate::task::process::do_schedule() };
			klog::info!("All tasks exited, halting");
			loop {
				unsafe { core::arch::asm!("hlt") };
			}
		}
		nr::SYS_CAP_SEND => {
			// a0 = endpoint_handle, a1 = msg_ptr (48-byte Message)
			let endpoint_handle = a0;
			let msg_ptr = a1;
			if !validate_user_ptr(msg_ptr, core::mem::size_of::<crate::ipc::Message>()) {
				return u64::MAX;
			}
			let msg = unsafe {
				core::ptr::read(msg_ptr as *const crate::ipc::Message)
			};
			crate::wasm::internal_cap_send(endpoint_handle, msg)
		}
		nr::SYS_CAP_RECV => {
			// a0 = msg_buf_ptr (48-byte buffer)
			let msg_buf_ptr = a0;
			if !validate_user_ptr(msg_buf_ptr, core::mem::size_of::<crate::ipc::Message>()) {
				return u64::MAX;
			}
			loop {
				{
					let mut sched = crate::task::process::SCHEDULER.lock();
					if let Some(current) = sched.current_mut() {
						if let Some(msg) = current.ipc_queue.pop() {
							unsafe {
								core::ptr::write(msg_buf_ptr as *mut crate::ipc::Message, msg);
							}
							return 0;
						}
						current.state = crate::task::process::ProcessState::Blocked;
					}
				}
				unsafe { crate::task::process::do_schedule(); }
			}
		}
		_ => {
			klog::warn!("[syscall] unknown syscall nr={}", nr);
			u64::MAX
		}
	}
}
