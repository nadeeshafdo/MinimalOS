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
use tinywasm::{Extern, Imports, Module, ModuleInstance, Store};

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
/// kernel internals.
fn build_imports() -> Imports {
	let mut imports = Imports::new();

	// env.sys_log(ptr: i32, len: i32)
	let _ = imports.define(
		"env",
		"sys_log",
		Extern::typed_func(
			|mut ctx: tinywasm::FuncContext<'_>, args: (i32, i32)| -> tinywasm::Result<()> {
				let (ptr, len) = args;
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
	let _ = imports.define(
		"env",
		"sys_exit",
		Extern::typed_func(
			|_ctx: tinywasm::FuncContext<'_>, code: i32| -> tinywasm::Result<()> {
				klog::info!("[wasm] sys_exit({})", code);
				// Mark current task dead and schedule away.
				{
					let mut sched = crate::task::process::SCHEDULER.lock();
					if let Some(current) = sched.current_mut() {
						current.state = crate::task::process::ProcessState::Dead;
					}
				}
				unsafe { crate::task::process::do_schedule() };
				Ok(())
			},
		),
	);

	// env.sys_spawn(name_ptr: i32, name_len: i32) -> i64
	let _ = imports.define(
		"env",
		"sys_spawn",
		Extern::typed_func(
			|mut ctx: tinywasm::FuncContext<'_>, args: (i32, i32)| -> tinywasm::Result<i64> {
				let (name_ptr, name_len) = args;
				let mem = ctx.exported_memory("memory")?;
				let n = (name_len as usize).min(64);
				let bytes = mem.load(name_ptr as usize, n)?;
				let name = core::str::from_utf8(bytes).unwrap_or("??");
				klog::info!("[wasm] sys_spawn(\"{}\")", name);

				// Try Wasm spawn first, fall back to ELF.
				match spawn_wasm(name, |_| {}) {
					Some(pid) => Ok(pid as i64),
					None => {
						match crate::task::process::spawn_from_ramdisk(name, "") {
							Ok(pid) => Ok(pid as i64),
							Err(e) => {
								klog::warn!("[wasm] spawn failed: {}", e);
								Ok(-1i64)
							}
						}
					}
				}
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
	let _ = imports.define(
		"env",
		"sys_cap_mem_read",
		Extern::typed_func(
			|mut ctx: tinywasm::FuncContext<'_>, args: (i64, i32, i32, i32)| -> tinywasm::Result<i64> {
				let (cap_handle, offset, dst_ptr, len) = args;
				let result = internal_cap_mem_read(
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
	let _ = imports.define(
		"env",
		"sys_cap_mem_write",
		Extern::typed_func(
			|mut ctx: tinywasm::FuncContext<'_>, args: (i64, i32, i32, i32)| -> tinywasm::Result<i64> {
				let (cap_handle, offset, src_ptr, len) = args;
				// Read source data from Wasm linear memory.
				let mem = ctx.exported_memory("memory")?;
				let src_bytes = mem.load(src_ptr as usize, len as usize)?;
				// Need to copy because we'll access SCHEDULER.
				let mut buf = alloc::vec![0u8; len as usize];
				buf.copy_from_slice(src_bytes);
				let result = internal_cap_mem_write(
					cap_handle as u64,
					offset as u64,
					&buf,
				);
				Ok(if result { 0 } else { -1i64 })
			},
		),
	);

	imports
}

// ── Capability Memory Blit ───────────────────────────────────────

/// Read `len` bytes from a Memory capability at `offset`.
///
/// Validates: cap exists, is Memory, has READ, offset+len in bounds.
/// Returns the data as a Vec, or None on error.
fn internal_cap_mem_read(cap_handle: u64, offset: u64, len: usize) -> Option<alloc::vec::Vec<u8>> {
	let sched = crate::task::process::SCHEDULER.lock();
	let current = sched.current()?;
	let cap = current.caps.get(cap_handle)?;

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

	let cap_size = pages * 4096;
	if offset as usize + len > cap_size {
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
fn internal_cap_mem_write(cap_handle: u64, offset: u64, data: &[u8]) -> bool {
	let sched = crate::task::process::SCHEDULER.lock();
	let current = match sched.current() {
		Some(p) => p,
		None => return false,
	};
	let cap = match current.caps.get(cap_handle) {
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

	let cap_size = pages * 4096;
	if offset as usize + data.len() > cap_size {
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

/// Internal implementation of SYS_CAP_SEND, callable from host functions.
fn internal_cap_send(endpoint_handle: u64, mut msg: crate::ipc::Message) -> u64 {
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

	// 2. Find target, do atomic cap-before-queue.
	let mut target_woken = false;
	let mut found = false;
	for target in sched.tasks_iter_mut() {
		if target.pid != target_actor_id { continue; }
		found = true;

		if target.ipc_queue.is_full() { return u64::MAX; }

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
		break;
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
	// 1. Extract the environment and DROP the scheduler lock.
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
	let ramdisk = crate::fs::ramdisk::get()?;
	let entry = crate::fs::tar::find_file(ramdisk, name)?;
	let wasm_bytes = entry.data;

	klog::info!("[wasm] Loading {} ({} bytes)", name, wasm_bytes.len());

	// 2. Parse the module.
	let module = match Module::parse_bytes(wasm_bytes) {
		Ok(m) => m,
		Err(e) => {
			klog::error!("[wasm] parse error: {:?}", e);
			return None;
		}
	};

	// 3. Build imports and instantiate.
	let imports = build_imports();
	let mut store = Store::default();
	let instance = match module.instantiate(&mut store, Some(imports)) {
		Ok(i) => i,
		Err(e) => {
			klog::error!("[wasm] instantiate error: {:?}", e);
			return None;
		}
	};

	// 4. Create the WasmEnv.
	let wasm_env = Box::new(WasmEnv { store, instance });

	// 5. Create a Process — SASOS: share the kernel's CR3.
	//    No user page table needed; Wasm SFI provides isolation.
	let kernel_cr3: u64;
	unsafe { core::arch::asm!("mov {}, cr3", out(reg) kernel_cr3) };

	let mut process = crate::task::process::Process::new(
		name,
		kernel_cr3, // SASOS: same address space as kernel
		wasm_actor_trampoline as u64,
		0, // user_rsp = 0 (not used for kernel threads)
	);
	process.wasm_env = Some(wasm_env);

	let pid = process.pid;
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
