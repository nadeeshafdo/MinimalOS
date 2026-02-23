//! Wasm Runtime — WebAssembly actor execution via `tinywasm`.
//!
//! Wasm actors run as **kernel threads** — the `tinywasm` interpreter
//! executes on the process's 32 KiB kernel stack, so `context_switch_asm`
//! naturally saves/restores interpreter state.  No Ring 3, no TLB flushes.
//!
//! **Lifecycle:**
//! 1. `spawn_wasm()` parses `.wasm`, stores `Store + Instance` in
//!    `Process.wasm_env`, pushes to scheduler.
//! 2. Scheduler switches to `wasm_actor_trampoline`, which extracts the
//!    env, drops the lock, enables interrupts, and runs `_start`.
//! 3. Host functions provide the syscall bridge (32→64 bit pointer
//!    translation via `tinywasm` memory API).
#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use core::sync::atomic::{AtomicU64, Ordering};
use tinywasm::{Extern, Imports, Module, ModuleInstance, Store};

// ── IRQ → Actor Routing Table ───────────────────────────────────
//
// When a Wasm actor calls `sys_cap_irq_wait(irq_cap)`, the kernel
// stores its PID in `IRQ_WAITERS[irq]` and blocks the process.
// When the hardware IRQ fires, the handler reads the PID from
// `IRQ_WAITERS[irq]`, atomically clears the slot, and calls
// `request_wake(pid)` to unblock the actor.
//
// One waiter per IRQ line — sufficient for dedicated driver actors.

/// Maximum number of IRQ lines (matches I/O APIC pin count).
pub const MAX_IRQS: usize = 24;

/// IRQ → blocked PID mapping.  0 = no waiter.
pub static IRQ_WAITERS: [AtomicU64; MAX_IRQS] = {
	const ZERO: AtomicU64 = AtomicU64::new(0);
	[ZERO; MAX_IRQS]
};

// ── Boot-time RAMDisk storage ───────────────────────────────────
//
// Replaces the deleted `kernel/src/fs/ramdisk.rs`.  The kernel
// stores the Limine module pointer once at boot; `spawn_wasm`
// uses it to locate `.wasm` files inside the TAR archive.

use khal::ramdisk::RamDisk;
use spin::Once;

/// The global ramdisk instance, initialised once during boot.
static RAMDISK: Once<RamDisk> = Once::new();

/// Store the ramdisk globally (called from `_start` in main.rs).
///
/// # Safety
/// The `base` pointer must remain valid for the kernel's lifetime.
pub unsafe fn init_ramdisk(base: *const u8, size: usize) {
	RAMDISK.call_once(|| unsafe { RamDisk::new(base, size) });
	klog::debug!("Ramdisk stored ({} bytes)", size);
}

/// Get a reference to the global ramdisk.
fn get_ramdisk() -> Option<&'static RamDisk> {
	RAMDISK.get()
}

// ── Minimal TAR file finder ─────────────────────────────────────
//
// Replaces the deleted `kernel/src/fs/tar.rs`.  Only the bare
// minimum needed to locate a named file inside a USTAR archive.

const TAR_BLOCK: usize = 512;

