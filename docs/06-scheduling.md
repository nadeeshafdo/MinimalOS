# Task Management & Scheduling

## Process Control Block (`kernel/src/task/process.rs`)

```rust
pub struct Process {
    pub pid: u64,                           // Monotonic, from AtomicU64
    pub name: String,                       // e.g. "vfs.wasm"
    pub state: ProcessState,                // Ready/Running/Blocked/Sleeping/Dead
    pub kernel_rsp: u64,                    // Saved kernel stack pointer
    pub cr3: u64,                           // Page table root (same for all in SASOS)
    pub entry_point: u64,                   // wasm_actor_trampoline address
    pub user_rsp: u64,                      // Unused for Wasm actors
    pub args_ptr: u64,                      // Legacy
    pub args_len: u64,                      // Legacy
    pub wake_tick: u64,                     // For Sleeping state
    pub wait_addr: u64,                     // For futex Blocked state
    pub kernel_stack: Box<KernelStack>,     // 32 KiB, heap-allocated
    pub caps: Box<CapTable>,                // 64-slot capability table
    pub ipc_queue: Box<IpcQueue>,           // 16-message receive queue
    pub wasm_env: Option<Box<WasmEnv>>,     // tinywasm Store + Instance
}
```

### Process States
```
Ready → Running → Ready (preempted by timer)
Running → Blocked (waiting for IPC recv)
Running → Dead (sys_exit or actor finished)
Blocked → Ready (woken by IPC send)
Running → Sleeping → Ready (wake_tick reached)
```

### Kernel Stack Layout (32 KiB)
When a task is first created, `prepare_initial_stack()` sets up:
```
[top - 8]  rip = task_entry_trampoline
[top - 16] rbp = 0
[top - 24] rbx = 0
[top - 32] r12 = 0
[top - 40] r13 = 0
[top - 48] r14 = 0
[top - 56] r15 = 0
```
This matches the pop order of `context_switch_asm`.

## Context Switch

### Assembly (`context_switch_asm`)
```asm
; rdi = &mut old_task.kernel_rsp
; rsi = new_task.kernel_rsp
push rbp, rbx, r12, r13, r14, r15    ; save callee-saved
mov [rdi], rsp                         ; save old RSP
mov rsp, rsi                           ; load new RSP
pop r15, r14, r13, r12, rbx, rbp      ; restore callee-saved
ret                                    ; jump to saved RIP
```

### `do_schedule()` — Lock-Safe Schedule

The key challenge: the scheduler lock must be dropped BEFORE `context_switch_asm`, because the target task's trampoline needs to re-acquire it.

```
do_schedule():
  1. Lock SCHEDULER
  2. Process pending wake requests (from IRQ handlers)
  3. Wake sleeping tasks whose wake_tick has passed
  4. Take current task out
  5. Find next Ready task (round-robin VecDeque pop)
  6. Move old task to back of queue (unless Dead → drop it)
  7. Install new task as current
  8. Update SYSCALL_KERNEL_RSP and TSS RSP0
  9. Extract raw (old_rsp_ptr, new_rsp, new_cr3) into locals
  10. DROP the MutexGuard ← lock released here
  11. Switch CR3 if different
  12. context_switch_asm(old_rsp_ptr, new_rsp)
  13. sti (re-enable interrupts)
```

## Trampoline (`task_entry_trampoline`)

When a new task runs for the first time:
1. `context_switch_asm` returns into `task_entry_trampoline`
2. Reads `entry_point` from current task (under lock)
3. For Wasm actors: `entry_point` = `wasm_actor_trampoline`
4. Calls it directly as `extern "C" fn()`

## Scheduler

- **Global**: `SCHEDULER: Mutex<Scheduler>` — single instance
- **Round-robin**: `VecDeque<Process>` ready queue
- **Timer-driven**: `timer_handler` calls `do_schedule()` via `try_lock` (avoids deadlock if scheduler already held)
- **Pending wakes**: 8-slot `AtomicU64` array — IRQ handlers call `request_wake(pid)`, drained at next `do_schedule()`

## Clock (`kernel/src/task/clock.rs`)

- `TICKS: AtomicU64` — incremented every APIC timer interrupt
- `tick()` / `now()` — called from timer handler

## Event System (`kernel/src/task/events.rs`)

- `EventBuffer`: 256-entry ring of 12-byte `InputEvent`
- `InputEvent { kind, code, flags, value, abs_x, abs_y }`
- Kinds: `KeyPress(1)`, `KeyRelease(2)`, `Mouse(3)`
- Populated by keyboard/mouse IRQ handlers

## Legacy Modules (from ELF era, unused by Wasm actors)

| Module | Purpose |
|---|---|
| `task/input.rs` | 256-byte keyboard char ring buffer + blocking waiter PID |
| `task/futex.rs` | `futex_wait`/`futex_wake` on user addresses |
| `task/pipe.rs` | 4 KiB ring-buffer pipes (16 max) |
| `task/usermode.rs` | `iretq` Ring 0→3 transition |
| `task/window.rs` | `FbInfo` storage (still used — stores framebuffer geometry) |
