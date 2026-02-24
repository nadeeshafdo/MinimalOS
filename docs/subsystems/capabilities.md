---
title: Capabilities & IPC
layout: default
parent: Subsystems
nav_order: 5
---

# Sprint 5 — Capabilities & IPC
{: .no_toc }

The security model — unforgeable tokens and message passing.
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

Sprint 5 implements the two defining features of MinimalOS: **capability-based access control** and **IPC (Inter-Process Communication)**. Together, they replace traditional Unix permissions with a mathematically sound security model where every resource access requires an explicit, unforgeable token.

---

## Capability System

{: .todo }
> Not yet implemented.

### What is a Capability?

A capability is a **kernel-managed token** that represents the right to perform specific operations on a specific resource. Capabilities are:

- **Unforgeable** — only the kernel creates them; userspace cannot fabricate them
- **Typed** — each capability refers to a specific kind of resource
- **Rights-bearing** — capabilities carry a bitmask of permitted operations
- **Transferable** — processes can send capabilities to other processes via IPC
- **Revocable** — the creator can invalidate a capability

### Capability Table

Each process has a **capability table** — an array of slots, each containing a capability or empty:

```
Process Capability Table:
┌──────┬───────────────┬────────────────┬──────────────┐
│ Slot │ Type          │ Resource       │ Rights       │
├──────┼───────────────┼────────────────┼──────────────┤
│  0   │ IPC Endpoint  │ serial_channel │ Send, Recv   │
│  1   │ Memory        │ 0x1000-0x2000  │ Read, Write  │
│  2   │ Interrupt     │ IRQ 4 (COM1)   │ Receive      │
│  3   │ (empty)       │ —              │ —            │
│  4   │ Process       │ PID 7          │ Suspend, Kill│
└──────┴───────────────┴────────────────┴──────────────┘
```

Syscalls reference capabilities by **slot index**: `sys_ipc_send(slot=0, message)`.

### Capability Types

| Type | Resource | Typical Rights |
|:-----|:---------|:--------------|
| **Memory** | Physical page range | Read, Write, Execute, Grant |
| **IPC Endpoint** | Communication channel | Send, Receive, Grant |
| **Interrupt** | Hardware IRQ line | Receive, Mask |
| **Process** | Another process | Inspect, Suspend, Resume, Kill |
| **Thread** | A thread | Suspend, Resume, Set Priority |

### Rights Bitmask

```rust
bitflags! {
    pub struct CapRights: u32 {
        const READ    = 1 << 0;   // Read data
        const WRITE   = 1 << 1;   // Write / modify
        const EXECUTE = 1 << 2;   // Execute code
        const GRANT   = 1 << 3;   // Transfer to another process
        const REVOKE  = 1 << 4;   // Revoke derived capabilities
    }
}
```

### Capability Operations

| Syscall | Description |
|:--------|:------------|
| `sys_cap_create(type, resource, rights)` | Kernel-only: create a new capability |
| `sys_cap_delete(slot)` | Remove a capability from the table |
| `sys_cap_transfer(src_slot, dest_process, rights)` | Send a capability (rights can only be reduced) |
| `sys_cap_revoke(slot)` | Revoke all capabilities derived from this one |
| `sys_cap_inspect(slot)` | Query type, resource, and rights |

### Monotonic Rights Reduction

When transferring a capability, rights can only be **reduced**, never increased. This ensures that delegation cannot escalate privileges:

```
Process A has: Memory(0x1000, Read+Write+Grant)
Process A transfers to B with reduced rights:
Process B receives: Memory(0x1000, Read)  ← no Write, no Grant
```

---

## IPC (Inter-Process Communication)

{: .todo }
> Not yet implemented.

### Design Principles

- **Synchronous** — send blocks until the receiver accepts; receive blocks until a message arrives
- **Capability-mediated** — both sender and receiver must hold an IPC endpoint capability
- **Zero-copy option** — large data transfers use memory grant capabilities instead of copying bytes
- **Small message optimization** — short messages (≤ 64 bytes) are passed in registers, avoiding copies entirely

### Message Format

```rust
pub struct IpcMessage {
    // Inline data (passed in registers for small messages)
    data: [u64; 8],              // 64 bytes of inline data
    data_len: usize,             // Actual length used

    // Optional capability transfers
    caps: [CapSlot; 4],          // Up to 4 capabilities transferred
    cap_count: usize,
}
```

### Communication Patterns

#### Send / Receive

Basic one-way message passing:

```rust
// Sender (process A)
sys_ipc_send(endpoint_slot, &message);

// Receiver (process B)
let msg = sys_ipc_recv(endpoint_slot);
```

#### Call / Reply (RPC)

Synchronous request-response, like a function call across processes:

```rust
// Client
let response = sys_ipc_call(server_endpoint, &request);

// Server
loop {
    let (client_badge, request) = sys_ipc_recv(listen_endpoint);
    let response = handle_request(request);
    sys_ipc_reply(client_badge, &response);
}
```

The `call` operation atomically sends a message and blocks waiting for the reply, while temporarily granting the server a reply capability.

#### Notifications

Lightweight event signaling — non-blocking, no data transfer:

```rust
// Signal an event (non-blocking)
sys_ipc_notify(endpoint_slot, EVENT_DATA_READY);

// Wait for notifications (blocking)
let events = sys_ipc_wait(endpoint_slot);
```

Notifications are OR'd together if multiple arrive before the receiver checks — they never queue up or overflow.

### Zero-Copy Memory Grants

For large data transfers (disk blocks, network packets), the sender shares physical pages directly:

1. Sender creates a Memory capability for the relevant pages
2. Sender transfers the capability via IPC to the receiver
3. Receiver maps the pages into its own address space
4. Both processes access the same physical memory — zero copies

---

## Security Guarantees

| Property | Mechanism |
|:---------|:----------|
| **No ambient authority** | All resources accessed via capability slots |
| **Information hiding** | Processes only see capabilities they were granted |
| **Controlled delegation** | Capabilities transfer with monotonically decreasing rights |
| **Complete mediation** | Every resource access goes through the kernel's capability check |
| **Revocation** | Creator can revoke derived capabilities instantly |

---

## Dependencies

- **Requires**: Sprint 4 (process/thread structures, scheduler)
- **Enables**: Sprint 6 (syscall dispatch needs capability validation), Sprint 7 (init process uses IPC for service registration)
