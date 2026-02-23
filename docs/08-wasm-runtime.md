# Wasm Runtime

## Overview

MinimalOS executes all user-space logic as **WebAssembly actors**. There is no traditional ELF loading or Ring 3 execution — every actor is a `.wasm` module interpreted by the `tinywasm` crate inside the kernel.

## Architecture: Single Address Space OS (SASOS)

All Wasm actors share the kernel address space (Ring 0). Memory isolation is provided by the Wasm sandbox — each actor can only access its own linear memory. The kernel exposes functionality through **host functions** that actors call as Wasm imports.

## tinywasm Integration (`kernel/src/wasm.rs`)

### Version & Configuration
- `tinywasm = "0.8"` — stack-based Wasm interpreter
- No JIT, no WASI — pure interpretation with custom host imports
- Each actor gets its own `tinywasm::Store` and `tinywasm::ModuleInstance`

### WasmEnv

```rust
pub struct WasmEnv {
    pub store: tinywasm::Store,
    pub instance: tinywasm::ModuleInstance,
}
```
Stored in `process.wasm_env: Option<Box<WasmEnv>>`.

## Spawning a Wasm Actor (`spawn_wasm`)

```
spawn_wasm(name, wasm_bytes, caps_to_grant):
  1. Create new Process with entry_point = wasm_actor_trampoline
  2. Parse wasm_bytes → tinywasm::Module
  3. Create Store, register all host function imports
  4. Instantiate module → ModuleInstance
  5. Store WasmEnv in process
  6. Grant initial capabilities (IPC endpoints, memory, framebuffer)
  7. Add process to scheduler ready queue
```

### Initial Capability Granting

Each actor receives capabilities before it starts running:
```rust
// Typical pattern from main.rs:
let vfs_pid = spawn_wasm("vfs.wasm", vfs_bytes, &[]);
let ui_pid  = spawn_wasm("ui_server.wasm", ui_bytes, &[
    (ObjectKind::Framebuffer, fb_addr, PERM_READ|PERM_WRITE, fb_len),
]);
let shell_pid = spawn_wasm("shell.wasm", shell_bytes, &[]);

// Then cross-grant IPC endpoints between actors
```

## Actor Trampoline (`wasm_actor_trampoline`)

When the scheduler runs a Wasm actor for the first time:
```
wasm_actor_trampoline:
  1. Lock SCHEDULER, get current process
  2. Extract WasmEnv from process
  3. Drop scheduler lock
  4. Call module.exported_func("_start") via tinywasm
  5. On return → sys_exit(0)
```

## Host Functions (Wasm Imports)

All imports live in the `"env"` module namespace.

### Logging
| Import | Signature | Description |
|---|---|---|
| `sys_log` | `(ptr: i32, len: i32)` | Read string from Wasm memory, print via `klog` |

### Process Control
| Import | Signature | Description |
|---|---|---|
| `sys_exit` | `(code: i32)` | Mark process Dead, call `do_schedule()` |

### IPC
| Import | Signature | Description |
|---|---|---|
| `sys_cap_send` | `(handle: i64, opcode: i32, payload_ptr: i32, payload_len: i32, cap_slot: i64) → i32` | Send IPC message via capability handle |
| `sys_cap_recv` | `(handle: i64, buf_ptr: i32, buf_len: i32) → i64` | Blocking receive; returns sender PID or -1 |

### Memory Access (Shared Buffers)
| Import | Signature | Description |
|---|---|---|
| `sys_cap_mem_read` | `(handle: i64, offset: i32, buf_ptr: i32, len: i32) → i32` | Copy from kernel shared buffer into Wasm memory |
| `sys_cap_mem_write` | `(handle: i64, offset: i32, data_ptr: i32, len: i32) → i32` | Copy from Wasm memory into kernel shared buffer |

### Framebuffer
| Import | Signature | Description |
|---|---|---|
| `sys_fb_info` | `() → i64` | Returns `(width << 32) | height` packed |

### Host Function Implementation Pattern

```rust
// Example: sys_cap_send
fn sys_cap_send(mut ctx: FuncContext, args: &[WasmValue]) -> Result<...> {
    let (cap_slot, payload_len, payload_ptr, opcode, handle) = // destructure args
    // NOTE: tinywasm 0.8 pushes args in LIFO order!
    
    let mem = ctx.exported_memory("memory")?;
    let payload = mem.load_vec(payload_ptr, payload_len);
    
    // Lock scheduler, look up current PID
    // Call ipc_send(pid, handle, opcode, &payload, cap_slot)
    Ok(vec![WasmValue::I32(result)])
}
```

### ⚠️ tinywasm 0.8 LIFO Argument Bug

When destructuring `args: &[WasmValue]`, the parameter order is **reversed** compared to the Wasm function signature. For example:

```rust
// Wasm signature: sys_cap_send(handle: i64, opcode: i32, ptr: i32, len: i32, cap: i64)
// But args slice arrives as: [cap, len, ptr, opcode, handle]
```

This is a known quirk of tinywasm 0.8's stack-based evaluation.

## Wasm Actor Memory

Each Wasm actor has its own linear memory (managed by tinywasm):
- Default: 1 page (64 KiB), growable
- Actors allocate within their linear memory using a simple bump allocator (in `actors/sdk`)
- The kernel reads/writes actor memory through `ctx.exported_memory("memory")`

## Actor Build Pipeline

```
actors/vfs/src/lib.rs  →  cargo build --target wasm32-unknown-unknown
                       →  target/wasm32-unknown-unknown/release/vfs.wasm
                       →  packed into ramdisk.tar
                       →  kernel extracts at boot via TAR parser
                       →  spawn_wasm("vfs.wasm", bytes)
```