/// Find a file by name in a USTAR TAR archive.
///
/// Returns `(data_ptr, data_len)` on success.
fn tar_find_file<'a>(disk: &'a RamDisk, name: &str) -> Option<&'a [u8]> {
	let buf = unsafe { disk.as_slice() };
	let search = name.strip_prefix("./").unwrap_or(name);
	let mut offset = 0usize;

	loop {
		if offset + TAR_BLOCK > buf.len() {
			return None;
		}
		let header = &buf[offset..offset + TAR_BLOCK];
		// Two consecutive zero blocks = end of archive.
		if header.iter().all(|&b| b == 0) {
			return None;
		}
		// Validate USTAR magic at offset 257.
		if &header[257..262] != b"ustar" {
			offset += TAR_BLOCK;
			continue;
		}
		// Parse name (bytes 0..100).
		let name_end = header[..100].iter().position(|&b| b == 0).unwrap_or(100);
		let entry_name = core::str::from_utf8(&header[..name_end]).unwrap_or("");
		let entry_name = entry_name.strip_prefix("./").unwrap_or(entry_name);
		// Parse size (bytes 124..136, octal).
		let size = parse_tar_octal(&header[124..136]);
		let data_start = offset + TAR_BLOCK;
		let data_end = data_start + size;

		if entry_name == search && size > 0 && data_end <= buf.len() {
			return Some(&buf[data_start..data_end]);
		}
		// Advance past header + data (data padded to block boundary).
		let data_blocks = (size + TAR_BLOCK - 1) / TAR_BLOCK;
		offset += TAR_BLOCK + data_blocks * TAR_BLOCK;
	}
}

fn parse_tar_octal(field: &[u8]) -> usize {
	let mut v: usize = 0;
	for &b in field {
		if b == 0 || b == b' ' { break; }
		if b >= b'0' && b <= b'7' {
			v = v * 8 + (b - b'0') as usize;
		}
	}
	v
}

// ── Wasm Environment ────────────────────────────────────────────

/// Holds the `tinywasm` execution state for a Wasm actor.
///
/// Stored in `Process.wasm_env`.  Extracted by the trampoline
/// onto the kernel stack before execution.
pub struct WasmEnv {
	pub store: Store,
	pub instance: ModuleInstance,
}

// SAFETY: WasmEnv is only ever accessed sequentially by the core
// currently executing the Process.  The SCHEDULER spinlock provides
// a full memory barrier during context switches.  A Process is
// never executed concurrently on multiple cores.
unsafe impl Send for WasmEnv {}
unsafe impl Sync for WasmEnv {}

// ── Host Function Bridge ────────────────────────────────────────

