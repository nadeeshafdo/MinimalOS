//! User-mode transition support.
//!
//! Provides the machinery to drop from Ring 0 into Ring 3 via `iretq`,
//! and helpers to set up a minimal user-mode environment for testing.

use core::arch::asm;

// ── iretq TrapFrame ([049]) ─────────────────────────────────────

/// The five quadwords that `iretq` pops off the stack to return to
/// an outer ring (Ring 3).
///
/// Pushed in the order the CPU expects (lowest address first):
///   [RSP+0x00] RIP   — instruction pointer to jump to
///   [RSP+0x08] CS	— code segment selector (Ring 3)
///   [RSP+0x10] RFLAGS — flags (must have IF=1)
///   [RSP+0x18] RSP   — user-mode stack pointer
///   [RSP+0x20] SS	— stack segment selector (Ring 3)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IretqFrame {
	pub rip: u64,
	pub cs: u64,
	pub rflags: u64,
	pub rsp: u64,
	pub ss: u64,
}

/// RFLAGS: Interrupt Flag — must be set so interrupts work in Ring 3.
const RFLAGS_IF: u64 = 1 << 9;

/// RFLAGS: Reserved bit 1 — must always be set.
const RFLAGS_RESERVED1: u64 = 1 << 1;

impl IretqFrame {
	/// Build an iretq frame for entering Ring 3.
	///
	/// # Arguments
	/// * `rip`	   — entry point in user-mode code
	/// * `user_cs`   — user code segment selector (e.g. 0x23)
	/// * `user_ss`   — user stack segment selector (e.g. 0x1b)
	/// * `user_rsp`  — top of the user-mode stack
	pub fn new(rip: u64, user_cs: u16, user_ss: u16, user_rsp: u64) -> Self {
		Self {
			rip,
			cs: user_cs as u64,
			rflags: RFLAGS_IF | RFLAGS_RESERVED1,
			rsp: user_rsp,
			ss: user_ss as u64,
		}
	}
}

// ── Jump to Ring 3 ([050]) ──────────────────────────────────────

/// Perform the `iretq` to transition from Ring 0 to Ring 3.
///
/// Pushes the frame fields onto the current stack in the order
/// `iretq` expects, then executes `iretq`.
///
/// # Safety
/// * The frame must describe a valid, mapped user-mode environment.
/// * This function never returns.
#[allow(dead_code)]
pub unsafe fn jump_to_ring3(frame: &IretqFrame) -> ! {
	unsafe {
		asm!(
			"push {ss}",	  // SS
			"push {rsp}",	 // user RSP
			"push {rflags}",  // RFLAGS
			"push {cs}",	  // CS
			"push {rip}",	 // RIP
			"iretq",
			ss	  = in(reg) frame.ss,
			rsp	 = in(reg) frame.rsp,
			rflags  = in(reg) frame.rflags,
			cs	  = in(reg) frame.cs,
			rip	 = in(reg) frame.rip,
			options(noreturn),
		);
	}
}

/// [071] Like `jump_to_ring3`, but also sets RDI and RSI so the
/// user `_start(arg0, arg1)` receives arguments.
///
/// # Safety
/// Same requirements as `jump_to_ring3`.
pub unsafe fn jump_to_ring3_with_args(frame: &IretqFrame, arg0: u64, arg1: u64) -> ! {
	unsafe {
		asm!(
			"push {ss}",
			"push {rsp}",
			"push {rflags}",
			"push {cs}",
			"push {rip}",
			"iretq",
			ss	  = in(reg) frame.ss,
			rsp	 = in(reg) frame.rsp,
			rflags  = in(reg) frame.rflags,
			cs	  = in(reg) frame.cs,
			rip	 = in(reg) frame.rip,
			in("rdi") arg0,
			in("rsi") arg1,
			options(noreturn),
		);
	}
}
