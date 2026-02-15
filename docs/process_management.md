# Process Management

## Current Status

The task/process subsystem (`kernel/src/task/mod.rs`) is **stubbed out** and awaiting
implementation. This document outlines the intended design.

## Planned Components

### Task Control Block (TCB)

Each task will be represented by a structure containing:

- Unique task/process ID.
- Saved CPU register state (for context switching).
- Stack pointer and kernel stack allocation.
- Task state (running, ready, blocked, terminated).

### Scheduler

A round-robin scheduler is planned:

- Maintain a ready queue of runnable tasks.
- On each timer tick (once the PIT/APIC timer is configured), select the next task.
- Perform a context switch by saving/restoring register state.

### Context Switching

Context switches on x86_64 involve:

- Saving general-purpose registers, `rflags`, `rip`, and `rsp` of the current task.
- Restoring the same for the next task.
- Updating the TSS `rsp0` field for kernel re-entry from user mode.

### System Calls

A system call interface via `syscall`/`sysret` or `int 0x80` is planned for future
user-space programs to request kernel services.

## Dependencies

- **Memory management** must be implemented first (tasks need stack allocations).
- **Interrupt handling** (`traps` module) is needed for timer-driven preemption.

## Quest Tracking

Related quests from `QUESTS.md`:

- **[012]** Task Scheduler
- **[013]** Userland Standard Library (ulib)
- **[014]** First Userspace Application