/// Build the `Imports` object mapping "env" module functions to
/// kernel internals.  `actor_pid` is captured by closures that need
/// to look up the calling process's capabilities.
fn build_imports(actor_pid: u64) -> Imports {
	let mut imports = Imports::new();

	// env.sys_log(ptr: i32, len: i32)
	// NOTE: tinywasm 0.8 typed_func presents tuple args in LIFO
	// (stack-pop) order, so we destructure reversed.
	let _ = imports.define(
		"env",
		"sys_log",
		Extern::typed_func(
			|mut ctx: tinywasm::FuncContext<'_>, args: (i32, i32)| -> tinywasm::Result<()> {
				let (len, ptr) = args;
				let mem = ctx.exported_memory("memory")?;
				let n = (len as usize).min(256);
				let bytes = mem.load(ptr as usize, n)?;
				if let Ok(s) = core::str::from_utf8(bytes) {
					klog::info!("[wasm] {}", s);
				}
				Ok(())
			},
		),
	);

	// env.sys_exit(code: i32)
	let exit_pid = actor_pid;
	let _ = imports.define(
		"env",
		"sys_exit",
		Extern::typed_func(
			move |_ctx: tinywasm::FuncContext<'_>, code: i32| -> tinywasm::Result<()> {
				klog::info!("[wasm] sys_exit({})", code);
				// Mark the wasm actor dead and schedule away.
				{
					let mut sched = crate::task::process::SCHEDULER.lock();
					// Find by PID (current() is unreliable on SMP).
					if let Some(current) = sched.current_mut() {
						if current.pid == exit_pid {
							current.state = crate::task::process::ProcessState::Dead;
						}
					}
					for task in sched.tasks_iter_mut() {
						if task.pid == exit_pid {
							task.state = crate::task::process::ProcessState::Dead;
							break;
						}
					}
				}
				unsafe { crate::task::process::do_schedule() };
				Ok(())
			},
		),
	);

	// env.sys_cap_send(endpoint_handle: i64, msg_ptr: i32) -> i64
	let _ = imports.define(
		"env",
		"sys_cap_send",
		Extern::typed_func(
			|mut ctx: tinywasm::FuncContext<'_>, args: (i64, i32)| -> tinywasm::Result<i64> {
				let (endpoint_handle, msg_ptr) = args;
				let mem = ctx.exported_memory("memory")?;

				// Read 48-byte Message from Wasm linear memory.
				let bytes = mem.load(msg_ptr as usize, 48)?;
				let msg: crate::ipc::Message =
					unsafe { core::ptr::read(bytes.as_ptr() as *const _) };

				let result = internal_cap_send(endpoint_handle as u64, msg);
				Ok(result as i64)
			},
		),
	);

	// env.sys_cap_recv(buf_ptr: i32) -> i64
	let _ = imports.define(
		"env",
		"sys_cap_recv",
		Extern::typed_func(
			|mut ctx: tinywasm::FuncContext<'_>, buf_ptr: i32| -> tinywasm::Result<i64> {
				// Blocking receive: loop until a message is available.
				loop {
					{
						let mut sched = crate::task::process::SCHEDULER.lock();
						if let Some(current) = sched.current_mut() {
							if let Some(msg) = current.ipc_queue.pop() {
								// Write message to Wasm linear memory.
								let msg_bytes: [u8; 48] = unsafe {
									core::mem::transmute(msg)
								};
								let mut mem = ctx.exported_memory_mut("memory")?;
								mem.store(buf_ptr as usize, 48, &msg_bytes)?;
								return Ok(0);
							}
							// Queue empty — block.
							current.state = crate::task::process::ProcessState::Blocked;
						}
					}
					// Yield CPU until woken by SYS_CAP_SEND.
					unsafe { crate::task::process::do_schedule() };
				}
			},
		),
	);
	// env.sys_cap_mem_read(cap_handle: i64, offset: i32, dst_ptr: i32, len: i32) -> i64
	// NOTE: tinywasm pop_params bug — i32 args come LIFO within their type-stack.
	// Original order: (cap, offset, dst, len)  Received: (cap, len, dst, offset)
	let read_pid = actor_pid;
	let _ = imports.define(
		"env",
		"sys_cap_mem_read",
		Extern::typed_func(
			move |mut ctx: tinywasm::FuncContext<'_>, args: (i64, i32, i32, i32)| -> tinywasm::Result<i64> {
				let (cap_handle, len, dst_ptr, offset) = args;
				let result = internal_cap_mem_read(
					read_pid,
					cap_handle as u64,
					offset as u64,
					len as usize,
				);
				match result {
					Some(data) => {
						let mut mem = ctx.exported_memory_mut("memory")?;
						mem.store(dst_ptr as usize, data.len(), &data)?;
						Ok(0)
					}
					None => Ok(-1i64),
				}
			},
		),
	);

	// env.sys_cap_mem_write(cap_handle: i64, offset: i32, src_ptr: i32, len: i32) -> i64
	// NOTE: tinywasm pop_params bug — i32 args come LIFO within their type-stack.
	// Original order: (cap, offset, src, len)  Received: (cap, len, src, offset)
	let write_pid = actor_pid;
	let _ = imports.define(
		"env",
		"sys_cap_mem_write",
		Extern::typed_func(
			move |mut ctx: tinywasm::FuncContext<'_>, args: (i64, i32, i32, i32)| -> tinywasm::Result<i64> {
				let (cap_handle, len, src_ptr, offset) = args;
				// Read source data from Wasm linear memory.
				let mem = ctx.exported_memory("memory")?;
				let src_bytes = mem.load(src_ptr as usize, len as usize)?;
				// Need to copy because we'll access SCHEDULER.
				let mut buf = alloc::vec![0u8; len as usize];
				buf.copy_from_slice(src_bytes);
				let result = internal_cap_mem_write(
					write_pid,
					cap_handle as u64,
					offset as u64,
					&buf,
				);
				Ok(if result { 0 } else { -1i64 })
			},
		),
	);

	// env.sys_cap_irq_wait(cap_handle: i64) -> i64
	//
	// Blocks the calling actor until the IRQ associated with an
	// IrqLine capability fires.  The handler EOIs the APIC and
	// wakes the actor via request_wake().
	let irq_pid = actor_pid;
	let _ = imports.define(
		"env",
		"sys_cap_irq_wait",
		Extern::typed_func(
			move |_ctx: tinywasm::FuncContext<'_>, cap_handle: i64| -> tinywasm::Result<i64> {
				// 1. Validate the cap is IrqLine and has READ permission.
				let irq = {
					let sched = crate::task::process::SCHEDULER.lock();
					let cap = match find_process_cap(&sched, irq_pid, cap_handle as u64) {
						Some(c) => c,
						None => return Ok(-1),
					};
					if !cap.has_perms(crate::cap::perms::READ) {
						return Ok(-1);
					}
					match &cap.object {
						crate::cap::ObjectKind::IrqLine { irq } => *irq,
						_ => return Ok(-1),
					}
				};
				if irq as usize >= MAX_IRQS {
					return Ok(-1);
				}

				// 2. Loop until a real IRQ fires.
				//
				// The IRQ handler atomically swaps IRQ_WAITERS[irq] to 0
				// and calls request_wake(pid).  If do_schedule() returns
				// without a real IRQ (spurious wake because this core had
				// no other Ready tasks), IRQ_WAITERS[irq] still holds our
				// PID — re-block and try again.
				loop {
					// Register as the waiter for this IRQ line.
					IRQ_WAITERS[irq as usize].store(irq_pid, Ordering::Release);

					// Block and yield.
					{
						let mut sched = crate::task::process::SCHEDULER.lock();
						if let Some(current) = sched.current_mut() {
							if current.pid == irq_pid {
								current.state = crate::task::process::ProcessState::Blocked;
							}
						}
					}
					unsafe { crate::task::process::do_schedule() };

					// Check if the IRQ handler consumed our registration.
					// If IRQ_WAITERS[irq] == 0, the handler fired and swapped
					// it to 0 — a real IRQ occurred, break out.
					// If it still holds our PID, we were spuriously woken.
					let waiter = IRQ_WAITERS[irq as usize].load(Ordering::Acquire);
					if waiter != irq_pid {
						// The handler cleared the slot — real IRQ.
						break;
					}
					// Spurious wake — re-block.
				}
				Ok(0)
			},
		),
	);

	// env.sys_cap_io_read(cap_handle: i64, port_offset: i32, size: i32) -> i32
	// NOTE: tinywasm LIFO bug: (i64, i32, i32) → (cap, size, port_offset)
	let ior_pid = actor_pid;
	let _ = imports.define(
		"env",
		"sys_cap_io_read",
		Extern::typed_func(
			move |_ctx: tinywasm::FuncContext<'_>, args: (i64, i32, i32)| -> tinywasm::Result<i32> {
				let (cap_handle, size, port_offset) = args;
				let port = {
					let sched = crate::task::process::SCHEDULER.lock();
					let cap = match find_process_cap(&sched, ior_pid, cap_handle as u64) {
						Some(c) => c,
						None => return Ok(-1),
					};
					if !cap.has_perms(crate::cap::perms::READ) {
						return Ok(-1);
					}
					match &cap.object {
						crate::cap::ObjectKind::IoPort { base, count } => {
							let off = port_offset as u16;
							if off >= *count {
								return Ok(-1);
							}
							base + off
						}
						_ => return Ok(-1),
					}
				};
				let val = match size {
					1 => unsafe { khal::port::inb(port) as i32 },
					_ => return Ok(-1), // only byte-width supported for now
				};
				Ok(val)
			},
		),
	);

	// env.sys_cap_io_write(cap_handle: i64, port_offset: i32, size: i32, value: i32) -> i32
	// NOTE: tinywasm LIFO bug: (i64, i32, i32, i32) → (cap, value, size, port_offset)
	let iow_pid = actor_pid;
	let _ = imports.define(
		"env",
		"sys_cap_io_write",
		Extern::typed_func(
			move |_ctx: tinywasm::FuncContext<'_>, args: (i64, i32, i32, i32)| -> tinywasm::Result<i32> {
				let (cap_handle, value, size, port_offset) = args;
				let port = {
					let sched = crate::task::process::SCHEDULER.lock();
					let cap = match find_process_cap(&sched, iow_pid, cap_handle as u64) {
						Some(c) => c,
						None => return Ok(-1),
					};
					if !cap.has_perms(crate::cap::perms::WRITE) {
						return Ok(-1);
					}
					match &cap.object {
						crate::cap::ObjectKind::IoPort { base, count } => {
							let off = port_offset as u16;
							if off >= *count {
								return Ok(-1);
							}
							base + off
						}
						_ => return Ok(-1),
					}
				};
				match size {
					1 => unsafe { khal::port::outb(port, value as u8) },
					_ => return Ok(-1),
				};
				Ok(0)
			},
		),
	);

	imports
}

