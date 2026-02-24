---
title: Processes & Scheduler
layout: default
parent: Subsystems
nav_order: 4
---

# Sprint 4 — Processes & Scheduler
{: .no_toc }

Run multiple threads of execution, share the CPU fairly across cores.
{: .fs-6 .fw-300 }

**Status**: 🔲 Planned
{: .label .label-yellow }

<details open markdown="block">
  <summary>Table of contents</summary>
  {: .text-delta }
- TOC
{:toc}
</details>

---

## Overview

Sprint 4 introduces **processes**, **threads**, and a **tickless scheduler** — the foundation for running multiple programs on the 4-core N3710 processor. This sprint also initializes the Application Processors (AP cores) for symmetric multiprocessing (SMP).

---

## Process & Thread Model

{: .todo }
> Not yet implemented.

### Process

A process owns an **address space** and a **capability table**:

```rust
pub struct Process {
    pid: ProcessId,
    address_space: AddressSpace,  // PML4 page table + VMAs
    capability_table: CapTable,   // Indexed capability slots
    threads: Vec<ThreadId>,       // Threads belonging to this process
    state: ProcessState,          // Running, Suspended, Dead
}
```

- Each process has its own PML4 (top-level page table) — complete isolation
- Processes communicate only through kernel-mediated IPC
- No shared memory by default — memory sharing requires explicit capability grants

### Thread

A thread is a schedulable unit of execution within a process:

```rust
pub struct Thread {
    tid: ThreadId,
    process: ProcessId,
    state: ThreadState,         // Ready, Running, Blocked, Dead
    kernel_stack: VirtAddr,     // Per-thread kernel stack
    saved_context: CpuContext,  // Saved registers on context switch
    priority: u8,               // Scheduler priority level
    time_slice_ns: u64,         // Remaining time in nanoseconds
}
```

### Thread States

```
         ┌─────────┐
         │  Ready  │◄──────────────────┐
         └────┬────┘                   │
              │ schedule()             │ wake() / timer
         ┌────▼────┐             ┌────┴────┐
         │ Running │────────────►│ Blocked │
         └────┬────┘  block()    └─────────┘
              │
              │ exit() / kill()
         ┌────▼────┐
         │  Dead   │
         └─────────┘
```

---

## Context Switching

{: .todo }
> Not yet implemented.

When the scheduler switches from thread A to thread B:

1. **Save** thread A's registers (GPRs, RSP, RIP, RFLAGS, FS/GS base)
2. **Switch kernel stack** to thread B's kernel stack
3. **Switch page tables** if threads are in different processes (`mov cr3, new_pml4`)
4. **Restore** thread B's registers
5. **Return** to thread B's saved instruction pointer

### FPU/SSE State

The kernel disables SSE in Ring 0 (`-target-feature=-sse,-sse2`). Userspace processes may use SSE/AVX. The kernel handles this via **lazy FPU save/restore**:

1. Mark FPU state as "owned by thread A"
2. On context switch to thread B, set CR0.TS (task switched flag)
3. If thread B uses FPU → #NM exception → save A's state, restore B's state, clear TS
4. If thread B never uses FPU → no save/restore overhead

---

## Tickless Scheduler

{: .todo }
> Not yet implemented.

### Design

Instead of periodic timer ticks (which waste power on idle systems), MinimalOS uses a **tickless** design:

1. When scheduling a thread, calculate its **deadline** (current time + time slice)
2. Program the **LAPIC one-shot timer** for that deadline
3. When the timer fires, preempt the thread and reschedule
4. If the thread blocks before the timer, cancel the timer and reschedule immediately

### Per-Core Run Queues

Each CPU core has its own run queue with multiple priority levels:

```
Core 0 Run Queue:
  Priority 0 (highest): [timer_thread]
  Priority 1:           [driver_thread_a, driver_thread_b]
  Priority 2 (normal):  [user_proc_1, user_proc_2, ...]
  Priority 3 (idle):    [idle_thread_0]
```

### Work Stealing

When a core's run queue is empty (except for the idle thread), it can **steal** threads from other cores' run queues. This balances load across the 4 N3710 cores automatically.

### Idle Thread

Each core has a dedicated idle thread that executes `hlt` in a loop. This puts the core into a low-power C-state until the next interrupt — important for battery life on the HP laptop.

---

## SMP Initialization

{: .todo }
> Not yet implemented.

On boot, only the BSP (Bootstrap Processor, core 0) is running. The other 3 cores (APs) are halted, waiting for a startup sequence.

### AP Boot Sequence

1. **Parse MADT** — find each AP's LAPIC ID
2. **Prepare AP trampoline** — 16-bit real-mode code at an address below 1 MB
3. **Send INIT IPI** — reset the AP
4. **Wait 10ms**
5. **Send SIPI (Startup IPI)** — AP starts executing the trampoline
6. **Trampoline**: switch to protected mode → long mode → jump to Rust AP entry
7. **AP initialization**: set up per-core GDT, IDT, TSS, LAPIC, run queue
8. **AP enters scheduler loop**

### Per-Core State

Each core maintains:
- Its own GDT (for per-core TSS)
- Its own IDT (same table, but loaded via `lidt` on each core)
- Its own LAPIC (memory-mapped, same physical address)
- Its own run queue
- Its own idle thread

---

## Dependencies

- **Requires**: Sprint 3 (IDT for timer interrupts, LAPIC for preemption)
- **Enables**: Sprint 5 (capabilities need process/thread structures), Sprint 6 (userspace needs Ring 3 processes)
