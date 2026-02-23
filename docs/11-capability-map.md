# Post-Spawn Capability Map

This document shows exactly which capabilities each process receives after `main.rs` completes the spawn and cross-grant phase.

## Capability Assignment (from `main.rs`)

### 1. Kernel (PID 0)

The kernel itself does not use capabilities — it has direct access to everything. The CapTable concept applies only to user-space (Wasm) actors.

### 2. VFS Actor (PID 1)

| Slot | Kind | Target | Perms | Source |
|---|---|---|---|---|
| 0 | IpcEndpoint | VFS (self) | R+W+G | Granted at spawn |
| 1 | Memory | ramdisk buf addr | R | Granted at spawn |
| 2 | IpcEndpoint | Shell | W | Cross-granted by kernel |

### 3. UI Server Actor (PID 2)

| Slot | Kind | Target | Perms | Source |
|---|---|---|---|---|
| 0 | IpcEndpoint | UI Server (self) | R+W+G | Granted at spawn |
| 1 | Framebuffer | fb phys addr | R+W | Granted at spawn |
| 2 | Memory | font buf addr | R | Granted at spawn |

### 4. Shell Actor (PID 3)

| Slot | Kind | Target | Perms | Source |
|---|---|---|---|---|
| 0 | IpcEndpoint | Shell (self) | R+W+G | Granted at spawn |
| 1 | IpcEndpoint | VFS | W+G | Cross-granted by kernel |
| 2 | IpcEndpoint | UI Server | W | Cross-granted by kernel |

> **Note**: Slot indices are approximate — they depend on insertion order in `cap_grant()`. The actual slot is returned as part of the composite handle.

---

## IPC Message Flow Diagram

```
                    ┌──────────────────┐
                    │     Kernel       │
                    │  (cap_grant +    │
                    │   cross-grant)   │
                    └──────┬───────────┘
                           │ spawns + grants
              ┌────────────┼────────────┐
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │   VFS    │ │ UI Server│ │  Shell   │
        │  PID 1   │ │  PID 2   │ │  PID 3   │
        └────┬─────┘ └────┬─────┘ └────┬─────┘
             │             │            │
             │◄────────────┼────────────┤ Shell sends "read file" to VFS
             │             │            │   (cap_slot = Shell's own endpoint)
             │             │            │
             ├─────────────┼───────────►│ VFS replies with file contents
             │             │            │   (via transferred reply cap)
             │             │            │
             │             │◄───────────┤ Shell sends "draw text" to UI Server
             │             │            │
             │             │──[writes   │
             │             │  to FB]    │
             │             │            │
```

---

## Composite Handle Encoding

```
Bits 63..32: generation (revocation counter)
Bits 31..0:  slot index (0-63)

Example: handle = 0x0000_0001_0000_0002
  → slot 2, generation 1
```

Handles are opaque to Wasm actors — they just store and pass `i64` values. The kernel validates generation on every capability lookup.

---

## Dynamic Capability Transfer

When Shell sends a "read file" request to VFS:

```
1. Shell prepares Message:
   - opcode = 2 (read file)
   - payload = "hello.txt"
   - cap_slot = Shell's own IPC endpoint handle (slot 0)

2. Kernel processes ipc_send():
   - Validates Shell has GRANT on slot 0 ✓
   - Copies Shell's IpcEndpoint cap into VFS's CapTable
   - VFS gets new handle in some free slot (e.g., slot 3)
   - msg.cap_slot updated to VFS's new handle

3. VFS receives Message:
   - Sees cap_slot = handle for slot 3 → IpcEndpoint pointing to Shell
   - Sends reply via that handle → message arrives in Shell's ipc_queue

4. Shell receives reply:
   - cap_recv() returns the file contents
```

This mechanism enables **dynamic service discovery** — actors can hand out reply endpoints to servers they contact, without the server needing prior knowledge of the client.