// ── Capability Memory Blit ───────────────────────────────────────

/// Find a process's capability by PID and handle.
///
/// Searches all cores' current tasks and the ready queue.
fn find_process_cap<'a>(
	sched: &'a crate::task::process::Scheduler,
	pid: u64,
	cap_handle: u64,
) -> Option<&'a crate::cap::Capability> {
	sched.get_process(pid).and_then(|p| p.caps.get(cap_handle))
}

/// Read `len` bytes from a Memory capability at `offset`.
///
/// Validates: cap exists, is Memory, has READ, offset+len in bounds.
/// Uses `checked_add` to prevent integer overflow exploits.
/// Returns the data as a Vec, or None on error.
fn internal_cap_mem_read(actor_pid: u64, cap_handle: u64, offset: u64, len: usize) -> Option<alloc::vec::Vec<u8>> {
	let sched = crate::task::process::SCHEDULER.lock();
	let cap = match find_process_cap(&sched, actor_pid, cap_handle) {
		Some(c) => c,
		None => {
			klog::warn!("[blit] cap_mem_read: no cap at handle {} for pid {}", cap_handle, actor_pid);
			return None;
		}
	};

	if !cap.has_perms(crate::cap::perms::READ) {
		klog::warn!("[blit] cap_mem_read: missing READ permission");
		return None;
	}

	let (phys, pages) = match &cap.object {
		crate::cap::ObjectKind::Memory { phys, pages } => (*phys, *pages),
		_ => {
			klog::warn!("[blit] cap_mem_read: not a Memory capability");
			return None;
		}
	};

	let cap_size = (pages as u64) * 4096;
	let end_offset = match offset.checked_add(len as u64) {
		Some(val) => val,
		None => {
			klog::warn!("[blit] cap_mem_read: integer overflow in bounds check");
			return None;
		}
	};
	if end_offset > cap_size {
		klog::warn!("[blit] cap_mem_read: offset+len exceeds cap bounds");
		return None;
	}

	// Translate: phys + HHDM offset + offset_in_cap.
	let hhdm = crate::memory::paging::hhdm_offset();
	let src_ptr = (hhdm + phys + offset) as *const u8;

	let mut buf = alloc::vec![0u8; len];
	unsafe { core::ptr::copy_nonoverlapping(src_ptr, buf.as_mut_ptr(), len) };
	Some(buf)
}

