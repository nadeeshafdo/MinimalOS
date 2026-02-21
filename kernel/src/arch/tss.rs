//! Task State Segment (TSS) for x86_64.
//!
//! The TSS holds the stack pointers for privilege level changes and
//! the Interrupt Stack Table (IST), used for guaranteed stack switches
//! during critical exceptions like Double Fault.

/// Size of each IST stack in bytes (16 KiB).
#[allow(dead_code)]
const IST_STACK_SIZE: usize = 4096 * 4;

/// Size of the kernel stack used when transitioning from Ring 3 to Ring 0.
#[allow(dead_code)]
const KERNEL_STACK_SIZE: usize = 4096 * 4;

/// Stack storage for IST entry 1 (used by Double Fault handler).
/// NOTE: Superseded by per-core stacks in `arch::smp::CoreLocal`.
#[allow(dead_code)]
static mut DOUBLE_FAULT_STACK: [u8; IST_STACK_SIZE] = [0; IST_STACK_SIZE];

/// Kernel stack for Ring 3 → Ring 0 transitions (RSP0).
/// NOTE: Superseded by per-core stacks in `arch::smp::CoreLocal`.
#[allow(dead_code)]
static mut KERNEL_STACK: [u8; KERNEL_STACK_SIZE] = [0; KERNEL_STACK_SIZE];

/// The 64-bit Task State Segment.
///
/// In long mode, the TSS does not store register state for task switching.
/// Instead it stores:
/// - RSP values for privilege level transitions (RSP0-RSP2)
/// - IST pointers for guaranteed stack switches on specific interrupts
/// - I/O permission bitmap offset
#[repr(C, packed)]
pub struct Tss {
	reserved0: u32,
	/// Stack pointers for privilege level transitions.
	/// RSP0 is used when transitioning from Ring 3 to Ring 0.
	pub rsp: [u64; 3],
	reserved1: u64,
	/// Interrupt Stack Table (IST) entries.
	/// IST1-IST7 provide dedicated stacks for specific interrupt handlers.
	pub ist: [u64; 7],
	reserved2: u64,
	reserved3: u16,
	/// Offset to the I/O permission bitmap from the TSS base.
	pub iomap_base: u16,
}

impl Tss {
	/// Create a new TSS with all fields zeroed.
	pub const fn new() -> Self {
		Self {
			reserved0: 0,
			rsp: [0; 3],
			reserved1: 0,
			ist: [0; 7],
			reserved2: 0,
			reserved3: 0,
			iomap_base: core::mem::size_of::<Self>() as u16,
		}
	}

	/// Initialize the TSS with IST stacks and RSP0 (BSP legacy path).
	///
	/// NOTE: Superseded by per-core init in `arch::smp::CoreLocal`.
	#[allow(dead_code)]
	pub fn init(&mut self) {
		// IST1: Double Fault handler stack
		let ist1_top = core::ptr::addr_of!(DOUBLE_FAULT_STACK) as *const u8;
		self.ist[0] = ist1_top as u64 + IST_STACK_SIZE as u64;

		// [045] RSP0: kernel stack for privilege transitions (Ring 3 → Ring 0)
		let rsp0_top = core::ptr::addr_of!(KERNEL_STACK) as *const u8;
		self.rsp[0] = rsp0_top as u64 + KERNEL_STACK_SIZE as u64;
	}

	/// Return the kernel RSP0 value (top of KERNEL_STACK).
	///
	/// NOTE: Superseded by `CoreLocal::kernel_rsp0()`.
	#[allow(dead_code)]
	pub fn kernel_rsp0() -> u64 {
		let base = core::ptr::addr_of!(KERNEL_STACK) as *const u8;
		base as u64 + KERNEL_STACK_SIZE as u64
	}

	/// Update TSS RSP0 at runtime (used during context switch).
	///
	/// When switching tasks, RSP0 must point to the top of the new
	/// task's kernel stack so that Ring 3→0 transitions land on the
	/// correct stack.
	///
	/// # Safety
	/// `tss` must be a valid pointer to the live TSS.
	pub unsafe fn set_rsp0(tss: *mut Tss, rsp0: u64) {
		unsafe {
			// TSS is #[repr(C, packed)], so rsp[0] is at byte offset 4
			// (after reserved0: u32). Use write_unaligned to avoid
			// alignment issues.
			let rsp0_ptr = (tss as *mut u8).add(4) as *mut u64;
			rsp0_ptr.write_unaligned(rsp0);
		}
	}
}
