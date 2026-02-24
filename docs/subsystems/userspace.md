---
title: Init & Userspace
layout: default
parent: Subsystems
nav_order: 7
---

# Sprint 7 — Init Process & First Real Program
{: .no_toc }

Life outside the kernel.
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

Sprint 7 is where MinimalOS becomes a real operating system. The kernel loads the **init process** — the first userspace program — and everything after that happens through capability-mediated IPC. This sprint also creates the **userspace library** (`libmnos`) that provides safe Rust wrappers around the raw syscall interface.

---

## Init Process

{: .todo }
> Not yet implemented.

### Role

The init process is the **root of the process tree** and the **service manager**. It's the only process that receives capabilities directly from the kernel at boot — all other processes receive their capabilities from init or its children.

### Boot Capabilities

The kernel creates these capabilities and places them in init's capability table:

| Slot | Type | Resource | Rights |
|:-----|:-----|:---------|:-------|
| 0 | Memory | All usable RAM | Read, Write, Grant |
| 1 | IPC Endpoint | Kernel log | Send |
| 2 | Process | Self | All |
| 3 | Interrupt | All unmasked IRQs | Receive, Grant |

### Responsibilities

1. **Spawn child processes** — load service binaries from the initramfs
2. **Distribute capabilities** — give each service the minimum capabilities it needs
3. **Service registration** — maintain a name→endpoint directory ("serial_driver" → endpoint #7)
4. **Service lookup** — respond to queries: "where is the VFS service?"
5. **Crash recovery** — restart failed services

### Service Registration Protocol

```
Init Process Service Directory:
┌──────────────────┬──────────────┐
│ Service Name     │ IPC Endpoint │
├──────────────────┼──────────────┤
│ serial_driver    │ endpoint_7   │
│ vfs_service      │ endpoint_12  │
│ network_stack    │ endpoint_15  │
└──────────────────┴──────────────┘
```

Services register with init via IPC:
```
→ IPC_SEND(init_endpoint, { type: REGISTER, name: "serial_driver", endpoint: my_endpoint })
← IPC_REPLY({ status: OK })
```

Clients look up services:
```
→ IPC_SEND(init_endpoint, { type: LOOKUP, name: "serial_driver" })
← IPC_REPLY({ status: OK, endpoint: serial_endpoint_cap })
```

---

## Userspace Library (`libmnos`)

{: .todo }
> Not yet implemented.

### Purpose

`libmnos` is a static library that userspace programs link against. It provides safe Rust abstractions over the raw SYSCALL interface.

### Syscall Wrappers

```rust
// Raw syscall (unsafe)
unsafe fn syscall3(nr: u64, arg1: u64, arg2: u64, arg3: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        in("rax") nr,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        out("rcx") _,  // clobbered by SYSCALL
        out("r11") _,  // clobbered by SYSCALL
        lateout("rax") ret,
    );
    ret
}

// Safe wrapper
pub fn ipc_send(endpoint: CapSlot, msg: &IpcMessage) -> Result<(), IpcError> {
    let ret = unsafe { syscall3(SYS_IPC_SEND, endpoint.0 as u64, msg as *const _ as u64, 0) };
    match ret {
        0 => Ok(()),
        e => Err(IpcError::from_code(e)),
    }
}
```

### IPC Helpers

```rust
// High-level RPC call
pub fn rpc_call<Req: Serialize, Resp: Deserialize>(
    endpoint: CapSlot,
    request: &Req,
) -> Result<Resp, IpcError> {
    let msg = IpcMessage::encode(request);
    let reply = ipc_call(endpoint, &msg)?;
    Ok(reply.decode()?)
}
```

### Memory Allocation

Userspace gets its own heap allocator that requests pages from the kernel:

```rust
pub fn alloc_pages(count: usize) -> Result<*mut u8, MemError> {
    let addr = unsafe { syscall2(SYS_MEM_ALLOC, count as u64, 0) };
    if addr < 0 { Err(MemError::OutOfMemory) }
    else { Ok(addr as *mut u8) }
}
```

---

## First Userspace Driver

{: .todo }
> Not yet implemented.

### Serial Console Driver

The first real userspace service — a serial console driver that demonstrates the full capability-based architecture:

```
Kernel                    Init Process              Serial Driver
  │                           │                          │
  │ ── boot caps ──►          │                          │
  │                           │ ── spawn + caps ──►      │
  │                           │   (IRQ 4 cap,            │
  │                           │    MMIO cap,             │
  │                           │    IPC endpoint)         │
  │                           │                          │
  │ ◄── IRQ 4 fires ──       │                          │
  │ ── notify ──────────────────────────────────►        │
  │                           │                   read serial port
  │                           │                   process input
  │                           │   ◄── IPC ──      echo to output
```

### What This Demonstrates

1. **Interrupt routing**: IRQ 4 (COM1) is routed to the serial driver via an interrupt capability
2. **MMIO access**: The driver has a memory capability for the serial port's I/O range
3. **Process isolation**: The driver runs in Ring 3 — a bug in the driver can't crash the kernel
4. **IPC communication**: The serial driver communicates with other processes via IPC
5. **Capability delegation**: Init gives the serial driver exactly the capabilities it needs — nothing more

---

## Initramfs

The init process and initial services are bundled into an **in-memory filesystem** (initramfs) that Limine loads alongside the kernel:

- Format: tar archive (simple, no compression needed for small images)
- Contains: init binary, serial driver binary, other initial services
- Loaded by Limine as a module
- Init unpacks it to find and load service binaries

---

## Dependencies

- **Requires**: Sprint 6 (SYSCALL/SYSRET, ELF loader, Ring 3 entry)
- **Enables**: Future milestones (VFS, disk drivers, networking — all built on this foundation)