/// Write `data` to a Memory capability at `offset`.
///
/// Validates: cap exists, is Memory, has WRITE, offset+len in bounds.
/// Uses `checked_add` to prevent integer overflow exploits.
fn internal_cap_mem_write(actor_pid: u64, cap_handle: u64, offset: u64, data: &[u8]) -> bool {
	let sched = crate::task::process::SCHEDULER.lock();
	let cap = match find_process_cap(&sched, actor_pid, cap_handle) {
		Some(c) => c,
		None => return false,
	};

	if !cap.has_perms(crate::cap::perms::WRITE) {
		klog::warn!("[blit] cap_mem_write: missing WRITE permission");
		return false;
	}

	let (phys, pages) = match &cap.object {
		crate::cap::ObjectKind::Memory { phys, pages } => (*phys, *pages),
		_ => {
			klog::warn!("[blit] cap_mem_write: not a Memory capability");
			return false;
		}
	};

	let cap_size = (pages as u64) * 4096;
	let end_offset = match offset.checked_add(data.len() as u64) {
		Some(val) => val,
		None => {
			klog::warn!("[blit] cap_mem_write: integer overflow in bounds check");
			return false;
		}
	};
	if end_offset > cap_size {
		klog::warn!("[blit] cap_mem_write: offset+len exceeds cap bounds");
		return false;
	}

	// Translate: phys + HHDM offset + offset_in_cap.
	let hhdm = crate::memory::paging::hhdm_offset();
	let dst_ptr = (hhdm + phys + offset) as *mut u8;

	unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), dst_ptr, data.len()) };
	true
}

