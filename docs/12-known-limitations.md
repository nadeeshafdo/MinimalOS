# Known Limitations & Next Steps

## Current Limitations

### 1. Single-Core Scheduling
- **Issue**: Only BSP (core 0) runs the scheduler. AP cores halt after boot.
- **Cause**: AP timer interrupts are intentionally disabled — `init_ap_timer()` is not called.
- **Impact**: All Wasm actors run on a single core. Multi-core performance is unused.
- **Fix path**: Enable AP timers, add per-core run queues, implement work-stealing or migration.

### 2. Hardcoded Display Resolution
- **Issue**: UI Server assumes 1280×800, 32bpp BGRA.
- **Cause**: Values are constants in `actors/ui_server/src/lib.rs` and mouse clamping in `khal/mouse.rs`.
- **Impact**: Wrong resolution causes rendering artifacts or out-of-bounds writes.
- **Fix path**: Pass framebuffer geometry via IPC or a shared memory capability at startup.

### 3. tinywasm 0.8 Argument Order Bug
- **Issue**: Host function arguments arrive in LIFO (reversed) order.
- **Cause**: tinywasm 0.8 stack evaluation quirk.
- **Impact**: All host functions must destructure args in reverse. Easy to introduce bugs when adding new syscalls.
- **Fix path**: Upgrade tinywasm or add a wrapper that normalizes argument order.

### 4. No Capability Revocation on Process Death
- **Issue**: When an actor dies, capabilities granted to other processes that point to the dead actor are not cleaned up.
- **Cause**: No reverse-mapping from target PID to granted capabilities.
- **Impact**: Sending IPC to a dead process's endpoint silently fails or corrupts.
- **Fix path**: Maintain a reverse capability index; on death, revoke all capabilities targeting that PID.

### 5. Legacy ELF-Era Modules
- **Issue**: Several modules exist from the pre-Wasm era and are no longer used.
- **Modules**: `task/futex.rs`, `task/pipe.rs`, `task/usermode.rs`, `task/input.rs`
- **Impact**: Dead code, potential confusion.
- **Fix path**: Remove or gate behind a feature flag.

### 6. No Memory Limits for Wasm Actors
- **Issue**: Wasm linear memory can grow without bounds (limited only by physical RAM).
- **Cause**: No `memory.maximum` enforcement in tinywasm configuration.
- **Impact**: A single actor can exhaust all memory.
- **Fix path**: Set maximum memory pages per actor; enforce in Wasm instantiation.

### 7. IPC Back-Pressure Is Minimal
- **Issue**: Only one blocked sender message is stored per process.
- **Cause**: `IpcQueue.blocked_sender: Option<(u64, Message)>` — single slot.
- **Impact**: If multiple senders send to a full queue simultaneously, messages can be lost.
- **Fix path**: Replace with a proper back-pressure queue or block senders until space is available.

### 8. No Persistent Storage
- **Issue**: The only filesystem is a read-only ramdisk loaded at boot.
- **Impact**: No way to save data across reboots.
- **Fix path**: Add a block device driver (virtio-blk for QEMU) and a writable filesystem actor.

### 9. No Networking
- **Impact**: No inter-machine communication capability.
- **Fix path**: Add a NIC driver (virtio-net) and a network stack actor.

### 10. Bump Allocator in Actor SDK
- **Issue**: The Wasm actor SDK uses a bump allocator that never frees.
- **Impact**: Long-running actors eventually exhaust their linear memory.
- **Fix path**: Implement a proper allocator (e.g., dlmalloc port for Wasm) in the actor SDK.

---

## Potential Next Steps (Prioritized)

### Short-Term
1. **Remove legacy modules** — Clean up dead code (futex, pipe, usermode, input)
2. **Dynamic FB resolution** — Pass framebuffer info from kernel to UI Server via IPC
3. **Improve actor SDK allocator** — Replace bump allocator with a freeing allocator
4. **Add more syscalls** — `sys_yield`, `sys_sleep`, `sys_time`

### Medium-Term
5. **SMP scheduling** — Enable AP cores, per-core run queues
6. **Capability revocation** — Reverse index + cleanup on death
7. **Wasm memory limits** — Enforce per-actor memory caps
8. **Block device driver** — virtio-blk for persistent storage
9. **Writable filesystem** — FAT32 or simple custom FS actor

### Long-Term
10. **Networking** — virtio-net + TCP/IP stack actor
11. **Dynamic actor loading** — Load .wasm files from disk at runtime
12. **Window manager** — Multi-window compositing in UI Server
13. **User input routing** — Keyboard/mouse events delivered to focused actor
14. **WASI subset** — Partial WASI compatibility for running existing Wasm programs
