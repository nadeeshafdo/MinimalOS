# Process Management

## Overview
MinimalOS implements a basic multitasking system with a round-robin scheduler. It supports kernel threads and has the infrastructure for user-mode processes (though currently running in kernel mode for simplicity).

## Process Structure (PCB)
Each process is represented by a `process_t` structure (Process Control Block), containing:
- **PID**: Unique Process ID.
- **State**: Current state (READY, RUNNING, BLOCKED, ZOMBIE).
- **Name**: Human-readable name (e.g., "Shell").
- **Page Directory**: Pointer to the process's page tables (currently shared kernel directory).
- **Kernel Stack**: 8KB stack for kernel execution.
- **Context**: Saved CPU registers (RIP, RSP, RFLAGS, etc.) for context switching.
- **Priority**: Scheduling priority (base 1).
- **Time Slice**: Remaining ticks before preemption.

## Scheduler
The scheduler is located in `kernel/process/scheduler.c`.
- **Algorithm**: Round Robin.
- **Time Slice**: Each process gets a fixed time slice.
- **Tick Handler**: The timer interrupt fires at 100Hz, calling `scheduler_tick()`.
- **Preemption**: When a process's time slice expires, `scheduler_tick()` triggers a context switch to the next runnable process in the ready queue.
- **Idle Process**: PID 0 serves as the idle task, running when no other tasks are ready (though usually the shell task is active).

## Context Switching
Context switching is performed by saving the current process's state and restoring the next process's state.
1. **Save Context**: Push all general-purpose registers to the current stack.
2. **Switch Stack**: Update `RSP` to the new process's stack.
3. **Switch CR3**: Load the new process's page directory (if different).
4. **Restore Context**: Pop registers from the new stack.
5. **Update TSS**: Update the Task State Segment's `RSP0` to point to the new process's kernel stack (crucial for future user-mode support).

## System Calls
System calls provide an interface for processes to request kernel services.
- **Mechanism**: Software Interrupt `0x80`.
- **Calling Convention**:
  - `RAX`: System Call Number
  - `RBX`, `RCX`, `RDX`, `RSI`, `RDI`: Arguments
- **Handler**: `syscall_handler` in `kernel/process/syscall.c` dispatches calls based on `RAX`.

### Implemented System Calls
| Number | Name | Description |
|--------|------|-------------|
| 1 | `sys_exit` | Terminate the current process. |
| 4 | `sys_write` | Write data to stdout/stderr (framebuffer). |