// ── Internal Send Logic ─────────────────────────────────────────

/// Internal implementation of SYS_CAP_SEND, callable from host functions
/// and the syscall dispatcher.
pub fn internal_cap_send(endpoint_handle: u64, mut msg: crate::ipc::Message) -> u64 {
	let mut sched = crate::task::process::SCHEDULER.lock();

	// 1. Resolve endpoint → target_actor_id and cap transfer.
	let (target_actor_id, cap_transfer) = {
		let caller = match sched.current() {
			Some(p) => p,
			None => return u64::MAX,
		};

		let ep = match caller.caps.get(endpoint_handle) {
			Some(c) => c,
			None => return u64::MAX,
		};
		let target_id = match &ep.object {
			crate::cap::ObjectKind::Endpoint { target_actor_id } => *target_actor_id,
			_ => return u64::MAX,
		};

		let transfer = if msg.cap_grant != 0 {
			let src = match caller.caps.get(msg.cap_grant) {
				Some(c) => c,
				None => return u64::MAX,
			};
			if !src.has_perms(crate::cap::perms::GRANT) {
				return u64::MAX;
			}
			let granted_perms = src.perms & msg.cap_perms;
			Some((src.object.clone(), granted_perms))
		} else {
			None
		};

		(target_id, transfer)
	};

	// 2. Find target (may be running on another core or in the ready queue),
	//    do atomic cap-before-queue.
	let mut target_woken = false;
	let found;
	if let Some(target) = sched.get_process_mut(target_actor_id) {
		// Return a distinct "queue full" error so the sender knows
		// the message was dropped (not a cap/perm failure).
		if target.ipc_queue.is_full() { return u64::MAX - 1; }

		if let Some((obj, perms)) = cap_transfer {
			match target.caps.insert(obj, perms) {
				Some(new_handle) => { msg.cap_grant = new_handle; }
				None => return u64::MAX,
			}
		}

		let _ = target.ipc_queue.push(msg);

		if target.state == crate::task::process::ProcessState::Blocked {
			target.state = crate::task::process::ProcessState::Ready;
			target_woken = true;
		}
		found = true;
	} else {
		found = false;
	}

	if !found { return u64::MAX; }

	drop(sched);
	if target_woken {
		khal::apic::send_ipi_all_excluding_self();
	}
	0
}

// ── Wasm Actor Trampoline ───────────────────────────────────────

