---
title: Syscalls & Userspace
layout: default
parent: Subsystems
nav_order: 6
---

# Sprint 6 — Syscall Interface & Userspace Entry
{: .no_toc }

Cross the Ring 0 / Ring 3 boundary.
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

Sprint 6 connects all the kernel subsystems together into a usable system by implementing the **SYSCALL/SYSRET** fast transition mechanism, an **ELF loader**, and the actual transition into **Ring 3** (user mode). After this sprint, the kernel can load and run a userspace program.

---

## SYSCALL/SYSRET

{: .todo }
> Not yet implemented.

### What is SYSCALL/SYSRET?

The `SYSCALL` instruction is the fast path for entering the kernel from userspace on x86_64. Unlike software interrupts (`int 0x80`), SYSCALL doesn't push to the stack or read the IDT — it uses pre-configured MSRs (Model-Specific Registers) for maximum speed.

### MSR Configuration

| MSR | Name | Purpose |
|:----|:-----|:--------|
| `STAR` | Segment Selectors | Bits 47:32 = kernel CS, Bits 63:48 = user CS base |
| `LSTAR` | Syscall Entry | 64-bit address of the syscall handler entry point |
| `SFMASK` | RFLAGS Mask | Flags to clear on syscall entry (disable interrupts) |

### Syscall Entry Point

When userspace executes `SYSCALL`:

1. CPU saves RIP in RCX, RFLAGS in R11
2. CPU loads CS/SS from STAR MSR → kernel mode
3. CPU masks RFLAGS with SFMASK → interrupts disabled
4. CPU jumps to LSTAR → our entry point

Our handler then:

1. **Swap to kernel stack** (from TSS RSP0)
2. **Save all user registers** to the thread's save area
3. **Dispatch** based on RAX (syscall number)
4. **Execute** the syscall handler
5. **Restore user registers**
6. **SYSRET** back to userspace

### Register Convention

| Register | Role |
|:---------|:-----|
| RAX | Syscall number (in) / return value (out) |
| RDI | Argument 1 |
| RSI | Argument 2 |
| RDX | Argument 3 |
| R10 | Argument 4 (RCX is clobbered by SYSCALL) |
| R8 | Argument 5 |
| R9 | Argument 6 |
| RCX | Saved RIP (by CPU) |
| R11 | Saved RFLAGS (by CPU) |

---

## Syscall Dispatch Table

{: .todo }
> Not yet implemented.

The kernel dispatches syscalls via a function pointer table indexed by RAX:

```rust
const SYSCALL_TABLE: [SyscallHandler; 22] = [
    sys_mem_map,         // 0
    sys_mem_unmap,       // 1
    sys_mem_grant,       // 2
    sys_mem_alloc,       // 3
    sys_ipc_send,        // 4
    sys_ipc_recv,        // 5
    sys_ipc_call,        // 6
    sys_ipc_reply,       // 7
    sys_ipc_notify,      // 8
    sys_cap_create,      // 9
    sys_cap_delete,      // 10
    sys_cap_transfer,    // 11
    sys_cap_revoke,      // 12
    sys_cap_inspect,     // 13
    sys_proc_create,     // 14
    sys_proc_destroy,    // 15
    sys_thread_create,   // 16
    sys_thread_destroy,  // 17
    sys_thread_yield,    // 18
    sys_thread_block,    // 19
    sys_thread_wake,     // 20
    sys_irq_bind,        // 21
];
```

Each handler validates capability arguments, performs the operation, and returns a result code.

---

## ELF Loader

{: .todo }
> Not yet implemented.

### What is ELF?

ELF (Executable and Linkable Format) is the standard binary format for executables on Linux and bare-metal systems. The kernel must parse ELF files to load userspace programs.

### Loading Process

1. **Read ELF header** — verify magic bytes, architecture (x86_64), type (executable)
2. **Parse program headers** — each `PT_LOAD` segment describes a chunk to map:
   - Virtual address, file offset, file size, memory size
   - Permissions (Read, Write, Execute)
3. **Allocate pages** — use PMM to allocate physical frames for each segment
4. **Map pages** — use VMM to create mappings in the process's address space with correct permissions
5. **Copy data** — copy segment contents from the ELF file into the mapped pages
6. **Zero BSS** — if memory size > file size, zero the remaining bytes
7. **Set up user stack** — allocate and map pages at the top of userspace (e.g., `0x7FFFFFFFE000`)
8. **Return entry point** — the ELF header contains the address where execution begins

### Address Space Layout (Userspace)

```
0x00007FFFFFFFFFFF  ┬──────────────────────┐
                    │  User stack (grows ↓) │
0x00007FFFFFFFE000  ├──────────────────────┤
                    │  (guard page)         │
                    ├──────────────────────┤
                    │  ...                  │
                    │  Heap (grows ↑)       │
                    ├──────────────────────┤
                    │  .bss                 │  R+W
                    ├──────────────────────┤
                    │  .data                │  R+W
                    ├──────────────────────┤
                    │  .rodata              │  R
                    ├──────────────────────┤
                    │  .text                │  R+X
0x0000000000400000  └──────────────────────┘  ← ELF base address
```

---

## Ring 3 Entry

{: .todo }
> Not yet implemented.

### Steps to Enter Userspace

1. **Create process** — allocate a new PML4, capability table, thread structure
2. **Load ELF** — parse and map the init binary into the process's address space
3. **Set up user stack** — map stack pages with User + Writable + NX permissions
4. **Prepare initial capabilities** — the init process receives:
   - IPC endpoint for serial driver communication
   - Memory capability for its own address space
   - Process capability for spawning children
5. **Switch to user page tables** — load the process's PML4 into CR3
6. **SYSRET** — pop into Ring 3 at the ELF entry point

### Verification

Test that the syscall round-trip works correctly:
1. Userspace calls `SYSCALL` → enters kernel
2. Kernel processes the request
3. Kernel returns via `SYSRET` → back in userspace
4. Verify registers are preserved, return value is correct

---

## Security Considerations

- **SMAP/SMEP**: Supervisor Mode Access/Execution Prevention — the kernel cannot accidentally access or execute user pages
- **KPTI**: Kernel Page Table Isolation — in the user PML4, only a minimal kernel stub is mapped (syscall entry/exit)
- **Stack guard pages**: Unmapped pages above the user stack to catch overflows
- **ASLR**: Address Space Layout Randomization — randomize base addresses (future enhancement)

---

## Dependencies

- **Requires**: Sprint 5 (capability system for access control during syscalls)
- **Enables**: Sprint 7 (init process loaded via ELF, communicates via syscalls)
