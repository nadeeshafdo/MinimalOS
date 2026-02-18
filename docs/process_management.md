---
layout: default
title: Process Management
---

# Process Management

## Overview

MinimalOS implements preemptive multitasking with:

- **Process Control Blocks (PCB)** storing per-process state
- **Assembly context switch** saving/restoring all general-purpose registers
- **Round-robin scheduler** driven by the APIC timer interrupt
- **User-mode execution** via `iretq` to Ring 3
- **Syscall interface** for process lifecycle (`spawn`, `exit`, `yield`)

## Process Control Block

**File:** `kernel/src/task/process.rs`

Each process is represented by a `Process` struct:

```rust
pub struct Process {
    pub pid: u64,
    pub name: &'static str,
    pub state: ProcessState,
    pub cr3: u64,               // Page table root
    pub kernel_rsp: u64,        // Saved kernel stack pointer
    pub kernel_stack: Box<KernelStack>,  // 32 KiB kernel stack
    pub entry_point: u64,       // User-mode RIP
    pub user_rsp: u64,          // User-mode stack pointer
}
```

### Process States

```
    ┌──────────┐
    │  Ready   │◄──────────────────┐
    └────┬─────┘                   │
         │ schedule()              │ preempt / yield
    ┌────▼─────┐                   │
    │ Running  │───────────────────┘
    └────┬─────┘
         │ sys_exit()
    ┌────▼──────┐
    │ Terminated│
    └───────────┘
```

| State | Description |
|-------|-------------|
| `Ready` | In the scheduler queue, waiting to run |
| `Running` | Currently executing on the CPU |
| `Terminated` | Finished; will be removed on next schedule |

### Kernel Stacks

Each process gets a dedicated 32 KiB kernel stack, heap-allocated using
`alloc_zeroed` + `Box::from_raw` to avoid placing the large array on the
current stack (which would overflow it).

The kernel stack is used for:
- Interrupt/exception handling while the process runs
- System call handling (the `syscall` entry point switches to the kernel stack)
- Context switch save/restore area

## Context Switching

### Assembly Stub

The context switch is implemented in inline assembly:

```nasm
context_switch_asm(old_rsp_ptr: *mut u64, new_rsp: u64)
    ; Save callee-saved registers on current stack
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15

    ; Save current RSP into old process's PCB
    mov [rdi], rsp

    ; Load new process's saved RSP
    mov rsp, rsi

    ; Restore callee-saved registers from new stack
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp

    ; Return — pops RIP from the new stack
    ret
```

This approach saves only callee-saved registers (the C ABI guarantees
caller-saved registers are already saved by the compiler). The `ret` instruction
pops the return address from the new process's stack, effectively resuming
execution where that process last called `context_switch_asm`.

### Initial Stack Preparation

For a newly created process, `prepare_initial_stack()` pushes a synthetic stack
frame so that the first `context_switch_asm` returns to a trampoline function:

```
Top of kernel stack:
  ┌──────────────┐
  │ trampoline   │ ← return address for ret
  │ rbp = 0      │
  │ rbx = 0      │
  │ r12 = 0      │
  │ r13 = 0      │
  │ r14 = 0      │
  │ r15 = 0      │
  └──────────────┘ ← kernel_rsp
```

The trampoline function (`task_entry_trampoline`) uses `iretq` to jump to the
process's user-mode entry point in Ring 3, setting up the correct CS (`0x23`),
SS (`0x1b`), RFLAGS (IF=1), RIP, and RSP.

### TSS RSP0 Update

On each context switch, the TSS's RSP0 field is updated to point to the top of
the new process's kernel stack. This ensures that when a Ring 3 process triggers
an interrupt or system call, the CPU switches to the correct kernel stack.

The update uses `write_unaligned` because the TSS is `#[repr(packed)]` and
the RSP0 field is at byte offset 4 (not 8-byte aligned):

```rust
pub fn set_rsp0(tss: *mut Tss, rsp0: u64) {
    let ptr = (tss as *mut u8).add(4) as *mut u64;
    ptr.write_unaligned(rsp0);
}
```

## Scheduler

### Design

The scheduler uses a `VecDeque<Process>` as a ready queue and maintains a
separate `current` process slot:

```rust
pub struct Scheduler {
    queue: VecDeque<Process>,
    current: Option<Process>,
}
```

### Scheduling Algorithm

Round-robin with APIC timer preemption:

1. Timer interrupt fires (IRQ 0, vector 32).
2. Handler calls `do_schedule()`.
3. `do_schedule()` locks the scheduler, performs bookkeeping, and unlocks
   **before** calling `context_switch_asm` (avoiding deadlock).
4. The old process is pushed to the back of the queue.
5. The next `Ready` process is popped from the front.
6. TSS RSP0 is updated to the new process's kernel stack.
7. `context_switch_asm` switches stacks.
8. Interrupts are re-enabled (`sti`) after the context switch returns.

### Deadlock Prevention

A critical design decision: the scheduler lock is **dropped before** calling
`context_switch_asm`. If the lock were held across the switch, the new process
would try to acquire the same lock (since it's a spinlock), resulting in deadlock.

The `do_schedule()` function is a free function (not a method on `Scheduler`)
that:

1. Acquires the lock
2. Extracts the old/new processes
3. Drops the lock
4. Calls `context_switch_asm`

### Voluntary Yield

Processes can yield the CPU voluntarily via `sys_yield()` (syscall number 2),
which calls `do_schedule()` directly.

## Process Lifecycle

### Creation

1. `sys_spawn("program.elf")` is called from user mode.
2. The kernel looks up the ELF binary in the ramdisk TAR archive.
3. ELF program headers are parsed; `PT_LOAD` segments are mapped into user pages.
4. A user stack is allocated at `0x800000 + pid * 0x10000`.
5. A 32 KiB kernel stack is heap-allocated.
6. The initial kernel stack frame is prepared (trampoline + registers).
7. The process is pushed to the scheduler's ready queue.

### Termination

When a process calls `sys_exit()`:

1. The process's state is set to `Terminated`.
2. `do_schedule()` is called to switch to another process.
3. On the next scheduling pass, terminated processes are removed from the queue.

### Idle Process

The kernel creates a special "idle" process that represents the initial kernel
thread. When all user processes terminate, execution returns to the idle loop:

```rust
loop {
    asm!("sti; hlt");
}
```

The `sti; hlt` sequence is idiomatic on x86: `sti` enables interrupts and `hlt`
halts until the next interrupt, with the instruction boundary between them
allowing exactly one interrupt to arrive.