/// Entry point for Wasm actor kernel threads.
///
/// Called by the scheduler when this task is first dispatched.
/// Extracts the `WasmEnv`, drops the scheduler lock, enables
/// interrupts, and runs `_start`.
pub extern "C" fn wasm_actor_trampoline() {
	// 1. Extract the environment, then DROP the scheduler lock.
	let mut wasm_env = {
		let mut sched = crate::task::process::SCHEDULER.lock();
		let current = sched.current_mut().expect("no current task in trampoline");
		current.wasm_env.take().expect("no wasm env in trampoline")
	};

	// 2. Enable interrupts — APIC timer can now preempt this actor.
	unsafe { core::arch::asm!("sti") };

	// 3. Execute the Wasm payload.
	klog::info!("[wasm] actor _start executing");
	match wasm_env.instance.exported_func::<(), ()>(&wasm_env.store, "_start") {
		Ok(func) => {
			if let Err(e) = func.call(&mut wasm_env.store, ()) {
				klog::error!("[wasm] _start trapped: {:?}", e);
			}
		}
		Err(e) => {
			klog::error!("[wasm] no _start export: {:?}", e);
		}
	}

	// 4. Actor finished — reap the task.
	klog::info!("[wasm] actor finished, exiting");
	{
		let mut sched = crate::task::process::SCHEDULER.lock();
		if let Some(current) = sched.current_mut() {
			current.state = crate::task::process::ProcessState::Dead;
		}
	}
	unsafe { crate::task::process::do_schedule() };
	// Should not reach here; if all tasks exit, idle loop takes over.
	loop { unsafe { core::arch::asm!("hlt") }; }
}

// ── Spawn ───────────────────────────────────────────────────────

/// Spawn a Wasm actor from the ramdisk.
///
/// Returns `Some(pid)` on success, `None` if the file is not a
/// `.wasm` file or parsing failed.
pub fn spawn_wasm<F>(name: &str, init_caps: F) -> Option<u64> 
where 
	F: FnOnce(&mut crate::cap::CapTable)
{
	if !name.ends_with(".wasm") {
		return None;
	}

	// 1. Read .wasm bytes from ramdisk.
	let ramdisk = get_ramdisk()?;
	let wasm_bytes = tar_find_file(ramdisk, name)?;

	klog::info!("[wasm] Loading {} ({} bytes)", name, wasm_bytes.len());

	// 2. Parse the module.
	let module = match Module::parse_bytes(wasm_bytes) {
		Ok(m) => m,
		Err(e) => {
			klog::error!("[wasm] parse error: {:?}", e);
			return None;
		}
	};

	// 3. Create a Process first so we know its PID for the host functions.
	let kernel_cr3: u64;
	unsafe { core::arch::asm!("mov {}, cr3", out(reg) kernel_cr3) };

	let mut process = crate::task::process::Process::new(
		name,
		kernel_cr3, // SASOS: same address space as kernel
		wasm_actor_trampoline as u64,
		0, // user_rsp = 0 (not used for kernel threads)
	);
	let pid = process.pid;

	// 4. Build imports with the actor's PID so closures can look up caps.
	let imports = build_imports(pid);
	let mut store = Store::default();
	// Use ModuleInstance::instantiate (NOT Module::instantiate) to avoid
	// auto-calling _start — tinywasm's Module::instantiate calls start()
	// which falls back to the _start export even without a start section.
	let instance = match ModuleInstance::instantiate(&mut store, module, Some(imports)) {
		Ok(i) => i,
		Err(e) => {
			klog::error!("[wasm] instantiate error: {:?}", e);
			return None;
		}
	};
	klog::info!("[wasm] instantiate complete for PID {}", pid);

	// 5. Create the WasmEnv and attach to process.
	let wasm_env = Box::new(WasmEnv { store, instance });
	process.wasm_env = Some(wasm_env);

	klog::info!("[wasm] Spawning '{}' (PID {})", name, pid);

	// 6. Seed capabilities BEFORE pushing to the scheduler (fixes SMP race condition)
	init_caps(&mut process.caps);

	// 7. Prepare the initial kernel stack and push to scheduler.
	process.prepare_initial_stack();
	{
		let mut sched = crate::task::process::SCHEDULER.lock();
		sched.push(process);
	}

	Some(pid)
}
