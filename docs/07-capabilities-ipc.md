# Capabilities & IPC

## Capability Engine (`kernel/src/cap.rs`)

### Design

Every kernel resource is accessed through **capabilities** — unforgeable handles that grant specific permissions on a specific object. There are no global namespaces; a process can only interact with objects it holds capabilities for.

### Object Kinds

```rust
pub enum ObjectKind {
    IpcEndpoint,   // Send/receive messages to/from another process
    Memory,        // Read/write a shared byte buffer
    Framebuffer,   // Read/write the framebuffer memory
    Interrupt,     // Receive hardware interrupt notifications
}
```

### Permissions (bitflags)

```rust
pub const PERM_READ:  u32 = 1;
pub const PERM_WRITE: u32 = 2;
pub const PERM_GRANT: u32 = 4;  // Can transfer this cap to another process via IPC
```

### Capability Structure

```rust
pub struct Capability {
    pub kind: ObjectKind,
    pub target: u64,      // PID for IPC, address for Memory/Framebuffer, IRQ for Interrupt
    pub permissions: u32,
    pub generation: u64,  // Revocation counter
    pub extra: u64,       // Length for Memory/Framebuffer, unused elsewhere
}
```

### CapTable

Each process has a 64-slot `CapTable`:
```rust
pub struct CapTable {
    pub slots: [Option<Capability>; 64],
}
```

### Composite Handles

Capability slots are exported to user code as **composite handles**:
```
handle = (slot_index as u64) | (generation << 32)
```
This prevents use-after-revoke — if a capability is revoked and the slot reused, old handles fail the generation check.

### Key Functions

| Function | Purpose |
|---|---|
| `cap_grant(table, kind, target, perms, extra)` | Insert new cap into first free slot; returns composite handle |
| `cap_lookup(table, handle)` | Validate handle, check generation, return &Capability |
| `cap_revoke(table, handle)` | Clear slot, bump generation |

---

## IPC System (`kernel/src/ipc.rs`)

### Message Structure (48 bytes)

```rust
pub struct Message {
    pub sender: u64,      // Sender PID (filled by kernel, not user)
    pub opcode: u32,      // Application-defined operation code
    pub flags: u32,       // Reserved
    pub payload: [u8; 16], // Inline data (fits u128 or 4×i32)
    pub cap_slot: u64,    // Capability handle being transferred (0 = none)
}
```

### IPC Queue

Each process has a 16-entry ring buffer:
```rust
pub struct IpcQueue {
    pub buffer: [Message; 16],
    pub head: usize,
    pub tail: usize,
    pub count: usize,
    pub blocked_sender: Option<(u64, Message)>, // Back-pressure
}
```

### Send Flow (`ipc_send`)

```
1. Lock SCHEDULER
2. Look up sender's CapTable[handle] → must be IpcEndpoint with WRITE
3. Find target process by PID
4. If msg.cap_slot ≠ 0:
   a. Look up transferred cap (must have GRANT permission)
   b. cap_grant() it into target's CapTable
   c. Store new handle in msg.cap_slot
5. Stamp msg.sender = caller PID
6. Push msg into target's ipc_queue
7. If target was Blocked → set Ready + request_wake(target_pid)
```

### Receive Flow (`ipc_recv`)

```
1. Lock SCHEDULER
2. Look up cap handle → must be IpcEndpoint with READ
3. Pop from own ipc_queue
4. If empty → set state = Blocked, return None
5. Return the Message
```

### Blocking Semantics

- **Receive blocks**: process enters `Blocked` state, resumed when any message arrives
- **Send never blocks**: message is enqueued or back-pressure slot used (sender continues)
- Wake path: `ipc_send` calls `request_wake()`, which stores PID in an `AtomicU64` array; the scheduler drains this array at the start of `do_schedule()`

### Capability Transfer Protocol

When a Wasm actor calls `sys_cap_send(handle, msg)` with `msg.cap_slot ≠ 0`:

1. Kernel validates the sender has GRANT permission on the capability being transferred
2. Kernel inserts a **copy** of the capability into the target's CapTable
3. The new composite handle (target's slot + generation) replaces `msg.cap_slot` in the delivered message
4. The sender retains its original capability (copy semantics, not move)

### Reply Routing Convention

MinimalOS uses a convention (not enforced by kernel) for request-reply:
1. Client sends IPC to server with `cap_slot` = client's own IPC endpoint handle
2. Server receives the message; `cap_slot` now holds a capability to reply to the client
3. Server sends response via that reply capability
4. This enables clients and servers that don't know each other at build time
